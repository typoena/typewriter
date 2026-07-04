//! Spike 4 — USB host: read keycodes from a USB HID boot keyboard.
//!
//! Drives the ESP-IDF USB Host Library directly through the raw `esp-idf-sys`
//! bindings (the convenience HID class driver is a managed component that isn't
//! vendored in mainline, and a boot keyboard doesn't need it). On attach it:
//!   1. opens the device and dumps its device/config descriptors,
//!   2. claims the boot-keyboard interface,
//!   3. sends SET_PROTOCOL(boot) + SET_IDLE(0) control transfers,
//!   4. polls the interrupt-IN endpoint and decodes each 8-byte boot report
//!      into modifiers + keycodes, logged over UART.
//!
//! The boot-keyboard parameters below were confirmed by Increment 1's
//! enumeration of the bench keyboard (VID:PID 19f5:3255): interface 0 is
//! class 03 / subclass 01 / protocol 01 with interrupt-IN endpoint 0x81,
//! wMaxPacketSize 8. A general driver would parse these from the config
//! descriptor; for the spike we target the confirmed layout.
//!
//! Logging goes over the CP2102 UART bridge (console = UART0), which is
//! independent of the USB PHY, so installing the host library does not
//! disturb the serial monitor.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use esp_idf_svc::sys::esp;
use esp_idf_svc::sys::{
    usb_config_desc_t, usb_device_desc_t, usb_device_handle_t, usb_host_client_config_t,
    usb_host_client_event_msg_t, usb_host_client_event_t_USB_HOST_CLIENT_EVENT_DEV_GONE,
    usb_host_client_event_t_USB_HOST_CLIENT_EVENT_NEW_DEV, usb_host_client_handle_events,
    usb_host_client_handle_t, usb_host_client_register, usb_host_config_t, usb_host_device_close,
    usb_host_device_open, usb_host_get_active_config_descriptor, usb_host_get_device_descriptor,
    usb_host_install, usb_host_interface_claim, usb_host_interface_release,
    usb_host_lib_handle_events, usb_host_transfer_alloc, usb_host_transfer_free,
    usb_host_transfer_submit, usb_host_transfer_submit_control, usb_print_config_descriptor,
    usb_print_device_descriptor, usb_transfer_status_t_USB_TRANSFER_STATUS_COMPLETED,
    usb_transfer_t, EspError, ESP_INTR_FLAG_LEVEL1,
};

/// Boot-keyboard parameters, confirmed by Increment 1's enumeration.
const KBD_INTERFACE: u8 = 0;
const KBD_ALT_SETTING: u8 = 0;
const KBD_EP_IN: u8 = 0x81;
const BOOT_REPORT_LEN: usize = 8;

/// HID class control requests. bmRequestType 0x21 = host→device | class |
/// interface recipient; wIndex (byte 4) = the interface number.
const SET_PROTOCOL_BOOT: [u8; 8] = [0x21, 0x0b, 0x00, 0x00, KBD_INTERFACE, 0x00, 0x00, 0x00];
const SET_IDLE_INFINITE: [u8; 8] = [0x21, 0x0a, 0x00, 0x00, KBD_INTERFACE, 0x00, 0x00, 0x00];

/// Address of a freshly-attached device, published by the client event
/// callback (a C function pointer, so it can't capture state) and consumed by
/// the main loop. 0 means "nothing pending".
static NEW_DEV_ADDR: AtomicU8 = AtomicU8::new(0);
/// Set when the open device is unplugged.
static DEV_GONE: AtomicBool = AtomicBool::new(false);
/// Control-transfer completion, published by `ctrl_cb` to the setup routine.
static CTRL_DONE: AtomicBool = AtomicBool::new(false);
static CTRL_STATUS: AtomicU32 = AtomicU32::new(0);

