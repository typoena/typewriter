//! Multi-file buffers and `:enew` / `:delete`.

use super::*;

#[test]
fn wrap_text_packs_words_and_splits_overlong_tokens() {
    // Short message: one line.
    assert_eq!(wrap_text("saved", 15), vec!["saved"]);
    // Word-wraps on the space, keeping the actionable tail.
    assert_eq!(
        wrap_text("save FAILED - retry :w", 15),
        vec!["save FAILED -", "retry :w"]
    );
    // A token longer than the width is hard-split rather than truncated.
    assert_eq!(
        wrap_text("supercalifragilistic", 8),
        vec!["supercal", "ifragili", "stic"]
    );
    assert!(wrap_text("", 15).is_empty());
}

#[test]
fn resolve_path_maps_prefixes_and_bare_names() {
    assert_eq!(
        resolve_path("/sd/local/j.md", Scope::Tracked),
        ("/sd/local/j.md".to_string(), Scope::Local)
    );
    assert_eq!(
        resolve_path("/sd/repo/n.md", Scope::Local),
        ("/sd/repo/n.md".to_string(), Scope::Tracked)
    );
    // A leading `local/` or `repo/` segment selects scope (the palette label
    // form), independent of the current buffer's scope.
    assert_eq!(
        resolve_path("local/j.md", Scope::Tracked),
        ("/sd/local/j.md".to_string(), Scope::Local)
    );
    assert_eq!(
        resolve_path("repo/n.md", Scope::Local),
        ("/sd/repo/n.md".to_string(), Scope::Tracked)
    );
    // The `/sd` prefix is optional: `/repo/x` and `/local/x` (leading slash,
    // no `/sd`) resolve into the same scopes as their `/sd/…` spellings.
    assert_eq!(
        resolve_path("/repo/n.md", Scope::Local),
        ("/sd/repo/n.md".to_string(), Scope::Tracked)
    );
    assert_eq!(
        resolve_path("/local/j.md", Scope::Tracked),
        ("/sd/local/j.md".to_string(), Scope::Local)
    );
    // A bare name lands in the current buffer's scope directory.
    assert_eq!(
        resolve_path("draft.md", Scope::Local),
        ("/sd/local/draft.md".to_string(), Scope::Local)
    );
    assert_eq!(
        resolve_path("draft.md", Scope::Tracked),
        ("/sd/repo/draft.md".to_string(), Scope::Tracked)
    );
}

#[test]
fn an_edit_marks_dirty_and_mark_saved_clears_it() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "hi".into());
    assert!(!e.dirty());
    e.handle(Key::Char('x'));
    assert!(e.dirty());
    e.mark_saved("/sd/repo/a.md");
    assert!(!e.dirty());
}

#[test]
fn opening_a_nonresident_file_queues_a_load() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    edit(&mut e, "/sd/local/j.md");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load {
            path: "/sd/local/j.md".into(),
            scope: Scope::Local,
        }]
    );
    // The active buffer does not change until the host loads and installs it.
    assert_eq!(e.path(), "/sd/repo/a.md");
}

#[test]
fn install_loaded_parks_current_and_activates_the_target() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "hello B".into());
    assert_eq!(e.path(), "/sd/repo/b.md");
    assert_eq!(e.text(), "hello B");
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn switching_back_to_a_resident_buffer_needs_no_load() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "AAA".into());
    assert_eq!(e.caret, 2);
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "BBBBB".into());
    // A is parked (resident) — switching back reads memory, not disk.
    edit(&mut e, "/sd/repo/a.md");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert_eq!(e.text(), "AAA");
    assert_eq!(e.caret, 2);
}

#[test]
fn the_register_is_global_across_buffers() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "word".into());
    e.handle(Key::Char('y'));
    e.handle(Key::Char('y'));
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char('p'));
    assert!(e.text().contains("word"));
}

