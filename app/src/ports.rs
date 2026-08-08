//! Application ports — the frontiers the run-loop [`Runtime`](crate::Runtime)
//! depends on.
//!
//! Each trait is a contract the application layer needs and the outer firmware
//! layer fulfils (dependency inversion: the port is owned by the consumer). The
//! esp-idf adapters live in the `firmware` crate and are injected at
//! composition; the `Runtime` names only these traits, never a concrete type,
//! so it builds and is tested on the host with in-memory doubles.
//!
//! The hardware-*device* ports ([`Screen`](hal::Screen),
//! [`Keyboard`](hal::Keyboard)) live one layer down in the `hal` crate; the
//! ports here are application/infrastructure capabilities — persistence, the
//! sync transport, the wall clock, platform lifecycle, and the file index.

use editor::{Date, PullIntent, Unsynced};

/// Durable storage of buffers on the card — the byte-level file operations the
/// loop performs. The dirty-path journal that couples a save to a later push
/// lives behind [`NetService`], not here.
pub trait Storage {
    /// Atomically write `contents` to `path`. Errors are surfaced, not fatal:
    /// the in-RAM buffer stays the source of truth for a retry.
    fn save_path(&self, path: &str, contents: &str) -> anyhow::Result<()>;
    /// Read `path` from the card.
    fn load_path(&self, path: &str) -> anyhow::Result<String>;
    /// Unlink `path` from the card.
    fn delete_path(&self, path: &str) -> anyhow::Result<()>;
    /// Record the active file, for the `open_last_on_boot` resume marker.
    fn record_last_file(&self, path: &str);
}

/// What dispatching a push (`:gs`) did — the loop maps this to a snackbar.
pub enum PushDispatch {
    /// Handed to the sync backend; the result arrives later via
    /// [`NetService::poll_outcome`].
    Dispatched,
    /// The backend is gone (thread down); nothing will report back.
    ThreadDown,
}

/// What dispatching a pull (`:gl`) did.
pub enum PullDispatch {
    Dispatched,
    /// The dirty journal is non-empty, so a bare [`PullIntent::Ask`] didn't
    /// dispatch: pulling would fold those saved-but-unpushed paths into a local
    /// commit first, and that commit is user-visible. Carries the journal's
    /// paths so the UI can *name* them on the unsynced card rather than just
    /// announce that some exist. The card's answer comes back as a second pull
    /// carrying [`Commit`](PullIntent::Commit) or
    /// [`Discard`](PullIntent::Discard).
    NeedsConfirm(Vec<Unsynced>),
    ThreadDown,
}

/// What dispatching a firmware update (`:update`) did. The "is a newer release
/// available" question needs the network, so it is answered later in
/// [`UpdateOutcome`], not here — dispatch only reports whether the request
/// reached the background thread.
pub enum UpdateDispatch {
    /// Handed to the background thread; the result arrives later via
    /// [`NetService::poll_outcome`].
    Dispatched,
    /// The backend is gone (thread down); nothing will report back.
    ThreadDown,
}

/// A completed push, mirrored from the git transport into a git-free shape so
/// the app layer stays pure.
pub enum PushOutcome {
    /// Pushed a new commit — the short oid.
    Pushed(String),
    UpToDate,
    /// Failed — a ready-to-show reason string.
    Failed(String),
}

/// A completed pull.
pub enum PullOutcome {
    Pulled(String),
    Rebased(String),
    UpToDate,
    LocalAhead,
    Failed(String),
}

/// A completed firmware update (`:update`).
pub enum UpdateOutcome {
    /// A newer image was fetched and written to the inactive OTA slot, which is
    /// now the boot target. Carries the new version string for the notice; the
    /// caller paints it and reboots into the new firmware.
    Installed(String),
    /// The running firmware is already the newest release — nothing to install.
    /// Carries the running version, shown in the notice.
    UpToDate(String),
    /// Something failed (no newer image found is *not* a failure — that is
    /// [`UpToDate`](UpdateOutcome::UpToDate)); the string is a short reason for
    /// the panel (full error is logged). The running slot is untouched.
    Failed(String),
}

