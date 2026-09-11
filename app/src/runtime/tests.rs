//! Host tests for the lifted run loop — the routing that used to be untestable
//! inline in the firmware binary. In-memory doubles stand in for every port, so
//! these run on the host with no esp-idf.

use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;
use std::time::Duration;

use editor::{Editor, Effect, Scope};

use super::*;
use crate::ports::{
    Clock, ClockDispatch, ClockOutcome, FileIndex, Power, PowerEvent, PushDispatch, PushOutcome,
    PullDispatch, PullOutcome, SetupDispatch, Storage, NetOutcome, NetService, System,
    UpdateDispatch, UpdateOutcome,
};
use crate::render::Panel;

/// A screen that accepts every frame — the render engine's paints are no-ops.
struct MockScreen;
impl hal::Screen for MockScreen {
    type Error = Infallible;
    fn display_frame(&mut self, _fb: &[u8]) -> Result<(), Infallible> {
        Ok(())
    }
    fn display_frame_partial_window(
        &mut self,
        _fb: &[u8],
        _y0: u16,
        _h: u16,
    ) -> Result<(), Infallible> {
        Ok(())
    }
}

/// A keyboard with nothing queued and nothing attached.
struct NoKeyboard;
impl hal::Keyboard for NoKeyboard {
    fn next_key(&mut self) -> Option<hal::Key> {
        None
    }
    fn keyboard_present(&self) -> bool {
        false
    }
}

/// A keyboard that is attached but has nothing queued.
struct PresentKeyboard;
impl hal::Keyboard for PresentKeyboard {
    fn next_key(&mut self) -> Option<hal::Key> {
        None
    }
    fn keyboard_present(&self) -> bool {
        true
    }
}

/// A screen that counts paints of either kind, so a test can assert that a
/// state change reached the panel without caring which waveform carried it.
#[derive(Clone, Default)]
struct CountingScreen(Rc<RefCell<u32>>);
impl hal::Screen for CountingScreen {
    type Error = Infallible;
    fn display_frame(&mut self, _fb: &[u8]) -> Result<(), Infallible> {
        *self.0.borrow_mut() += 1;
        Ok(())
    }
    fn display_frame_partial_window(
        &mut self,
        _fb: &[u8],
        _y0: u16,
        _h: u16,
    ) -> Result<(), Infallible> {
        *self.0.borrow_mut() += 1;
        Ok(())
    }
}

#[derive(Default)]
struct StorageLog {
    saves: Vec<(String, String)>,
    loads: Vec<String>,
    deletes: Vec<String>,
    last_files: Vec<String>,
    /// Per-path `load_path` bodies; paths not listed echo `"loaded-body"`.
    bodies: Vec<(String, String)>,
    /// Paths whose `load_path` fails — a file that isn't on the card.
    missing: Vec<String>,
}

/// Records every call; `load_path` echoes a canned body back.
#[derive(Clone, Default)]
struct RecStorage(Rc<RefCell<StorageLog>>);
impl RecStorage {
    /// Canned `load_path` body for `path`.
    fn with_body(self, path: &str, body: &str) -> Self {
        self.0.borrow_mut().bodies.push((path.into(), body.into()));
        self
    }

    /// Make `load_path` fail for `path` — the file is not on the card.
    fn with_missing(self, path: &str) -> Self {
        self.0.borrow_mut().missing.push(path.into());
        self
    }
}
impl Storage for RecStorage {
    fn save_path(&self, path: &str, contents: &str) -> anyhow::Result<()> {
        self.0.borrow_mut().saves.push((path.into(), contents.into()));
        Ok(())
    }
    fn load_path(&self, path: &str) -> anyhow::Result<String> {
        let mut log = self.0.borrow_mut();
        log.loads.push(path.into());
        if log.missing.iter().any(|p| p == path) {
            anyhow::bail!("no such file: {path}");
        }
        let body = log.bodies.iter().find(|(p, _)| p == path).map(|(_, b)| b.clone());
        Ok(body.unwrap_or_else(|| "loaded-body".into()))
    }
    fn delete_path(&self, path: &str) -> anyhow::Result<()> {
        self.0.borrow_mut().deletes.push(path.into());
        Ok(())
    }
    fn record_last_file(&self, path: &str) {
        self.0.borrow_mut().last_files.push(path.into());
    }
}

#[derive(Default)]
struct SyncLog {
    pushes: u32,
    pulls: u32,
    /// The intent of each dispatched pull, in order.
    pull_intents: Vec<PullIntent>,
    update_checks: u32,
    update_installs: Vec<String>,
    clocks: u32,
    outcome: Option<NetOutcome>,
}

/// Configurable dispatch results + a single queued outcome.
#[derive(Clone)]
struct RecSync {
    log: Rc<RefCell<SyncLog>>,
    push_ret: Rc<dyn Fn() -> PushDispatch>,
    pull_ret: Rc<dyn Fn() -> PullDispatch>,
    update_ret: Rc<dyn Fn() -> UpdateDispatch>,
    clock_ret: Rc<dyn Fn() -> ClockDispatch>,
}
impl RecSync {
    fn new() -> Self {
        Self {
            log: Rc::new(RefCell::new(SyncLog::default())),
            push_ret: Rc::new(|| PushDispatch::Dispatched),
            pull_ret: Rc::new(|| PullDispatch::Dispatched),
            update_ret: Rc::new(|| UpdateDispatch::Dispatched),
            clock_ret: Rc::new(|| ClockDispatch::Dispatched),
        }
    }
}
impl NetService for RecSync {
    fn push(&self) -> PushDispatch {
        self.log.borrow_mut().pushes += 1;
        (self.push_ret)()
    }
    fn pull(&self, intent: PullIntent) -> PullDispatch {
        let mut log = self.log.borrow_mut();
        log.pulls += 1;
        log.pull_intents.push(intent);
        drop(log);
        (self.pull_ret)()
    }
    fn check_update(&self) -> UpdateDispatch {
        self.log.borrow_mut().update_checks += 1;
        (self.update_ret)()
    }
    fn install_update(&self, version: String) -> UpdateDispatch {
        self.log.borrow_mut().update_installs.push(version);
        (self.update_ret)()
    }
    fn sync_clock(&self) -> ClockDispatch {
        self.log.borrow_mut().clocks += 1;
        (self.clock_ret)()
    }
    fn poll_outcome(&self) -> Option<NetOutcome> {
        self.log.borrow_mut().outcome.take()
    }
}

struct FixedClock;
impl Clock for FixedClock {
    fn today(&self) -> Option<editor::Date> {
        None
    }
    fn idle_yield(&self) {}
}