/// Client event callback — runs inside `usb_host_client_handle_events`. Keep
/// it minimal: stash what happened and let the main loop do the FFI work.
unsafe extern "C" fn client_event_cb(msg: *const usb_host_client_event_msg_t, _arg: *mut c_void) {
    let msg = unsafe { &*msg };
    #[allow(non_upper_case_globals)]
    match msg.event {
        usb_host_client_event_t_USB_HOST_CLIENT_EVENT_NEW_DEV => {
            let addr = unsafe { msg.__bindgen_anon_1.new_dev.address };
            NEW_DEV_ADDR.store(addr, Ordering::SeqCst);
        }
        usb_host_client_event_t_USB_HOST_CLIENT_EVENT_DEV_GONE => {
            DEV_GONE.store(true, Ordering::SeqCst);
        }
        _ => {}
    }
}

/// Control-transfer completion callback. Publishes status to the setup routine
/// waiting in `control_request`.
unsafe extern "C" fn ctrl_cb(transfer: *mut usb_transfer_t) {
    let status = unsafe { (*transfer).status };
    CTRL_STATUS.store(status as u32, Ordering::SeqCst);
    CTRL_DONE.store(true, Ordering::SeqCst);
}

/// Interrupt-IN completion callback: decode the boot report and resubmit to
/// keep polling. Runs inside `usb_host_client_handle_events`. On any
/// non-completed status (e.g. the device was unplugged and the transfer was
/// canceled) it stops resubmitting.
unsafe extern "C" fn report_cb(transfer: *mut usb_transfer_t) {
    let t = unsafe { &mut *transfer };
    if t.status == usb_transfer_status_t_USB_TRANSFER_STATUS_COMPLETED {
        let n = (t.actual_num_bytes as usize).min(BOOT_REPORT_LEN);
        let report = unsafe { core::slice::from_raw_parts(t.data_buffer, n) };
        decode_boot_report(report);
        let err = unsafe { usb_host_transfer_submit(transfer) };
        if err != 0 {
            log::error!("interrupt resubmit failed: {err}");
        }
    } else {
        log::info!("interrupt transfer stopped, status {}", t.status as u32);
    }
}

/// Install the USB Host Library, spawn the daemon event pump, register a
/// client, and service attach/detach forever. Does not return under normal
/// operation.
pub fn run() -> Result<(), EspError> {
    // Internal PHY (skip_phy_setup = false), root port powered on install,
    // default full-speed peripheral (BIT0 — the S3 has a single USB-OTG).
    let mut host_config: usb_host_config_t = unsafe { core::mem::zeroed() };
    host_config.intr_flags = ESP_INTR_FLAG_LEVEL1 as i32;
    host_config.peripheral_map = 1 << 0;
    esp!(unsafe { usb_host_install(&host_config) })?;
    log::info!("USB Host Library installed; waiting for a device…");

    // The daemon pump services enumeration and root-port events. It must run
    // continuously or an attach never completes. Own thread, blocking forever.
    std::thread::Builder::new()
        .stack_size(4096)
        .name("usb_host_daemon".into())
        .spawn(|| loop {
            let mut event_flags: u32 = 0;
            unsafe { usb_host_lib_handle_events(u32::MAX, &mut event_flags) };
        })
        .expect("spawn usb host daemon thread");

    // Register the client that receives device attach/detach callbacks.
    let mut client_config: usb_host_client_config_t = unsafe { core::mem::zeroed() };
    client_config.max_num_event_msg = 5;
    client_config.__bindgen_anon_1.async_.client_event_callback = Some(client_event_cb);
    client_config.__bindgen_anon_1.async_.callback_arg = ptr::null_mut();
    let mut client: usb_host_client_handle_t = ptr::null_mut();
    esp!(unsafe { usb_host_client_register(&client_config, &mut client) })?;

    let mut open_dev: usb_device_handle_t = ptr::null_mut();
    let mut report_xfer: *mut usb_transfer_t = ptr::null_mut();
    loop {
        // Blocks until a client event; the callbacks (attach/detach, control
        // completion, interrupt reports) all fire from within here.
        unsafe { usb_host_client_handle_events(client, u32::MAX) };

        let addr = NEW_DEV_ADDR.swap(0, Ordering::SeqCst);
        if addr != 0 {
            match setup_keyboard(client, addr) {
                Ok((dev, xfer)) => {
                    open_dev = dev;
                    report_xfer = xfer;
                }
                Err(e) => log::error!("keyboard setup failed: {e:?}"),
            }
        }
        if DEV_GONE.swap(false, Ordering::SeqCst) && !open_dev.is_null() {
            log::info!("device unplugged; releasing interface and closing");
            // Order per the USB Host Library: free transfers, release
            // interfaces, then close the device.
            if !report_xfer.is_null() {
                unsafe { usb_host_transfer_free(report_xfer) };
                report_xfer = ptr::null_mut();
            }
            unsafe { usb_host_interface_release(client, open_dev, KBD_INTERFACE) };
            unsafe { usb_host_device_close(client, open_dev) };
            open_dev = ptr::null_mut();
        }
    }
}

