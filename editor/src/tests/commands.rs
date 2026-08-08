//! Ex commands (`:w`, `:gs`, `:gl`, format-on-save, aliases) and command-line editing.

use super::*;

#[test]
fn w_command_signals_save_and_returns_to_normal() {
    let (e, effs) = command("w");
    assert_eq!(
        effs,
        vec![Effect::Save {
            path: "/sd/repo/notes.md".into(),
            scope: Scope::Tracked,
            contents: String::new(),
        }]
    );
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn gs_command_saves_then_pushes() {
    // `:gs` queues a save of the current buffer, then the git push.
    assert_eq!(kinds(&command("gs").1), vec![Kind::Save, Kind::Push]);
}

#[test]
fn gl_command_signals_pull() {
    // A bare `:gl` pulls without committing — the host decides whether to prompt
    // for a pre-fetch commit based on its own dirty journal.
    let effs = command("gl").1;
    assert_eq!(kinds(&effs), vec![Kind::Pull]);
    assert!(
        matches!(effs.as_slice(), [Effect::Pull(PullIntent::Ask)]),
        "bare :gl must not pre-authorize the commit",
    );
}

#[test]
fn normal_mode_gs_and_gl_mirror_the_colon_commands() {
    // `gs` (go-sync) and `gl` in Normal mode are the `:gs` push and `:gl`
    // pull, one keystroke shorter. `gs`, not `gp`: vim binds `gp` to
    // paste-after, and a paste habit must never fire a push.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "hi".into());
    send(&mut e, "gs");
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Save, Kind::Push]);
    send(&mut e, "gl");
    assert!(
        matches!(e.take_effects().as_slice(), [Effect::Pull(PullIntent::Ask)]),
        "bare gl must not pre-authorize the commit",
    );
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn normal_mode_gs_from_a_local_buffer_is_refused() {
    // Same guard as `:gs`: Local files never push.
    let mut e = Editor::with_file("/sd/local/j.md".into(), Scope::Local, "diary".into());
    send(&mut e, "gs");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.notice.as_deref(), Some("Push unavailable (Local)"));
}

/// An editor showing the unsynced card for two files, one of them deleted.
fn with_unsynced_card() -> Editor {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.show_unsynced(vec![
        Unsynced { path: "notes.md".into(), deleted: false },
        Unsynced { path: "inbox/2026-08-05.md".into(), deleted: true },
    ]);
    e
}

#[test]
fn unsynced_card_lists_the_files_and_waits() {
    // The host raises this when `:gl` found unpushed saves. It must name them
    // and do nothing until answered — the whole point is seeing what you left
    // behind before choosing.
    let e = with_unsynced_card();
    assert_eq!(e.mode(), Mode::Unsynced, "expected the unsynced card");
    assert_eq!(e.unsynced().len(), 2, "the card must carry both files");
    assert!(e.showing_unsynced(), "the card must be on screen");
}

#[test]
fn unsynced_card_enter_queues_a_committing_pull() {
    let mut e = with_unsynced_card();
    assert!(e.take_effects().is_empty(), "must not act before an answer");
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Normal);
    let effs = e.take_effects();
    assert!(
        matches!(effs.as_slice(), [Effect::Pull(PullIntent::Commit)]),
        "Enter must authorize the commit; got {:?}",
        kinds(&effs),
    );
}

#[test]
fn unsynced_card_discard_needs_a_second_confirm() {
    // `d` alone must not throw work away — it opens a y/n naming the count,
    // and the card stays up underneath so the list is still readable.
    let mut e = with_unsynced_card();
    e.handle(Key::Char('d'));
    assert_eq!(e.mode(), Mode::Confirm, "`d` must prompt, not act");
    assert!(e.take_effects().is_empty(), "`d` alone must queue nothing");
    assert!(e.showing_unsynced(), "the list must stay visible behind the prompt");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("discard 2 files"),
        "the prompt must name the count, got {:?}",
        e.notice,
    );
    confirm(&mut e); // presses y
    let effs = e.take_effects();
    assert!(
        matches!(effs.as_slice(), [Effect::Pull(PullIntent::Discard)]),
        "confirmed discard must queue a discarding pull; got {:?}",
        kinds(&effs),
    );
    assert_eq!(
        e.unsynced().len(),
        2,
        "the list must outlive the answer — the host reads it to settle the buffers",
    );
}

