//! Fleeting-note commands: `:inbox`/`:in` (open/create today's note) and
//! `:oldest`/`:old` (open the oldest note for cleanup), plus the `>` palette
//! entries that spell them out.

use super::*;

/// A fixed "today" so the dated filename/title are deterministic.
const TODAY: Date = Date { year: 2026, month: 7, day: 18 };
/// The note `:inbox` names for [`TODAY`].
const INBOX_TODAY: &str = "/sd/repo/_inbox/2026-07-18.md";

#[test]
fn inbox_creates_todays_note_prefilled_dirty_and_in_normal() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_today(Some(TODAY));
    ex(&mut e, "inbox");
    assert_eq!(e.path(), INBOX_TODAY);
    assert_eq!(e.scope(), Scope::Tracked);
    assert_eq!(e.text(), "# 18/07/2026\n\n");
    assert!(e.dirty(), "a fresh note must be dirty so eviction/:w persists it");
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.file_list_contains(INBOX_TODAY), "must be findable in the palette at once");
    assert!(e.take_effects().is_empty(), "creation is in-RAM; no host IO until saved");
}

#[test]
fn inbox_alias_in_creates_the_same_note() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_today(Some(TODAY));
    ex(&mut e, "in");
    assert_eq!(e.path(), INBOX_TODAY);
    assert_eq!(e.text(), "# 18/07/2026\n\n");
}

#[test]
fn inbox_opens_an_existing_note_from_disk_without_clobbering() {
    // Today's note is already on the card (in the palette file list) but not
    // resident: `:inbox` must Load it, not replace it with an empty buffer.
    let mut e = palette_editor(&[INBOX_TODAY]);
    e.set_today(Some(TODAY));
    ex(&mut e, "inbox");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: INBOX_TODAY.into(), scope: Scope::Tracked }],
    );
    assert_eq!(e.path(), "/sd/repo/notes.md", "active unchanged until the Load lands");
    assert!(!e.dirty(), "must not have created/dirtied a new empty buffer");
}

#[test]
fn inbox_reopening_the_active_note_keeps_its_edits() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_today(Some(TODAY));
    ex(&mut e, "inbox");
    e.handle(Key::Char('i'));
    for c in "hello".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Escape);
    assert!(e.text().contains("hello"));
    ex(&mut e, "inbox");
    assert_eq!(e.path(), INBOX_TODAY);
    assert!(e.text().contains("hello"), "reopening today's note must keep its content");
    assert!(e.take_effects().is_empty(), "re-opening the active buffer is a no-op");
}

#[test]
fn inbox_without_a_clock_asks_for_one_and_holds() {
    // `today` defaults to None (nothing has set the clock this power cycle).
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "inbox");
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SyncClock], "must ask for the clock, not IO");
    assert!(e.inbox_pending(), "the request is held, not dropped");
    assert_eq!(e.path(), "/sd/repo/notes.md", "must not create or switch buffers yet");
    assert!(!e.dirty());
}

#[test]
fn a_held_inbox_opens_itself_when_the_date_lands() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "inbox");
    let _ = e.take_effects();

    e.set_today(Some(TODAY));

    assert!(!e.inbox_pending(), "one-shot: the hold is spent");
    assert_eq!(e.path(), INBOX_TODAY);
    assert_eq!(e.text(), "# 18/07/2026\n\n");
    assert!(e.dirty(), "a fresh note must be dirty so eviction/:w persists it");
}

#[test]
fn a_held_inbox_loads_todays_note_when_the_card_already_has_it() {
    // The same "switch, never clobber" rule as the dated path — the hold must
    // resolve through `open_inbox_today`, not a second implementation.
    let mut e = palette_editor(&[INBOX_TODAY]);
    ex(&mut e, "inbox");
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SyncClock]);

    e.set_today(Some(TODAY));

    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: INBOX_TODAY.into(), scope: Scope::Tracked }],
    );
    assert!(!e.dirty(), "must not have created a new empty buffer over it");
}

#[test]
fn a_date_with_nothing_held_opens_nothing() {
    // The host feeds the date every pass; only a held `:inbox` may act on it.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_today(Some(TODAY));
    e.set_today(Some(TODAY));
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert!(e.take_effects().is_empty());
}

#[test]
fn a_clock_that_never_arrives_drops_the_hold_once() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "inbox");
    let _ = e.take_effects();

    assert!(e.take_pending_inbox(), "the failure has an `:inbox` to report to");
    assert!(!e.take_pending_inbox(), "and only the one — a boot sync speaks to nobody");

    // Dropped, not deferred: a later date must not open a note nobody is
    // waiting for any more.
    e.set_today(Some(TODAY));
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert!(e.take_effects().is_empty());
}