/// Open a newly-attached device, dump its descriptors, claim the keyboard
/// interface, put it in boot protocol, and start polling for reports.
/// Returns the device handle and the in-flight report transfer.
fn setup_keyboard(
    client: usb_host_client_handle_t,
    addr: u8,
) -> Result<(usb_device_handle_t, *mut usb_transfer_t), EspError> {
    let dev = open_and_dump(client, addr)?;

    log::info!("claiming interface {KBD_INTERFACE}");
    esp!(unsafe { usb_host_interface_claim(client, dev, KBD_INTERFACE, KBD_ALT_SETTING) })?;

    // Boot protocol gives the fixed 8-byte report; SET_IDLE(0) means "report
    // only on change" (no auto-repeat spam). A keyboard may STALL either — we
    // log and continue, since interface 0 reports boot format regardless.
    control_request(client, dev, &SET_PROTOCOL_BOOT, "SET_PROTOCOL(boot)")?;
    control_request(client, dev, &SET_IDLE_INFINITE, "SET_IDLE(0)")?;

    let xfer = start_report_polling(dev)?;
    log::info!("polling EP {KBD_EP_IN:#04x} — type on the keyboard");
    Ok((dev, xfer))
}

/// Open a device and print its descriptors over the console.
fn open_and_dump(
    client: usb_host_client_handle_t,
    addr: u8,
) -> Result<usb_device_handle_t, EspError> {
    log::info!("device attached at address {addr}; opening");
    let mut dev: usb_device_handle_t = ptr::null_mut();
    esp!(unsafe { usb_host_device_open(client, addr, &mut dev) })?;

    // usb_device_desc_t is a union { #[repr(C, packed)] struct; [u8; 18] }.
    // Copy the struct out and then each field into aligned locals — packed
    // fields can't be referenced (and the format machinery takes references).
    let mut dev_desc: *const usb_device_desc_t = ptr::null();
    esp!(unsafe { usb_host_get_device_descriptor(dev, &mut dev_desc) })?;
    let d = unsafe { (*dev_desc).__bindgen_anon_1 };
    let (vid, pid, class, sub, proto, ncfg) = (
        d.idVendor,
        d.idProduct,
        d.bDeviceClass,
        d.bDeviceSubClass,
        d.bDeviceProtocol,
        d.bNumConfigurations,
    );
    log::info!(
        "VID:PID {vid:04x}:{pid:04x}  class {class:02x}/{sub:02x}/{proto:02x}  {ncfg} configuration(s)"
    );
    unsafe { usb_print_device_descriptor(dev_desc) };

    let mut cfg_desc: *const usb_config_desc_t = ptr::null();
    esp!(unsafe { usb_host_get_active_config_descriptor(dev, &mut cfg_desc) })?;
    unsafe { usb_print_config_descriptor(cfg_desc, None) };

    Ok(dev)
}

