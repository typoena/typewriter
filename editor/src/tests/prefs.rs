//! Preferences (`.typoena.toml`), the live gutter toggle, and the
//! `hidden_folders` visibility filter.

use super::*;

#[test]
fn prefs_default_matches_the_documented_defaults() {
    let p = Prefs::default();
    assert!(p.save_on_idle);
    assert!(p.format_on_save);
    assert!(p.line_numbers);
    assert!(p.open_last_on_boot);
    assert_eq!(p.theme, "light");
    assert_eq!(p.auto_sync, "10m");
    assert_eq!(p.scroll_margin, 2);
    assert_eq!(p.timezone, "");
}

#[test]
fn prefs_parse_reads_timezone_posix_string() {
    let p = Prefs::parse("timezone = \"CET-1CEST,M3.5.0,M10.5.0/3\"\n");
    assert_eq!(p.timezone, "CET-1CEST,M3.5.0,M10.5.0/3");
    // Missing key -> empty (UTC), never a bogus zone.
    assert_eq!(Prefs::parse("").timezone, "");
}

#[test]
fn prefs_parse_falls_back_to_defaults_for_missing_keys() {
    // Only one key present; the rest stay at their defaults.
    let p = Prefs::parse("line_numbers = false\n");
    assert!(!p.line_numbers);
    assert!(p.save_on_idle);
    assert!(p.format_on_save);
    assert_eq!(p.auto_sync, "10m");
}

#[test]
fn prefs_parse_reads_all_keys_and_ignores_comments_and_junk() {
    let src = "\
        # a header comment\n\
        save_on_idle = false   # trailing comment\n\
        format_on_save = false\n\
        line_numbers = false\n\
        open_last_on_boot = false\n\
        auto_sync = \"2m\"\n\
        bogus_key = whatever\n\
        not a pair\n";
    let p = Prefs::parse(src);
    assert!(!p.save_on_idle);
    assert!(!p.format_on_save);
    assert!(!p.line_numbers);
    assert!(!p.open_last_on_boot);
    assert_eq!(p.auto_sync, "2m");
}

#[test]
fn prefs_parse_keeps_default_on_an_unparseable_bool() {
    // A typo in a bool value leaves that key at its default, not `false`.
    let p = Prefs::parse("save_on_idle = yes\n");
    assert!(p.save_on_idle);
}

#[test]
fn prefs_to_toml_round_trips_through_parse() {
    let p = Prefs {
        save_on_idle: false,
        format_on_save: true,
        line_numbers: false,
        open_last_on_boot: false,
        theme: "dark".into(),
        font: "jetbrains-mono".into(),
        auto_sync: "5m".into(),
        scroll_margin: 3,
        fast_partial: true,
        companion: false,
        face: "curious".into(),
        timezone: "CET-1CEST,M3.5.0,M10.5.0/3".into(),
        hidden_folders: "_archive,attachments".into(),
    };
    assert_eq!(Prefs::parse(&p.to_toml()), p);
}

#[test]
fn prefs_parse_reads_companion_and_defaults_on() {
    assert!(Prefs::default().companion);
    assert!(!Prefs::parse("companion = false\n").companion);
    assert!(Prefs::parse("companion = true\n").companion);
    // A non-bool value leaves it at the (on) default rather than reading false.
    assert!(Prefs::parse("companion = off\n").companion);
}

#[test]
fn prefs_parse_reads_fast_partial_and_defaults_off() {
    assert!(!Prefs::default().fast_partial);
    assert!(Prefs::parse("fast_partial = true\n").fast_partial);
    assert!(!Prefs::parse("fast_partial = false\n").fast_partial);
    // A non-bool value leaves it at the (off) default rather than reading false.
    assert!(!Prefs::parse("fast_partial = sometimes\n").fast_partial);
}

#[test]
fn prefs_parse_reads_scroll_margin_and_keeps_default_on_junk() {
    assert_eq!(Prefs::parse("scroll_margin = 0\n").scroll_margin, 0);
    assert_eq!(Prefs::parse("scroll_margin = 4\n").scroll_margin, 4);
    // A non-numeric value leaves the key at its default rather than 0.
    assert_eq!(Prefs::parse("scroll_margin = lots\n").scroll_margin, 2);
}

#[test]
fn prefs_parse_reads_theme_and_auto_sync_strings() {
    let p = Prefs::parse("theme = \"dark\"\nauto_sync = \"15m\"\n");
    assert_eq!(p.theme, "dark");
    assert_eq!(p.auto_sync, "15m");
}