/// A wall clock the test can set mid-session, the way SNTP does on the device.
#[derive(Clone, Default)]
struct SettableClock(Rc<std::cell::Cell<Option<editor::Date>>>);
impl SettableClock {
    fn set(&self, date: editor::Date) {
        self.0.set(Some(date));
    }
}
impl Clock for SettableClock {
    fn today(&self) -> Option<editor::Date> {
        self.0.get()
    }
    fn idle_yield(&self) {}
}

struct PanicSystem;
impl System for PanicSystem {
    fn prepare_setup(&self) -> SetupDispatch {
        SetupDispatch::MarkerFailed
    }
    fn reboot(&self) -> ! {
        panic!("reboot in test")
    }
}

/// Hardware with no charger on the bus and a button nobody touches — the
/// default for every test that isn't about power.
struct NoPower;
impl Power for NoPower {
    fn poll(&mut self) -> Option<PowerEvent> {
        None
    }
    fn status(&self) -> Option<editor::Battery> {
        None
    }
    fn power_off(&mut self) -> ! {
        panic!("power off in test")
    }
}

#[derive(Default)]
struct PowerState {
    queued: Option<PowerEvent>,
    battery: Option<editor::Battery>,
    powered_off: bool,
}

/// A power port a test drives: queue an event, set a reading, and read back
/// whether the loop reached the shutdown.
#[derive(Clone, Default)]
struct ScriptedPower(Rc<RefCell<PowerState>>);
impl ScriptedPower {
    fn queue(&self, event: PowerEvent) {
        self.0.borrow_mut().queued = Some(event);
    }
    fn set_battery(&self, battery: editor::Battery) {
        self.0.borrow_mut().battery = Some(battery);
    }
    fn powered_off(&self) -> bool {
        self.0.borrow().powered_off
    }
}
impl Power for ScriptedPower {
    fn poll(&mut self) -> Option<PowerEvent> {
        self.0.borrow_mut().queued.take()
    }
    fn status(&self) -> Option<editor::Battery> {
        self.0.borrow().battery
    }
    fn power_off(&mut self) -> ! {
        self.0.borrow_mut().powered_off = true;
        // The trait promises never to return and the run loop is entitled to
        // believe it, so the double has to diverge too. A panic is the one
        // divergence a test can catch (`shut_down`) — the same trick
        // `PanicSystem::reboot` plays.
        panic!("power off in test")
    }
}

#[derive(Clone, Default)]
struct RecFiles(Rc<RefCell<u32>>);
impl FileIndex for RecFiles {
    fn request_rewalk(&self) {
        *self.0.borrow_mut() += 1;
    }
    fn poll_result(&self) -> Option<String> {
        None
    }
}

/// A file walk whose (single) result is ready to be polled — the newline-joined
/// absolute-path blob the real walk thread sends.
struct WalkFiles(RefCell<Option<String>>);
impl FileIndex for WalkFiles {
    fn request_rewalk(&self) {}
    fn poll_result(&self) -> Option<String> {
        self.0.borrow_mut().take()
    }
}

/// A keyboard that types a queued script; keys can be pushed between ticks.
#[derive(Clone, Default)]
struct ScriptedKeyboard(Rc<RefCell<std::collections::VecDeque<hal::Key>>>);
impl ScriptedKeyboard {
    /// Queue one keystroke, so the next `tick` takes the key-batch branch.
    fn press(&self, key: hal::Key) {
        self.0.borrow_mut().push_back(key);
    }

    /// Queue `s` followed by Enter (an ex command, e.g. `:pub`).
    fn type_line(&self, s: &str) {
        let mut q = self.0.borrow_mut();
        q.extend(s.chars().map(hal::Key::Char));
        q.push_back(hal::Key::Enter);
    }
}
impl hal::Keyboard for ScriptedKeyboard {
    fn next_key(&mut self) -> Option<hal::Key> {
        self.0.borrow_mut().pop_front()
    }
    fn keyboard_present(&self) -> bool {
        true
    }
}

/// Build a runtime around the given storage/sync/files, defaulting the rest.
fn runtime(
    ed: Editor,
    storage: RecStorage,
    sync: RecSync,
    files: RecFiles,
) -> Runtime<MockScreen> {
    let mut ed = ed;
    let panel = Panel::new(MockScreen, &mut ed).expect("first paint");
    Runtime::new(
        ed,
        panel,
        Box::new(NoKeyboard),
        Box::new(storage),
        Box::new(sync),
        Box::new(FixedClock),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(files),
    )
}

/// Build a runtime that can be *typed at*, over a screen that counts its
/// paints: the two things the default `runtime` above can't do, and both needed
/// to prove an outcome rides the key batch's own repaint.
fn typing_runtime(
    ed: Editor,
    screen: CountingScreen,
    keyboard: ScriptedKeyboard,
    sync: RecSync,
) -> Runtime<CountingScreen> {
    let mut ed = ed;
    let panel = Panel::new(screen, &mut ed).expect("first paint");
    Runtime::new(
        ed,
        panel,
        Box::new(keyboard),
        Box::new(RecStorage::default()),
        Box::new(sync),
        Box::new(FixedClock),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(RecFiles::default()),
    )
}

#[test]
fn file_stem_strips_dir_and_extension() {
    assert_eq!(file_stem("/sd/repo/notes.md"), "notes");
    assert_eq!(file_stem("/sd/local/2026-07-18.md"), "2026-07-18");
    assert_eq!(file_stem("bare"), "bare");
}

#[test]
fn push_notice_covers_every_variant() {
    assert_eq!(push_notice(&PushOutcome::Pushed("abc123".into())), "synced abc123");
    assert_eq!(push_notice(&PushOutcome::UpToDate), "up to date");
    assert_eq!(push_notice(&PushOutcome::Failed("no wifi".into())), "no wifi");
}

#[test]
fn an_in_flight_progress_line_settles_nothing() {
    // A progress line is a repaint and nothing else: the operation is still
    // running, so anything that belongs to a *finished* one — reloading buffers,
    // re-walking the palette, settling a pending discard — must not fire, or a
    // mid-push status line would reload the file being pushed.
    let storage = RecStorage::default().with_body("/sd/repo/notes.md", "on-card");
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "in-buffer".into());
    let files = RecFiles::default();
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), files.clone());

    rt.handle_net_outcome(NetOutcome::Progress("sending 3/7".into()));

    assert_eq!(rt.ed.text(), "in-buffer", "the buffer must not be re-read mid-push");
    assert!(storage.0.borrow().loads.is_empty(), "no load belongs to a non-terminal line");
    assert_eq!(*files.0.borrow(), 0, "the palette re-walk waits for the outcome");
}