#[test]
fn cancelled_discard_returns_to_the_card_not_out_of_the_pull() {
    // Backing out of the scariest prompt on the device must land you back on
    // the list, still able to commit — not silently out of the sync.
    let mut e = with_unsynced_card();
    e.handle(Key::Char('d'));
    e.handle(Key::Char('n')); // not y → cancel
    assert_eq!(e.mode(), Mode::Unsynced, "cancel must drop back onto the card");
    assert!(e.take_effects().is_empty(), "a cancelled discard must queue nothing");
    e.handle(Key::Enter);
    assert!(
        matches!(e.take_effects().as_slice(), [Effect::Pull(PullIntent::Commit)]),
        "the pull must still be answerable after a cancelled discard",
    );
}

#[test]
fn unsynced_card_escape_cancels_the_pull() {
    let mut e = with_unsynced_card();
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.take_effects().is_empty(), "a cancelled pull must queue nothing");
    assert!(e.unsynced().is_empty(), "the card's list must not outlive a cancel");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("cancelled"),
        "expected a cancellation notice, got {:?}",
        e.notice,
    );
}

#[test]
fn unsynced_card_swallows_editing_keys() {
    // Modal like Rest: the buffer behind the card must not take edits, and
    // Cmd+S must not slip a save past a decision about the dirty journal.
    let mut e = with_unsynced_card();
    e.handle(Key::Char('i'));
    e.handle(Key::Char('x'));
    e.handle(Key::Save);
    assert_eq!(e.mode(), Mode::Unsynced, "the card must hold");
    assert_eq!(e.text(), "", "no key may reach the buffer behind the card");
    assert!(e.take_effects().is_empty(), "no key may queue an effect from the card");
}

#[test]
fn draw_the_unsynced_card_does_not_panic() {
    // The rows do char arithmetic to keep a long path clear of the `(deleted)`
    // tag, and the scroll offset is clamped in the painter (the key handler
    // lets it run free) — both are only exercised by actually drawing.
    let mut e = with_unsynced_card();
    let _ = e.draw(true);
    e.handle(Key::Char('d')); // the confirm draws *over* the card
    let _ = e.draw(true);
    e.handle(Key::Char('n'));

    // A path far past the column edge, tagged deleted, in a list longer than
    // the card — then scrolled well beyond its end.
    let long = "a-very/deeply/nested/folder/chain/".repeat(4) + "note.md";
    let many: Vec<Unsynced> = (0..60)
        .map(|i| Unsynced { path: format!("{long}{i}"), deleted: i % 2 == 0 })
        .collect();
    e.show_unsynced(many);
    let _ = e.draw(true);
    for _ in 0..80 {
        e.handle(Key::Char('j'));
    }
    let _ = e.draw(true);
    for _ in 0..200 {
        e.handle(Key::Char('k'));
    }
    let _ = e.draw(true);
}

#[test]
fn an_empty_unsynced_list_skips_the_card() {
    // The journal and this list are read a moment apart; a card with nothing
    // to decide about would just be a dead end.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.show_unsynced(Vec::new());
    assert_eq!(e.mode(), Mode::Normal);
    assert!(matches!(e.take_effects().as_slice(), [Effect::Pull(PullIntent::Commit)]));
}

#[test]
fn setup_command_requests_the_wizard_when_clean() {
    // A fresh clean buffer → `:setup` prompts; on `y` it asks the host to reboot
    // into the wizard.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "setup");
    assert!(e.take_effects().is_empty(), "must not act before confirmation");
    confirm(&mut e);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Setup]);
}