#[test]
fn prefs_parse_reads_font_and_defaults_to_builtin() {
    assert_eq!(Prefs::default().font, "default");
    assert_eq!(Prefs::parse("font = \"jetbrains-mono\"\n").font, "jetbrains-mono");
    // The font key is emitted by to_toml, so it survives a `:gs` to other devices.
    assert!(Prefs::default().to_toml().contains("font = \"default\""));
}

#[test]
fn prefs_parse_reads_face_and_defaults_to_random() {
    assert_eq!(Prefs::default().face, "random");
    assert_eq!(Prefs::parse("face = \"curious\"\n").face, "curious");
    // Emitted by to_toml, so a pinned face rides `:gs` to other devices.
    assert!(Prefs::default().to_toml().contains("face = \"random\""));
}

#[test]
fn empty_prefs_file_yields_defaults() {
    assert_eq!(Prefs::parse(""), Prefs::default());
}

#[test]
fn line_numbers_off_reclaims_the_gutter_columns() {
    let mut e = Editor::with_text("one\ntwo\nthree".into());
    assert!(e.text_cols() < WRITE_COLS);
    e.prefs.line_numbers = false;
    assert_eq!(e.gutter_cols(), 0);
    assert_eq!(e.text_cols(), WRITE_COLS);
}

#[test]
fn draw_with_line_numbers_off_does_not_panic() {
    // The `gutter - 1` field width would underflow if unguarded.
    let mut e = Editor::with_text("alpha\nbeta\ngamma".into());
    e.prefs.line_numbers = false;
    let _ = e.draw(true);
}

/// A palette editor over `files` with `_archive` hidden.
fn hiding_archive(files: &[&str]) -> Editor {
    let mut e = palette_editor(files);
    e.prefs.hidden_folders = "_archive".into();
    e
}

#[test]
fn prefs_parse_reads_hidden_folders_and_defaults_to_none() {
    assert_eq!(Prefs::default().hidden_folders, "");
    let p = Prefs::parse("hidden_folders = \"_archive, attachments\"\n");
    assert_eq!(p.hidden_folders, "_archive, attachments");
    // Emitted by to_toml, so the list rides `:gs` to every device.
    assert!(Prefs::default().to_toml().contains("hidden_folders = \"\""));
}

#[test]
fn a_hidden_folder_entry_matches_whole_segments_at_any_depth() {
    let p = Prefs { hidden_folders: "_archive".into(), ..Prefs::default() };
    assert!(p.hides_file("/sd/repo/_archive/old.md"));
    // Any depth, and in both scopes.
    assert!(p.hides_file("/sd/repo/notes/_archive/old.md"));
    assert!(p.hides_file("/sd/local/_archive/old.md"));
    // FAT names are case-insensitive, so the match is too.
    assert!(p.hides_file("/sd/repo/_Archive/old.md"));
    // Near misses: a longer folder name, a partial segment, a *file* of that
    // name (the entry names folders), and an unrelated note.
    assert!(!p.hides_file("/sd/repo/_archives/old.md"));
    assert!(!p.hides_file("/sd/repo/my_archive/old.md"));
    assert!(!p.hides_file("/sd/repo/_archive.md"));
    assert!(!p.hides_file("/sd/repo/notes/old.md"));
}

#[test]
fn a_multi_segment_hidden_entry_needs_consecutive_segments() {
    let p = Prefs { hidden_folders: "notes/_archive".into(), ..Prefs::default() };
    assert!(p.hides_file("/sd/repo/notes/_archive/old.md"));
    assert!(!p.hides_file("/sd/repo/_archive/old.md"));
    assert!(!p.hides_file("/sd/repo/notes/drafts/_archive/old.md"));
}

#[test]
fn empty_hidden_folder_entries_hide_nothing() {
    // A stray or trailing comma must not read as "hide everything".
    let p = Prefs { hidden_folders: " , ".into(), ..Prefs::default() };
    assert!(!p.hides_file("/sd/repo/notes.md"));
    assert!(!p.hides_file("/sd/repo/_archive/old.md"));
}

