// epd is proven (Spike 2) but unused by the Spike 4 harness; keep it compiled
// so it doesn't bit-rot while USB host bring-up is in progress.
#[allow(dead_code)]
mod epd;
mod usb_kbd;

/// Injected by build.rs so serial output identifies the exact build.
const BUILD_TAG: &str = concat!("build ", env!("BUILD_TIME"), " @", env!("BUILD_GIT"));

fn main() -> anyhow::Result<()> {
    // Required once before any esp-idf-svc call; some runtime patches
    // only link if this symbol is referenced. See esp-idf-template#71.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Typoena Spike 4 — USB host keyboard, {BUILD_TAG}");
    log::info!("Plug the keyboard into the native USB port, then type.");

    // Runs forever: installs the USB Host Library and logs descriptors of any
    // device that attaches. Returns only on a fatal host-library error.
    usb_kbd::run()?;
    Ok(())
}
