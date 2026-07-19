//! Hardware abstraction layer — the device-capability frontier.
//!
//! Pure trait definitions ("ports") for the hardware devices the render/run
//! loop drives, expressed in generic vocabulary with no esp-idf types. The
//! concrete esp-idf implementations live in the `firmware` crate's drivers and
//! are injected at composition time; the layers above depend only on these
//! contracts, never on who fulfils them.
//!
//! This mirrors the `hal/` layer of the C `../typing-machine` reference:
//! interface definitions only, no implementation, no outward dependencies.
//! Deliberately narrow — only the seams that were previously welded to concrete
//! hardware types (the `Epd` panel driver, the USB-host key queue) get a port
//! here. A capability that is inseparably fused with one adapter (e.g. the
//! Wi-Fi radio owned by the git sync service) is *not* abstracted; forcing a
//! port there would be fiction.

// Re-exported so a `Keyboard` implementor can name the event type through the
// frontier (`hal::Key`) without depending on `keymap` directly.
pub use keymap::Key;

/// A 1-bit e-paper panel the render engine paints whole framebuffers onto.
///
/// The framebuffer bytes are the panel's native layout (see `display::Frame`);
/// this port carries only the two refresh calls the render engine actually
/// makes. [`Error`](Screen::Error) absorbs the driver's error type (e.g.
/// esp-idf's `EspError`) so nothing above this layer has to name it.
pub trait Screen {
    /// The driver's refresh error type. Bounded `Display` so the render engine
    /// can log a failed refresh (`"… FAILED ({e})"`) without naming the concrete
    /// type — esp-idf's `EspError` satisfies it, as does `core::convert::Infallible`
    /// for a test double.
    type Error: core::fmt::Display;

    /// Blit and full-refresh the whole framebuffer (792×272, `FB_BYTES` long).
    fn display_frame(&mut self, fb: &[u8]) -> Result<(), Self::Error>;

    /// Partial-refresh only rows `y0..y0+h` from a full framebuffer — the fast
    /// per-keystroke path (pass `(0, HEIGHT)` for the whole panel).
    fn display_frame_partial_window(
        &mut self,
        fb: &[u8],
        y0: u16,
        h: u16,
    ) -> Result<(), Self::Error>;

    /// Like [`display_frame_partial_window`](Screen::display_frame_partial_window),
    /// but drives an *accelerated* waveform when the panel has one (the render
    /// engine calls this only for the per-keystroke windowed-additive repaint, and
    /// only when the `fast_partial` pref is on). The default delegates to the plain
    /// partial, so a panel without a custom fast waveform — or a test double — is
    /// simply not accelerated rather than needing its own implementation.
    fn display_frame_partial_window_fast(
        &mut self,
        fb: &[u8],
        y0: u16,
        h: u16,
    ) -> Result<(), Self::Error> {
        self.display_frame_partial_window(fb, y0, h)
    }
}

/// The keyboard as an event source: decoded key events plus attach state.
///
/// Implementations bridge a hardware keyboard (e.g. the USB-host boot keyboard)
/// to a queue of decoded [`Key`]s. `next_key` pops the next queued event;
/// `keyboard_present` reports whether a keyboard is currently attached (drives
/// the side-panel disconnect flag).
pub trait Keyboard {
    /// The next decoded key event, or `None` when the queue is empty.
    fn next_key(&mut self) -> Option<Key>;

    /// Whether a keyboard is currently attached.
    fn keyboard_present(&self) -> bool;
}