#[test]
fn a_net_outcome_lands_on_a_typing_pass() {
    // A pass that drains a key never reaches the idle sequence, and a batch
    // repaint is most of a second — so at ordinary typing speed nearly every
    // pass drains one. Poll only there and a pull's outcome waits in the queue
    // until the writer stops, which reads as a pull that never ran.
    let sync = RecSync::new();
    sync.log.borrow_mut().outcome = Some(NetOutcome::Pull(PullOutcome::UpToDate));
    let keyboard = ScriptedKeyboard::default();
    keyboard.press(hal::Key::Char('i'));
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "body".into());
    let mut rt = typing_runtime(ed, CountingScreen::default(), keyboard, sync.clone());

    rt.tick();

    assert!(sync.log.borrow().outcome.is_none(), "the outcome must be drained while typing");
    assert_eq!(rt.ed.notice(), Some("up to date"), "and reach the panel on that same pass");
}

#[test]
fn an_outcome_arriving_mid_batch_costs_no_extra_repaint() {
    // The settle must fold into the repaint the batch was already doing: a
    // second whole-panel pass is ~630 ms of drive and another of the 64-partial
    // ghosting budget, for every keystroke that happens to carry an outcome.
    let sync = RecSync::new();
    sync.log.borrow_mut().outcome = Some(NetOutcome::Push(PushOutcome::Pushed("abc123".into())));
    let keyboard = ScriptedKeyboard::default();
    keyboard.press(hal::Key::Char('i'));
    let screen = CountingScreen::default();
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "body".into());
    let mut rt = typing_runtime(ed, screen.clone(), keyboard.clone(), sync);

    let before = *screen.0.borrow();
    rt.tick();
    let with_outcome = *screen.0.borrow() - before;
    assert_eq!(rt.ed.notice(), Some("synced abc123"));

    // The same pass again with an empty queue: the batch repaint on its own.
    keyboard.press(hal::Key::Char('x'));
    rt.tick();
    let batch_only = *screen.0.borrow() - before - with_outcome;

    assert_eq!(with_outcome, batch_only, "the notice rode the batch's own repaint");
}

#[test]
fn a_dispatched_sync_shows_a_panel_sign_typing_cannot_clear() {
    // The `pulling...` snackbar dies on the very next keystroke, so it cannot be
    // the evidence that a sync is running. The panel flag is.
    let keyboard = ScriptedKeyboard::default();
    let sync = RecSync::new();
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "body".into());
    let mut rt = typing_runtime(ed, CountingScreen::default(), keyboard.clone(), sync.clone());
    rt.service_one(Effect::Pull(PullIntent::Ask));
    assert_eq!(rt.ed.notice(), Some("pulling..."));
    assert!(rt.ed.net_flag().is_some(), "a dispatched pull raises the flag");

    keyboard.press(hal::Key::Char('i'));
    rt.tick();
    assert_eq!(rt.ed.notice(), None, "the snackbar is gone with the keystroke");
    assert!(rt.ed.net_flag().is_some(), "the flag is not");

    sync.log.borrow_mut().outcome = Some(NetOutcome::Pull(PullOutcome::LocalAhead));
    rt.tick();
    assert!(rt.ed.net_flag().is_none(), "and the outcome lowers it");
}

#[test]
fn a_progress_line_leaves_the_sync_flag_up() {
    // Only a terminal outcome ends the operation; a status line means it is
    // still running, so the flag — and the idle-save hold-off with it — stands.
    let mut rt = runtime(Editor::new(), RecStorage::default(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Push);
    rt.handle_net_outcome(NetOutcome::Progress("sending 3/7".into()));
    assert!(rt.ed.net_flag().is_some(), "a progress line is not the end of the push");
    rt.handle_net_outcome(NetOutcome::Push(PushOutcome::UpToDate));
    assert!(rt.ed.net_flag().is_none());
}

#[test]
fn the_idle_save_is_held_off_until_a_pull_settles() {
    // An idle-save landing mid-pull rewrites a file the backend already folded
    // into its pre-fetch commit, and the pull's apply pass then refuses the
    // whole thing — the writer's "typing cancelled my pull". The write has to
    // wait for the outcome, then land normally at the next pause.
    let storage = RecStorage::default();
    let sync = RecSync::new();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.handle(hal::Key::Char('i'));
    ed.handle(hal::Key::Char('x'));
    assert!(ed.dirty());
    let mut rt = runtime(ed, storage.clone(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Ask));

    rt.last_activity = Instant::now() - Duration::from_millis(IDLE_SAVE_MS as u64 + 1);
    rt.tick();
    assert!(storage.0.borrow().saves.is_empty(), "no write may land mid-pull");
    assert!(rt.ed.dirty(), "the buffer stays dirty, which is what makes its edits win");

    sync.log.borrow_mut().outcome = Some(NetOutcome::Pull(PullOutcome::UpToDate));
    rt.tick(); // settles the outcome and paints its notice
    rt.last_activity = Instant::now() - Duration::from_millis(IDLE_SAVE_MS as u64 + 1);
    rt.tick();
    assert_eq!(
        storage.0.borrow().saves,
        vec![("/sd/repo/notes.md".to_string(), "x".to_string())],
        "the safety net resumes at the first pause after the outcome"
    );
}

#[test]
fn a_second_syncs_outcome_does_not_settle_the_first() {
    // The git thread takes an unbounded request queue, so a `:gl` dispatched
    // behind a `:gs` overlaps it. If the push's outcome lifted the hold-off, the
    // idle-save would fire straight into the pull this branch exists to protect.
    let storage = RecStorage::default();
    let sync = RecSync::new();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.handle(hal::Key::Char('i'));
    ed.handle(hal::Key::Char('x'));
    let mut rt = runtime(ed, storage.clone(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::Push);
    rt.service_one(Effect::Pull(PullIntent::Ask));

    sync.log.borrow_mut().outcome = Some(NetOutcome::Push(PushOutcome::UpToDate));
    rt.tick();
    assert!(rt.ed.net_flag().is_some(), "the pull is still running, so the flag stands");

    rt.last_activity = Instant::now() - Duration::from_millis(IDLE_SAVE_MS as u64 + 1);
    rt.tick();
    assert!(storage.0.borrow().saves.is_empty(), "no write may land mid-pull");

    sync.log.borrow_mut().outcome = Some(NetOutcome::Pull(PullOutcome::UpToDate));
    rt.tick();
    assert!(rt.ed.net_flag().is_none(), "the last outcome lowers it");
}

#[test]
fn a_sync_that_goes_silent_gives_the_safety_net_back() {
    // A git thread that dies without reporting must not hold the save-on-idle
    // net off — nor leave the panel claiming a sync — for the rest of the
    // session. Flag and hold-off lift together.
    let storage = RecStorage::default();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.handle(hal::Key::Char('i'));
    ed.handle(hal::Key::Char('x'));
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Ask));
    assert!(rt.ed.net_flag().is_some());

    rt.net_in_flight =
        [(Instant::now() - Duration::from_millis(SYNC_HOLDOFF_MS as u64 + 1), NetFlag::Syncing)]
            .into_iter()
            .collect();
    rt.last_activity = Instant::now() - Duration::from_millis(IDLE_SAVE_MS as u64 + 1);
    rt.tick();
    assert!(rt.ed.net_flag().is_none(), "the panel stops claiming a sync it cannot finish");
    assert_eq!(
        storage.0.borrow().saves,
        vec![("/sd/repo/notes.md".to_string(), "x".to_string())],
        "and the safety net is back"
    );
}