#[test]
fn setup_prompt_cancels_on_any_other_key() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "setup");
    e.handle(Key::Escape); // not y → cancel
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.take_effects().is_empty(), "cancelled :setup must queue nothing");
}

#[test]
fn setup_command_is_refused_with_unsaved_changes() {
    // Dirty the buffer, then `:setup` — the reboot would lose the edit, so it
    // refuses with a notice and queues nothing.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char('i'));
    send(&mut e, "hi");
    e.handle(Key::Escape);
    ex(&mut e, "setup");
    assert!(e.take_effects().is_empty(), "dirty :setup must queue nothing");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("unsaved"),
        "expected an unsaved-changes notice, got {:?}",
        e.notice
    );
}

#[test]
fn reboot_command_requests_a_restart_when_clean() {
    // A fresh clean buffer → `:reboot` prompts; on `y` it asks the host to
    // restart, nothing to save.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "reboot");
    assert!(e.take_effects().is_empty(), "must not restart before confirmation");
    confirm(&mut e);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Reboot]);
}

#[test]
fn reboot_prompt_cancels_on_any_other_key() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "reboot");
    e.handle(Key::Char('n')); // not y → cancel
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.take_effects().is_empty(), "cancelled :reboot must queue nothing");
}

#[test]
fn reboot_command_autosaves_a_dirty_buffer_then_restarts() {
    // A named dirty buffer → `:reboot` saves it first, then restarts. The Save is
    // queued ahead of the Reboot so the host flushes it to the card before reset.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char('i'));
    send(&mut e, "hi");
    e.handle(Key::Escape);
    ex(&mut e, "reboot");
    confirm(&mut e);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Save, Kind::Reboot]);
}

#[test]
fn reboot_command_refuses_an_unsaved_unnamed_buffer() {
    // The unnamed scratch buffer (empty path) has nowhere to save to, so `:reboot`
    // blocks with a notice and queues nothing rather than lose the text on reset.
    let mut e = Editor::with_text(String::new());
    e.handle(Key::Char('i'));
    send(&mut e, "hi");
    e.handle(Key::Escape);
    ex(&mut e, "reboot");
    assert!(e.take_effects().is_empty(), "unnamed dirty :reboot must queue nothing");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("unnamed"),
        "expected an unnamed-buffer notice, got {:?}",
        e.notice
    );
}

#[test]
fn update_command_requests_an_ota_check_when_clean() {
    // A fresh clean buffer → `:update` prompts; on `y` it asks the host to run the
    // over-the-air update (which ends in a reboot into the new image).
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "update");
    assert_eq!(e.mode(), Mode::Confirm, "expected the update confirm prompt");
    assert!(e.take_effects().is_empty(), "must not act before confirmation");
    confirm(&mut e);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Update]);
}

#[test]
fn update_prompt_cancels_on_any_other_key() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "update");
    e.handle(Key::Char('n')); // not y → cancel
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.take_effects().is_empty(), "cancelled :update must queue nothing");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("cancelled"),
        "expected a cancellation notice, got {:?}",
        e.notice
    );
}

#[test]
fn update_command_is_refused_with_unsaved_changes() {
    // Dirty the buffer, then `:update` — the post-install reboot would lose the
    // edit, so it refuses with a notice and queues nothing (mirrors `:setup`).
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char('i'));
    send(&mut e, "hi");
    e.handle(Key::Escape);
    ex(&mut e, "update");
    assert!(e.take_effects().is_empty(), "dirty :update must queue nothing");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("unsaved"),
        "expected an unsaved-changes notice, got {:?}",
        e.notice
    );
}

#[test]
fn about_command_opens_the_full_screen_splash() {
    // `:about` raises the read-only splash and queues nothing — it neither saves
    // nor touches the buffer.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_version("0.7.8");
    ex(&mut e, "about");
    assert_eq!(e.mode(), Mode::About);
    assert!(e.take_effects().is_empty(), ":about must queue nothing");
}

