//! System-control driver: reboot + first-boot setup, behind [`app::System`].
//!
//! The `git` build's `EspSystem` writes the one-shot setup marker (so the boot
//! gate re-enters the wizard) and restarts the chip; the light build's
//! `NullSystem` has no wizard to reboot into but still restarts on `:reboot`.

/// [`app::System`] for a full build: `:setup` writes the boot marker (then the
/// caller reboots into the wizard), and `reboot` restarts the chip.
#[cfg(feature = "git")]
pub struct EspSystem(pub std::rc::Rc<crate::infrastructure::storage_sd::Storage>);

#[cfg(feature = "git")]
impl app::System for EspSystem {
    fn prepare_setup(&self) -> app::SetupDispatch {
        // One-shot marker: the boot gate re-enters the wizard prefilled. The
        // running editor can't reclaim the radio from the git thread, so `:setup`
        // reboots rather than reopening in place.
        match self.0.request_setup() {
            Ok(()) => app::SetupDispatch::Ready,
            Err(e) => {
                log::warn!(":setup marker write failed: {e:#}");
                app::SetupDispatch::MarkerFailed
            }
        }
    }
    fn reboot(&self) -> ! {
        // esp_restart resets the chip and does not return; the loop makes the
        // divergence explicit to the type system.
        loop {
            unsafe { esp_idf_svc::sys::esp_restart() };
        }
    }
}

/// [`app::System`] for a light build: no wizard to reboot into, but `:reboot`
/// still restarts the chip.
#[cfg(not(feature = "git"))]
pub struct NullSystem;

#[cfg(not(feature = "git"))]
impl app::System for NullSystem {
    fn prepare_setup(&self) -> app::SetupDispatch {
        app::SetupDispatch::Unsupported
    }
    fn reboot(&self) -> ! {
        loop {
            unsafe { esp_idf_svc::sys::esp_restart() };
        }
    }
}