#[test]
fn a_progress_line_keeps_a_slow_sync_from_expiring() {
    // The bound is silence, not duration: a fetch that keeps reporting keeps its
    // hold-off however long it runs.
    let storage = RecStorage::default();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.handle(hal::Key::Char('i'));
    ed.handle(hal::Key::Char('x'));
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Ask));

    rt.net_in_flight =
        [(Instant::now() - Duration::from_millis(SYNC_HOLDOFF_MS as u64 - 10), NetFlag::Syncing)]
            .into_iter()
            .collect();
    rt.handle_net_outcome(NetOutcome::Progress("receiving 40%".into()));
    rt.last_activity = Instant::now() - Duration::from_millis(IDLE_SAVE_MS as u64 + 1);
    rt.tick();
    assert!(rt.ed.net_flag().is_some(), "proof of life refreshed the deadline");
    assert!(storage.0.borrow().saves.is_empty(), "so the hold-off still stands");
}

#[test]
fn a_local_note_keeps_saving_through_a_sync() {
    // A `/sd/local` note lives outside the repo: the pull's hash belt never
    // reads it and the dirty journal never records it, so holding its idle-save
    // off would spend the safety net and buy nothing.
    let storage = RecStorage::default();
    let mut ed = Editor::with_file("/sd/local/idea.md".into(), Scope::Local, String::new());
    ed.handle(hal::Key::Char('i'));
    ed.handle(hal::Key::Char('x'));
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Ask));
    assert!(rt.ed.net_flag().is_some(), "the sync is running");

    rt.last_activity = Instant::now() - Duration::from_millis(IDLE_SAVE_MS as u64 + 1);
    rt.tick();
    assert_eq!(
        storage.0.borrow().saves,
        vec![("/sd/local/idea.md".to_string(), "x".to_string())],
        "the local note is saved anyway"
    );
}

#[test]
fn pull_notice_covers_every_variant() {
    assert_eq!(pull_notice(&PullOutcome::Pulled("abc".into())), "pulled abc");
    assert_eq!(pull_notice(&PullOutcome::Rebased("def".into())), "rebased def - :gs to push");
    assert_eq!(pull_notice(&PullOutcome::UpToDate), "up to date");
    assert_eq!(pull_notice(&PullOutcome::LocalAhead), "ahead - :gs to push");
    assert_eq!(pull_notice(&PullOutcome::Failed("boom".into())), "boom");
}

#[test]
fn attach_between_boot_seed_and_runtime_start_repaints_the_kbd_flag() {
    // Editor::new() carries the boot-frame seed (keyboard_present = false, the
    // NO KBD flag painted); the hardware says present by the time the runtime
    // starts. The first idle tick must catch the missed transition and repaint —
    // diffing hardware-vs-hardware here left the stale flag up until the next
    // unrelated repaint (the file walk, ~6 s after cursor-ready).
    let mut ed = Editor::new();
    let screen = CountingScreen::default();
    let panel = Panel::new(screen.clone(), &mut ed).expect("first paint");
    let mut rt = Runtime::new(
        ed,
        panel,
        Box::new(PresentKeyboard),
        Box::new(RecStorage::default()),
        Box::new(RecSync::new()),
        Box::new(FixedClock),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(RecFiles::default()),
    );
    let boot_paints = *screen.0.borrow();
    rt.tick();
    assert_eq!(
        *screen.0.borrow(),
        boot_paints + 1,
        "first tick must repaint the stale NO KBD flag"
    );
    rt.tick();
    assert_eq!(*screen.0.borrow(), boot_paints + 1, "settled — no repaint on the next tick");
}

#[test]
fn save_effect_writes_through_storage() {
    let storage = RecStorage::default();
    let mut rt = runtime(Editor::new(), storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Save {
        path: "/sd/repo/notes.md".into(),
        scope: Scope::Tracked,
        contents: "hello".into(),
    });
    assert_eq!(storage.0.borrow().saves, vec![("/sd/repo/notes.md".into(), "hello".into())]);
}

#[test]
fn save_prefs_effect_writes_the_prefs_path() {
    let storage = RecStorage::default();
    let mut rt = runtime(Editor::new(), storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::SavePrefs { contents: "line_numbers = true\n".into() });
    let saves = &storage.0.borrow().saves;
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].0, editor::PREFS_PATH);
}

#[test]
fn delete_effect_unlinks_through_storage() {
    let storage = RecStorage::default();
    let mut rt = runtime(Editor::new(), storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Delete { path: "/sd/local/scratch.md".into(), scope: Scope::Local });
    assert_eq!(storage.0.borrow().deletes, vec!["/sd/local/scratch.md".to_string()]);
}

#[test]
fn rename_effect_writes_the_new_path_then_unlinks_the_old() {
    // `:pub`/`:publish` is a write-new + unlink-old at the storage layer, so the
    // file is never missing and both paths land in the dirty journal for `:gs`.
    let storage = RecStorage::default();
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "body".into());
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Rename {
        from: "/sd/repo/notes.md".into(),
        to: "/sd/repo/notes.pub.md".into(),
        contents: "body".into(),
        retarget: vec![],
    });
    let log = storage.0.borrow();
    assert_eq!(log.saves, vec![("/sd/repo/notes.pub.md".into(), "body".into())]);
    assert_eq!(log.deletes, vec!["/sd/repo/notes.md".to_string()]);
}

#[test]
fn rename_effect_retargets_links_in_the_listed_files() {
    // Each `retarget` file that links to the old name is rewritten and saved —
    // joining the dirty journal, so `:gs` ships the rename and its link updates
    // together — while a file with no matching link is left unwritten.
    let storage = RecStorage::default()
        .with_body("/sd/repo/essay.md", "see [n](notes.md) and [n#](notes.md#top)")
        .with_body("/sd/repo/other.md", "no links here");
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "body".into());
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Rename {
        from: "/sd/repo/notes.md".into(),
        to: "/sd/repo/notes.pub.md".into(),
        contents: "body".into(),
        retarget: vec!["/sd/repo/essay.md".into(), "/sd/repo/other.md".into()],
    });
    let log = storage.0.borrow();
    assert_eq!(
        log.saves,
        vec![
            ("/sd/repo/notes.pub.md".into(), "body".into()),
            (
                "/sd/repo/essay.md".into(),
                "see [n](notes.pub.md) and [n#](notes.pub.md#top)".into()
            ),
        ]
    );
    assert_eq!(log.deletes, vec!["/sd/repo/notes.md".to_string()]);
}

