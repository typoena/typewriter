use std::rc::Rc;

use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{AnyIOPin, PinDriver, Pull};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::spi::config::{Config, DriverConfig};
use esp_idf_svc::hal::spi::{Dma, SpiBusDriver, SpiDriver};
use esp_idf_svc::hal::units::FromValueType;

use app::{FileIndex, Panel, Runtime};
use display::Frame;
use editor::{Editor, Prefs, Scope, Snippets, LOCAL_DIR, PREFS_PATH, SNIPPETS_PATH};
use firmware::drivers::clock_esp::{self, EspClock};
use firmware::drivers::keyboard_usb as usb_kbd;
use firmware::drivers::screen_epd::Epd;
use firmware::drivers::system_esp::EspSystem;
use firmware::infrastructure::file_index::EspFileWalk;
use firmware::infrastructure::net::NetService;
use firmware::infrastructure::panic_scribe;
use firmware::infrastructure::storage_sd::{SdStorage, Storage, CONF_PATH, NOTES};

/// Injected by build.rs so serial output identifies the exact build.
const BUILD_TAG: &str = concat!("build ", env!("BUILD_TIME"), " @", env!("BUILD_GIT"));

fn main() -> anyhow::Result<()> {
    // Required once before any esp-idf-svc call; some runtime patches
    // only link if this symbol is referenced. See esp-idf-template#71.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Typoena — modal editor (vim modes), {BUILD_TAG}");

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    // Positional arg order: SCK 12 · MOSI 11. CS 7 · DC 6 · RST 5 · BUSY 4 below.
    let spi = SpiDriver::new(
        peripherals.spi2,
        pins.gpio12,
        pins.gpio11,
        None::<AnyIOPin>,
        &DriverConfig::new().dma(Dma::Auto(4096)),
    )?;
    // SSD1683 takes 10–20 MHz. This clock sets only the pixel clock-out, not the
    // waveform BUSY time, so it trims the pre-kick band write (~43 ms at 4 MHz).
    // Sweeping higher risks signal-integrity glitches (garbled/missing bands).
    // See docs/tradeoff-curves/epd-refresh-latency.md.
    let bus = SpiBusDriver::new(spi, &Config::new().baudrate(20.MHz().into()))?;
    let cs = PinDriver::output(pins.gpio7)?;
    let dc = PinDriver::output(pins.gpio6)?;
    let rst = PinDriver::output(pins.gpio5)?;
    let busy = PinDriver::input(pins.gpio4, Pull::Down)?;
    let mut epd = Epd::new(bus, dc, rst, cs, busy);

    log::info!("EPD reset + init…");
    epd.reset()?;
    epd.init()?;
    // Async: the ~2.2 s full-refresh waveform runs while the SD mounts and the
    // note loads below. It writes both RAM banks, so it doubles as the baseline;
    // the first editor render waits it out (`wait_ready`) and replaces it.
    epd.display_frame_async(Frame::splash().bytes())?;

    // SD after the EPD, against the doc's SD-first boot order: a dead panel
    // can't explain a missing card. Fatal-by-design rationale: `boot_storage`.
    let storage = boot_storage(&mut epd);

    // Before the wizard gate — first-boot setup types on this keyboard.
    usb_kbd::start()?;

    // Device runtime config + the first-boot wizard gate. Whatever this block
    // yields is installed before the net thread spawns. Secrets stay out of the
    // log — only which keys exist.
    let (sys_loop, nvs, modem) = {
        use esp_idf_svc::eventloop::EspSystemEventLoop;
        use esp_idf_svc::nvs::EspDefaultNvsPartition;

        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;
        let mut modem = peripherals.modem;

        let card = match std::fs::read_to_string(CONF_PATH) {
            Ok(body) => conf::Conf::parse(&body),
            Err(_) => conf::Conf::default(),
        };
        let provided: Vec<&str> = conf::Field::ALL
            .iter()
            .filter(|f| !card.get(**f).trim().is_empty())
            .map(|f| f.conf_key())
            .collect();
        log::info!(
            "typoena.conf on card provides: {}",
            if provided.is_empty() { "nothing".into() } else { provided.join(", ") }
        );

        let effective = firmware::infrastructure::net::effective_conf_from(&card);
        let unconfigured = !effective.missing_required().is_empty() || !storage.repo_present();
        // `:setup` reboots into the wizard prefilled (the running editor can't
        // reclaim the radio from the net thread). One-shot: clear the marker on
        // read so a power-pull mid-setup boots the editor, not setup again.
        let setup_requested = storage.setup_requested();
        if setup_requested {
            storage.clear_setup_request();
        }
        let final_conf = if unconfigured || setup_requested {
            if unconfigured {
                log::info!("unconfigured card (conf incomplete or repo missing) — entering the onboarding wizard");
            } else {
                log::info!(":setup requested — reopening the wizard prefilled from the card conf");
            }
            // The wizard provisions the *card*, so it resumes from the card's own
            // state, never the baked fallback — which would skip the very steps a
            // blank card needs. `run` is opt-level 2; see its doc comment.
            match firmware::infrastructure::wizard_io::run(&mut epd, &storage, &card, setup_requested && !unconfigured, &sys_loop, &nvs, &mut modem) {
                Ok(c) => c,
                Err(e) => boot_halt(&mut epd, "Setup stopped", &format!("{e:#}")),
            }
        } else {
            card
        };
        firmware::infrastructure::net::set_card_conf(final_conf);
        (sys_loop, nvs, modem)
    };

    // Read before the boot buffer is chosen (`open_last_on_boot` decides which
    // file) and before the first render (`line_numbers` shapes the opening frame).
    let prefs = match storage.load_path(PREFS_PATH) {
        Ok(src) => Prefs::parse(&src),
        Err(_) => Prefs::default(),
    };
    log::info!("prefs: {prefs:?}");
    // Before anything reads the wall clock, so `localtime_r` — and thus the
    // `:inbox` note's dated name/title — reflects the local calendar day. Empty
    // (the default) leaves the ESP clock at UTC.
    if !prefs.timezone.is_empty() {
        clock_esp::apply_timezone(&prefs.timezone);
    }
    let (boot_path, boot_scope, saved) = boot_note(&mut epd, &storage, &prefs);

    // The net thread owns the Wi-Fi stack, brought up lazily on the first
    // request, so the radio stays off until you sync or update.
    let (net_tx, net_rx) = {
        use firmware::infrastructure::net::{run_net_service, NetOutcome, NetRequest, GIT_STACK};

        let (req_tx, req_rx) = std::sync::mpsc::channel::<NetRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<NetOutcome>();
        std::thread::Builder::new()
            .name("net".into())
            .stack_size(GIT_STACK)
            .spawn(move || run_net_service(modem, sys_loop, nvs, req_rx, res_tx))?;
        log::info!(
            "net thread up ({} KB stack); Wi-Fi comes up on the first :gs/:gl/:update",
            GIT_STACK / 1024
        );
        (req_tx, res_rx)
    };

    let mut ed = Editor::with_file(boot_path.clone(), boot_scope, saved);
    ed.set_prefs(prefs);
    ed.set_version(firmware::infrastructure::ota::FW_VERSION);
    let snippets = match storage.load_path(SNIPPETS_PATH) {
        Ok(src) => match Snippets::parse(&src) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("snippets parse FAILED ({e}); none loaded");
                Snippets::default()
            }
        },
        Err(_) => Snippets::default(),
    };
    log::info!("snippets: {} loaded", snippets.0.len());
    ed.set_snippets(snippets);

    // Seed both before the first render.
    ed.set_keyboard_present(usb_kbd::keyboard_present());
    ed.refresh_stats();

    // First editor render — the splash disappears here. `Panel::new` paints the
    // opening frame as a partial that first waits out the splash's waveform, so
    // the swap rides the partial instead of a second full refresh (~1.3 s saved).
    // The panel then owns the EPD and both reused framebuffers: a repaint never
    // allocates, so a background `:gs` can take the heap to the floor safely.
    let mut panel = Panel::new(epd, &mut ed)?;
    // Hardware entropy, so the face order differs run to run. Host builds skip
    // this and stay deterministic for the tests.
    panel.reseed_humor(unsafe { esp_idf_svc::sys::esp_random() });

    // Two clocks, ~1.4 s apart. `esp_log_timestamp()` counts from ~power-on — the
    // real cold-boot number. `esp_timer_get_time()` only starts after the
    // 2nd-stage bootloader + the ~0.74 s PSRAM memtest, so it is app-side init
    // only. "Cursor ready" = first editor frame on the panel.
    let total_ms = unsafe { esp_idf_svc::sys::esp_log_timestamp() };
    let app_ms = (unsafe { esp_idf_svc::sys::esp_timer_get_time() } / 1000) as u32;
    log::info!("boot: cursor ready — {total_ms} ms since power-on ({app_ms} ms app-side)");

    // Reaching cursor-ready is the health bar: on the first boot after an OTA
    // `:update` this cancels the pending rollback, otherwise it is a logged no-op.
    firmware::infrastructure::ota::mark_running_firmware_valid();

    // AFTER the first editor frame: the seconds-long readdir over SPI would
    // otherwise starve the boot-critical SD reads and delay the first paint.
    let files = EspFileWalk::new();
    files.request_rewalk();

    // Every adapter below runs on this single UI task, so `Rc` (not `Arc`) is enough.
    let card = Rc::new(storage);

    let net: Box<dyn app::NetService> =
        Box::new(NetService::new(card.clone(), net_tx, net_rx));
    let system: Box<dyn app::System> = Box::new(EspSystem(card.clone()));

    // The only exits are `:reboot`/`:setup`, which restart the device — so `run`
    // never returns.
    let mut runtime = Runtime::new(
        ed,
        panel,
        Box::new(usb_kbd::UsbKeyboard),
        Box::new(SdStorage(card.clone())),
        net,
        Box::new(EspClock),
        system,
        Box::new(files),
    );

    if std::path::Path::new(panic_scribe::EMERGENCY_PATH).exists() {
        log::warn!(
            "panic dump from a previous crash present: {}",
            panic_scribe::EMERGENCY_PATH
        );
    }
    // Arm the panic scribe now that `runtime` has its final address (`run`
    // borrows it until the device reboots, so it never moves again).
    panic_scribe::arm(&raw const runtime as *const (), |p| {
        // SAFETY: `arm`'s contract — only ever called back on THIS thread,
        // halted at the panic site, so the `&mut` inside `run` is suspended and
        // the read cannot race. The `'static` stands in for the Epd's true
        // pin-borrow lifetime: this stack frame never returns.
        let rt = unsafe { &*(p as *const Runtime<Epd<'static>>) };
        rt.scribe_snapshot().map(|(path, text)| (path.to_string(), text.to_string()))
    });
    runtime.run()
}

/// Mount the SD card, or halt with the reason on the panel. A missing CARD is
/// fatal by design: the note is the whole point of the appliance, so we refuse to
/// run in a state where the next save could destroy it. A missing REPO is not
/// fatal — the wizard gate in `main` enters first-boot setup instead.
fn boot_storage(epd: &mut Epd) -> Storage {
    // The firmware shares this mount with the net thread, and libgit2 keeps the
    // pack + idx descriptors open across a push — that overruns the editor's
    // tight 4-FD budget, so mount with the 16-FD one (persistence.rs,
    // MAX_FILES_GIT).
    match Storage::mount_for_git() {
        Ok(s) => s,
        Err(e) => boot_halt(epd, "SD card not ready", &format!("{e:#}")),
    }
}

/// Choose and load the boot buffer. With `open_last_on_boot` set and a marker
/// naming a still-existing file (`Storage::last_file`), resume that file;
/// otherwise the default note. Only the default note is fatal (`boot_halt`) —
/// a stale or unreadable last file falls back rather than refusing to boot.
fn boot_note(epd: &mut Epd, storage: &Storage, prefs: &Prefs) -> (String, Scope, String) {
    if prefs.open_last_on_boot {
        if let Some(path) = storage.last_file() {
            match storage.load_path(&path) {
                Ok(text) => {
                    log::info!("boot: resumed {path} ({} bytes)", text.len());
                    let scope = if path.starts_with(LOCAL_DIR) { Scope::Local } else { Scope::Tracked };
                    return (path, scope, text);
                }
                // Unreadable (e.g. grown past MAX_FILE_BYTES on a computer) —
                // the default note still boots.
                Err(e) => log::warn!("boot: can't resume {path} ({e:#}); falling back to {NOTES}"),
            }
        }
    }
    let note = match storage.load() {
        Ok(text) => text,
        Err(e) => boot_halt(epd, "Could not read your note", &format!("{e:#}")),
    };
    log::info!("boot: loaded {} bytes from {NOTES}", note.len());
    (NOTES.to_string(), Scope::Tracked, note)
}

/// Show a terminal boot error on the panel and idle forever. Rebooting into the
/// same missing card would just thrash, so we stop and explain instead.
fn boot_halt(epd: &mut Epd, headline: &str, detail: &str) -> ! {
    log::error!("boot halt — {headline}: {detail}");
    if let Err(e) = show_message(epd, &format!("{headline}\n\n{detail}\n")) {
        log::error!("(could not paint the boot error either: {e:#})");
    }
    loop {
        FreeRtos::delay_ms(1000);
    }
}

/// Render a plain full-frame message by borrowing the editor purely as a
/// text-layout engine, so boot failures surface on the panel, not a dead screen.
fn show_message(epd: &mut Epd, msg: &str) -> anyhow::Result<()> {
    let frame = Editor::with_text(msg.to_string()).draw(false);
    epd.display_frame(frame.bytes())?;
    Ok(())
}