#[test]
fn about_splash_leaves_on_enter_q_or_esc() {
    for leave in [Key::Enter, Key::Char('q'), Key::Escape] {
        let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
        ex(&mut e, "about");
        assert_eq!(e.mode(), Mode::About);
        e.handle(leave);
        assert_eq!(e.mode(), Mode::Normal, "{leave:?} should leave the splash");
    }
}

#[test]
fn about_splash_swallows_other_keys() {
    // Every key but the leave keys is swallowed — the card is read-only, so a
    // stray press can't leave by accident or edit the hidden buffer.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "hello".into());
    ex(&mut e, "about");
    e.handle(Key::Char('x')); // would delete a char in Normal
    assert_eq!(e.mode(), Mode::About, "a stray key must not leave the splash");
    assert_eq!(e.text, "hello", "the buffer stays untouched behind the card");
}

#[test]
fn about_splash_renders_the_injected_version() {
    // The card paints the version, so a different version yields a different
    // frame — proof the host-fed number actually reaches the splash.
    let mut a = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    a.set_version("0.7.8");
    ex(&mut a, "about");
    let mut b = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    b.set_version("9.9.9");
    ex(&mut b, "about");
    assert_ne!(
        a.draw(true).bytes(),
        b.draw(true).bytes(),
        "the version must show on the card"
    );
}

#[test]
fn gs_formats_the_buffer_before_pushing() {
    // fmt → save → commit → push: `:gs` runs :fmt in-core first (default on).
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "hello   \nworld".to_string(), // trailing spaces
    );
    e.handle(Key::Char(':'));
    for c in "gs".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Save, Kind::Push]);
    assert_eq!(e.text(), "hello\nworld\n"); // :fmt stripped the spaces, ended the buffer
}

#[test]
fn gs_is_refused_in_a_local_buffer() {
    // Push is Tracked-only; `:gs` in Local queues nothing and warns.
    let mut e = Editor::with_file(
        "/sd/local/journal.md".into(),
        Scope::Local,
        "dear diary".to_string(),
    );
    e.handle(Key::Char(':'));
    for c in "gs".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert!(e.take_effects().is_empty());
}

#[test]
fn format_on_save_off_leaves_the_buffer_untouched() {
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "hello   \nworld".to_string(),
    );
    e.prefs.format_on_save = false;
    e.handle(Key::Char(':'));
    e.handle(Key::Char('w'));
    e.handle(Key::Enter);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Save]);
    assert_eq!(e.text(), "hello   \nworld"); // unchanged when the pref is off
}

#[test]
fn format_keeps_at_most_one_trailing_blank_line() {
    // The writer's trailing blank line (pressed Enter to open the next line) is
    // kept; a run of them collapses to one.
    assert_eq!(format_markdown("hello\n"), "hello\n"); // one blank kept
    assert_eq!(format_markdown("hello\n\n\n"), "hello\n"); // extras collapsed to one
}

#[test]
fn format_terminates_the_buffer_with_a_newline() {
    // A buffer that ends mid-line gains the POSIX terminator (and with it the
    // empty last row) — no second pass, and an empty buffer stays empty.
    assert_eq!(format_markdown("hello"), "hello\n");
    assert_eq!(format_markdown("hello\nworld"), "hello\nworld\n");
    assert_eq!(format_markdown("hello\n"), "hello\n"); // idempotent
    assert_eq!(format_markdown(""), ""); // a blank scratch has no line to end
}

#[test]
fn format_on_save_adds_the_missing_trailing_newline() {
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "hello".to_string(), // no terminator — as a file authored elsewhere arrives
    );
    e.handle(Key::Char(':'));
    e.handle(Key::Char('w'));
    e.handle(Key::Enter);

    assert_eq!(e.text(), "hello\n", "lint-on-save terminated the buffer");
    let Some(Effect::Save { contents, .. }) = e.take_effects().into_iter().next() else {
        panic!("`:w` queued no Save effect");
    };
    assert_eq!(contents, "hello\n", "the saved contents carry the terminator");
}

