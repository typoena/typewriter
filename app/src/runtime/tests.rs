//! Host tests for the lifted run loop — the routing that used to be untestable
//! inline in the firmware binary. In-memory doubles stand in for every port, so
//! these run on the host with no esp-idf.

use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

use editor::{Editor, Effect, Scope};

use super::*;
use crate::ports::{
    Clock, FileIndex, PushDispatch, PushOutcome, PullDispatch, PullOutcome, SetupDispatch,
    Storage, NetOutcome, NetService, System, UpdateDispatch, UpdateOutcome,
};
use crate::render::Panel;

// ---- test doubles ---------------------------------------------------------

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

/// A screen that counts partial-window paints (the kbd-flag repaint path).
#[derive(Clone, Default)]
struct CountingScreen(Rc<RefCell<u32>>);
impl hal::Screen for CountingScreen {
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
}
impl Storage for RecStorage {
    fn save_path(&self, path: &str, contents: &str) -> anyhow::Result<()> {
        self.0.borrow_mut().saves.push((path.into(), contents.into()));
        Ok(())
    }
    fn load_path(&self, path: &str) -> anyhow::Result<String> {
        let mut log = self.0.borrow_mut();
        log.loads.push(path.into());
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
    updates: u32,
    outcome: Option<NetOutcome>,
}

/// Configurable dispatch results + a single queued outcome.
#[derive(Clone)]
struct RecSync {
    log: Rc<RefCell<SyncLog>>,
    push_ret: Rc<dyn Fn() -> PushDispatch>,
    pull_ret: Rc<dyn Fn() -> PullDispatch>,
    update_ret: Rc<dyn Fn() -> UpdateDispatch>,
}
impl RecSync {
    fn new() -> Self {
        Self {
            log: Rc::new(RefCell::new(SyncLog::default())),
            push_ret: Rc::new(|| PushDispatch::Dispatched),
            pull_ret: Rc::new(|| PullDispatch::Dispatched),
            update_ret: Rc::new(|| UpdateDispatch::Dispatched),
        }
    }
}
impl NetService for RecSync {
    fn push(&self) -> PushDispatch {
        self.log.borrow_mut().pushes += 1;
        (self.push_ret)()
    }
    fn pull(&self, _commit_dirty: bool) -> PullDispatch {
        self.log.borrow_mut().pulls += 1;
        (self.pull_ret)()
    }
    fn update(&self) -> UpdateDispatch {
        self.log.borrow_mut().updates += 1;
        (self.update_ret)()
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

struct PanicSystem;
impl System for PanicSystem {
    fn prepare_setup(&self) -> SetupDispatch {
        SetupDispatch::MarkerFailed
    }
    fn reboot(&self) -> ! {
        panic!("reboot in test")
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
        Box::new(files),
    )
}

// ---- pure helpers ---------------------------------------------------------

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
fn pull_notice_covers_every_variant() {
    assert_eq!(pull_notice(&PullOutcome::Pulled("abc".into())), "pulled abc");
    assert_eq!(pull_notice(&PullOutcome::Rebased("def".into())), "rebased def - :gs to push");
    assert_eq!(pull_notice(&PullOutcome::UpToDate), "up to date");
    assert_eq!(pull_notice(&PullOutcome::LocalAhead), "ahead - :gs to push");
    assert_eq!(pull_notice(&PullOutcome::Failed("boom".into())), "boom");
}

// ---- keyboard flag --------------------------------------------------------

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

// ---- effect routing -------------------------------------------------------

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
        Box::new(WalkFiles(RefCell::new(Some(
            "/sd/repo/index.md\n/sd/repo/llm/the-file.md\n".into(),
        )))),
    );
    rt.tick(); // idle: the finished walk lands in the palette
    keyboard.type_line(":pub");
    rt.tick(); // the typed publish drains and its effects are serviced
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
    rt.service_one(Effect::Pull { commit_dirty: false });
    assert_eq!(sync.log.borrow().pulls, 1);
}

#[test]
fn pull_with_unsynced_saves_opens_the_commit_confirm() {
    // The backend reports NeedsCommitConfirm when the dirty journal is non-empty;
    // the runtime must open the editor's y/n prompt rather than dispatch or fail.
    let sync = RecSync {
        pull_ret: Rc::new(|| PullDispatch::NeedsCommitConfirm),
        ..RecSync::new()
    };
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync, RecFiles::default());
    rt.service_one(Effect::Pull { commit_dirty: false });
    assert_eq!(rt.ed.mode(), editor::Mode::Confirm, "unsynced :gl must prompt");
}

// ---- sync outcome ---------------------------------------------------------

#[test]
fn pull_that_moves_the_tree_reloads_active_and_rewalks() {
    let storage = RecStorage::default();
    let files = RecFiles::default();
    // A clean, named active buffer — a moving pull re-reads it from disk.
    let ed = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "old".into());
    let mut rt = runtime(ed, storage.clone(), RecSync::new(), files.clone());

    rt.handle_net_outcome(NetOutcome::Pull(PullOutcome::Pulled("abc".into())));

    assert_eq!(storage.0.borrow().loads, vec!["/sd/repo/notes.md".to_string()]);
    assert_eq!(*files.0.borrow(), 1, "palette should be re-walked after a moving pull");
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

// ---- firmware update ------------------------------------------------------

#[test]
fn update_effect_dispatches_to_sync() {
    let sync = RecSync::new();
    let mut rt = runtime(Editor::new(), RecStorage::default(), sync.clone(), RecFiles::default());
    rt.service_one(Effect::Update);
    assert_eq!(sync.log.borrow().updates, 1);
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
