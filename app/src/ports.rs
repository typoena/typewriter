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

use editor::{Battery, Date, PullIntent, Unsynced};

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

/// What dispatching either half of a firmware update — the check or the
/// confirmed install — did. The "is a newer release available" question needs
/// the network, so it is answered later in [`UpdateOutcome`], not here; dispatch
/// only reports whether the request reached the background thread.
pub enum UpdateDispatch {
    /// Handed to the background thread; the result arrives later via
    /// [`NetService::poll_outcome`].
    Dispatched,
    /// The backend is gone (thread down); nothing will report back.
    ThreadDown,
}

/// What dispatching a clock-only sync ([`editor::Effect::SyncClock`]) did. The
/// sync itself is Wi-Fi + SNTP and nothing else — no fetch, no TLS handshake to
/// the git remote — so it is the cheapest thing the radio thread can be asked
/// to do, and what lets `:inbox` date a note without paying for a pull.
pub enum ClockDispatch {
    /// Handed to the radio thread; the result arrives later via
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

/// A completed half of a firmware update — one of these settles a
/// [`check`](NetService::check_update), the other a
/// [`install`](NetService::install_update).
pub enum UpdateOutcome {
    /// The check found a newer release. Carries its version; nothing has been
    /// downloaded, and the caller raises the install prompt naming it.
    Available(String),
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

/// A completed clock-only sync. Carries no date: the wall clock is the transport
/// here, and the loop reads the day back through [`Clock::today`] as it does
/// every pass.
pub enum ClockOutcome {
    /// The wall clock now holds a real date (it may already have held one — an
    /// ask that found the clock set is a success, not a special case).
    Synced,
    /// No date — a ready-to-show reason string (no Wi-Fi, SNTP timed out).
    Failed(String),
}

/// The outcome of a finished background operation on the radio-owning thread.
pub enum NetOutcome {
    Push(PushOutcome),
    Pull(PullOutcome),
    Update(UpdateOutcome),
    Clock(ClockOutcome),
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
/// transport (plus the dirty-path journal that gates it), firmware update over
/// the air, and the clock-only sync `:inbox` needs for today's date. They share
/// the one thread because the device has a single Wi-Fi modem the editor loop
/// cannot reclaim — so they multiplex over one dispatch/outcome channel rather
/// than each owning a radio. Fire-and-forget: [`push`](NetService::push) /
/// [`pull`](NetService::pull) / [`update`](NetService::update) /
/// [`sync_clock`](NetService::sync_clock) dispatch, and the result returns later
/// via [`poll_outcome`](NetService::poll_outcome). The backend owns the dirty
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
    /// Dispatch a clock-only sync: join Wi-Fi and run SNTP, nothing else. The
    /// cheap half of a sync's bring-up (no fetch, no git, no TLS to the remote),
    /// so a writer who only needs today's date pays seconds instead of the tens
    /// a pull costs. Reports back via [`ClockOutcome`]. The loop dispatches one
    /// unprompted at boot and one per `:inbox` that finds the clock unset.
    fn sync_clock(&self) -> ClockDispatch;
    /// Dispatch a firmware-update check: read the release manifest and compare
    /// it against the running image. Downloads nothing and touches no OTA slot —
    /// it answers [`Available`](UpdateOutcome::Available) or
    /// [`UpToDate`](UpdateOutcome::UpToDate), and the install is a separate ask.
    fn check_update(&self) -> UpdateDispatch;
    /// Dispatch the confirmed install of `version`: download that release into
    /// the inactive OTA slot and make that the boot target. Reports back via
    /// [`UpdateOutcome`] — the caller reboots on
    /// [`Installed`](UpdateOutcome::Installed).
    fn install_update(&self, version: String) -> UpdateDispatch;
    /// Non-blocking poll for a finished operation. The backend has already
    /// settled the dirty journal by the time this returns.
    fn poll_outcome(&self) -> Option<NetOutcome>;
}

/// The wall clock and the idle CPU-yield the loop needs. `today` is `None` until
/// the clock is trustworthy — there is no battery-backed RTC, so it sits at the
/// epoch until SNTP sets it this power cycle (see [`editor::Date`]); the loop
/// asks for that through [`NetService::sync_clock`].
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

/// What the power hardware has to say this pass — the switch and the cell, the
/// two things that can interrupt a writing session from below the firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    /// The power switch was flipped to off. It has two states and no press to
    /// measure, so this is an edge, not a gesture — and there is no counterpart
    /// event for "flipped on", because that one arrives as a boot.
    SwitchedOff,
    /// The cell just fell under the warn threshold. Raised once per crossing, so
    /// a session spent in the twenties is warned about once, not every poll.
    Low,
    /// The cell is nearly flat. The loop shuts down on this exactly as it would
    /// on [`SwitchedOff`](Self::SwitchedOff) — a save and an off card now beat a
    /// brownout mid-write later.
    Critical,
}

/// The charger, the cell and the physical power switch.
///
/// One port rather than three: they are one chip and one switch on the board,
/// they are polled together, and the only caller is the run loop's per-pass
/// [`poll`](Self::poll). A board whose charger never answered on the bus still
/// fulfils this — [`status`](Self::status) stays `None` and the switch half
/// keeps working.
pub trait Power {
    /// Poll the switch and (on its own schedule) the charger. Returns at most
    /// one event per pass; the loop calls this every iteration, so it must never
    /// block on the I2C bus for longer than a keystroke can wait.
    fn poll(&mut self) -> Option<PowerEvent>;