#[test]
fn format_on_save_keeps_the_caret_on_a_trailing_blank_line() {
    // Regression: `:w` used to drop the trailing blank line and yank the caret
    // up onto the last non-empty line. The blank line — and the caret — stay.
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "hello\n".to_string(), // row 0 "hello", row 1 "" (a fresh empty line)
    );
    e.caret = e.text().len(); // caret at the very end = on the trailing blank row
    let lay = e.layout();
    assert_eq!(e.caret_rc(&lay).0, 1, "precondition: caret on the blank row");

    e.handle(Key::Char(':'));
    e.handle(Key::Char('w'));
    e.handle(Key::Enter);

    assert_eq!(e.text(), "hello\n", "trailing blank line survived format-on-save");
    let lay = e.layout();
    assert_eq!(e.caret_rc(&lay).0, 1, "caret stayed on the blank row");
}

#[test]
fn wq_and_x_alias_save_dropping_the_quit() {
    assert_eq!(kinds(&command("wq").1), vec![Kind::Save]);
    assert_eq!(kinds(&command("x").1), vec![Kind::Save]);
}

// --- Cmd+S (Key::Save) -----------------------------------------------------

#[test]
fn cmd_s_saves_a_dirty_buffer_like_w() {
    // From Normal on a dirty buffer, Cmd+S queues exactly the Save `:w` would.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char('i'));
    send(&mut e, "hi");
    e.handle(Key::Escape);
    e.handle(Key::Save);
    assert_eq!(
        e.take_effects(),
        vec![Effect::Save {
            path: "/sd/repo/notes.md".into(),
            scope: Scope::Tracked,
            contents: "hi\n".into(), // Normal-mode Cmd+S formats, so the terminator is there
        }]
    );
}

#[test]
fn cmd_s_on_a_clean_buffer_skips_the_write() {
    // The habitual repeat tap: nothing changed since the last save, so Cmd+S
    // must not queue a redundant SD write — it only re-confirms "saved".
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "done".into());
    e.handle(Key::Save);
    assert!(e.take_effects().is_empty(), "clean Cmd+S must queue no Save");
    assert_eq!(e.notice.as_deref(), Some("saved"));
    // And again — still free, still no write.
    e.handle(Key::Save);
    assert!(e.take_effects().is_empty());
}

#[test]
fn cmd_s_on_an_unnamed_clean_buffer_posts_no_file_name() {
    // A scratch buffer has nowhere to save to; the clean-path confirmation must
    // not falsely claim "saved".
    let mut e = Editor::new();
    e.handle(Key::Save);
    assert!(e.take_effects().is_empty());
    assert_eq!(e.notice.as_deref(), Some("no file name"));
}

#[test]
fn cmd_s_from_insert_saves_without_leaving_insert() {
    // Mid-typing Cmd+S is a quick checkpoint: it saves but neither types an 's'
    // nor drops out of Insert, so you keep typing where you were.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char('i'));
    send(&mut e, "draft");
    e.handle(Key::Save);
    assert_eq!(e.mode(), Mode::Insert, "Cmd+S must not leave Insert");
    assert_eq!(e.text(), "draft\n", "Cmd+S must not type an 's'");
    assert_eq!(e.caret, "draft".len(), "the caret stayed where the typing left it");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Save {
            path: "/sd/repo/notes.md".into(),
            scope: Scope::Tracked,
            contents: "draft\n".into(),
        }]
    );
}