#[test]
fn a_dirty_parked_buffer_is_saved_when_evicted() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    // Dirty the active buffer, then push it out of the ≤3 resident window.
    e.handle(Key::Char('i'));
    e.handle(Key::Char('!'));
    e.handle(Key::Escape);
    assert!(e.dirty());
    e.take_effects();
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "B".into());
    e.install_loaded("/sd/repo/c.md".into(), Scope::Tracked, "C".into()); // parked: [A,B]
    assert!(e.take_effects().is_empty());
    e.install_loaded("/sd/repo/d.md".into(), Scope::Tracked, "D".into());
    let effs = e.take_effects();
    assert_eq!(effs.len(), 1, "the evicted dirty buffer must be saved");
    match &effs[0] {
        Effect::Save { path, .. } => assert_eq!(path, "/sd/repo/a.md"),
        other => panic!("expected a Save of A, got {other:?}"),
    }
}

#[test]
fn reboot_autosaves_every_dirty_resident_buffer() {
    // Dirty the active buffer and a parked one, then `:reboot` saves both (active
    // first, then parked) ahead of the restart — the fan-out loses nothing.
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    e.handle(Key::Char('i'));
    e.handle(Key::Char('!'));
    e.handle(Key::Escape);
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "B".into());
    e.handle(Key::Char('i'));
    e.handle(Key::Char('!'));
    e.handle(Key::Escape);
    e.take_effects();
    ex(&mut e, "reboot");
    confirm(&mut e);
    let effs = e.take_effects();
    assert_eq!(kinds(&effs), vec![Kind::Save, Kind::Save, Kind::Reboot]);
    let saved: Vec<&str> = effs
        .iter()
        .filter_map(|ef| match ef {
            Effect::Save { path, .. } => Some(path.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(saved, vec!["/sd/repo/b.md", "/sd/repo/a.md"]);
}

#[test]
fn a_clean_parked_buffer_is_dropped_silently_on_eviction() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    // A is never edited (clean); filling past ≤3 must evict it without a Save.
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "B".into());
    e.install_loaded("/sd/repo/c.md".into(), Scope::Tracked, "C".into());
    e.take_effects();
    e.install_loaded("/sd/repo/d.md".into(), Scope::Tracked, "D".into());
    assert!(e.take_effects().is_empty());
}

/// Boot on `a.md`, then open the rest via the palette (installing each load),
/// so `recent` is seeded most-recent-last-argument-first and all files are
/// resident up to the ≤3 window.
fn opened(paths: &[&str]) -> Editor {
    let mut e = Editor::with_file(
        paths.first().expect("at least one file").to_string(),
        Scope::Tracked,
        "A".into(),
    );
    for p in paths.get(1..).unwrap_or_default() {
        edit(&mut e, p);
        e.take_effects();
        e.install_loaded(p.to_string(), Scope::Tracked, String::new());
    }
    e
}

#[test]
fn ctrl_tab_toggles_between_the_last_two_notes() {
    let mut e = opened(&["/sd/repo/a.md", "/sd/repo/b.md"]);
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert!(e.take_effects().is_empty());
    e.handle(Key::Char('j'));
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/b.md");
    e.handle(Key::Char('j'));
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/a.md");
}

#[test]
fn ctrl_release_commits_so_ctrl_tab_toggles_without_typing() {
    // The real gesture: Ctrl+Tab, release Ctrl (the decoder's CycleCommit),
    // Ctrl+Tab again — must bounce between the two last notes, not walk
    // deeper (c → b → a was the on-device bug: nothing committed the walk
    // when no other key was typed between presses).
    let mut e = opened(&["/sd/repo/a.md", "/sd/repo/b.md", "/sd/repo/c.md"]);
    e.handle(Key::CycleRecent);
    e.handle(Key::CycleCommit);
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/c.md");
    e.handle(Key::CycleCommit);
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/b.md");
}

#[test]
fn repeated_ctrl_tab_walks_deeper_into_the_mru_and_wraps() {
    // recent: [c, b, a], c active. Without a commit in between, each press
    // must reach an *older* note, not bounce between the top two.
    let mut e = opened(&["/sd/repo/a.md", "/sd/repo/b.md", "/sd/repo/c.md"]);
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/b.md");
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/a.md");
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/c.md");
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/b.md");
}