#[test]
fn a_hidden_folders_files_stay_off_the_palette_list() {
    let mut e = hiding_archive(&["/sd/repo/notes.md", "/sd/repo/_archive/old.md"]);
    e.handle(Key::Palette);
    assert_eq!(palette_labels(&e), vec!["repo/notes.md"]);
    // A query that only matches the hidden note leaves the list empty…
    send(&mut e, "old");
    assert!(palette_labels(&e).is_empty());
    // …while the card still knows the file: hiding is a view filter, not a
    // deletion, and the note keeps syncing.
    assert!(e.file_list_contains("/sd/repo/_archive/old.md"));
}

#[test]
fn naming_a_hidden_folder_in_the_palette_lists_it_again() {
    let mut e = hiding_archive(&["/sd/repo/notes.md", "/sd/repo/_archive/old.md"]);
    e.handle(Key::Palette);
    send(&mut e, "_archive");
    assert_eq!(palette_labels(&e), vec!["repo/_archive/old.md"]);
}

#[test]
fn a_wildcard_entry_is_named_by_the_folder_it_hides() {
    // The escape hatch has to survive the entry that hides a whole family: under
    // `_*` the writer never types the entry itself, only the folder's real name.
    let p = Prefs { hidden_folders: "_*,!_inbox".into(), ..Prefs::default() };
    assert!(p.query_reveals_hidden("_archive"));
    assert!(p.query_reveals_hidden("repo/_drafts/wip"));
    assert!(p.query_reveals_hidden("_"), "revealed as the prefix is typed");
    // Whole segments only: an underscore mid-word is not the name of a folder,
    // and treating it as one would unhide the entire family on any such query.
    assert!(!p.query_reveals_hidden("my_note"));
    assert!(!p.query_reveals_hidden("notes"));
    // An exception entry reveals nothing on its own — with only `!_inbox` there
    // is no filter to lift, since nothing is hidden.
    let kept = Prefs { hidden_folders: "!_inbox".into(), ..Prefs::default() };
    assert!(!kept.query_reveals_hidden("_inbox"));
}

#[test]
fn naming_a_wildcard_hidden_folder_lists_it_again_end_to_end() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/_archive/old.md"]);
    e.prefs.hidden_folders = "_*".into();
    e.handle(Key::Palette);
    assert_eq!(palette_labels(&e), vec!["repo/notes.md"]);
    send(&mut e, "_archive");
    assert_eq!(palette_labels(&e), vec!["repo/_archive/old.md"]);
}

#[test]
fn hidden_folders_are_not_offered_as_new_file_folders() {
    let e = hiding_archive(&["/sd/repo/_archive/old.md", "/sd/repo/notes/a.md"]);
    assert_eq!(e.folder_completions("repo/"), vec!["repo/notes/".to_string()]);
    // Typing the folder's own name is an explicit request, so it completes.
    assert_eq!(e.folder_completions("repo/_archive"), vec!["repo/_archive/".to_string()]);
}

#[test]
fn oldest_skips_a_hidden_inbox() {
    let mut e = palette_editor(&["/sd/repo/_inbox/2026-01-01.md"]);
    e.prefs.hidden_folders = "_inbox".into();
    ex(&mut e, "oldest");
    assert_eq!(e.notice.as_deref(), Some("inbox empty"));
    assert!(e.take_effects().is_empty());
}

#[test]
fn a_hidden_note_still_opens_when_named_exactly() {
    // `> new file` on an existing hidden name switches to it rather than
    // clobbering it with an empty buffer — the exact-path lookup deliberately
    // ignores the filter.
    let mut e = hiding_archive(&["/sd/repo/_archive/old.md"]);
    e.handle(Key::Palette);
    send(&mut e, ">new");
    e.handle(Key::Enter);
    e.handle(Key::DeleteLine);
    send(&mut e, "repo/_archive/old");
    e.handle(Key::Enter);
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/_archive/old.md".into(), scope: Scope::Tracked }]
    );
}

#[test]
fn inbox_opens_todays_hidden_note_instead_of_clobbering_it() {
    // The whole point of the walk indexing hidden folders: `:inbox`'s
    // "already on the card — switch to it" guard reads the file list, so a
    // pruned list would send it down the create branch and the idle save would
    // write a fresh stub over a note the writer filled this morning.
    const TODAY: Date = Date { year: 2026, month: 7, day: 18 };
    const NOTE: &str = "/sd/repo/_inbox/2026-07-18.md";
    let mut e = palette_editor(&[NOTE]);
    e.prefs.hidden_folders = "_inbox".into();
    e.set_today(Some(TODAY));
    ex(&mut e, "inbox");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: NOTE.into(), scope: Scope::Tracked }],
        "the existing note is loaded, not replaced"
    );
    assert!(!e.dirty(), "nothing was seeded over it");
}

