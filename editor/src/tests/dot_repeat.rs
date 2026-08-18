//! The `.` repeat command.

use super::*;

#[test]
fn dot_repeats_x() {
    let mut e = Editor::with_text("abcde".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('x'));
    e.handle(Key::Char('.'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), "de");
}

#[test]
fn dot_repeats_dd() {
    let mut e = Editor::with_text("a\nb\nc\nd".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('d'));
    e.handle(Key::Char('d'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), "c\nd");
}

#[test]
fn dot_repeats_dw() {
    let mut e = Editor::with_text("foo bar baz".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('d'));
    e.handle(Key::Char('w'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), "baz");
}

#[test]
fn dot_repeats_a_change_operator_with_its_inserted_text() {
    // The reason `.` records keystrokes: it must replay `ciw` *and* the text
    // typed in the insert session that followed.
    let mut e = Editor::with_text("foo bar".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('c'));
    e.handle(Key::Char('i'));
    e.handle(Key::Char('w'));
    e.handle(Key::Char('X'));
    e.handle(Key::Escape);
    assert_eq!(e.text(), "X bar");
    e.handle(Key::Char('w'));
    e.handle(Key::Char('.')); // repeat: change that word to "X" too
    assert_eq!(e.text(), "X X");
}

#[test]
fn dot_repeats_a_paste() {
    let mut e = Editor::with_text("x\na\nb".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('p'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), "x\nx\nx\na\nb");
}

#[test]
fn dot_ignores_pure_motions() {
    let mut e = Editor::with_text("abc".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('l'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), "abc");
}

#[test]
fn a_yank_does_not_become_the_dot_change() {
    // `y` is not a `.`-repeatable change; the prior `x` must remain the dot.
    let mut e = Editor::with_text("abcdef".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('x')); // dot = x; "bcdef"
    e.handle(Key::Char('y'));
    e.handle(Key::Char('w'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), "cdef");
}

#[test]
fn dot_in_insert_mode_is_a_literal_character() {
    let mut e = Editor::new();
    e.handle(Key::Char('i'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), ".");
}

#[test]
fn text_getter_reflects_edits() {
    let e = typed("hello");
    assert_eq!(e.text(), "hello");
}

#[test]
fn a_notice_shows_until_the_next_key_dismisses_it() {
    let mut e = Editor::new();
    e.set_notice("saved");
    assert_eq!(e.notice.as_deref(), Some("saved"));
    e.handle(Key::Char('j'));
    assert_eq!(e.notice, None);
}