#[test]
fn holding_never_dates_a_note_at_the_epoch() {
    // The whole point of the hold: no `1970-01-01.md`, whatever happens next.
    let mut e = palette_editor(&["/sd/repo/notes.md"]);
    ex(&mut e, "inbox");
    e.handle(Key::Char('i'));
    send(&mut e, "a thought");
    e.handle(Key::Escape);
    assert_eq!(e.path(), "/sd/repo/notes.md", "typing lands in the buffer we kept");
    assert!(!e.file_list_contains("/sd/repo/_inbox/1970-01-01.md"));

    // And the text typed while waiting stays with the file it was typed into.
    e.set_today(Some(TODAY));
    assert_eq!(e.path(), INBOX_TODAY);
    assert_eq!(e.resident_text("/sd/repo/notes.md"), Some("a thought"));
}

#[test]
fn a_held_inbox_waits_for_normal_rather_than_land_mid_insert() {
    // Opening a buffer resets the input state: arriving mid-Insert would drop
    // the writer into Normal and turn their next words into motions.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "inbox");
    let _ = e.take_effects();
    e.handle(Key::Char('i'));
    send(&mut e, "mid sentence");

    e.set_today(Some(TODAY));

    assert_eq!(e.mode(), Mode::Insert, "the writer keeps typing where they were");
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert!(e.inbox_pending(), "still held — the seam has not come round yet");

    e.handle(Key::Escape);
    e.set_today(Some(TODAY));
    assert_eq!(e.path(), INBOX_TODAY, "and it opens at the first Normal-mode pass");
    assert_eq!(e.resident_text("/sd/repo/notes.md"), Some("mid sentence"));
}

#[test]
fn a_second_inbox_while_held_asks_again() {
    // The retry path after a failed sync: `:in` re-asks rather than sitting on a
    // hold nothing will ever resolve.
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    ex(&mut e, "inbox");
    let _ = e.take_effects();
    ex(&mut e, "in");
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SyncClock]);
    assert!(e.inbox_pending());
}

#[test]
fn oldest_opens_the_chronologically_first_inbox_note() {
    // ISO-dated names sort chronologically, so the oldest is the first `_inbox/`
    // entry in the (sorted) file list — 2026-06-15 here.
    let mut e = palette_editor(&[
        "/sd/repo/notes.md",
        "/sd/repo/_inbox/2026-07-08.md",
        "/sd/repo/_inbox/2026-06-15.md",
        "/sd/repo/_inbox/2026-06-30.md",
        "/sd/repo/zzz.md",
    ]);
    ex(&mut e, "oldest");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/_inbox/2026-06-15.md".into(), scope: Scope::Tracked }],
    );
}

#[test]
fn oldest_alias_old_works() {
    let mut e = palette_editor(&["/sd/repo/_inbox/2026-06-15.md", "/sd/repo/_inbox/2026-07-08.md"]);
    ex(&mut e, "old");
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Load]);
}

#[test]
fn oldest_on_an_empty_inbox_notices_and_does_nothing() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/journal.md"]);
    ex(&mut e, "oldest");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.notice.as_deref(), Some("inbox empty"));
}

#[test]
fn oldest_ignores_non_md_files_and_lookalike_dirs() {
    let mut e = palette_editor(&[
        "/sd/repo/_inboxes/2020-01-01.md",
        "/sd/repo/_inbox/2026-06-15.txt",
        "/sd/repo/_inbox/2026-06-20.md",
    ]);
    ex(&mut e, "oldest");
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/_inbox/2026-06-20.md".into(), scope: Scope::Tracked }],
    );
}

// ---- the `>` palette entries -------------------------------------------

/// Open the command palette and run the entry `filter` selects.
fn run_palette_cmd(e: &mut Editor, filter: &str) {
    e.handle(Key::CommandPalette);
    for c in filter.chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
}

#[test]
fn palette_new_fleeting_note_creates_todays_note() {
    let mut e = palette_editor(&["/sd/repo/notes.md"]);
    e.set_today(Some(TODAY));
    run_palette_cmd(&mut e, "new fleeting");
    assert_eq!(e.mode(), Mode::Normal, "a one-shot closes the palette");
    assert_eq!(e.path(), INBOX_TODAY);
    assert_eq!(e.text(), "# 18/07/2026\n\n");
}

#[test]
fn palette_oldest_fleeting_note_opens_the_oldest() {
    let mut e = palette_editor(&["/sd/repo/_inbox/2026-06-15.md", "/sd/repo/_inbox/2026-07-08.md"]);
    run_palette_cmd(&mut e, "oldest fleeting");
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(
        e.take_effects(),
        vec![Effect::Load { path: "/sd/repo/_inbox/2026-06-15.md".into(), scope: Scope::Tracked }],
    );
}

#[test]
fn palette_new_fleeting_note_holds_without_a_clock() {
    // Same contract as `:inbox`: hold and ask for the clock, never `1970-01-01.md`.
    let mut e = palette_editor(&["/sd/repo/notes.md"]);
    run_palette_cmd(&mut e, "new fleeting");
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SyncClock]);
    assert!(e.inbox_pending());
}