#[test]
fn a_hidden_folders_file_stays_in_the_index_pub_walks() {
    // `:pub` builds its retarget list, and its destination-exists guard, from
    // the file list. A hidden file missing from it means a link that dangles
    // once the target grows its `.pub` tail — and a `.pub.md` overwritten
    // because the guard could not see it.
    let e = hiding_archive(&["/sd/repo/notes.md", "/sd/repo/_archive/old.md"]);
    assert!(
        (0..e.file_count()).any(|i| e.file_at(i) == "/sd/repo/_archive/old.md"),
        "a hidden file stays in the index :pub walks"
    );
}

#[test]
fn a_trailing_star_hides_a_whole_family_of_folders() {
    let p = Prefs { hidden_folders: "_*".into(), ..Prefs::default() };
    assert!(p.hides_file("/sd/repo/_archive/old.md"));
    assert!(p.hides_file("/sd/repo/_drafts/wip.md"));
    assert!(p.hides_file("/sd/repo/notes/_todo/x.md"), "at any depth, like a bare entry");
    assert!(!p.hides_file("/sd/repo/notes.md"), "an unprefixed folder is untouched");
    assert!(!p.hides_file("/sd/repo/archive/old.md"), "the prefix has to be there");
    assert!(!p.hides_file("/sd/repo/my_todo/x.md"), "and has to start the segment");
}

#[test]
fn a_bang_entry_keeps_one_folder_out_of_a_wildcard() {
    // The whole point of the pair: hide the underscore folders, keep the inbox.
    let p = Prefs { hidden_folders: "_*,!_inbox".into(), ..Prefs::default() };
    assert!(p.hides_file("/sd/repo/_archive/old.md"));
    assert!(p.hides_file("/sd/repo/_drafts/wip.md"));
    assert!(!p.hides_file("/sd/repo/_inbox/2026-09-06.md"), "the exception wins");
    // Order must not matter, so the writer never has to reason about it.
    let flipped = Prefs { hidden_folders: "!_inbox,_*".into(), ..Prefs::default() };
    assert!(!flipped.hides_file("/sd/repo/_inbox/2026-09-06.md"));
    assert!(flipped.hides_file("/sd/repo/_archive/old.md"));
}

#[test]
fn a_starred_entry_still_matches_whole_segments_in_a_run() {
    let p = Prefs { hidden_folders: "notes/_*".into(), ..Prefs::default() };
    assert!(p.hides_file("/sd/repo/notes/_todo/x.md"));
    assert!(!p.hides_file("/sd/repo/_todo/x.md"), "the run has to start at `notes`");
}

#[test]
fn a_scope_root_segment_is_not_a_hidden_folder_entry() {
    // Entries match below the scope root, so the `sd`, `repo` and `local`
    // segments every card path carries cannot be turned into a blanket hide.
    for entry in ["repo", "sd", "local"] {
        let p = Prefs { hidden_folders: entry.into(), ..Prefs::default() };
        assert!(!p.hides_file("/sd/repo/notes.md"), "`{entry}` must not hide the whole card");
        assert!(!p.hides_file("/sd/local/idea.md"), "`{entry}` must not hide local either");
    }
    // The same word one level down is a normal entry and still hides.
    let p = Prefs { hidden_folders: "repo".into(), ..Prefs::default() };
    assert!(p.hides_file("/sd/repo/repo/x.md"));
}

#[test]
fn gf_follows_a_link_into_a_hidden_folder() {
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "see [old](_archive/old.md)".into(),
    );
    e.prefs.hidden_folders = "_archive".into();
    e.caret = 6;
    send(&mut e, "gf");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/_archive/old.md".into(), scope: Scope::Tracked }]
    );
}

#[test]
fn publish_still_retargets_links_inside_a_hidden_folder() {
    // Hiding a folder must not leave its links pointing at a renamed file.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.prefs.hidden_folders = "_archive".into();
    e.set_file_list(vec!["/sd/repo/_archive/old.md".into(), "/sd/repo/notes.md".into()]);
    ex(&mut e, "publish");
    assert!(matches!(
        e.take_effects().as_slice(),
        [Effect::Rename { retarget, .. }]
            if *retarget == vec!["/sd/repo/_archive/old.md".to_string()]
    ));
}