/// Send an 8-byte control request (setup packet, no data stage) and block
/// until it completes, pumping client events so the callback can fire.
fn control_request(
    client: usb_host_client_handle_t,
    dev: usb_device_handle_t,
    setup: &[u8; 8],
    label: &str,
) -> Result<(), EspError> {
    let mut xfer: *mut usb_transfer_t = ptr::null_mut();
    esp!(unsafe { usb_host_transfer_alloc(64, 0, &mut xfer) })?;
    unsafe {
        let t = &mut *xfer;
        // First 8 bytes of a control transfer's buffer are the setup packet.
        core::ptr::copy_nonoverlapping(setup.as_ptr(), t.data_buffer, 8);
        t.num_bytes = 8; // setup packet only, no data stage
        t.device_handle = dev;
        t.bEndpointAddress = 0; // control endpoint EP0
        t.callback = Some(ctrl_cb);
        t.context = ptr::null_mut();
    }

    CTRL_DONE.store(false, Ordering::SeqCst);
    esp!(unsafe { usb_host_transfer_submit_control(client, xfer) })?;
    while !CTRL_DONE.load(Ordering::SeqCst) {
        unsafe { usb_host_client_handle_events(client, u32::MAX) };
    }

    let status = CTRL_STATUS.load(Ordering::SeqCst);
    unsafe { usb_host_transfer_free(xfer) };
    if status == usb_transfer_status_t_USB_TRANSFER_STATUS_COMPLETED as u32 {
        log::info!("{label} ok");
    } else {
        log::warn!("{label} completed with status {status} (continuing)");
    }
    Ok(())
}

/// Allocate and submit the interrupt-IN transfer for boot reports. The
/// `report_cb` resubmits it on each completion to keep polling.
fn start_report_polling(dev: usb_device_handle_t) -> Result<*mut usb_transfer_t, EspError> {
    let mut xfer: *mut usb_transfer_t = ptr::null_mut();
    esp!(unsafe { usb_host_transfer_alloc(BOOT_REPORT_LEN, 0, &mut xfer) })?;
    unsafe {
        let t = &mut *xfer;
        t.num_bytes = BOOT_REPORT_LEN as i32; // must be a multiple of wMaxPacketSize (8)
        t.device_handle = dev;
        t.bEndpointAddress = KBD_EP_IN;
        t.callback = Some(report_cb);
        t.context = ptr::null_mut();
    }
    esp!(unsafe { usb_host_transfer_submit(xfer) })?;
    Ok(xfer)
}

/// Decode an 8-byte HID boot keyboard report into modifiers + keycodes and log
/// it. Layout: [modifiers, reserved, key1..key6]; 0 means "no key".
fn decode_boot_report(report: &[u8]) {
    if report.len() < 3 {
        return;
    }
    let modifiers = report[0];
    const MOD_NAMES: [(u8, &str); 8] = [
        (0x01, "LCtrl"),
        (0x02, "LShift"),
        (0x04, "LAlt"),
        (0x08, "LGui"),
        (0x10, "RCtrl"),
        (0x20, "RShift"),
        (0x40, "RAlt"),
        (0x80, "RGui"),
    ];
    let mods: Vec<&str> = MOD_NAMES
        .iter()
        .filter(|(bit, _)| modifiers & bit != 0)
        .map(|(_, name)| *name)
        .collect();

    let keys: Vec<String> = report[2..]
        .iter()
        .filter(|&&k| k != 0)
        .map(|&k| keycode_name(k))
        .collect();

    if mods.is_empty() && keys.is_empty() {
        log::info!("report: (all keys released)");
    } else {
        log::info!("report: mods=[{}] keys=[{}]", mods.join("+"), keys.join(" "));
    }
}

/// Map a HID keyboard usage ID to a readable label. Covers the common range;
/// anything else falls back to hex.
fn keycode_name(k: u8) -> String {
    match k {
        0x04..=0x1d => ((b'a' + (k - 0x04)) as char).to_string(),
        0x1e..=0x26 => ((b'1' + (k - 0x1e)) as char).to_string(), // 1-9
        0x27 => "0".into(),
        0x28 => "Enter".into(),
        0x29 => "Esc".into(),
        0x2a => "Backspace".into(),
        0x2b => "Tab".into(),
        0x2c => "Space".into(),
        0x2d => "-".into(),
        0x2e => "=".into(),
        0x2f => "[".into(),
        0x30 => "]".into(),
        0x31 => "\\".into(),
        0x33 => ";".into(),
        0x34 => "'".into(),
        0x36 => ",".into(),
        0x37 => ".".into(),
        0x38 => "/".into(),
        0x39 => "CapsLock".into(),
        0x3a..=0x45 => format!("F{}", k - 0x3a + 1), // F1-F12
        0x4f => "Right".into(),
        0x50 => "Left".into(),
        0x51 => "Down".into(),
        0x52 => "Up".into(),
        _ => format!("0x{k:02x}"),
    }
}
