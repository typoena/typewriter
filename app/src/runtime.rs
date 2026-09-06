//! The editor run loop, lifted from `firmware/src/main.rs` into a host-testable
//! application service.
//!
//! [`Runtime`] owns the [`Editor`], the [`Panel`], the [`FocusTimer`], and every
//! injected port, and drives one iteration per [`tick`](Runtime::tick). It is a
//! faithful lift of what used to be inline in the binary's loop — the same key
//! drain, the same effect servicing, the same idle sequence and ordering — with
//! the concrete hardware calls replaced by port calls. The composition root
//! (the firmware binary) keeps only what is genuinely platform: bringing up the
//! peripherals, spawning the git and file-walk threads, and choosing which port
//! adapters to inject.
//!
//! It is generic only over the [`Screen`] (the per-pixel hot path stays static
//! dispatch, via [`Panel`]); the remaining ports are trait objects built once at
//! composition, so the struct has a single type parameter and the tests inject
//! in-memory doubles.

use std::collections::VecDeque;
use std::time::Instant;

use display::Frame;
use editor::{Editor, Effect, Mode, NetFlag, Prefs, PullIntent, Scope, PREFS_PATH, REPO_DIR};
use hal::{Keyboard, Screen};

use crate::ports::{
    Clock, ClockDispatch, ClockOutcome, FileIndex, PushDispatch, PushOutcome, PullDispatch,
    PullOutcome, SetupDispatch, Storage, NetOutcome, NetService, System, UpdateDispatch,
    UpdateOutcome,
};
use crate::render::{FocusTimer, Panel};

/// How long input must pause before `save_on_idle` persists a dirty buffer. The
/// save is silent (no snackbar, no forced e-ink flash) — a safety net against
/// power loss, not a user action — so unlike the caret it can fire during a
/// mid-sentence pause.
const IDLE_SAVE_MS: u128 = 1500;

/// How long a dispatched sync may go *silent* — no progress line, no outcome —
/// before the loop gives up on it. Every progress line refreshes the deadline,
/// so a slow-but-live fetch keeps its hold-off however long it runs; only a git
/// thread that died without reporting expires, and it must not leave the panel
/// claiming a sync or the save-on-idle net off for the rest of the session.
const SYNC_HOLDOFF_MS: u128 = 120_000;

/// The editor run loop: owns the editor, the panel, and the injected ports.
pub struct Runtime<S: Screen> {
    ed: Editor,
    panel: Panel<S>,
    keyboard: Box<dyn Keyboard>,
    storage: Box<dyn Storage>,
    net: Box<dyn NetService>,
    clock: Box<dyn Clock>,
    system: Box<dyn System>,
    files: Box<dyn FileIndex>,
    /// Focus-mode (Pomodoro) block timer — off until `:focus`.
    focus: FocusTimer,
    /// Monotonic time of the last keystroke, for the caret / save-on-idle /
    /// longevity debounces.
    last_activity: Instant,
    /// Whether `save_on_idle` already persisted the current idle window, so it
    /// fires once per typing burst; reset on the next activity.
    idle_saved: bool,
    /// What the last-file marker was last written with — starts empty so the
    /// first pass records the boot buffer.
    last_file: String,
    /// Keyboard attach state, for the panel disconnect flag.
    last_kbd: bool,
    /// The current-pass keyboard state and whether it changed (set in `tick`,
    /// read by the idle branch's kbd-flag repaint).
    kbd: bool,
    kbd_changed: bool,
    /// Dispatched push/pull/update operations whose outcome is still
    /// outstanding, oldest first — the git thread takes an unbounded request
    /// queue, so a `:gl` dispatched behind a `:gs` overlaps it and the push's
    /// outcome must not settle the pull. Each entry is that operation's
    /// last-heard-from instant, refreshed by its progress lines and expired at
    /// [`SYNC_HOLDOFF_MS`]. Non-empty raises the panel's `Syncing` flag and, for
    /// a repo buffer, holds off the idle-save in [`idle_step`](Self::idle_step)
    /// — the writer's evidence that a sync is running is exactly the state that
    /// protects it.
    net_in_flight: VecDeque<(Instant, NetFlag)>,
    /// Absolute paths of an in-flight [`PullIntent::Discard`], captured from
    /// the unsynced card at dispatch. Consumed when the pull outcome lands, to
    /// evict the resident buffers whose text the discard threw away. Empty
    /// except across that one round trip.
    pending_discard: Vec<String>,
}