    /// The latest cell reading, or `None` before the first conversion lands —
    /// and for the whole session on a board with no charger on the bus.
    fn status(&self) -> Option<Battery>;

    /// Cut the rails and stop. Never returns: the machine comes back only
    /// through a fresh boot, so the caller must already have saved and painted.
    fn power_off(&mut self) -> !;
}

/// Cell internal resistance, used to back out the IR drop and estimate the
/// resting voltage the charge curve is written against. A charging cell reads
/// high by `ichg × r` — at 1.8 A that is most of 200 mV, which is 20 points of
/// state of charge, so ignoring it would have the gauge sit near full for most
/// of a charge. Estimated for a 3700 mAh pouch; measure it at the bench and
/// correct this one number.
const CELL_MILLIOHMS: u32 = 100;

/// Resting open-circuit voltage against state of charge for a single LiPo cell,
/// highest first. Interpolated between the points; the curve is flat in the
/// middle and steep at both ends, which is why it is a table and not a line.
const SOC_CURVE: [(u16, u8); 16] = [
    (4150, 100),
    (4100, 95),
    (4050, 90),
    (4000, 85),
    (3950, 78),
    (3900, 70),
    (3850, 62),
    (3800, 55),
    (3750, 47),
    (3700, 40),
    (3650, 32),
    (3600, 25),
    (3550, 18),
    (3500, 12),
    (3400, 5),
    (3300, 0),
];

/// State of charge from the cell's terminal voltage.
///
/// A voltage gauge, not a coulomb counter: the board carries no fuel-gauge IC,
/// so the charger's ADC is all any [`Power`] implementation has to work with.
/// Current through the cell's own resistance offsets the terminal voltage from
/// the resting voltage [`SOC_CURVE`] is written against, so charging backs that
/// offset out (see [`CELL_MILLIOHMS`]). Discharge is left uncorrected — the load
/// swings between a few milliamps idle and several hundred during a panel
/// refresh, so a correction there would track the refresh, not the cell.
pub fn state_of_charge(vbat_mv: u16, ichg_ma: u16, charging: bool) -> u8 {
    let resting = if charging {
        let drop = (u32::from(ichg_ma) * CELL_MILLIOHMS / 1000) as u16;
        vbat_mv.saturating_sub(drop)
    } else {
        vbat_mv
    };
    let Some(&(top_mv, top_pct)) = SOC_CURVE.first() else {
        return 0;
    };
    if resting >= top_mv {
        return top_pct;
    }
    for pair in SOC_CURVE.windows(2) {
        let [(hi_mv, hi_pct), (lo_mv, lo_pct)] = pair else {
            continue;
        };
        if resting >= *lo_mv {
            let span = u32::from(hi_mv - lo_mv);
            let into = u32::from(resting - lo_mv);
            let pct = u32::from(*lo_pct) + into * u32::from(hi_pct - lo_pct) / span;
            return pct.min(100) as u8;
        }
    }
    0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_charge_curve_runs_from_empty_to_full_without_stepping_back() {
        let mut last = 0;
        for mv in (3200..=4200).step_by(10) {
            let pct = state_of_charge(mv, 0, false);
            assert!(pct >= last, "{mv} mV read {pct}% after {last}%");
            last = pct;
        }
        assert_eq!(state_of_charge(3200, 0, false), 0);
        assert_eq!(state_of_charge(4200, 0, false), 100);
    }

    #[test]
    fn charging_backs_out_the_ir_drop() {
        // 4.0 V at 1.8 A is a cell resting near 3.82 V, not one near full.
        assert!(state_of_charge(4000, 1800, true) < state_of_charge(4000, 0, false));
    }
}