#[test]
fn committing_the_walk_floats_the_landed_note() {
    let mut e = opened(&["/sd/repo/a.md", "/sd/repo/b.md", "/sd/repo/c.md"]);
    e.handle(Key::CycleRecent);
    e.handle(Key::Char('j')); // commit: recent now [b, c, a]
    // The next walk's first hop is the note we came *from*, not a.
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/c.md");
}

#[test]
fn ctrl_tab_reaches_a_note_evicted_from_residency() {
    // Four opens: a fell out of the ≤3 resident window, but recency outlives
    // residency — the walk reaches it via a Load like any non-resident open.
    let mut e =
        opened(&["/sd/repo/a.md", "/sd/repo/b.md", "/sd/repo/c.md", "/sd/repo/d.md"]);
    e.take_effects();
    e.handle(Key::CycleRecent);
    e.handle(Key::CycleRecent);
    e.handle(Key::CycleRecent);
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/a.md".into(), scope: Scope::Tracked }]
    );
    assert_eq!(e.path(), "/sd/repo/b.md");
}

#[test]
fn ctrl_tab_with_nowhere_to_go_posts_a_notice() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    e.handle(Key::CycleRecent); // recent = [a] and a is on screen
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.notice.as_deref(), Some("no other note"));
}

#[test]
fn ctrl_tab_works_from_insert_and_lands_in_normal() {
    let mut e = opened(&["/sd/repo/a.md", "/sd/repo/b.md"]);
    e.handle(Key::Char('i'));
    assert_eq!(e.mode(), Mode::Insert);
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn ctrl_tab_derives_scope_for_local_notes() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    edit(&mut e, "local/j.md");
    e.take_effects();
    e.install_loaded("/sd/local/j.md".into(), Scope::Local, "J".into());
    e.handle(Key::CycleRecent);
    e.handle(Key::Char('j'));
    e.handle(Key::CycleRecent);
    assert_eq!(e.path(), "/sd/local/j.md");
    assert_eq!(e.scope(), Scope::Local);
    assert!(e.take_effects().is_empty());
}

#[test]
fn enew_creates_a_dirty_empty_buffer_and_asks_the_host_for_nothing() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "A".into());
    ex(&mut e, "enew draft.md");
    assert_eq!(e.path(), "/sd/repo/draft.md");
    assert_eq!(e.scope(), Scope::Tracked);
    assert_eq!(e.text(), "");
    assert!(e.dirty());
    assert_eq!(e.mode(), Mode::Normal);
    // `:enew` allocates no card IO — it neither loads nor saves.
    assert!(e.take_effects().is_empty());
}

#[test]
fn enew_derives_local_scope_from_the_path() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "A".into());
    ex(&mut e, "enew local/journal.md");
    assert_eq!(e.path(), "/sd/local/journal.md");
    assert_eq!(e.scope(), Scope::Local);
}

#[test]
fn enew_adds_the_new_file_to_the_palette_list() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/todo.md"]);
    ex(&mut e, "enew draft.md");
    assert!(files_vec(&e).contains(&"/sd/repo/draft.md".to_string()));
    // and it is findable in the palette without a disk re-enumeration
    e.handle(Key::Palette);
    for c in "draft".chars() {
        e.handle(Key::Char(c));
    }
    assert_eq!(palette_labels(&e), vec!["repo/draft.md"]);
}

#[test]
fn enew_of_an_already_open_file_switches_without_clobbering() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "AAA".into());
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "BBB".into());
    e.take_effects();
    ex(&mut e, "enew /sd/repo/a.md");
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert_eq!(e.text(), "AAA");
    assert!(e.take_effects().is_empty()); // resident: no Load
}

#[test]
fn enew_without_a_name_is_a_usage_noop() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "A".into());
    ex(&mut e, "enew");
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn delete_prompts_before_touching_anything() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "delete");
    // The prompt is up; nothing has happened yet — no effect, file still active.
    assert_eq!(e.mode(), Mode::Confirm);
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert!(e.take_effects().is_empty());
}

