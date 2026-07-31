//! Panic scribe — last-chance flush of the active buffer when the firmware
//! panics.
//!
//! `save_on_idle` only fires at a typing pause (`app::Runtime`), so a panic
//! mid-flow would otherwise lose the whole unbroken burst since the last pause.
//! The hook dumps the dirty buffer to [`EMERGENCY_PATH`] — never the buffer's
//! own path, so a suspect panic-time buffer can never clobber the good copy
//! from the last save — then lets esp-idf's abort handler reboot as usual.
//! Recovery is manual: the dump sits on the card root; boot logs a pointer
//! when one exists (`main`).

use std::sync::OnceLock;
use std::thread::ThreadId;

/// Card-root dump target, next to `typoena.conf` — outside `/sd/repo`, so a
/// dump is never swept into a sync commit.
pub const EMERGENCY_PATH: &str = "/sd/typoena-emergency.md";

/// Reads the runtime's dirty active buffer: `Some((path, text))`, or `None`
/// when clean. Monomorphized over the concrete `Runtime` at the arm site.
type Snap = fn(*const ()) -> Option<(String, String)>;

/// `(UI thread, runtime address, snapshot reader)`. The address is a `usize`
/// only because raw pointers aren't `Send`/`Sync`; see [`arm`] for validity.
static SCRIBE: OnceLock<(ThreadId, usize, Snap)> = OnceLock::new();

/// Arm the scribe around the assembled runtime. Contract: call on the UI
/// thread (the editor's owner) once `rt` has its final address — `Runtime::run`
/// never returns, so the pointer stays valid for the device's lifetime — and
/// `snap(rt)` is only ever invoked back on this same thread, halted at the
/// panic site, so its read of the runtime cannot race the `&mut` inside `run`.
pub fn arm(rt: *const (), snap: Snap) {
    if SCRIBE.set((std::thread::current().id(), rt as usize, snap)).is_err() {
        return;
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Report first — the flush below is best-effort and may itself fail.
        prev(info);
        let Some(&(ui_thread, rt, snap)) = SCRIBE.get() else { return };
        // A panic on any other thread (net, file walk, USB pumps) can't have
        // corrupted the buffer, and reading the editor from here would race
        // the still-running UI thread — skip; the last save stands.
        if std::thread::current().id() != ui_thread {
            return;
        }
        if let Some((path, text)) = snap(rt as *const ()) {
            let name = if path.is_empty() { "(unnamed)" } else { &path };
            let dump = format!("<!-- typoena panic dump of {name} -->\n{text}");
            match std::fs::write(EMERGENCY_PATH, dump) {
                Ok(()) => log::error!("panic scribe: buffer dumped to {EMERGENCY_PATH}"),
                Err(e) => log::error!("panic scribe: dump FAILED ({e})"),
            }
        }
    }));
}