impl<S: Screen> Runtime<S> {
    /// Assemble the runtime after boot: the editor is already seeded (boot note,
    /// prefs, snippets, keyboard flag, first stats) and the panel has painted the
    /// first frame. Seeds the loop bookkeeping from the keyboard's current state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ed: Editor,
        panel: Panel<S>,
        keyboard: Box<dyn Keyboard>,
        storage: Box<dyn Storage>,
        net: Box<dyn NetService>,
        clock: Box<dyn Clock>,
        system: Box<dyn System>,
        files: Box<dyn FileIndex>,
    ) -> Self {
        // A cold boot has no date (no battery-backed RTC) and `:inbox` needs one,
        // so ask for the clock unprompted rather than make the first fleeting note
        // of the session wait on it. The whole cost here is a channel send: the
        // composition root builds the runtime *after* the first editor frame, and
        // the sync runs on the radio thread, so the boot-to-cursor budget never
        // sees it. Skipped when the clock already holds a date (a `:update` or
        // `:setup` restart keeps it), so a warm reboot doesn't wake the radio for
        // nothing. Silent either way: nobody asked, so nothing is painted (see
        // `settle_clock`).
        if clock.today().is_none() {
            match net.sync_clock() {
                ClockDispatch::Dispatched => log::info!("boot: clock sync dispatched (SNTP only)"),
                ClockDispatch::ThreadDown => log::warn!("boot: no net thread; clock stays unset"),
            }
        }

        // Diff against the flag baked into the painted boot frame, NOT the
        // hardware: a keyboard that attached between the editor seed and here
        // would make both readings agree and the stale NO KBD flag would never
        // repaint (until an unrelated repaint, e.g. the file walk ~6 s later).
        let last_kbd = ed.keyboard_present();
        Self {
            ed,
            panel,
            keyboard,
            storage,
            net,
            clock,
            system,
            files,
            focus: FocusTimer::default(),
            last_activity: Instant::now(),
            idle_saved: false,
            last_file: String::new(),
            last_kbd,
            kbd: last_kbd,
            kbd_changed: false,
            net_in_flight: VecDeque::new(),
            pending_discard: Vec::new(),
        }
    }

    /// The active buffer for the platform's panic scribe: `Some((path, text))`
    /// while there are unsaved edits, `None` when clean (a clean buffer needs
    /// no dump — the file on the card is already current).
    pub fn scribe_snapshot(&self) -> Option<(&str, &str)> {
        self.ed.dirty().then(|| (self.ed.path(), self.ed.text()))
    }

    /// Run forever. The only exits are `:reboot`/`:setup`, which restart the
    /// device (so [`System::reboot`] diverges); the loop itself never returns.
    pub fn run(&mut self) -> ! {
        loop {
            self.tick();
        }
    }

    /// One loop iteration: drain the keyboard, service the queued effects, then
    /// either repaint the batch or run the idle sequence. Split out so the host
    /// tests can drive single passes.
    pub fn tick(&mut self) {
        // Feed today's date each pass, so a session crossing midnight (or one
        // whose clock is only set mid-session by the first sync) sees the current
        // day. `None` until the clock is trustworthy. The first real date also
        // completes an `:inbox` held waiting for one — a buffer switch nobody
        // pressed a key for, so remember it for the idle branch below to paint.
        let held_inbox = self.ed.inbox_pending();
        self.ed.set_today(self.clock.today());
        let inbox_opened = held_inbox && !self.ed.inbox_pending();

        let prev_mode = self.ed.mode();
        let keys = self.drain_keys();

        // Service the effects the batch queued, draining to empty: servicing a
        // Load can itself queue an eviction Save that must be persisted now.
        self.service_effects();

        // Keep the last-file marker on the active named buffer. An unnamed
        // scratch (empty path) keeps the previous marker.
        if !self.ed.path().is_empty() && self.ed.path() != self.last_file {
            self.last_file = self.ed.path().to_string();
            self.storage.record_last_file(&self.last_file);
        }

        // Keyboard attach/detach feeds the panel's disconnect flag.
        self.kbd = self.keyboard.keyboard_present();
        self.ed.set_keyboard_present(self.kbd);
        self.kbd_changed = self.kbd != self.last_kbd;
        self.last_kbd = self.kbd;

        // A git operation reports here on *every* pass, not only an idle one: a
        // batch repaint is most of a second of e-paper drive, so at typing speed
        // the idle branch below is never reached and a sync's progress lines and
        // terminal outcome would sit in the queue until the writer stopped —
        // reading as a cancelled pull. Strictly after `drain_keys` (every key
        // clears the notice this is about to set) and after `service_effects` (a
        // queued Save carries text snapshotted before a pull moved the tree).
        // Exactly one outcome per pass: the backend already coalesces a progress
        // burst to its newest line and lets a terminal outcome win, and two
        // settles in one pass would each want their own paint.
        let settled = self.net.poll_outcome().is_some_and(|o| self.handle_net_outcome(o));
        self.expire_silent_dispatches();

        if keys == 0 {
            // The date that landed this pass opened the held `:inbox` (its Load,
            // if any, was serviced just above) — paint it instead of idling, or
            // the note stays invisible until the next keystroke.
            if inbox_opened {
                self.panel.repaint_if_changed(&mut self.ed);
                return;
            }
            self.idle_step(settled);
            return;
        }

        self.last_activity = Instant::now();
        self.idle_saved = false;
        self.panel.render_batch(&mut self.ed, prev_mode, keys);
    }

    /// Drain all queued keystrokes (type-ahead absorbed during a refresh), apply
    /// them, and return the count. Leaving the Rest curtain drops the rest of the
    /// batch so an accidental bump only ever lands on a clean Normal screen.
    fn drain_keys(&mut self) -> u32 {
        let mut keys = 0;
        while let Some(k) = self.keyboard.next_key() {
            let was_rest = self.ed.mode() == Mode::Rest;
            self.ed.handle(k);
            keys += 1;
            if was_rest && self.ed.mode() != Mode::Rest {
                while self.keyboard.next_key().is_some() {}
                break;
            }
        }
        keys
    }

    /// Service the host-side effects the key batch queued, in order, draining to
    /// empty. The queue strictly shrinks (a Save/Push/Pull queues nothing; a
    /// Load queues at most one eviction Save), so this terminates.
    fn service_effects(&mut self) {
        loop {
            let effects = self.ed.take_effects();
            if effects.is_empty() {
                break;
            }
            for effect in effects {
                self.service_one(effect);
            }
        }
    }

    fn service_one(&mut self, effect: Effect) {
        match effect {
            Effect::Save { path, contents, .. } => self.save_buffer(&path, &contents),
            Effect::Load { path, scope } => {
                self.open_buffer(path, scope);
                // A file-switch repaints the whole column anyway — piggyback a
                // panel-cleaning full refresh here if ghosting has built up, so the
                // flash hides behind the "loading a document" transition.
                self.panel.full_refresh_on_switch(&self.ed);
            }
            // Non-blocking: the ~10 s push never stalls the editor; the outcome
            // returns via `poll_outcome` in `tick`. The Save that preceded this
            // in the batch already persisted the buffer.
            Effect::Push => match self.net.push() {
                PushDispatch::Dispatched => {
                    self.mark_net_dispatched(NetFlag::Syncing);
                    self.ed.set_notice("syncing...")
                }
                PushDispatch::ThreadDown => self.ed.set_notice("sync: git thread down"),
            },
            Effect::Pull(intent) => {
                // A confirmed discard is about to roll these paths back on the
                // SD card, so remember them *before* dispatching: the resident
                // buffers holding the text being thrown away have to be settled
                // when the outcome lands, or the next save writes it back.
                // Taken from the list the writer actually answered, not the
                // dirty journal — that belongs to the backend, which has by now
                // already taken and moved on from it.
                if intent == PullIntent::Discard {
                    self.pending_discard = self
                        .ed
                        .unsynced()
                        .iter()
                        .map(|u| format!("{REPO_DIR}/{}", u.path))
                        .collect();
                }
                match self.net.pull(intent) {
                    PullDispatch::Dispatched => {
                        self.mark_net_dispatched(NetFlag::Syncing);
                        self.ed.set_notice("pulling...")
                    }
                    // Unpushed saves: name them and ask. The card's answer
                    // re-queues Pull with Commit or Discard.
                    PullDispatch::NeedsConfirm(files) => self.ed.show_unsynced(files),
                    PullDispatch::ThreadDown => {
                        self.pending_discard.clear();
                        self.ed.set_notice("pull: git thread down")
                    }
                }
            }
            Effect::LoadLinkTarget { path } => {
                let text = match self.storage.load_path(&path) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        log::warn!("link target {path} unreadable ({e:#}); filename title");
                        None
                    }
                };
                self.ed.insert_link_loaded(&path, text.as_deref());
            }
            Effect::Delete { path, scope } => self.delete_buffer(path, scope),
            // The rename plus card-wide link retarget is synchronous SD work
            // (seconds on a big tree), and the batch repaint only lands after
            // it — which reads as a frozen panel. Paint a clue first (the
            // `:setup` notice does the same before its reboot).
            Effect::Rename { from, to, contents, retarget } => {
                self.ed.set_notice("publishing...");
                self.panel.show_notice(&mut self.ed);
                self.rename_buffer(&from, &to, &contents, &retarget)
            }
            Effect::SavePrefs { contents } => self.save_prefs(&contents),
            Effect::Setup => match self.system.prepare_setup() {
                SetupDispatch::Ready => {
                    // Paint the notice with a blocking full refresh (visible
                    // before the reset), then restart into the boot-time wizard.
                    self.ed.set_notice("opening setup - restarting...");
                    self.panel.blit_editor_full(&mut self.ed);
                    log::info!(":setup — rebooting into the wizard");
                    self.system.reboot();
                }
                SetupDispatch::MarkerFailed => self.ed.set_notice("setup: could not save marker"),
            },
            Effect::Reboot => {
                // Paint the branded splash (so the reboot reads as intentional),
                // then restart; the bistable panel carries it into the boot splash.
                log::info!(":reboot — restarting");
                self.panel.blit_full(&Frame::reboot());
                self.system.reboot();
            }
            // Non-blocking, like Push: the multi-second download + flash runs on
            // the radio-owning thread while the editor keeps running; the terminal
            // outcome returns via `poll_outcome` in `tick`. `:update` was
            // gated on a clean buffer set in the editor, so the eventual reboot on
            // success (see `handle_net_outcome`) can't strand unsaved edits.
            Effect::Update => match self.net.update() {
                UpdateDispatch::Dispatched => {
                    self.mark_net_dispatched(NetFlag::Updating);
                    self.ed.set_notice("checking for update...")
                }
                UpdateDispatch::ThreadDown => self.ed.set_notice("update: git thread down"),
            },
            // Non-blocking like Push: SNTP's budget is 20 s and the writer must
            // never wait behind it — they keep typing, and the held `:inbox`
            // opens itself when the date lands.
            Effect::SyncClock => match self.net.sync_clock() {
                ClockDispatch::Dispatched => self.ed.set_notice("inbox: setting the clock..."),
                ClockDispatch::ThreadDown => {
                    // Nothing will report back, so a hold here would never end.
                    self.ed.take_pending_inbox();
                    self.ed.set_notice("clock: net thread down");
                }
            },
            Effect::FocusStart => self.focus.start(self.ed.word_count()),
            Effect::FocusStop => self.focus.stop(),
        }
    }

    /// The idle branch: the same sequence and ordering as the old inline loop.
    /// Each rung that paints returns early (the old `continue`); `save_on_idle`
    /// deliberately falls through to the longevity/caret tail.
    ///
    /// `settled` says whether [`tick`](Self::tick) handled a net outcome this
    /// pass, whose notice this branch owns the paint for — see
    /// [`handle_net_outcome`](Self::handle_net_outcome) for why it never paints
    /// its own.
    fn idle_step(&mut self, settled: bool) {
        // Focus mode: a running block that has reached its length drops the rest
        // card at this typing pause.
        if self.panel.rest_if_due(&mut self.ed, &self.focus, self.last_activity) {
            return;
        }
        // The git outcome `tick` settled this pass: paint its notice. Behind a
        // full-screen card (the rest curtain, the `:about` splash or the `:help`
        // reference) the panel is masked, so the state is settled and the notice
        // waits for the writer to leave the card. Returns either way — the rungs
        // below must not run on an outcome pass (chiefly the idle-save, which is
        // the very write a pull's apply pass would refuse).
        if settled {
            if !matches!(self.ed.mode(), Mode::Rest | Mode::About | Mode::Help) {
                self.panel.show_notice(&mut self.ed);
            }
            return;
        }
        // A finished background file walk (boot or post-pull) feeds the palette;
        // repaint only if the visible frame changed.
        if let Some(files) = self.files.poll_result() {
            let held_inbox = self.ed.inbox_pending();
            self.ed.set_file_list_joined(files);
            // The walk can complete an `:inbox` that was held for it. Its Load
            // has to be serviced here: `service_effects` already ran this pass,
            // and the repaint below is the one that shows the note.
            if held_inbox && !self.ed.inbox_pending() {
                self.service_effects();
            }
            self.panel.repaint_if_changed(&mut self.ed);
            return;
        }
        // A connect/disconnect while idle must still repaint the panel flag.
        if self.panel.kbd_repaint(&mut self.ed, self.kbd_changed, self.kbd) {
            return;
        }
        // save_on_idle: once input has paused, quietly persist a dirty named
        // buffer. Silent — no snackbar, no forced flash. Fires once per idle
        // window, so a failing save can't busy-loop. Falls through afterwards.
        //
        // Held off while the unsynced card is up: that card is a decision about
        // the exact set of paths it is displaying, and a save landing behind it
        // would journal a file the list never showed — so a confirmed discard
        // would take one more file than the writer agreed to.
        //
        // Held off likewise while a sync is in flight, for the same reason one
        // step later: the backend has already folded the journal into a
        // pre-fetch commit, and its apply pass refuses the *whole* pull if a
        // file it is about to overwrite no longer hashes to what HEAD recorded.
        // So an idle-save landing mid-pull cancels it — and, having marked the
        // buffer clean, would let the post-pull reload in `handle_net_outcome`
        // overwrite the edits it just persisted. The buffer stays RAM-dirty
        // instead, which is what makes those edits win there; the panic scribe
        // still covers the window, and `idle_saved` is cleared by the next
        // keystroke, so the first pause after the outcome saves normally.
        if !self.idle_saved
            && self.ed.prefs().save_on_idle
            && !self.ed.showing_unsynced()
            && !self.sync_holds_off_save()
            && self.ed.dirty()
            && !self.ed.path().is_empty()
            && self.last_activity.elapsed().as_millis() >= IDLE_SAVE_MS
        {
            self.idle_saved = true;
            let path = self.ed.path().to_string();
            match self.storage.save_path(&path, self.ed.text()) {
                Ok(()) => {
                    log::info!("idle-save: {} bytes to {path}", self.ed.text().len());
                    self.ed.mark_saved(&path);
                }
                Err(e) => log::warn!("idle-save FAILED ({e:#}); buffer kept in RAM"),
            }
        }
        // Panel-longevity full refresh, deferred to a typing pause, then the
        // debounced Insert caret or a brief CPU-yielding sleep — the tail.
        if self.panel.longevity_full(&mut self.ed, self.last_activity) {
            return;
        }
        if !self.panel.caret_if_due(&mut self.ed, self.last_activity) {
            self.clock.idle_yield();
        }
    }

    /// Record a dispatched net operation, whose outcome is now outstanding. The
    /// panel reports the OLDEST one still running, so a `:gl` queued behind an
    /// `:update` does not relabel the row the writer is watching.
    fn mark_net_dispatched(&mut self, flag: NetFlag) {
        self.net_in_flight.push_back((Instant::now(), flag));
        self.show_oldest_dispatch();
    }

    /// Point the panel's activity row at the oldest outstanding operation, or
    /// clear it when none is left.
    fn show_oldest_dispatch(&mut self) {
        self.ed.set_net_flag(self.net_in_flight.front().map(|&(_, flag)| flag));
    }

    /// Settle the oldest outstanding sync. The flag drops only once the last
    /// one has landed, so an outcome never lifts a sibling's protection.
    fn settle_net_dispatch(&mut self) {
        self.net_in_flight.pop_front();
        self.show_oldest_dispatch();
    }

    /// Drop any sync that has gone silent past [`SYNC_HOLDOFF_MS`]. Flag and
    /// hold-off lift together here as everywhere: a git thread that died
    /// without reporting must not leave the panel claiming a sync it cannot
    /// finish, nor the idle-save standing down for it.
    fn expire_silent_dispatches(&mut self) {
        let before = self.net_in_flight.len();
        self.net_in_flight.retain(|(t, _)| t.elapsed().as_millis() < SYNC_HOLDOFF_MS);
        if self.net_in_flight.len() != before {
            log::warn!("sync silent past {SYNC_HOLDOFF_MS}ms: hold-off lifted");
            self.show_oldest_dispatch();
        }
    }

    /// Whether the idle-save rung must stand down for an in-flight sync — see
    /// that rung for why. Only a repo buffer is at risk: a `/sd/local` note
    /// lives outside [`REPO_DIR`], so a pull's hash belt never reads it and the
    /// dirty journal never records it — holding its save off would spend the
    /// safety net and buy nothing.
    fn sync_holds_off_save(&self) -> bool {
        !self.net_in_flight.is_empty() && matches!(self.ed.scope(), Scope::Tracked)
    }

    /// Handle a finished sync operation's outcome: settle the notice, and (for a
    /// pull that moved the working copy) reload the stale active buffer and
    /// re-walk the palette. The dirty-journal settlement already happened inside
    /// the sync backend before this returned.
    ///
    /// Settles only — it never paints. The caller owns that: a key batch folds
    /// the new notice into the repaint it was already doing, and an idle pass
    /// paints it in [`idle_step`](Self::idle_step). Painting here would cost a
    /// second whole-panel area partial (~630 ms of drive and another of the
    /// 64-partial ghosting budget) on every keystroke that carries an outcome.
    fn handle_net_outcome(&mut self, outcome: NetOutcome) -> bool {
        match outcome {
            // Proof of life for the oldest outstanding operation, so a slow
            // fetch keeps its hold-off (see `expire_silent_dispatches`).
            NetOutcome::Progress(_) => {
                if let Some((t, _)) = self.net_in_flight.front_mut() {
                    *t = Instant::now();
                }
            }
            // The boot clock sync is dispatched without a slot — it must not
            // raise the panel flag or hold off the idle-save — so settling one
            // here would pop the slot of a `:gs`/`:gl` that is still running and
            // hand back the protection that sync depends on.
            NetOutcome::Clock(_) => {}
            // Every other variant is terminal: it frees the idle-save and drops
            // the panel flag once the last outstanding operation lands.
            _ => self.settle_net_dispatch(),
        }
        let notice = match outcome {
            NetOutcome::Push(o) => push_notice(&o),
            NetOutcome::Pull(o) => {
                // A confirmed discard rolled the working copy back *before* the
                // fetch, so its buffers are stale whatever the fetch then did —
                // including a failure. Settled first, and unconditionally.
                let discarded = std::mem::take(&mut self.pending_discard);
                if !discarded.is_empty() {
                    self.settle_discarded(&discarded);
                }
                // Pulled and Rebased both move the working copy under us; the
                // stale resident buffers must re-read the disk.
                let moved_working_copy =
                    matches!(o, PullOutcome::Pulled(_) | PullOutcome::Rebased(_));
                let notice = pull_notice(&o);
                // Either way the card changed under us, so `.typoena.toml` may
                // have too — re-read it before anything else reads a pref.
                if moved_working_copy || !discarded.is_empty() {
                    self.reload_prefs();
                }
                if !discarded.is_empty() && !moved_working_copy {
                    // The discard alone changed the card, so the palette's file
                    // list is stale even though the pull moved nothing.
                    self.files.request_rewalk();
                }
                if moved_working_copy {
                    // Clean parked buffers are dropped (they reload on the next
                    // switch); the clean active buffer is re-read now; a RAM-dirty
                    // buffer is left alone — its edits win, last-writer-wins.
                    self.ed.drop_clean_parked();
                    if self.ed.dirty() {
                        log::info!(
                            "post-pull: {} is RAM-dirty — kept (its edits win)",
                            self.ed.path()
                        );
                    } else if !self.ed.path().is_empty() {
                        match self.storage.load_path(self.ed.path()) {
                            Ok(text) => self.ed.refresh_active(text),
                            Err(e) => log::warn!(
                                "post-pull reload of {} FAILED ({e:#}); buffer kept",
                                self.ed.path()
                            ),
                        }
                    }
                    self.files.request_rewalk();
                }
                notice
            }
            NetOutcome::Update(o) => match o {
                UpdateOutcome::Installed(ver) => {
                    // The new image is written and is now the boot slot. Paint the
                    // notice with a blocking full refresh (visible before the
                    // reset), then restart into it; the boot path self-tests and
                    // marks it valid, or the bootloader rolls back to this slot.
                    self.ed.set_notice(format!("updated to {ver} - restarting..."));
                    self.panel.blit_editor_full(&mut self.ed);
                    log::info!(":update — installed {ver}; rebooting into the new image");
                    self.system.reboot();
                }
                UpdateOutcome::UpToDate(ver) => format!("firmware up to date ({ver})"),
                UpdateOutcome::Failed(reason) => reason,
            },
            // Silent unless an `:inbox` was actually waiting on the date: the
            // boot sync runs unasked, so an offline session must not open on a
            // network error nobody provoked.
            // A silent clock sync leaves nothing on screen, and saying so is
            // what keeps it from costing a paint: `show_notice` has no change
            // detection, so a `settled` of `true` would drive a full-panel
            // partial every boot for something the writer never asked for.
            NetOutcome::Clock(o) => match self.settle_clock(o) {
                Some(reason) => reason,
                None => return false,
            },
            // In-flight status line: nothing to settle, just paint it (the tail
            // below). The operation is still running and will report again.
            NetOutcome::Progress(line) => line,
        };
        self.ed.set_notice(notice);
        true
    }

    /// Settle a finished clock-only sync into the notice it deserves, or `None`
    /// for silence. Success says nothing: the date travels through
    /// [`Clock::today`], which `tick` feeds every pass, and the `:inbox` that was
    /// held waiting for it opens — and paints — from there. Failure speaks only
    /// if a request was in fact held; the other caller of this sync is the boot
    /// kick, which nobody asked for.
    fn settle_clock(&mut self, outcome: ClockOutcome) -> Option<String> {
        match outcome {
            ClockOutcome::Synced => {
                log::info!("clock synced; today = {:?}", self.clock.today());
                None
            }
            ClockOutcome::Failed(reason) => {
                log::warn!("clock sync failed: {reason}");
                self.ed.take_pending_inbox().then_some(reason)
            }
        }
    }

    /// Settle the resident buffers after a confirmed discard rolled `paths`
    /// (absolute) back on the card. Every RAM copy of those files now holds
    /// text that no longer exists anywhere else, so it must not survive to be
    /// written back — this is the one place that overrides the
    /// last-writer-wins rule the pull path uses, because throwing that text
    /// away is precisely what was confirmed.
    ///
    /// A discarded path that won't re-read is a note the remote never had: the
    /// rollback had no version to restore it to, so it left the card entirely
    /// and the buffer goes with it.
    fn settle_discarded(&mut self, paths: &[String]) {
        log::info!("discard: settling {} resident path(s)", paths.len());
        self.ed.drop_parked_paths(paths);
        if !paths.iter().any(|p| p == self.ed.path()) {
            return;
        }
        match self.storage.load_path(self.ed.path()) {
            Ok(text) => self.ed.refresh_active(text),
            Err(e) => {
                log::info!("discard removed {} ({e:#}); dropping the buffer", self.ed.path());
                self.ed.abandon_active();
            }
        }
    }

    /// Persist a buffer to `path`. Errors are logged, never propagated: the
    /// in-RAM buffer is the source of truth and must survive a failed write.
    fn save_buffer(&mut self, path: &str, contents: &str) {
        match self.storage.save_path(path, contents) {
            Ok(()) => {
                log::info!(":w — saved {} bytes to {path}", contents.len());
                self.ed.mark_saved(path);
                self.ed.set_notice("saved");
            }
            Err(e) => {
                log::error!("save FAILED ({e:#}); buffer kept in RAM, retry :w");
                self.ed.set_notice("save FAILED - retry :w");
            }
        }
    }

    /// Re-read [`PREFS_PATH`] and install it, after a pull's apply (or a confirmed
    /// discard) rewrote the card. Boot is not the only time preferences arrive: a
    /// pull carries whatever was edited on a computer, and the palette writes this
    /// whole struct back out from RAM ([`Effect::SavePrefs`]), so a boot-time copy
    /// left in place here does not merely lag — the next `>` toggle serializes it
    /// over the pulled values, and the pull that would repair the card never
    /// comes, because the tree-to-tree apply only writes paths two commits
    /// disagree on.
    ///
    /// A read failure keeps the current preferences rather than resetting them to
    /// [`Prefs::default`]: a card with no prefs file is a normal card, and a
    /// transient SD error must not silently undo the writer's settings.
    fn reload_prefs(&mut self) {
        let Ok(src) = self.storage.load_path(PREFS_PATH) else {
            log::info!("post-pull: no readable {PREFS_PATH} — preferences unchanged");
            return;
        };
        let prefs = Prefs::parse(&src);
        if prefs == *self.ed.prefs() {
            return;
        }
        log::info!("post-pull: preferences reloaded from {PREFS_PATH} — {prefs:?}");
        self.ed.set_prefs(prefs);
    }

    /// Persist the preferences file after a palette `>` command changed a pref.
    fn save_prefs(&mut self, contents: &str) {
        match self.storage.save_path(PREFS_PATH, contents) {
            Ok(()) => log::info!("prefs saved to {PREFS_PATH}"),
            Err(e) => {
                log::error!("prefs save FAILED ({e:#})");
                self.ed.set_notice("prefs save FAILED");
            }
        }
    }

    /// Read `path` and install it as the active buffer. A read failure keeps the
    /// current buffer and surfaces the reason rather than swapping to an empty one.
    fn open_buffer(&mut self, path: String, scope: Scope) {
        match self.storage.load_path(&path) {
            Ok(text) => {
                log::info!("opened {path} ({} bytes, {scope:?})", text.len());
                self.ed.install_loaded(path, scope, text);
            }
            Err(e) => {
                log::error!("open {path} FAILED ({e:#})");
                self.ed.set_notice(format!("can't open {}", file_stem(&path)));
            }
        }
    }

    /// Unlink a file from the card. The editor has already dropped it from its
    /// model and switched away, so this is pure IO plus the snackbar.
    fn delete_buffer(&mut self, path: String, scope: Scope) {
        // Scope-qualified label (`repo/notes.md`), so the snackbar names exactly
        // which file left the card and, for Tracked, that it's local until `:gs`.
        let label = path.strip_prefix("/sd/").unwrap_or(&path);
        match self.storage.delete_path(&path) {
            Ok(()) => {
                log::info!("deleted {path} ({scope:?})");
                self.ed.set_notice(match scope {
                    Scope::Tracked => format!("deleted {label} - :gs to push"),
                    Scope::Local => format!("deleted {label}"),
                });
            }
            Err(e) => {
                log::error!("delete {path} FAILED ({e:#})");
                self.ed.set_notice(format!("delete FAILED: {label}"));
            }
        }
    }

    /// `:pub`/`:publish` — persist the active buffer under its new `.pub.md` name,
    /// then unlink the old path. The write lands first so the file is never
    /// missing; the removal — plus the dirty-journal entries `save_path` and
    /// `delete_path` record — makes the next `:gs` carry the move to the remote as
    /// a rename. The unlink is best-effort: if it fails the new file already exists,
    /// so the publish stands and the stale `.md` is dropped on the next card
    /// re-walk. A failed *write* keeps the buffer dirty for a retry (like a `:w`
    /// save failure); the editor already switched to the new name, so `:w` re-saves
    /// it there.
    ///
    /// `retarget` — the non-resident `.md` files that might link to `from` — is
    /// then rewritten through [`editor::publish_retarget_links`]; each hit's save
    /// joins the dirty journal, so the next `:gs` ships the rename and its link
    /// updates together. Runs even after a failed write (see [`Effect::Rename`]);
    /// a per-file failure is logged and skipped — that file's links just stay on
    /// the old name.
    fn rename_buffer(&mut self, from: &str, to: &str, contents: &str, retarget: &[String]) {
        // Scope-qualified label (`repo/notes.pub.md`), matching the delete snackbar.
        let label = to.strip_prefix("/sd/").unwrap_or(to);
        let renamed = match self.storage.save_path(to, contents) {
            Ok(()) => {
                if let Err(e) = self.storage.delete_path(from) {
                    log::warn!("publish: wrote {to} but couldn't unlink {from} ({e:#})");
                }
                log::info!("published {from} -> {to}");
                self.ed.mark_saved(to);
                true
            }
            Err(e) => {
                log::error!("publish (rename {from} -> {to}) FAILED ({e:#})");
                false
            }
        };
        let mut links = 0;
        for path in retarget {
            let text = match self.storage.load_path(path) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("retarget: {path} unreadable ({e:#}); links kept as-is");
                    continue;
                }
            };
            let Some((new, sites)) = editor::publish_retarget_links(path, &text, from) else {
                continue;
            };
            match self.storage.save_path(path, &new) {
                Ok(()) => {
                    log::info!("retarget: {} link(s) to {to} in {path}", sites.len());
                    links += sites.len();
                }
                Err(e) => log::warn!("retarget: rewrite of {path} FAILED ({e:#})"),
            }
        }
        // Always one summary line, so a bench trace answers "did the retarget
        // run, over what, and find anything?" at a glance.
        log::info!("retarget: {links} link(s) rewritten across {} candidate file(s)", retarget.len());
        self.ed.set_notice(if !renamed {
            "publish FAILED - retry :w".to_string()
        } else if links > 0 {
            let s = if links == 1 { "" } else { "s" };
            format!("published {label} +{links} link{s} - :gs to push")
        } else {
            format!("published {label} - :gs to push")
        });
    }
}

/// A file's display name — its basename without extension (`/sd/repo/notes.md`
/// → `notes`), for the snackbar. Falls back to the raw path if it has no stem.
/// Pure; shared with the firmware boot path.
pub fn file_stem(path: &str) -> &str {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

/// The snackbar line for a finished push. Pure — the notice mapping only.
fn push_notice(o: &PushOutcome) -> String {
    match o {
        PushOutcome::Pushed(oid) => format!("synced {oid}"),
        PushOutcome::UpToDate => "up to date".to_string(),
        PushOutcome::Failed(reason) => reason.clone(),
    }
}

/// The snackbar line for a finished pull. Pure — the notice mapping only.
fn pull_notice(o: &PullOutcome) -> String {
    match o {
        PullOutcome::Pulled(oid) => format!("pulled {oid}"),
        PullOutcome::Rebased(oid) => format!("rebased {oid} - :gs to push"),
        PullOutcome::UpToDate => "up to date".to_string(),
        PullOutcome::LocalAhead => "ahead - :gs to push".to_string(),
        PullOutcome::Failed(reason) => reason.clone(),
    }
}

#[cfg(test)]
mod tests;