#[test]
fn confirming_the_prompt_queues_the_delete() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "delete");
    e.handle(Key::Char('y'));
    assert_eq!(
        e.take_effects(),
        vec![Effect::Delete {
            path: "/sd/repo/notes.md".into(),
            scope: Scope::Tracked,
        }]
    );
    // No file remains active (nothing else was resident): a scratch buffer.
    assert_eq!(e.path(), "");
    assert_eq!(e.text(), "");
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn cancelling_the_prompt_leaves_the_file_untouched() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/todo.md"]);
    ex(&mut e, "delete");
    e.handle(Key::Char('n'));
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert!(files_vec(&e).contains(&"/sd/repo/notes.md".to_string()));
    assert!(e.take_effects().is_empty());
}

#[test]
fn esc_at_the_prompt_cancels_too() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "delete");
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert!(e.take_effects().is_empty());
}

#[test]
fn d_is_an_alias_for_delete() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "d");
    assert_eq!(e.mode(), Mode::Confirm);
    e.handle(Key::Char('y'));
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Delete]);
}

#[test]
fn delete_never_saves_the_discarded_buffer_even_when_dirty() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "A".into());
    e.handle(Key::Char('x'));
    assert!(e.dirty());
    ex(&mut e, "delete");
    e.handle(Key::Char('y'));
    // The buffer is being deleted, so it is discarded, not saved: Delete only.
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Delete]);
}

#[test]
fn delete_switches_to_the_most_recently_parked_buffer() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "AAA".into());
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "BBB".into());
    e.take_effects();
    ex(&mut e, "delete");
    e.handle(Key::Char('y'));
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert_eq!(e.text(), "AAA");
    match &e.take_effects()[..] {
        [Effect::Delete { path, .. }] => assert_eq!(path, "/sd/repo/b.md"),
        other => panic!("expected a single Delete of B, got {other:?}"),
    }
}

#[test]
fn delete_drops_the_file_from_the_palette_list() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/todo.md"]);
    ex(&mut e, "delete");
    e.handle(Key::Char('y'));
    e.take_effects();
    assert!(!files_vec(&e).contains(&"/sd/repo/notes.md".to_string()));
    e.handle(Key::Palette);
    for c in "md".chars() {
        e.handle(Key::Char(c));
    }
    assert_eq!(palette_labels(&e), vec!["repo/todo.md"]);
}

#[test]
fn delete_of_a_local_file_carries_local_scope() {
    let mut e = Editor::with_file("/sd/local/j.md".into(), Scope::Local, "diary".into());
    ex(&mut e, "delete");
    e.handle(Key::Char('y'));
    match &e.take_effects()[..] {
        [Effect::Delete { path, scope }] => {
            assert_eq!(path, "/sd/local/j.md");
            assert_eq!(*scope, Scope::Local);
        }
        other => panic!("expected a Local Delete, got {other:?}"),
    }
}