#[test]
fn cmd_s_from_insert_does_not_reformat_mid_session() {
    // format_on_save is on by default, but a Cmd+S while still in Insert must
    // NOT reflow the line — stripping the trailing spaces the user is mid-way
    // through and yanking the caret to line start would be hostile. Only the
    // terminator lands. `:w` from Normal still formats (see
    // `gs_formats_the_buffer_before_pushing`).
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    assert!(e.prefs.format_on_save);
    e.handle(Key::Char('i'));
    send(&mut e, "hello   "); // trailing spaces a formatter would strip
    e.handle(Key::Save);
    assert_eq!(
        e.take_effects(),
        vec![Effect::Save {
            path: "/sd/repo/notes.md".into(),
            scope: Scope::Tracked,
            contents: "hello   \n".into(), // spaces kept — only the terminator added
        }]
    );
}

#[test]
fn cmd_s_mid_line_in_insert_leaves_the_caret_untouched() {
    // The caret need not sit at the end mid-Insert (arrow keys, Backspace): the
    // terminator is appended past every offset, so nothing under the caret moves.
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "one\ntwo".to_string(), // no terminator, as an externally authored file arrives
    );
    e.handle(Key::Char('i')); // entering Insert checkpoints, so the buffer is dirty
    e.caret = 1; // between 'o' and 'n' on row 0
    let before = e.caret;
    e.handle(Key::Save);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Save]);
    assert_eq!(e.text(), "one\ntwo\n");
    assert_eq!(e.caret, before, "the caret did not move");
    assert_eq!(e.mode(), Mode::Insert);
}

#[test]
fn fmt_stays_in_core_and_asks_the_host_for_nothing() {
    assert!(command("fmt").1.is_empty());
}

#[test]
fn fmt_keeps_the_caret_column_instead_of_snapping_to_the_line_start() {
    // Line 0 has trailing spaces, so format_markdown actually rewrites the buffer;
    // the caret sits mid-word on the untouched line below and must stay put rather
    // than jump to the start of its line.
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "first line   \nsecond line here".into(),
    );
    e.caret = "first line   \n".len() + 7; // after "second " on row 1
    e.handle(Key::Char(':'));
    for c in "fmt".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.text(), "first line\nsecond line here\n"); // spaces trimmed, buffer ended
    assert_eq!(e.caret, "first line\n".len() + 7); // same column, not the line start
    assert_eq!(e.mode(), Mode::Normal); // and formatting never touches the mode
}

#[test]
fn unknown_command_is_ignored() {
    let (e, effs) = command("q"); // quit is deliberately unimplemented
    assert!(effs.is_empty());
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn w_on_an_unnamed_buffer_posts_no_file_name() {
    // A scratch buffer (empty path) has nowhere to save to.
    let mut e = Editor::new();
    e.handle(Key::Char(':'));
    e.handle(Key::Char('w'));
    e.handle(Key::Enter);
    assert!(e.take_effects().is_empty());
}

#[test]
fn with_text_boots_normal_with_caret_on_last_char() {
    let e = Editor::with_text("resumed draft".to_string());
    assert_eq!(e.text(), "resumed draft");
    assert_eq!(e.caret, 12); // on the last char ('t'), the resume point
    assert_eq!(e.mode(), Mode::Normal); // vim-style: open a file in Normal
}

#[test]
fn with_text_empty_matches_new() {
    let e = Editor::with_text(String::new());
    assert_eq!(e.text(), "");
    assert_eq!(e.caret, 0);
    assert_eq!(e.mode(), Mode::Normal);
}

// ---- Command-line editing (Ctrl-W / Cmd-Backspace while typing `:`) ----

#[test]
fn ctrl_w_deletes_the_last_word_of_the_command_line() {
    let mut e = Editor::new();
    e.handle(Key::Char(':'));
    for c in "sync now".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::DeleteWord);
    assert_eq!(e.cmdline, "sync ");
    assert_eq!(e.mode(), Mode::Command); // stays on the command line
}