#[test]
fn rename_effect_paints_a_publishing_clue_before_the_card_work() {
    // The rename + retarget is synchronous SD work; without an up-front paint
    // the panel sits frozen on the `:pub` command line until it finishes.
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "body".into());
    let screen = CountingScreen::default();
    let panel = Panel::new(screen.clone(), &mut ed).expect("first paint");
    let mut rt = Runtime::new(
        ed,
        panel,
        Box::new(NoKeyboard),
        Box::new(RecStorage::default()),
        Box::new(RecSync::new()),
        Box::new(FixedClock),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(RecFiles::default()),
    );
    let before = *screen.0.borrow();
    rt.service_one(Effect::Rename {
        from: "/sd/repo/notes.md".into(),
        to: "/sd/repo/notes.pub.md".into(),
        contents: "body".into(),
        retarget: vec![],
    });
    assert_eq!(*screen.0.borrow(), before + 1, "the publishing... clue must paint immediately");
}

#[test]
fn typed_publish_rewrites_a_subfolder_link_end_to_end() {
    // The whole chain, as the device runs it: the walk blob feeds the palette
    // file list on an idle tick, then a typed `:pub` publishes a subfolder file
    // — and the root file linking it as `llm/the-file.md` is rewritten on the
    // card in the same batch.
    let storage = RecStorage::default()
        .with_body("/sd/repo/index.md", "see [something](llm/the-file.md) here");
    let keyboard = ScriptedKeyboard::default();
    let mut ed =
        Editor::with_file("/sd/repo/llm/the-file.md".into(), Scope::Tracked, "# The file".into());
    let panel = Panel::new(MockScreen, &mut ed).expect("first paint");
    let mut rt = Runtime::new(
        ed,
        panel,
        Box::new(keyboard.clone()),
        Box::new(storage.clone()),
        Box::new(RecSync::new()),
        Box::new(FixedClock),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(WalkFiles(RefCell::new(Some(
            "/sd/repo/index.md\n/sd/repo/llm/the-file.md\n".into(),
        )))),
    );
    rt.tick(); // idle: the finished walk lands in the palette
    keyboard.type_line(":pub");
    rt.tick();
    let log = storage.0.borrow();
    assert_eq!(
        log.saves,
        vec![
            ("/sd/repo/llm/the-file.pub.md".into(), "# The file".into()),
            ("/sd/repo/index.md".into(), "see [something](llm/the-file.pub.md) here".into()),
        ]
    );
    assert_eq!(log.deletes, vec!["/sd/repo/llm/the-file.md".to_string()]);
}

#[test]
fn push_effect_dispatches_to_sync() {
    let sync = RecSync::new();
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::Push);
    assert_eq!(sync.log.borrow().pushes, 1);
}

#[test]
fn pull_effect_dispatches_to_sync() {
    let sync = RecSync::new();
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Ask));
    assert_eq!(sync.log.borrow().pulls, 1);
}

#[test]
fn pull_with_unsynced_saves_raises_the_card_with_the_file_list() {
    // The backend reports NeedsConfirm with the journal's paths when it is
    // non-empty; the runtime must raise the card naming them rather than
    // dispatch, fail, or ask about an unnamed "some files".
    let sync = RecSync {
        pull_ret: Rc::new(|| {
            PullDispatch::NeedsConfirm(vec![
                editor::Unsynced { path: "notes.md".into(), deleted: false },
                editor::Unsynced { path: "inbox/day.md".into(), deleted: true },
            ])
        }),
        ..RecSync::new()
    };
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync, RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Ask));
    assert_eq!(rt.ed.mode(), editor::Mode::Unsynced, "unsynced :gl must raise the card");
    let listed: Vec<&str> = rt.ed.unsynced().iter().map(|u| u.path.as_str()).collect();
    assert_eq!(listed, vec!["notes.md", "inbox/day.md"], "the card must name every file");
}

#[test]
fn a_confirmed_discard_evicts_the_buffers_it_threw_away() {
    // The discarded file is the active buffer AND is RAM-dirty — the case the
    // pull path deliberately protects ("its edits win"). A discard must do the
    // opposite: re-read the rolled-back file, or the next save writes the
    // thrown-away text straight back onto the card.
    let storage = RecStorage::default().with_body("/sd/repo/notes.md", "last-synced");
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "old".into());
    ed.show_unsynced(vec![editor::Unsynced { path: "notes.md".into(), deleted: false }]);
    let sync = RecSync::new();
    let files = RecFiles::default();
    let mut rt = runtime(ed, storage.clone(), sync.clone(), files.clone());
    // Serviced with the card still up — that is where the runtime learns which
    // paths it is about to throw away (the editor keeps the list alive for it).
    rt.service_one(Effect::Pull(PullIntent::Discard));
    assert_eq!(sync.log.borrow().pull_intents, vec![PullIntent::Discard]);

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::UpToDate));

    assert_eq!(
        storage.0.borrow().loads,
        vec!["/sd/repo/notes.md".to_string(), editor::PREFS_PATH.to_string()],
        "the roll-back can have restored the prefs file too, so it is re-read"
    );
    assert_eq!(rt.ed.text(), "last-synced", "the buffer must show the rolled-back file");
    assert!(!rt.ed.dirty(), "the reloaded buffer must be clean");
    assert_eq!(*files.0.borrow(), 1, "a discard changes the card, so the palette re-walks");
}

#[test]
fn a_discard_that_removed_the_file_drops_its_buffer() {
    // A note written on the device and never synced has no version to roll
    // back to, so the discard unlinked it. The buffer must go with it rather
    // than sit there ready to re-create the file on the next save.
    let storage = RecStorage::default().with_missing("/sd/repo/new.md");
    let mut ed = Editor::with_file("/sd/repo/new.md".into(), Scope::Tracked, "draft".into());
    ed.show_unsynced(vec![editor::Unsynced { path: "new.md".into(), deleted: false }]);
    let mut rt = runtime(ed, storage, RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Discard));

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::UpToDate));

    assert_eq!(rt.ed.path(), "", "the buffer for a removed file must be abandoned");
    assert_eq!(rt.ed.text(), "", "and must not keep the discarded text");
}

#[test]
fn a_failed_discarding_pull_still_evicts_the_buffers() {
    // The rollback happens before the fetch, so a fetch failure does not put
    // the discarded text back — the buffers are stale either way.
    let storage = RecStorage::default().with_body("/sd/repo/notes.md", "last-synced");
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "old".into());
    ed.show_unsynced(vec![editor::Unsynced { path: "notes.md".into(), deleted: false }]);
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Discard));

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::Failed("offline".into())));

    assert_eq!(rt.ed.text(), "last-synced", "a failed fetch must not resurrect discarded text");
}

