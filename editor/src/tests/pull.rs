//! `:gl` pull support.

use super::*;

#[test]
fn refresh_active_replaces_text_and_resets_state() {
    let mut e = over("old text");
    e.handle(Key::Char('v'));
    e.refresh_active("pulled text".into());
    assert_eq!(e.text, "pulled text");
    assert_eq!(e.mode(), Mode::Normal);
    assert!(!e.dirty());
    assert!(e.undo.is_empty());
    assert_eq!(e.path(), "/sd/repo/notes.md");
    assert_eq!(e.caret, 10);
}

#[test]
fn joined_file_list_sorts_dedups_and_survives_blank_lines() {
    let mut e = Editor::new();
    e.set_file_list_joined("/sd/repo/b.md\n\n/sd/repo/a.md\n/sd/repo/b.md\n".into());
    assert_eq!(files_vec(&e), vec!["/sd/repo/a.md", "/sd/repo/b.md"]);
    e.add_to_file_list("/sd/repo/ab.md");
    e.add_to_file_list("/sd/repo/a.md");
    assert_eq!(
        files_vec(&e),
        vec!["/sd/repo/a.md", "/sd/repo/ab.md", "/sd/repo/b.md"]
    );
    e.remove_from_file_list("/sd/repo/b.md");
    assert_eq!(files_vec(&e), vec!["/sd/repo/a.md", "/sd/repo/ab.md"]);
}

#[test]
fn drop_clean_parked_keeps_only_dirty_buffers() {
    let mut e = over("one"); // active: notes.md, clean
    e.handle(Key::Char(':'));
    for c in "enew /sd/repo/b.md".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    e.handle(Key::Char(':'));
    for c in "enew /sd/repo/c.md".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.parked.len(), 2);
    e.drop_clean_parked();
    let kept: Vec<&str> = e.parked.iter().map(|b| b.path.as_str()).collect();
    assert_eq!(kept, ["/sd/repo/b.md"]);
}

#[test]
fn an_update_says_updating_not_syncing() {
    // `:update` reboots the device when it lands, so the row must not read like
    // an ordinary push — the writer should be able to tell what is running from
    // the panel alone, since the snackbar that said so is gone at the first
    // keystroke.
    let mut e = over("hello");
    e.set_net_flag(Some(NetFlag::Syncing));
    let syncing = e.draw(true).bytes().to_vec();
    e.set_net_flag(Some(NetFlag::Updating));
    let updating = e.draw(true).bytes().to_vec();
    assert_ne!(syncing, updating, "the two states must not render the same row");
}

#[test]
fn a_held_inbox_shows_on_the_panel_until_it_resolves() {
    // The clock sync it waits for flies no flag, and the hold ends in a buffer
    // switch nobody pressed a key for. Without a row, a writer who types after
    // `:inbox` has no way to tell it is still coming.
    let mut e = over("hello");
    let quiet = e.draw(true).bytes().to_vec();
    ex(&mut e, "inbox");
    assert!(e.inbox_pending(), "no date and no walk yet");
    let held = e.draw(true).bytes().to_vec();
    assert_ne!(held, quiet, "a held :inbox has to be visible");

    // A keystroke clears the snackbar but must not clear the row.
    e.handle(Key::Char('x'));
    assert_eq!(e.notice(), None);
    assert_ne!(e.draw(true).bytes().to_vec(), quiet, "typing must not hide it");
}

#[test]
fn a_net_operation_outranks_a_held_inbox_on_the_shared_row() {
    // One row, two possible claimants. The net operation wins: it is the one
    // that also holds off the idle-save.
    let mut e = over("hello");
    ex(&mut e, "inbox");
    assert_eq!(e.activity(), Some(NetFlag::Inbox));
    e.set_net_flag(Some(NetFlag::Syncing));
    assert_eq!(e.activity(), Some(NetFlag::Syncing), "the sync owns the row while it runs");
    e.set_net_flag(None);
    assert_eq!(e.activity(), Some(NetFlag::Inbox), "and the hold gets it back");
}

#[test]
fn the_syncing_flag_owns_the_reserved_sync_row_and_nothing_else() {
    // The in-flight flag has to sit on the row `scope_y` already reserves: the
    // snackbar under it and the face-collision math both key off that row, so
    // anything taller would move the notice and could push Typo out of frame.
    let mut e = over("hello");
    e.set_notice("pulling...");
    let quiet = e.draw(true).bytes().to_vec();
    e.set_net_flag(Some(NetFlag::Syncing));
    let flagged = e.draw(true).bytes().to_vec();

    let row = |bytes: &[u8], y: usize| {
        bytes[y * display::FB_BYTES_W..(y + 1) * display::FB_BYTES_W].to_vec()
    };
    let changed: Vec<usize> = (0..display::HEIGHT as usize)
        .filter(|&y| row(&flagged, y) != row(&quiet, y))
        .collect();
    // One-line filename → words row at 2 + PANEL_CH, sync row a blank row below.
    let scope_y = (2 + PANEL_CH + 2 * PANEL_CH) as usize;
    assert!(!changed.is_empty(), "the flag must actually show in the panel");
    assert!(
        changed.iter().all(|&y| (scope_y..scope_y + PANEL_CH as usize).contains(&y)),
        "the flag drew outside the reserved sync row: rows {changed:?}"
    );
}