/// The outcome of a finished background operation on the radio-owning thread.
pub enum NetOutcome {
    Push(PushOutcome),
    Pull(PullOutcome),
    Update(UpdateOutcome),
    /// A short status line from an operation still in flight — the panel's only
    /// sign of life through the multi-second grind (`syncing...` otherwise sits
    /// unchanged from dispatch to outcome). Non-terminal: it settles nothing, so
    /// more messages follow, ending in one of the variants above.
    ///
    /// Deliberately *not* a timer-driven spinner: every line repaints the whole
    /// panel (~630 ms of e-paper drive, one of the 64-partial ghosting budget),
    /// so a line has to earn its place by reporting real state. The backend gates
    /// them; the adapter coalesces a queued burst.
    Progress(String),
}

/// Everything the radio-owning background thread does: the git push/pull
/// transport (plus the dirty-path journal that gates it) and firmware update
/// over the air. All three share the one thread because the device has a single
/// Wi-Fi modem the editor loop cannot reclaim — so they multiplex over one
/// dispatch/outcome channel rather than each owning a radio. Fire-and-forget:
/// [`push`](NetService::push) / [`pull`](NetService::pull) /
/// [`update`](NetService::update) dispatch, and the result returns later via
/// [`poll_outcome`](NetService::poll_outcome). The backend owns the dirty
/// journal — it takes the pending paths on push (and on a committing pull)
/// and settles them when the outcome lands — so the app layer never touches it.
pub trait NetService {
    /// Dispatch a push of the whole Tracked working copy.
    fn push(&self) -> PushDispatch;
    /// Dispatch a fetch + fast-forward/rebase pull. [`Ask`](PullIntent::Ask) is
    /// a bare `:gl`: if the dirty journal is non-empty it returns
    /// [`NeedsConfirm`](PullDispatch::NeedsConfirm) with the journal's paths
    /// instead of dispatching, so the UI can show them and ask.
    /// [`Commit`](PullIntent::Commit) folds the journal into a local commit,
    /// then pulls. [`Discard`](PullIntent::Discard) instead rolls the journal's
    /// paths back to their last-synced state — restoring them from HEAD, and
    /// unlinking the ones HEAD never had — then pulls; the work in them is gone.
    ///
    /// A discard settles the journal even when the pull that follows it fails:
    /// the rollback is local and already done by then, so those paths are
    /// genuinely no longer dirty.
    fn pull(&self, intent: PullIntent) -> PullDispatch;
    /// Dispatch a firmware-update check: fetch the latest release, and if it is
    /// newer than the running image, download it into the inactive OTA slot and
    /// make that the boot target. Reports back via [`UpdateOutcome`] — the caller
    /// reboots on [`Installed`](UpdateOutcome::Installed).
    fn update(&self) -> UpdateDispatch;
    /// Non-blocking poll for a finished operation. The backend has already
    /// settled the dirty journal by the time this returns.
    fn poll_outcome(&self) -> Option<NetOutcome>;
}

/// The wall clock and the idle CPU-yield the loop needs. `today` is `None` until
/// the clock is trustworthy — there is no battery-backed RTC, so it sits at the
/// epoch until the first sync sets it (see [`editor::Date`]).
pub trait Clock {
    /// Today's calendar day, or `None` while the clock is unset.
    fn today(&self) -> Option<Date>;
    /// Briefly yield the CPU when the idle loop has nothing to paint.
    fn idle_yield(&self);
}

/// What preparing a `:setup` reboot did.
pub enum SetupDispatch {
    /// Marker written; the caller paints the notice, then calls
    /// [`System::reboot`].
    Ready,
    /// Could not persist the setup marker — stay put and report it.
    MarkerFailed,
}

/// Platform lifecycle: the device restart, and preparing a reboot-into-setup.
pub trait System {
    /// Prepare a `:setup` reboot (persist the boot marker). See [`SetupDispatch`].
    fn prepare_setup(&self) -> SetupDispatch;
    /// Restart the device. Never returns.
    fn reboot(&self) -> !;
}

/// The palette's background file index — a recursive walk of the card, run off
/// the UI loop on its own thread. [`request_rewalk`](FileIndex::request_rewalk)
/// kicks a fresh walk; [`poll_result`](FileIndex::poll_result) picks up a
/// finished one as a newline-joined path blob.
pub trait FileIndex {
    /// Spawn a fresh walk (at boot, and after a pull moves the working copy).
    fn request_rewalk(&self);
    /// A finished walk's path blob, if one is ready.
    fn poll_result(&self) -> Option<String>;
}