#[test]
fn a_thread_down_discard_leaves_the_buffers_alone() {
    // Nothing reached the net thread, so nothing was rolled back — the text
    // the writer confirmed away is still the only copy, and must survive.
    let sync = RecSync { pull_ret: Rc::new(|| PullDispatch::ThreadDown), ..RecSync::new() };
    let storage = RecStorage::default().with_body("/sd/repo/notes.md", "last-synced");
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "old".into());
    ed.show_unsynced(vec![editor::Unsynced { path: "notes.md".into(), deleted: false }]);
    let mut rt = runtime(ed, storage.clone(), sync, RecFiles::default());
    rt.service_one(Effect::Pull(PullIntent::Discard));
    assert!(storage.0.borrow().loads.is_empty(), "a dispatch that never left must reload nothing");
    assert_eq!(rt.ed.text(), "old", "the buffer must be untouched");
}

#[test]
fn pull_that_moves_the_tree_reloads_active_and_rewalks() {
    let storage = RecStorage::default();
    let files = RecFiles::default();
    // A clean, named active buffer — a moving pull re-reads it from disk.
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "old".into());
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), files.clone());

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::Pulled("abc".into())));

    assert_eq!(
        storage.0.borrow().loads,
        vec![editor::PREFS_PATH.to_string(), "/sd/repo/notes.md".to_string()],
        "the pulled prefs are read before the buffer that renders under them"
    );
    assert_eq!(*files.0.borrow(), 1, "palette should be re-walked after a moving pull");
}

#[test]
fn a_moving_pull_installs_the_prefs_it_pulled() {
    // The whole point: a pref edited on a computer takes effect on the pull, not
    // one reboot later (`Runtime::reload_prefs` for what a stale copy costs).
    let storage = RecStorage::default()
        .with_body(editor::PREFS_PATH, "hidden_folders = \"_*,!_inbox\"\nline_numbers = false\n");
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "old".into());
    let mut rt = runtime(ed, storage, RecSync::new(), RecFiles::default());
    assert_eq!(rt.ed.prefs().hidden_folders, "", "boot default");

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::Pulled("abc".into())));

    assert_eq!(rt.ed.prefs().hidden_folders, "_*,!_inbox");
    assert!(!rt.ed.prefs().line_numbers, "every key rides the reload, not just the new one");
}

#[test]
fn a_pull_that_moved_nothing_leaves_the_prefs_alone() {
    // No apply ran, so the card's prefs file is the one already in RAM. Reading it
    // again would cost an SD read on every auto-sync tick for nothing.
    let storage = RecStorage::default();
    let mut rt = runtime(Editor::new(), storage.clone(), RecSync::new(), RecFiles::default());

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::UpToDate));

    assert!(storage.0.borrow().loads.is_empty());
}

#[test]
fn an_unreadable_prefs_file_keeps_the_running_preferences() {
    // A card with no prefs file is normal, and an SD hiccup must not silently
    // reset the writer's settings to the defaults mid-session.
    let storage = RecStorage::default().with_missing(editor::PREFS_PATH);
    let mut ed = Editor::new();
    ed.set_prefs(editor::Prefs { hidden_folders: "_archive".into(), ..Default::default() });
    let mut rt = runtime(ed, storage, RecSync::new(), RecFiles::default());

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::Pulled("abc".into())));

    assert_eq!(rt.ed.prefs().hidden_folders, "_archive");
}

#[test]
fn up_to_date_pull_leaves_the_tree_untouched() {
    let storage = RecStorage::default();
    let files = RecFiles::default();
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "old".into());
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), files.clone());

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::UpToDate));

    assert!(storage.0.borrow().loads.is_empty(), "no reload when the tree didn't move");
    assert_eq!(*files.0.borrow(), 0, "no re-walk when the tree didn't move");
}

// ---- the clock-only sync behind `:inbox` --------------------------------

/// A fixed "today" for the held-`:inbox` tests.
const TODAY: editor::Date = editor::Date { year: 2026, month: 7, day: 18 };
const INBOX_TODAY: &str = "/sd/repo/_inbox/2026-07-18.md";

/// Build a runtime on a clock the test drives, so a date can land mid-session
/// the way SNTP lands one on the device.
fn runtime_on_clock(
    ed: Editor,
    sync: RecSync,
    keyboard: ScriptedKeyboard,
    clock: SettableClock,
) -> Runtime<MockScreen> {
    let mut ed = ed;
    let panel = Panel::new(MockScreen, &mut ed).expect("first paint");
    Runtime::new(
        ed,
        panel,
        Box::new(keyboard),
        Box::new(RecStorage::default()),
        Box::new(sync),
        Box::new(clock),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(RecFiles::default()),
    )
}

#[test]
fn boot_asks_for_the_clock_when_it_is_unset() {
    // No battery-backed RTC: a cold boot has no date, so the radio thread is
    // asked for one unprompted — `:inbox` must not be the thing that discovers it.
    let sync = RecSync::new();
    let _rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    assert_eq!(sync.log.borrow().clocks, 1);
    assert_eq!(sync.log.borrow().pulls, 0, "a date must cost no fetch");
    assert_eq!(sync.log.borrow().pushes, 0);
}

#[test]
fn boot_leaves_an_already_set_clock_alone() {
    // A `:update` / `:setup` restart keeps the wall clock, so waking the radio
    // again would be drain for a date we already hold.
    let sync = RecSync::new();
    let clock = SettableClock::default();
    clock.set(TODAY);
    let _rt = runtime_on_clock(Editor::new(), sync.clone(), ScriptedKeyboard::default(), clock);
    assert_eq!(sync.log.borrow().clocks, 0);
}

#[test]
fn sync_clock_effect_dispatches_to_the_net_thread() {
    let sync = RecSync::new();
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    let boot = sync.log.borrow().clocks;
    rt.service_one(Effect::SyncClock);
    assert_eq!(sync.log.borrow().clocks, boot + 1);
}

#[test]
fn a_held_inbox_opens_when_the_date_lands_on_an_idle_pass() {
    // The whole fast path: `:inbox` on a cold clock holds, the SNTP-only sync
    // sets the wall clock, and the next pass opens the dated note with no
    // keystroke behind it — so this pass has to paint it itself.
    let clock = SettableClock::default();
    let keyboard = ScriptedKeyboard::default();
    let sync = RecSync::new();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    // The card's contents have to be known before `:inbox` may decide the note
    // is new: an unwalked list would send it down the fresh-note branch.
    ed.set_file_list(vec!["/sd/repo/notes.md".into()]);
    let mut rt = runtime_on_clock(ed, sync.clone(), keyboard.clone(), clock.clone());

    keyboard.type_line(":inbox");
    rt.tick();
    assert!(rt.ed.inbox_pending(), "no date yet — the request waits");
    assert_eq!(rt.ed.path(), "/sd/repo/notes.md");
    assert_eq!(sync.log.borrow().clocks, 2, "the boot kick, then `:inbox` asking again");

    clock.set(TODAY);
    rt.tick();

    assert!(!rt.ed.inbox_pending());
    assert_eq!(rt.ed.path(), INBOX_TODAY);
    assert_eq!(rt.ed.text(), "# 18/07/2026\n\n");
    assert_eq!(sync.log.borrow().pulls, 0, "and still no fetch anywhere in it");
}