#[test]
fn delete_on_an_unnamed_buffer_is_a_noop() {
    let mut e = Editor::new();
    ex(&mut e, "delete");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn link_target_at_finds_the_span_containing_col() {
    let line = "see [intro](intro.md) and [b](../b.md) done";
    // Anywhere from `[` through `)` inclusive selects the first link…
    assert_eq!(link_target_at(line, 4), Some("intro.md"));
    assert_eq!(link_target_at(line, 7), Some("intro.md"));
    assert_eq!(link_target_at(line, 15), Some("intro.md"));
    assert_eq!(link_target_at(line, 20), Some("intro.md"));
    // …and the second link resolves independently.
    assert_eq!(link_target_at(line, 30), Some("../b.md"));
    // Between and around links there is nothing to follow.
    assert_eq!(link_target_at(line, 0), None);
    assert_eq!(link_target_at(line, 23), None);
    assert_eq!(link_target_at(line, 41), None);
    // Brackets without a `(…)` tail are not a link.
    assert_eq!(link_target_at("plain [brackets] here", 8), None);
}

#[test]
fn resolve_link_target_joins_and_normalizes() {
    let f = "/sd/repo/lectures/meadows/a.md";
    // Sibling and parent-relative forms — the shapes `relative_link_path` writes.
    assert_eq!(
        resolve_link_target(f, Scope::Tracked, "b.md"),
        Some(("/sd/repo/lectures/meadows/b.md".to_string(), Scope::Tracked))
    );
    assert_eq!(
        resolve_link_target(f, Scope::Tracked, "../intro.md"),
        Some(("/sd/repo/lectures/intro.md".to_string(), Scope::Tracked))
    );
    // A cross-scope link climbs into the other scope, which its path selects.
    assert_eq!(
        resolve_link_target("/sd/repo/notes.md", Scope::Tracked, "../local/journal.md"),
        Some(("/sd/local/journal.md".to_string(), Scope::Local))
    );
    // The palette-label form works from anywhere, including an unnamed scratch.
    assert_eq!(
        resolve_link_target(f, Scope::Tracked, "local/journal.md"),
        Some(("/sd/local/journal.md".to_string(), Scope::Local))
    );
    assert_eq!(
        resolve_link_target("", Scope::Tracked, "repo/notes.md"),
        Some(("/sd/repo/notes.md".to_string(), Scope::Tracked))
    );
    // Climbing off the card, or landing outside both scopes, is unresolvable.
    assert_eq!(resolve_link_target(f, Scope::Tracked, "../../../../../x.md"), None);
    assert_eq!(resolve_link_target("/sd/repo/n.md", Scope::Tracked, "../conf.toml"), None);
}

#[test]
fn gf_follows_a_relative_link_and_queues_the_load() {
    let mut e = Editor::with_file(
        "/sd/repo/lectures/a.md".into(),
        Scope::Tracked,
        "see [intro](../intro.md) for more".into(),
    );
    e.caret = 6;
    send(&mut e, "gf");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/intro.md".into(), scope: Scope::Tracked }]
    );
}

#[test]
fn gf_follows_a_cross_scope_link_with_the_target_scope() {
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "[journal](../local/journal.md)".into(),
    );
    e.caret = 3;
    send(&mut e, "gf");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/local/journal.md".into(), scope: Scope::Local }]
    );
}

#[test]
fn gf_unwraps_angle_brackets_and_drops_the_fragment() {
    let mut e = Editor::with_file(
        "/sd/repo/a.md".into(),
        Scope::Tracked,
        "[spaced](<my notes.md>) [sec](b.md#heading)".into(),
    );
    e.caret = 2;
    send(&mut e, "gf");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/my notes.md".into(), scope: Scope::Tracked }]
    );
    e.caret = 26;
    send(&mut e, "gf");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/b.md".into(), scope: Scope::Tracked }]
    );
}

#[test]
fn gf_switches_to_a_resident_target_without_a_load() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "go [b](b.md)".into());
    edit(&mut e, "b.md");
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "bee".into());
    edit(&mut e, "a.md");
    e.take_effects();
    e.caret = 4;
    send(&mut e, "gf");
    assert!(e.take_effects().is_empty(), "resident switch needs no Load");
    assert_eq!(e.path, "/sd/repo/b.md");
    assert_eq!(e.text, "bee");
}

#[test]
fn gf_posts_notices_for_no_link_external_and_unresolvable() {
    let mut e = Editor::with_file(
        "/sd/repo/a.md".into(),
        Scope::Tracked,
        "plain text [web](https://x.dev) [out](../../x.md)".into(),
    );
    e.caret = 2;
    send(&mut e, "gf");
    assert_eq!(e.notice.as_deref(), Some("no link under caret"));
    e.caret = 12;
    send(&mut e, "gf");
    assert_eq!(e.notice.as_deref(), Some("external link"));
    e.caret = 33;
    send(&mut e, "gf");
    assert_eq!(e.notice.as_deref(), Some("can't follow link"));
    assert!(e.take_effects().is_empty());
    assert_eq!(e.path, "/sd/repo/a.md");
}
