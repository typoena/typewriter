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
fn the_syncing_flag_owns_the_reserved_sync_row_and_nothing_else() {
    // The in-flight flag has to sit on the row `scope_y` already reserves: the
    // snackbar under it and the face-collision math both key off that row, so
    // anything taller would move the notice and could push Typo out of frame.
    let mut e = over("hello");
    e.set_notice("pulling...");
    let quiet = e.draw(true).bytes().to_vec();
    e.set_syncing(true);
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