#[test]
fn a_clock_outcome_does_not_settle_a_running_sync() {
    // The boot clock sync is dispatched without a dispatch slot, so settling one
    // when it lands would pop the slot of a `:gs`/`:gl` that is still running —
    // dropping the panel flag and handing back the idle-save hold-off that the
    // pull's hash belt depends on.
    let storage = RecStorage::default();
    let sync = RecSync::new();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.handle(hal::Key::Char('i'));
    ed.handle(hal::Key::Char('x'));
    let mut rt = runtime(ed, storage.clone(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::Push);
    assert!(rt.ed.net_flag().is_some());

    sync.log.borrow_mut().outcome = Some(NetOutcome::Clock(ClockOutcome::Synced));
    rt.tick();
    assert!(rt.ed.net_flag().is_some(), "the push is still running");
    assert!(rt.sync_holds_off_save(), "and still protected");

    rt.last_activity = Instant::now() - Duration::from_millis(IDLE_SAVE_MS as u64 + 1);
    rt.tick();
    assert!(storage.0.borrow().saves.is_empty(), "so no write lands behind it");
}

#[test]
fn a_silent_clock_sync_costs_no_repaint() {
    // Nothing about a successful boot clock sync is visible, and `show_notice`
    // has no change detection — reporting it as settled would spend a full-panel
    // partial (~630ms of drive, one of 64 ghosting slots) on every boot.
    let clock = SettableClock::default();
    let screen = CountingScreen::default();
    let sync = RecSync::new();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.set_file_list(Vec::new());
    let panel = Panel::new(screen.clone(), &mut ed).expect("first paint");
    let mut rt = Runtime::new(
        ed,
        panel,
        Box::new(ScriptedKeyboard::default()),
        Box::new(RecStorage::default()),
        Box::new(sync.clone()),
        Box::new(clock.clone()),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(RecFiles::default()),
    );
    rt.tick();
    let before = *screen.0.borrow();

    sync.log.borrow_mut().outcome = Some(NetOutcome::Clock(ClockOutcome::Synced));
    rt.tick();
    assert_eq!(*screen.0.borrow(), before, "a silent outcome paints nothing");
}

#[test]
fn a_held_inbox_waits_for_the_card_walk_before_deciding_the_note_is_new() {
    // The boot clock lands inside the walk window, so the "already on the card"
    // guard would be answered from an empty list: `:inbox` would seed a dated
    // stub over the note the writer filled this morning.
    let clock = SettableClock::default();
    let keyboard = ScriptedKeyboard::default();
    let storage = RecStorage::default().with_body(INBOX_TODAY, "this morning's prose");
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    let panel = Panel::new(MockScreen, &mut ed).expect("first paint");
    let mut rt = Runtime::new(
        ed,
        panel,
        Box::new(keyboard.clone()),
        Box::new(storage.clone()),
        Box::new(RecSync::new()),
        Box::new(clock.clone()),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(WalkFiles(RefCell::new(Some(INBOX_TODAY.to_string())))),
    );

    keyboard.type_line(":inbox");
    rt.tick();
    clock.set(TODAY);
    rt.tick();
    assert_eq!(rt.ed.path(), INBOX_TODAY, "the walk landed, so the note resolves");
    assert_eq!(
        storage.0.borrow().loads,
        vec![INBOX_TODAY.to_string()],
        "loaded from the card, not seeded over"
    );
    assert_eq!(rt.ed.text(), "this morning's prose", "the writer's text, not a stub");
}

#[test]
fn the_note_a_held_inbox_opens_paints_itself() {
    // No keystroke is behind this open, so nothing else in the pass would paint
    // it: the note would sit invisible behind the buffer the writer kept until
    // they happened to press a key.
    let clock = SettableClock::default();
    let keyboard = ScriptedKeyboard::default();
    let screen = CountingScreen::default();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.set_file_list(vec!["/sd/repo/notes.md".into()]);
    let panel = Panel::new(screen.clone(), &mut ed).expect("first paint");
    let mut rt = Runtime::new(
        ed,
        panel,
        Box::new(keyboard.clone()),
        Box::new(RecStorage::default()),
        Box::new(RecSync::new()),
        Box::new(clock.clone()),
        Box::new(PanicSystem),
        Box::new(NoPower),
        Box::new(RecFiles::default()),
    );

    keyboard.type_line(":inbox");
    rt.tick();
    let before = *screen.0.borrow();

    clock.set(TODAY);
    rt.tick();

    assert_eq!(rt.ed.path(), INBOX_TODAY);
    assert_eq!(*screen.0.borrow(), before + 1, "the note has to reach the panel unprompted");
}

#[test]
fn a_failed_clock_outcome_releases_the_hold_over_the_net_channel() {
    // The same failure as `settle_clock`'s, taken through the arm that actually
    // receives it — a hold left standing here waits on a date nothing will send.
    let clock = SettableClock::default();
    let keyboard = ScriptedKeyboard::default();
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    let mut rt = runtime_on_clock(ed, RecSync::new(), keyboard.clone(), clock);

    keyboard.type_line(":inbox");
    rt.tick();
    assert!(rt.ed.inbox_pending());

    rt.handle_net_outcome(NetOutcome::Clock(ClockOutcome::Failed("clock: no wifi".into())));

    assert!(!rt.ed.inbox_pending());
    assert_eq!(rt.ed.path(), "/sd/repo/notes.md", "and the writer keeps their buffer");
}

#[test]
fn a_failed_clock_sync_speaks_only_to_a_held_inbox() {
    let clock = SettableClock::default();
    let keyboard = ScriptedKeyboard::default();
    let mut rt =
        runtime_on_clock(Editor::new(), RecSync::new(), keyboard.clone(), clock);

    // Nobody asked: the boot kick's failure must not open an offline session on
    // a network error.
    assert!(rt.settle_clock(ClockOutcome::Failed("clock: no wifi".into())).is_none());

    keyboard.type_line(":inbox");
    rt.tick();
    assert!(rt.ed.inbox_pending());
    assert_eq!(
        rt.settle_clock(ClockOutcome::Failed("clock: no wifi".into())).as_deref(),
        Some("clock: no wifi"),
    );
    assert!(!rt.ed.inbox_pending(), "the hold is dropped, not left to a stale date");
}

#[test]
fn a_dead_net_thread_releases_the_held_inbox() {
    // Nothing is ever going to report back, so a hold would sit there for the
    // rest of the session waiting on a date that cannot arrive.
    let sync = RecSync { clock_ret: Rc::new(|| ClockDispatch::ThreadDown), ..RecSync::new() };
    let keyboard = ScriptedKeyboard::default();
    let mut rt =
        runtime_on_clock(Editor::new(), sync, keyboard.clone(), SettableClock::default());

    keyboard.type_line(":inbox");
    rt.tick();

    assert!(!rt.ed.inbox_pending(), "a hold nothing can resolve must not be kept");
}

#[test]
fn a_successful_clock_sync_announces_nothing_of_its_own() {
    // The date is the whole message, and it travels through `Clock::today`.
    let mut rt = runtime(Editor::new(), RecStorage::default(), RecSync::new(), RecFiles::default());
    assert!(rt.settle_clock(ClockOutcome::Synced).is_none());
}

#[test]
fn a_clock_outcome_settles_no_buffers_and_no_walk() {
    // Unlike a pull, an SNTP sync never touches the working copy.
    let storage = RecStorage::default().with_body("/sd/repo/notes.md", "on-card");
    let files = RecFiles::default();
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "in-buffer".into());
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), files.clone());

    rt.handle_net_outcome(NetOutcome::Clock(ClockOutcome::Synced));

    assert_eq!(rt.ed.text(), "in-buffer");
    assert!(storage.0.borrow().loads.is_empty());
    assert_eq!(*files.0.borrow(), 0);
}