#[test]
fn ctrl_w_on_a_one_word_command_does_not_cancel() {
    let mut e = Editor::new();
    e.handle(Key::Char(':'));
    e.handle(Key::Char('w'));
    e.handle(Key::DeleteWord);
    assert_eq!(e.cmdline, "");
    assert_eq!(e.mode(), Mode::Command); // unlike Backspace, does not exit
}

#[test]
fn cmd_backspace_clears_the_command_line() {
    let mut e = Editor::new();
    e.handle(Key::Char(':'));
    for c in "fmt".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::DeleteLine);
    assert_eq!(e.cmdline, "");
    assert_eq!(e.mode(), Mode::Command);
}

// ---- `:pub` / `:publish` — rename `<name>.md` to `<name>.pub.md` ----

#[test]
fn publish_renames_the_active_md_file_to_pub_md() {
    // `:publish` renames the buffer in-core and queues the disk move (write the
    // new path, unlink the old) — the next `:gs` carries it to the remote.
    let (e, effs) = command("publish");
    assert_eq!(e.path(), "/sd/repo/notes.pub.md");
    assert_eq!(
        effs,
        vec![Effect::Rename {
            from: "/sd/repo/notes.md".into(),
            to: "/sd/repo/notes.pub.md".into(),
            contents: String::new(),
            retarget: vec![],
        }]
    );
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn pub_is_an_alias_for_publish() {
    let (e, effs) = command("pub");
    assert_eq!(e.path(), "/sd/repo/notes.pub.md");
    assert_eq!(kinds(&effs), vec![Kind::Rename]);
}

#[test]
fn publish_carries_the_buffer_contents_to_the_new_path() {
    // The in-RAM buffer (with unsaved edits) is the source of truth, not the
    // on-disk `.md` — so the rename write ships the current text.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "draft".into());
    ex(&mut e, "publish");
    assert!(matches!(
        e.take_effects().as_slice(),
        [Effect::Rename { to, contents, .. }]
            if to == "/sd/repo/notes.pub.md" && contents == "draft"
    ));
}

#[test]
fn publish_on_an_already_pub_file_is_a_noop_with_a_notice() {
    let mut e = Editor::with_file("/sd/repo/notes.pub.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "publish");
    assert!(e.take_effects().is_empty(), "already-.pub.md must queue nothing");
    assert_eq!(e.path(), "/sd/repo/notes.pub.md", "path unchanged");
    assert_eq!(e.notice.as_deref(), Some("already published"));
}

#[test]
fn publish_is_refused_in_a_local_buffer() {
    // `.pub.md` marks a file for the remote; a Local file never reaches one.
    let mut e = Editor::with_file("/sd/local/journal.md".into(), Scope::Local, String::new());
    ex(&mut e, "publish");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.path(), "/sd/local/journal.md", "path unchanged");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("Local"),
        "expected a Local-scope notice, got {:?}",
        e.notice,
    );
}

#[test]
fn publish_refuses_to_clobber_an_existing_pub_file() {
    // A sibling `notes.pub.md` already on the card must not be overwritten.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_file_list(vec!["/sd/repo/notes.md".into(), "/sd/repo/notes.pub.md".into()]);
    ex(&mut e, "publish");
    assert!(e.take_effects().is_empty(), "must not clobber the existing target");
    assert_eq!(e.path(), "/sd/repo/notes.md", "path unchanged");
    assert!(
        e.notice.as_deref().unwrap_or_default().contains("exists"),
        "expected a target-exists notice, got {:?}",
        e.notice,
    );
}

#[test]
fn publish_on_an_unnamed_scratch_warns() {
    let mut e = Editor::new(); // unnamed scratch — no path
    ex(&mut e, "publish");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.notice.as_deref(), Some("no file to publish"));
}

// ---- publish retargets links to the renamed file ----

#[test]
fn retarget_rewrites_every_spelling_gf_would_follow() {
    // Sibling, `./`, `<>`-wrapped, and `#fragment` forms all resolve to the
    // published file; each keeps its own spelling and just grows a `.pub`.
    let text = "[a](notes.md) [b](./notes.md)\n[c](<notes.md>) [d](notes.md#top)";
    let (out, sites) =
        publish_retarget_links("/sd/repo/essay.md", text, "/sd/repo/notes.md").unwrap();
    assert_eq!(
        out,
        "[a](notes.pub.md) [b](./notes.pub.md)\n[c](<notes.pub.md>) [d](notes.pub.md#top)"
    );
    assert_eq!(sites.len(), 4);
}

#[test]
fn retarget_reaches_across_scopes() {
    // A Local file's `../repo/…` link follows the published Tracked file.
    let (out, _) = publish_retarget_links(
        "/sd/local/journal.md",
        "see [n](../repo/notes.md)",
        "/sd/repo/notes.md",
    )
    .unwrap();
    assert_eq!(out, "see [n](../repo/notes.pub.md)");
}

#[test]
fn retarget_leaves_other_targets_alone() {
    // External links, other files, and a same-named file in the other scope
    // are not the published file; `None` = nothing to rewrite.
    let text = "[w](https://x.dev/notes.md) [o](other.md) [l](../local/notes.md)";
    assert_eq!(publish_retarget_links("/sd/repo/essay.md", text, "/sd/repo/notes.md"), None);
}

#[test]
fn publish_rewrites_self_links_into_the_rename_contents() {
    // A self-link (`notes.md` inside notes.md) must follow the rename — the
    // Rename effect snapshots the already-retargeted text.
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "[top](notes.md#top)".into(),
    );
    e.caret = e.text.len(); // past the link: the 4-byte splice shifts it
    ex(&mut e, "publish");
    assert_eq!(e.text, "[top](notes.pub.md#top)");
    assert_eq!(e.caret, e.text.len());
    assert!(matches!(
        e.take_effects().as_slice(),
        [Effect::Rename { contents, .. }] if contents == "[top](notes.pub.md#top)"
    ));
}

#[test]
fn publish_retargets_parked_buffers_in_core_and_persists_them() {
    // A resident buffer's RAM is its source of truth (its next save would
    // clobber a disk-side rewrite), so its links are rewritten in-core — as
    // their own undo group — and persisted immediately.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    edit(&mut e, "essay.md");
    e.install_loaded("/sd/repo/essay.md".into(), Scope::Tracked, "see [n](notes.md)".into());
    edit(&mut e, "notes.md"); // back; essay.md stays parked
    e.take_effects();
    ex(&mut e, "publish");
    let effs = e.take_effects();
    assert!(
        matches!(
            effs.as_slice(),
            [Effect::Save { path, contents, .. }, Effect::Rename { retarget, .. }]
                if path == "/sd/repo/essay.md"
                    && contents == "see [n](notes.pub.md)"
                    && retarget.is_empty() // resident: not the host's to rewrite
        ),
        "expected the parked rewrite's Save then the Rename, got {effs:?}"
    );
    // The rewrite is one undo group in that buffer.
    edit(&mut e, "essay.md");
    assert_eq!(e.text, "see [n](notes.pub.md)");
    e.handle(Key::Char('u'));
    assert_eq!(e.text, "see [n](notes.md)");
}

#[test]
fn publish_hands_the_host_the_non_resident_md_files() {
    // Everything the editor can't rewrite in RAM goes to the host: every card
    // `.md` except the published file itself — and never a non-markdown file.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_file_list(vec![
        "/sd/local/journal.md".into(),
        "/sd/repo/a.md".into(),
        "/sd/repo/img.png".into(),
        "/sd/repo/notes.md".into(),
    ]);
    ex(&mut e, "publish");
    assert!(matches!(
        e.take_effects().as_slice(),
        [Effect::Rename { retarget, .. }]
            if *retarget == vec!["/sd/local/journal.md".to_string(), "/sd/repo/a.md".to_string()]
    ));
}