#[test]
fn update_check_effect_dispatches_to_sync() {
    let sync = RecSync::new();
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::UpdateCheck);
    assert_eq!(sync.log.borrow().update_checks, 1);
}

#[test]
fn install_effect_carries_the_version_the_check_offered() {
    let sync = RecSync::new();
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::UpdateInstall("0.15.0".into()));
    assert_eq!(sync.log.borrow().update_installs, vec!["0.15.0".to_string()]);
}

#[test]
fn an_available_release_prompts_instead_of_installing() {
    // The check is terminal: nothing is downloaded until the writer says yes.
    let sync = RecSync::new();
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    rt.handle_net_outcome(NetOutcome::Update(UpdateOutcome::Available("0.15.0".into())));
    assert_eq!(rt.ed.mode(), editor::Mode::Confirm);
    assert!(sync.log.borrow().update_installs.is_empty());
}

#[test]
#[should_panic(expected = "reboot in test")]
fn installed_update_reboots_into_the_new_image() {
    // A successful install makes the new slot the boot target; the runtime must
    // reboot into it. PanicSystem's reboot panics, which is the reboot signal here.
    let mut rt =
        runtime(Editor::new(), RecStorage::default(), RecSync::new(), RecFiles::default());
    rt.handle_net_outcome(NetOutcome::Update(UpdateOutcome::Installed("0.8.0".into())));
}

#[test]
fn up_to_date_update_does_not_reboot() {
    // Already newest → a notice, no restart. The test completing (PanicSystem's
    // reboot never fires) is the assertion; Failed takes the same non-reboot path.
    let mut rt =
        runtime(Editor::new(), RecStorage::default(), RecSync::new(), RecFiles::default());
    rt.handle_net_outcome(NetOutcome::Update(UpdateOutcome::UpToDate("0.7.7".into())));
    rt.handle_net_outcome(NetOutcome::Update(UpdateOutcome::Failed("no wifi".into())));
}

// ─── Power ────────────────────────────────────────────────────────────────────

/// Build a runtime over a scripted power port, everything else defaulted.
fn power_runtime(ed: Editor, storage: RecStorage, power: ScriptedPower) -> Runtime<MockScreen> {
    let mut ed = ed;
    let panel = Panel::new(MockScreen, &mut ed).expect("first paint");
    Runtime::new(
        ed,
        panel,
        Box::new(NoKeyboard),
        Box::new(storage),
        Box::new(RecSync::new()),
        Box::new(FixedClock),
        Box::new(PanicSystem),
        Box::new(power),
        Box::new(RecFiles::default()),
    )
}

#[test]
fn a_held_button_saves_the_buffer_before_it_cuts_power() {
    // The whole point of a soft power button: the card is current by the time the
    // rails drop. The Save must land in the same pass as the shutdown, since no
    // further pass is coming.
    let storage = RecStorage::default();
    let power = ScriptedPower::default();
    let mut ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ed.handle(hal::Key::Char('i'));
    ed.handle(hal::Key::Char('h'));
    ed.handle(hal::Key::Escape);
    let mut rt = power_runtime(ed, storage.clone(), power.clone());

    power.queue(PowerEvent::OffAsked);
    shut_down(&mut rt);

    assert!(power.powered_off(), "a held button must reach the shutdown");
    assert_eq!(
        storage.0.borrow().saves.first().map(|(p, _)| p.clone()),
        Some("/sd/repo/notes.md".to_string()),
        "the dirty buffer must be on the card before the rails drop"
    );
}

/// Drive one tick that is expected to end in [`ScriptedPower::power_off`], and
/// swallow the divergence it panics with.
fn shut_down(rt: &mut Runtime<MockScreen>) {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt.tick()));
    assert!(caught.is_err(), "the tick was expected to reach power_off");
}

#[test]
fn a_tapped_button_reports_the_charge_and_leaves_the_machine_on() {
    let power = ScriptedPower::default();
    power.set_battery(editor::Battery { percent: 62, charging: true });
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    let mut rt = power_runtime(ed, RecStorage::default(), power.clone());

    power.queue(PowerEvent::StatusAsked);
    rt.tick();

    assert!(!power.powered_off(), "a tap must not switch the machine off");
    assert_eq!(rt.ed.notice(), Some("battery 62% - charging"));
}

#[test]
fn a_flat_cell_shuts_down_on_its_own() {
    // Nobody is at the keyboard when this fires, so the loop has to do the whole
    // sequence itself — the alternative is a brownout mid-write.
    let power = ScriptedPower::default();
    power.set_battery(editor::Battery { percent: 2, charging: false });
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    let mut rt = power_runtime(ed, RecStorage::default(), power.clone());

    power.queue(PowerEvent::Critical);
    shut_down(&mut rt);

    assert!(power.powered_off(), "a critical cell must reach the shutdown");
}

#[test]
fn the_cell_reading_reaches_the_panel() {
    let power = ScriptedPower::default();
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    let mut rt = power_runtime(ed, RecStorage::default(), power.clone());

    power.set_battery(editor::Battery { percent: 17, charging: false });
    rt.tick();

    assert_eq!(rt.ed.battery().map(|b| b.percent), Some(17));
}
