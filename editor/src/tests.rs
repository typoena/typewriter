use super::*;

/// Type a run of characters in Insert mode, entered with `i` from the
/// power-on Normal mode.
fn typed(s: &str) -> Editor {
    let mut e = Editor::new();
    e.handle(Key::Char('i')); // Normal -> Insert
    for c in s.chars() {
        e.handle(Key::Char(c));
    }
    e
}

/// From a fresh editor over a named Tracked file, run `:{cmd}<Enter>`,
/// returning the editor and the drained [`Effect`]s the command queued.
fn command(cmd: &str) -> (Editor, Vec<Effect>) {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char(':')); // Normal -> Command
    for c in cmd.chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    let effects = e.take_effects();
    (e, effects)
}

/// Coarse kind of an [`Effect`], ignoring `Save`/`Load` payloads, so the
/// command tests can assert intent without pinning path/scope/contents.
#[derive(Debug, PartialEq)]
enum Kind {
    Save,
    Load,
    Publish,
    Pull,
    Delete,
    SavePrefs,
}

fn kinds(effects: &[Effect]) -> Vec<Kind> {
    effects
        .iter()
        .map(|e| match e {
            Effect::Save { .. } => Kind::Save,
            Effect::Load { .. } => Kind::Load,
            Effect::Publish => Kind::Publish,
            Effect::Pull => Kind::Pull,
            Effect::Delete { .. } => Kind::Delete,
            Effect::SavePrefs { .. } => Kind::SavePrefs,
        })
        .collect()
}

#[test]
fn insert_builds_buffer_and_advances_caret() {
    let e = typed("hello");
    assert_eq!(e.text, "hello");
    assert_eq!(e.caret, 5);
    assert_eq!(e.mode(), Mode::Insert);
}

#[test]
fn backspace_deletes_previous_char() {
    let mut e = typed("hello");
    e.handle(Key::Backspace);
    assert_eq!(e.text, "hell");
    assert_eq!(e.caret, 4);
}

#[test]
fn enter_splits_the_line() {
    let mut e = typed("ab");
    e.handle(Key::Enter);
    e.handle(Key::Char('c'));
    assert_eq!(e.text, "ab\nc");
    assert_eq!(e.caret, 4);
}

#[test]
fn enter_in_a_blockquote_continues_the_marker() {
    let mut e = typed("> quote");
    e.handle(Key::Enter);
    e.handle(Key::Char('m'));
    assert_eq!(e.text, "> quote\n> m");
    assert_eq!(e.caret, 11);
}

#[test]
fn enter_on_an_empty_blockquote_exits_the_quote() {
    let mut e = typed("> quote");
    e.handle(Key::Enter); // -> "> quote\n> "
    e.handle(Key::Enter); // empty quote: drop the "> ", leaving a blank line
    assert_eq!(e.text, "> quote\n");
    assert_eq!(e.caret, 8);
}

#[test]
fn enter_in_a_nested_blockquote_keeps_the_depth() {
    let mut e = typed("> > deep");
    e.handle(Key::Enter);
    e.handle(Key::Char('x'));
    assert_eq!(e.text, "> > deep\n> > x");
}

#[test]
fn escape_enters_normal_and_steps_onto_last_char() {
    let mut e = typed("abc");
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(e.caret, 2); // vim: caret drops onto the last inserted char
}

#[test]
fn normal_h_and_l_step_one_char() {
    let mut e = typed("abc");
    e.handle(Key::Escape); // Normal, caret = 2
    e.handle(Key::Char('h'));
    assert_eq!(e.caret, 1);
    e.handle(Key::Char('h'));
    assert_eq!(e.caret, 0);
    e.handle(Key::Char('l'));
    assert_eq!(e.caret, 1);
}

#[test]
fn normal_x_deletes_char_under_caret() {
    let mut e = typed("abc");
    e.handle(Key::Escape); // caret on 'c'
    e.handle(Key::Char('h')); // caret on 'b'
    e.handle(Key::Char('x'));
    assert_eq!(e.text, "ac");
}

#[test]
fn word_forward_lands_on_next_word_start() {
    let mut e = typed("foo bar");
    e.handle(Key::Escape); // Normal
    e.handle(Key::Char('0')); // line start
    e.handle(Key::Char('w'));
    assert_eq!(e.caret, 4); // 'b' of "bar"
}

/// The buffer round-trips and `draw()` runs for a plain-ASCII buffer — the
/// current, byte==char world. UTF-8 (accented-input) correctness is the next
/// change; when it lands, add the accented-motion cases here.
#[test]
fn draw_produces_a_full_frame_for_ascii() {
    let mut e = typed("hello world");
    let frame = e.draw(true);
    assert_eq!(frame.bytes().len(), display::FB_BYTES);
}

// ---- UTF-8 correctness: accented (Latin-9) input the composer feeds ----

#[test]
fn insert_accented_char_advances_by_utf8_len() {
    let e = typed("é");
    assert_eq!(e.text, "é");
    assert_eq!(e.caret, 2); // 'é' is two bytes; caret is a byte offset
}

#[test]
fn backspace_deletes_whole_multibyte_char() {
    let mut e = typed("café");
    e.handle(Key::Backspace);
    assert_eq!(e.text, "caf");
    assert_eq!(e.caret, 3);
}

#[test]
fn normal_hl_step_over_multibyte_chars() {
    let mut e = typed("aéb"); // bytes: a(1) é(2) b(1)
    e.handle(Key::Escape); // Normal, caret onto 'b' at byte 3
    assert_eq!(e.caret, 3);
    e.handle(Key::Char('h')); // onto 'é'
    assert_eq!(e.caret, 1);
    e.handle(Key::Char('h')); // onto 'a'
    assert_eq!(e.caret, 0);
    e.handle(Key::Char('l')); // back onto 'é'
    assert_eq!(e.caret, 1);
    e.handle(Key::Char('l')); // onto 'b'
    assert_eq!(e.caret, 3);
}

#[test]
fn delete_char_under_caret_removes_whole_multibyte() {
    let mut e = typed("aéb");
    e.handle(Key::Escape); // caret on 'b'
    e.handle(Key::Char('h')); // caret on 'é'
    e.handle(Key::Char('x'));
    assert_eq!(e.text, "ab");
}

#[test]
fn de_deletes_through_end_of_accented_word() {
    let mut e = typed("café bar");
    e.handle(Key::Escape);
    e.handle(Key::Char('0')); // line start, on 'c'
    e.handle(Key::Char('d'));
    e.handle(Key::Char('e')); // delete to the end of "café"
    assert_eq!(e.text, " bar");
}

#[test]
fn vertical_move_keeps_char_column_across_accents() {
    let mut e = typed("éé"); // line 0: two 2-byte chars
    e.handle(Key::Enter);
    for c in "xxx".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Escape); // Normal, on last 'x'
    e.handle(Key::Char('k')); // up to line 0 at the same character column
    assert!(e.text.is_char_boundary(e.caret)); // never lands mid-character
}

#[test]
fn draw_runs_for_accented_buffer() {
    // Every glyph here is in ISO-8859-15, which the composer is limited to.
    let mut e = typed("café naïve garçon çÿ");
    let frame = e.draw(true);
    assert_eq!(frame.bytes().len(), display::FB_BYTES);
}

#[test]
fn extra_glyph_covers_snippet_and_prose_symbols() {
    // The curated non-Latin-9 set the overlay draws (→ ≠ Σ … – — ' ' " " •).
    let targets = [
        '\u{2192}', '\u{2260}', '\u{03A3}', '\u{2026}', '\u{2013}', '\u{2014}', '\u{2018}',
        '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}',
    ];
    for c in targets {
        assert!(extra_glyph(c).is_some(), "missing glyph for U+{:04X}", c as u32);
    }
    // The base font already draws these — the overlay must defer to it,
    // including œ/€ (which *are* in ISO-8859-15, at 0xBD/0xA4).
    for c in ['a', 'é', 'œ', '€', ' ', '#', '-'] {
        assert!(extra_glyph(c).is_none(), "should defer to base font: {c}");
    }
}

#[test]
fn draw_runs_for_symbol_buffer() {
    // Insert the whole extra set and render with a caret — no panic, right size.
    let mut e = typed("\u{2192} \u{2260} \u{03A3} \u{2026} \u{2013} \u{2014} \u{2018}x\u{2019} \u{201C}y\u{201D} \u{2022}");
    let frame = e.draw(true);
    assert_eq!(frame.bytes().len(), display::FB_BYTES);
    assert!(e.text.is_char_boundary(e.caret));
}

/// 1 = white paper, 0 = black ink (SSD16xx convention). Reads one pixel.
fn ink_at(frame: &Frame, x: usize, y: usize) -> bool {
    frame.bytes()[y * display::FB_BYTES_W + x / 8] & (0x80 >> (x % 8)) == 0
}

#[test]
fn overlay_paints_extra_glyph_over_fallback_box() {
    // The em dash is two solid mid-height bars and nothing else; a fallback
    // box would ink the cell's top row. Gutter off so it lands in column 0.
    let mut e = Editor::with_text("\u{2014}".into()); // —
    e.prefs.line_numbers = false;
    let f = e.draw(false); // no caret
    assert!((0..10).all(|x| !ink_at(&f, x, 0)), "cell top row must be blank");
    assert!((0..10).all(|x| ink_at(&f, x, 9)), "row 9 must be solid ink");
    assert!((0..10).all(|x| ink_at(&f, x, 10)), "row 10 must be solid ink");
}

#[test]
fn overlay_inverts_extra_glyph_under_selection() {
    // Same em dash, but selected: reverse-video flips the cell — the fill
    // goes black and the dash bars punch back to white paper.
    let mut e = Editor::with_text("\u{2014}x".into());
    e.prefs.line_numbers = false;
    e.handle(Key::Char('0')); // to column 0 (the em dash)
    e.handle(Key::Char('v')); // charwise Visual selects the char under the caret
    let f = e.draw(false); // no active-end caret punch, just the selection
    assert!((0..10).all(|x| ink_at(&f, x, 0)), "selected cell top row must be inked");
    assert!((0..10).all(|x| !ink_at(&f, x, 9)), "row 9 dash must punch to white");
    assert!((0..10).all(|x| !ink_at(&f, x, 10)), "row 10 dash must punch to white");
}

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
fn gp_command_saves_then_publishes() {
    // `:gp` queues a save of the current buffer, then the git publish.
    assert_eq!(kinds(&command("gp").1), vec![Kind::Save, Kind::Publish]);
}

#[test]
fn gl_command_signals_pull() {
    assert_eq!(kinds(&command("gl").1), vec![Kind::Pull]);
}

#[test]
fn gp_formats_the_buffer_before_publishing() {
    // fmt → save → commit → push: `:gp` runs :fmt in-core first (default on).
    let mut e = Editor::with_file(
        "/sd/repo/notes.md".into(),
        Scope::Tracked,
        "hello   \nworld".to_string(), // trailing spaces
    );
    e.handle(Key::Char(':'));
    for c in "gp".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Save, Kind::Publish]);
    assert_eq!(e.text(), "hello\nworld"); // :fmt stripped the trailing whitespace
}

#[test]
fn gp_is_refused_in_a_local_buffer() {
    // Publish is Tracked-only; `:gp` in Local queues nothing and warns.
    let mut e = Editor::with_file(
        "/sd/local/journal.md".into(),
        Scope::Local,
        "dear diary".to_string(),
    );
    e.handle(Key::Char(':'));
    for c in "gp".chars() {
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
    // kept; a run of them collapses to one; a note with none gains none.
    assert_eq!(format_markdown("hello\n"), "hello\n"); // one blank kept
    assert_eq!(format_markdown("hello\n\n\n"), "hello\n"); // extras collapsed to one
    assert_eq!(format_markdown("hello"), "hello"); // none added
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

#[test]
fn fmt_stays_in_core_and_asks_the_host_for_nothing() {
    assert!(command("fmt").1.is_empty());
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

// ---- Ctrl-d / Ctrl-u half-page scroll (v0.2) ----

/// The core reason this isn't `HALF_PAGE × move_down`: on one long paragraph
/// that soft-wraps, half-page-down steps *display* rows, advancing the caret
/// half a window into the wrap — whereas `j` (logical-line) can't move
/// within the single line at all.
#[test]
fn half_page_down_steps_display_rows_within_a_wrapped_line() {
    let mut e = Editor::with_text("a".repeat(WRITE_COLS * 10)); // one long wrapped line
    let cols = e.text_cols(); // wrap width shrinks by the gutter
    e.caret = 0;
    e.handle(Key::HalfPageDown);
    assert_eq!(e.caret, cols * HALF_PAGE); // down HALF_PAGE *display* rows

    // Contrast: `j` on the same single logical line is a no-op.
    let mut j = Editor::with_text("a".repeat(WRITE_COLS * 10));
    j.caret = 0;
    j.handle(Key::Char('j'));
    assert_eq!(j.caret, 0);
}

/// Up is the inverse of down within a wrapped line.
#[test]
fn half_page_up_is_the_inverse_within_a_wrapped_line() {
    let mut e = Editor::with_text("a".repeat(WRITE_COLS * 10));
    e.caret = e.text_cols() * HALF_PAGE; // start on a display-row boundary
    e.handle(Key::HalfPageUp);
    assert_eq!(e.caret, 0);
}

/// Clamps at both ends: up from the top stays; down past the bottom lands on
/// the last row on a character boundary, never out of range.
#[test]
fn half_page_clamps_at_both_ends() {
    let mut e = Editor::with_text("a".repeat(WRITE_COLS * 3)); // 3 rows
    e.caret = 0;
    e.handle(Key::HalfPageUp);
    assert_eq!(e.caret, 0);
    e.handle(Key::HalfPageDown);
    e.handle(Key::HalfPageDown);
    assert!(e.caret <= e.text.len());
    assert!(e.text.is_char_boundary(e.caret));
}

/// The viewport follows the caret past the window: after enough half-pages,
/// `scroll_top` advances (in draw) and the caret stays visible.
#[test]
fn half_page_down_scrolls_the_viewport() {
    let text = vec!["a"; 40].join("\n"); // 40 one-char lines = 40 display rows
    let mut e = Editor::with_text(text);
    e.caret = 0;
    for _ in 0..4 {
        e.handle(Key::HalfPageDown);
    }
    e.draw(true); // adjust_scroll runs here
    assert!(e.scroll_top() > 0, "viewport should have scrolled");
    let lay = e.layout();
    let (row, _) = e.caret_rc(&lay);
    assert!(row >= e.scroll_top() && row < e.scroll_top() + ROWS);
}

/// In View mode (read-only) half-page moves the viewport directly and leaves
/// the caret alone.
#[test]
fn half_page_scrolls_viewport_in_view_mode() {
    let mut e = Editor::with_text(vec!["a"; 40].join("\n"));
    let caret_before = e.caret;
    e.handle(Key::Char('g')); // `gr` -> View (v/V are now Visual)
    e.handle(Key::Char('r'));
    assert_eq!(e.mode(), Mode::View);
    e.handle(Key::HalfPageDown);
    assert_eq!(e.scroll_top(), HALF_PAGE);
    assert_eq!(e.caret, caret_before); // caret untouched in View
    e.handle(Key::HalfPageUp);
    assert_eq!(e.scroll_top(), 0);
}

/// Inert in Insert mode — it must not yank the caret off the text you're
/// typing.
#[test]
fn half_page_is_a_noop_in_insert_mode() {
    let mut e = Editor::with_text(vec!["a"; 40].join("\n"));
    e.caret = 0;
    e.handle(Key::Char('i')); // Normal -> Insert
    e.handle(Key::HalfPageDown);
    assert_eq!(e.caret, 0);
    assert_eq!(e.mode(), Mode::Insert);
}

// ---- Absolute line-number gutter (v0.2) ----

#[test]
fn gutter_is_two_digits_plus_separator_for_small_files() {
    let e = Editor::with_text("one\ntwo\nthree".to_string()); // 3 logical lines
    assert_eq!(e.logical_lines(), 3);
    assert_eq!(e.gutter_cols(), 3); // 2 digit cols + 1 separator
    assert_eq!(e.text_cols(), WRITE_COLS - 3);
}

#[test]
fn gutter_widens_past_ninety_nine_lines() {
    let e = Editor::with_text("x\n".repeat(120)); // 121 logical lines
    assert_eq!(e.gutter_cols(), 4); // 3 digit cols + 1 separator
    assert_eq!(e.text_cols(), WRITE_COLS - 4);
}

#[test]
fn gutter_narrows_the_soft_wrap_width() {
    let e = Editor::with_text("a".repeat(WRITE_COLS)); // 60 chars, one logical line
    let cols = e.text_cols();
    assert!(cols < WRITE_COLS); // the gutter stole columns
    let lay = e.layout();
    assert_eq!(lay[0].text.chars().count(), cols); // first row fills the text width
    assert!(lay.len() >= 2); // 60 chars no longer fit one row
}

#[test]
fn draw_with_gutter_produces_a_full_frame() {
    let mut e = Editor::with_text("line one\nline two\nline three".to_string());
    assert_eq!(e.draw(true).bytes().len(), display::FB_BYTES);
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

// ---- Register + yank / paste (v0.3) ----

#[test]
fn yy_then_p_opens_a_copy_of_the_line_below() {
    let mut e = Editor::with_text("foo\nbar".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g')); // gg -> caret on line "foo"
    e.handle(Key::Char('y'));
    e.handle(Key::Char('y')); // yank the line, linewise
    e.handle(Key::Char('p')); // paste it after the current line
    assert_eq!(e.text(), "foo\nfoo\nbar");
}

#[test]
fn yy_then_capital_p_pastes_the_line_above() {
    let mut e = Editor::with_text("foo\nbar".to_string()); // caret on line "bar"
    e.handle(Key::Char('y'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('P'));
    assert_eq!(e.text(), "foo\nbar\nbar");
}

#[test]
fn dd_then_p_moves_a_line_down() {
    let mut e = Editor::with_text("one\ntwo\nthree".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g')); // caret on "one"
    e.handle(Key::Char('d'));
    e.handle(Key::Char('d')); // cut "one" into the register
    e.handle(Key::Char('p')); // paste after "two"
    assert_eq!(e.text(), "two\none\nthree");
}

#[test]
fn count_dd_captures_all_lines_for_paste() {
    let mut e = Editor::with_text("a\nb\nc\nd".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('3'));
    e.handle(Key::Char('d'));
    e.handle(Key::Char('d')); // 3dd — cut three lines
    assert_eq!(e.text(), "d");
    e.handle(Key::Char('p')); // paste all three back below "d"
    assert_eq!(e.text(), "d\na\nb\nc");
}

#[test]
fn x_then_p_replays_the_deleted_char_after_the_caret() {
    let mut e = Editor::with_text("abc".to_string());
    e.handle(Key::Char('0')); // caret on 'a'
    e.handle(Key::Char('x')); // delete 'a' -> "bc", register = "a" (charwise)
    e.handle(Key::Char('p')); // paste after 'b'
    assert_eq!(e.text(), "bac");
}

#[test]
fn yw_yanks_charwise_and_p_inserts_after_the_caret() {
    let mut e = Editor::with_text("foo bar".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('w')); // yank "foo " (word + trailing space), caret stays put
    e.handle(Key::Char('p'));
    assert_eq!(e.text(), "ffoo oo bar"); // charwise paste after the cursor char
}

#[test]
fn capital_p_pastes_a_char_before_the_caret() {
    let mut e = Editor::with_text("abc".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('x')); // register = "a", text "bc", caret on 'b'
    e.handle(Key::Char('l')); // caret on 'c'
    e.handle(Key::Char('P')); // paste "a" before 'c'
    assert_eq!(e.text(), "bac");
}

#[test]
fn paste_with_an_empty_register_is_a_noop() {
    let mut e = Editor::with_text("abc".to_string());
    e.handle(Key::Char('p'));
    e.handle(Key::Char('P'));
    assert_eq!(e.text(), "abc");
}

#[test]
fn multiline_paste_at_the_bottom_reveals_the_whole_block() {
    // A screenful+ of lines, caret on the last line; paste two lines after
    // it. Both pasted lines must be visible without a manual scroll — the
    // caret stays on the first pasted line, but the viewport reveals the end.
    let mut e = Editor::with_text(vec!["x"; 20].join("\n")); // 20 display rows
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('2'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('y')); // yank two lines
    e.handle(Key::Char('G')); // to the last line
    e.handle(Key::Char('p')); // paste two lines below it (22 rows total)
    e.draw(true); // adjust_scroll runs; reveal already applied by paste
    let last_row = e.layout().len() - 1; // the second pasted line
    assert!(
        last_row >= e.scroll_top() && last_row < e.scroll_top() + ROWS,
        "pasted block end (row {last_row}) off-screen at scroll_top {}",
        e.scroll_top()
    );
}

// ---- Undo / redo (v0.3) ----

#[test]
fn undo_reverts_a_whole_insert_session_at_once() {
    let mut e = Editor::new();
    e.handle(Key::Char('i'));
    for c in "hello".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Escape);
    assert_eq!(e.text(), "hello");
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), ""); // the entire typed run, not one char
    assert_eq!(e.mode(), Mode::Normal); // undo always lands in Normal
}

#[test]
fn redo_reapplies_an_undone_change() {
    let mut e = Editor::new();
    e.handle(Key::Char('i'));
    e.handle(Key::Char('x'));
    e.handle(Key::Escape); // "x"
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), "");
    e.handle(Key::Redo); // Ctrl-r
    assert_eq!(e.text(), "x");
}

#[test]
fn undo_reverts_dd() {
    let mut e = Editor::with_text("one\ntwo".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('d'));
    e.handle(Key::Char('d'));
    assert_eq!(e.text(), "two");
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), "one\ntwo");
}

#[test]
fn undo_reverts_x_and_restores_the_caret() {
    let mut e = Editor::with_text("abc".to_string()); // caret on 'c'
    e.handle(Key::Char('x'));
    assert_eq!(e.text(), "ab");
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), "abc");
    assert_eq!(e.caret, 2); // caret came back to where the change began
}

#[test]
fn undo_reverts_a_paste() {
    let mut e = Editor::with_text("foo\nbar".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('p'));
    assert_eq!(e.text(), "foo\nfoo\nbar");
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), "foo\nbar");
}

#[test]
fn a_fresh_edit_after_undo_clears_the_redo_history() {
    let mut e = Editor::new();
    e.handle(Key::Char('i'));
    e.handle(Key::Char('a'));
    e.handle(Key::Escape); // "a"
    e.handle(Key::Char('u')); // -> ""
    e.handle(Key::Char('i'));
    e.handle(Key::Char('b'));
    e.handle(Key::Escape); // new branch: "b"
    e.handle(Key::Redo); // nothing to redo — the "a" branch is gone
    assert_eq!(e.text(), "b");
}

#[test]
fn successive_undos_walk_the_history_back() {
    let mut e = Editor::new();
    e.handle(Key::Char('i'));
    e.handle(Key::Char('a'));
    e.handle(Key::Escape); // "a"
    e.handle(Key::Char('A'));
    e.handle(Key::Char('b'));
    e.handle(Key::Escape); // "ab"
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), "a");
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), "");
}

#[test]
fn undo_with_empty_history_is_a_noop() {
    let mut e = Editor::with_text("x".to_string());
    e.handle(Key::Char('u'));
    assert_eq!(e.text(), "x");
    e.handle(Key::Redo);
    assert_eq!(e.text(), "x");
}

// ---- `.` repeat (v0.3) ----

#[test]
fn dot_repeats_x() {
    let mut e = Editor::with_text("abcde".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('x')); // "bcde"
    e.handle(Key::Char('.')); // "cde"
    e.handle(Key::Char('.')); // "de"
    assert_eq!(e.text(), "de");
}

#[test]
fn dot_repeats_dd() {
    let mut e = Editor::with_text("a\nb\nc\nd".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('d'));
    e.handle(Key::Char('d')); // delete "a"
    e.handle(Key::Char('.')); // delete "b"
    assert_eq!(e.text(), "c\nd");
}

#[test]
fn dot_repeats_dw() {
    let mut e = Editor::with_text("foo bar baz".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('d'));
    e.handle(Key::Char('w')); // "bar baz"
    e.handle(Key::Char('.')); // "baz"
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
    e.handle(Key::Escape); // "X bar"
    assert_eq!(e.text(), "X bar");
    e.handle(Key::Char('w')); // caret onto "bar"
    e.handle(Key::Char('.')); // repeat: change that word to "X" too
    assert_eq!(e.text(), "X X");
}

#[test]
fn dot_repeats_a_paste() {
    let mut e = Editor::with_text("x\na\nb".to_string());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e.handle(Key::Char('y'));
    e.handle(Key::Char('y')); // yank line "x"
    e.handle(Key::Char('p')); // "x\nx\na\nb"
    e.handle(Key::Char('.')); // paste again below
    assert_eq!(e.text(), "x\nx\nx\na\nb");
}

#[test]
fn dot_ignores_pure_motions() {
    let mut e = Editor::with_text("abc".to_string());
    e.handle(Key::Char('0'));
    e.handle(Key::Char('l')); // motions only — nothing to repeat
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
    e.handle(Key::Char('w')); // yank — must not overwrite the dot
    e.handle(Key::Char('.')); // repeat the x, not the yank
    assert_eq!(e.text(), "cdef");
}

#[test]
fn dot_in_insert_mode_is_a_literal_character() {
    let mut e = Editor::new();
    e.handle(Key::Char('i'));
    e.handle(Key::Char('.'));
    assert_eq!(e.text(), "."); // '.' only repeats from Normal
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
    e.handle(Key::Char('j')); // any key dismisses the snackbar
    assert_eq!(e.notice, None);
}

// ---- Visual mode (v0.4) ----

/// Feed a run of characters as Normal-mode keys.
fn send(e: &mut Editor, s: &str) {
    for c in s.chars() {
        e.handle(Key::Char(c));
    }
}

#[test]
fn v_enters_charwise_visual_and_anchors_at_the_caret() {
    let mut e = Editor::with_text("hello".into());
    e.caret = 2;
    e.handle(Key::Char('v'));
    assert_eq!(e.mode(), Mode::Visual);
    assert_eq!(e.visual_anchor, Some(2));
}

#[test]
fn capital_v_enters_linewise_visual() {
    let mut e = Editor::with_text("hello".into());
    e.handle(Key::Char('V'));
    assert_eq!(e.mode(), Mode::VisualLine);
}

#[test]
fn charwise_yank_is_inclusive_and_lands_the_caret_at_the_start() {
    let mut e = Editor::with_text("hello world".into());
    e.caret = 0;
    send(&mut e, "vey"); // select "hello" (e -> last char of the word), yank
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(e.caret, 0);
    assert_eq!(e.register, "hello");
    assert!(!e.register_linewise);
}

#[test]
fn vy_yanks_the_single_char_under_the_caret() {
    let mut e = Editor::with_text("hello".into());
    e.caret = 1;
    send(&mut e, "vy");
    assert_eq!(e.register, "e");
}

#[test]
fn charwise_delete_removes_the_span_and_fills_the_register() {
    let mut e = Editor::with_text("hello world".into());
    e.caret = 0;
    send(&mut e, "ved"); // select "hello", delete
    assert_eq!(e.text(), " world");
    assert_eq!(e.caret, 0);
    assert_eq!(e.register, "hello");
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn charwise_change_deletes_the_span_and_enters_insert() {
    let mut e = Editor::with_text("hello".into());
    e.caret = 0;
    send(&mut e, "v$c"); // select the whole line, change
    assert_eq!(e.mode(), Mode::Insert);
    assert_eq!(e.text(), "");
    send(&mut e, "bye");
    assert_eq!(e.text(), "bye");
}

#[test]
fn count_in_visual_extends_the_selection() {
    let mut e = Editor::with_text("abcdef".into());
    e.caret = 0;
    send(&mut e, "v2ld"); // select a,b,c (2l from a), delete
    assert_eq!(e.text(), "def");
}

#[test]
fn linewise_delete_removes_the_whole_line_like_dd() {
    let mut e = Editor::with_text("one\ntwo\nthree".into());
    e.caret = e.text().find("two").unwrap();
    send(&mut e, "Vd");
    assert_eq!(e.text(), "one\nthree");
    assert!(e.register_linewise);
    assert_eq!(e.register, "two\n");
}

#[test]
fn linewise_selection_spans_multiple_lines_with_j() {
    let mut e = Editor::with_text("a\nb\nc\nd".into());
    e.caret = 0;
    send(&mut e, "Vjd"); // select lines a and b, delete both
    assert_eq!(e.text(), "c\nd");
}

#[test]
fn linewise_yank_then_paste_copies_the_line_below() {
    let mut e = Editor::with_text("one\ntwo".into());
    e.caret = 0;
    send(&mut e, "Vy"); // yank line "one" linewise
    assert_eq!(e.register, "one\n");
    send(&mut e, "p");
    assert_eq!(e.text(), "one\none\ntwo");
}

#[test]
fn linewise_change_clears_the_line_but_keeps_one_to_type_on() {
    let mut e = Editor::with_text("one\ntwo\nthree".into());
    e.caret = e.text().find("two").unwrap();
    send(&mut e, "Vc");
    assert_eq!(e.mode(), Mode::Insert);
    assert_eq!(e.text(), "one\n\nthree"); // the line's text is gone, the row remains
    send(&mut e, "X");
    assert_eq!(e.text(), "one\nX\nthree");
}

#[test]
fn esc_leaves_visual_without_touching_the_buffer() {
    let mut e = Editor::with_text("hello".into());
    e.caret = 2;
    send(&mut e, "vll");
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(e.text(), "hello");
    assert_eq!(e.visual_anchor, None);
}

#[test]
fn v_toggles_charwise_visual_off() {
    let mut e = Editor::with_text("hello".into());
    send(&mut e, "vv");
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn capital_v_then_v_switches_to_charwise() {
    let mut e = Editor::with_text("hello".into());
    send(&mut e, "Vv");
    assert_eq!(e.mode(), Mode::Visual);
}

#[test]
fn gr_enters_view_and_v_no_longer_does() {
    let mut e = Editor::with_text("hello".into());
    send(&mut e, "gr");
    assert_eq!(e.mode(), Mode::View);
    e.handle(Key::Escape);
    e.handle(Key::Char('v'));
    assert_eq!(e.mode(), Mode::Visual); // v is Visual now, not View
}

#[test]
fn visual_ops_do_not_clobber_the_dot_register() {
    let mut e = Editor::with_text("abcdef".into());
    e.caret = 0;
    e.handle(Key::Char('x')); // dot = x ; "bcdef"
    send(&mut e, "vld"); // a visual delete must not become the new dot
    e.handle(Key::Char('.')); // repeats the x
    // buffer after x -> "bcdef"; vld deletes "bc" -> "def"; . deletes 'd' -> "ef"
    assert_eq!(e.text(), "ef");
}

#[test]
fn draw_inverts_the_selected_cells() {
    let mut e = Editor::with_text("hello world".into());
    e.caret = 0;
    let normal = e.draw(true).bytes().to_vec();
    send(&mut e, "ve"); // select "hello"
    let visual = e.draw(true).bytes().to_vec();
    assert_ne!(normal, visual); // the selection changed pixels
}

#[test]
fn draw_runs_for_a_linewise_selection_over_a_blank_line() {
    let mut e = Editor::with_text("a\n\nb".into());
    e.caret = 0;
    send(&mut e, "Vjj"); // select all three rows, including the blank one
    let _ = e.draw(true); // must not panic on the empty-row highlight path
    assert_eq!(e.mode(), Mode::VisualLine);
}

// ---- Multi-file buffers (v0.5) ----

/// Open `arg` the way a user now does — via the file palette. (`:e` was
/// retired in v0.6; bare `Cmd-P` opens files.) Lists the target, opens the
/// palette, and types its exact label so the fuzzy matcher ranks it first,
/// then Enter selects it — routing through the same `open_path` `:e` used.
fn edit(e: &mut Editor, arg: &str) {
    let (path, _) = resolve_path(arg, e.scope);
    e.add_to_file_list(&path);
    e.open_palette();
    for c in palette_label(&path).chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
}

/// Drive an arbitrary `:{cmd}<Enter>` from Normal.
fn ex(e: &mut Editor, cmd: &str) {
    e.handle(Key::Char(':'));
    for c in cmd.chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
}

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
    assert!(!e.dirty()); // a freshly loaded buffer is clean
    e.handle(Key::Char('x')); // delete a char
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
    assert_eq!(e.caret, 2); // caret on A's last char
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "BBBBB".into());
    // A is parked (resident) — switching back reads memory, not disk.
    edit(&mut e, "/sd/repo/a.md");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert_eq!(e.text(), "AAA");
    assert_eq!(e.caret, 2); // its caret came back with it
}

#[test]
fn the_register_is_global_across_buffers() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "word".into());
    e.handle(Key::Char('y')); // yy — yank the line
    e.handle(Key::Char('y'));
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, String::new());
    e.handle(Key::Char('p')); // paste it into the other buffer
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
    e.take_effects(); // discard anything queued so far
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "B".into()); // parks A(dirty)
    e.install_loaded("/sd/repo/c.md".into(), Scope::Tracked, "C".into()); // parked: [A,B]
    assert!(e.take_effects().is_empty()); // nothing evicted yet
    e.install_loaded("/sd/repo/d.md".into(), Scope::Tracked, "D".into()); // evicts A
    let effs = e.take_effects();
    assert_eq!(effs.len(), 1, "the evicted dirty buffer must be saved");
    match &effs[0] {
        Effect::Save { path, .. } => assert_eq!(path, "/sd/repo/a.md"),
        other => panic!("expected a Save of A, got {other:?}"),
    }
}

#[test]
fn a_clean_parked_buffer_is_dropped_silently_on_eviction() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    // A is never edited (clean); filling past ≤3 must evict it without a Save.
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "B".into());
    e.install_loaded("/sd/repo/c.md".into(), Scope::Tracked, "C".into());
    e.take_effects();
    e.install_loaded("/sd/repo/d.md".into(), Scope::Tracked, "D".into());
    assert!(e.take_effects().is_empty()); // clean buffer: no save on evict
}

// ---- :enew / :delete (v0.5 slice 3) ----

#[test]
fn enew_creates_a_dirty_empty_buffer_and_asks_the_host_for_nothing() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "A".into());
    ex(&mut e, "enew draft.md");
    assert_eq!(e.path(), "/sd/repo/draft.md"); // bare name → current (Tracked) scope
    assert_eq!(e.scope(), Scope::Tracked);
    assert_eq!(e.text(), "");
    assert!(e.dirty()); // fresh + unsaved, so eviction/`:w` will persist it
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
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "BBB".into()); // parks A
    e.take_effects();
    ex(&mut e, "enew /sd/repo/a.md"); // A is parked (resident) — switch, don't empty it
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert_eq!(e.text(), "AAA"); // contents preserved, not clobbered to empty
    assert!(e.take_effects().is_empty()); // resident: no Load
}

#[test]
fn enew_without_a_name_is_a_usage_noop() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "A".into());
    ex(&mut e, "enew");
    assert_eq!(e.path(), "/sd/repo/notes.md"); // unchanged
    assert!(e.take_effects().is_empty());
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn delete_queues_a_delete_of_the_current_file() {
    let (mut e, effs) = command("delete");
    assert_eq!(
        effs,
        vec![Effect::Delete {
            path: "/sd/repo/notes.md".into(),
            scope: Scope::Tracked,
        }]
    );
    // No file remains active (nothing else was resident): a scratch buffer.
    assert_eq!(e.path(), "");
    assert_eq!(e.text(), "");
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.take_effects().is_empty());
}

#[test]
fn delete_never_saves_the_discarded_buffer_even_when_dirty() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "A".into());
    e.handle(Key::Char('x')); // dirty it
    assert!(e.dirty());
    ex(&mut e, "delete");
    // The buffer is being deleted, so it is discarded, not saved: Delete only.
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Delete]);
}

#[test]
fn delete_switches_to_the_most_recently_parked_buffer() {
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "AAA".into());
    e.install_loaded("/sd/repo/b.md".into(), Scope::Tracked, "BBB".into()); // active B, A parked
    e.take_effects();
    ex(&mut e, "delete"); // deletes B, restores A
    assert_eq!(e.path(), "/sd/repo/a.md");
    assert_eq!(e.text(), "AAA"); // A came back from RAM, caret/undo with it
    match &e.take_effects()[..] {
        [Effect::Delete { path, .. }] => assert_eq!(path, "/sd/repo/b.md"),
        other => panic!("expected a single Delete of B, got {other:?}"),
    }
}

#[test]
fn delete_drops_the_file_from_the_palette_list() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/todo.md"]);
    ex(&mut e, "delete"); // notes.md is active
    e.take_effects();
    assert!(!files_vec(&e).contains(&"/sd/repo/notes.md".to_string()));
    e.handle(Key::Palette);
    for c in "md".chars() {
        e.handle(Key::Char(c)); // reach the search threshold
    }
    assert_eq!(palette_labels(&e), vec!["repo/todo.md"]); // only the survivor
}

#[test]
fn delete_of_a_local_file_carries_local_scope() {
    let mut e = Editor::with_file("/sd/local/j.md".into(), Scope::Local, "diary".into());
    ex(&mut e, "delete");
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
    let mut e = Editor::new(); // scratch, empty path — nothing on disk to delete
    ex(&mut e, "delete");
    assert!(e.take_effects().is_empty());
    assert_eq!(e.mode(), Mode::Normal);
}

// ---- File palette (Ctrl-P) ----

/// A fresh editor over `/sd/repo/notes.md` with a palette file list.
fn palette_editor(files: &[&str]) -> Editor {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, String::new());
    e.set_file_list(files.iter().map(|s| s.to_string()).collect());
    e
}

/// The palette's current result as display labels, in ranked order.
fn palette_labels(e: &Editor) -> Vec<&str> {
    e.palette_matches().iter().map(|&i| palette_label(e.file_at(i))).collect()
}

/// The interned file list back as owned strings, in base (sorted) order.
fn files_vec(e: &Editor) -> Vec<String> {
    (0..e.file_count()).map(|i| e.file_at(i).to_string()).collect()
}

#[test]
fn fuzzy_score_matches_subsequence_case_insensitively() {
    assert!(fuzzy_score("notes", "repo/notes.md").is_some());
    assert!(fuzzy_score("NOTES", "repo/notes.md").is_some()); // case-insensitive
    assert!(fuzzy_score("rpnm", "repo/notes.md").is_some()); // scattered subsequence
    assert!(fuzzy_score("xyz", "repo/notes.md").is_none()); // not a subsequence
    assert_eq!(fuzzy_score("", "anything"), Some(0)); // empty query matches all
}

#[test]
fn fuzzy_score_space_matches_any_separator() {
    assert!(fuzzy_score("la conv", "repo/la-convergence.md").is_some()); // space finds '-'
    assert!(fuzzy_score("notes md", "repo/notes.md").is_some()); // space finds '.'
    assert!(fuzzy_score("repo notes", "repo/notes.md").is_some()); // space finds '/'
    assert!(fuzzy_score("la conv", "repo/laconvergence.md").is_none()); // still needs a separator
}

#[test]
fn fuzzy_score_ranks_word_boundaries_above_midword() {
    // "no" after the "/" boundary in repo/notes beats a mid-word hit.
    let boundary = fuzzy_score("no", "repo/notes.md").unwrap();
    let midword = fuzzy_score("no", "cannotes.md").unwrap();
    assert!(boundary > midword, "{boundary} !> {midword}");
}

#[test]
fn set_file_list_sorts_and_dedups() {
    let mut e = Editor::new();
    e.set_file_list(vec![
        "/sd/repo/b.md".into(),
        "/sd/repo/a.md".into(),
        "/sd/repo/b.md".into(),
    ]);
    assert_eq!(files_vec(&e), vec!["/sd/repo/a.md", "/sd/repo/b.md"]);
}

#[test]
fn cmd_p_opens_the_palette_and_esc_closes_it() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/todo.md"]);
    e.handle(Key::Palette);
    assert_eq!(e.mode(), Mode::Palette);
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn cmd_p_toggles_the_palette_closed() {
    let mut e = palette_editor(&["/sd/repo/notes.md"]);
    e.handle(Key::Palette);
    e.handle(Key::Palette); // same chord closes
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn cmd_p_opens_the_palette_from_insert_ending_the_session_like_esc() {
    let mut e = typed("hi");
    e.handle(Key::Palette);
    assert_eq!(e.mode(), Mode::Palette);
    assert_eq!(e.caret, 1); // caret dropped onto the last inserted char, as Esc does
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal); // closing lands in Normal, not back in Insert
    assert_eq!(e.text, "hi");
}

#[test]
fn cmd_p_opens_the_palette_from_visual_dropping_the_selection() {
    let mut e = typed("hello");
    e.handle(Key::Escape);
    e.handle(Key::Char('v')); // charwise Visual, anchor set
    assert_eq!(e.mode(), Mode::Visual);
    e.handle(Key::Palette);
    assert_eq!(e.mode(), Mode::Palette);
    assert_eq!(e.visual_anchor, None); // selection gone, as Esc would leave it
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn cmd_p_opens_the_palette_from_visual_line() {
    let mut e = typed("hello");
    e.handle(Key::Escape);
    e.handle(Key::Char('V'));
    assert_eq!(e.mode(), Mode::VisualLine);
    e.handle(Key::Palette);
    assert_eq!(e.mode(), Mode::Palette);
    assert_eq!(e.visual_anchor, None);
}

#[test]
fn cmd_p_opens_the_palette_from_view_mode() {
    let mut e = typed("hello");
    e.handle(Key::Escape);
    e.handle(Key::Char('g'));
    e.handle(Key::Char('r')); // gr — go-read (View)
    assert_eq!(e.mode(), Mode::View);
    e.handle(Key::Palette);
    assert_eq!(e.mode(), Mode::Palette);
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn cmd_p_opens_the_palette_from_command_abandoning_the_line() {
    let mut e = palette_editor(&["/sd/repo/notes.md"]);
    e.handle(Key::Char(':'));
    for c in "del".chars() {
        e.handle(Key::Char(c)); // half-typed command
    }
    e.handle(Key::Palette);
    assert_eq!(e.mode(), Mode::Palette);
    assert_eq!(e.cmdline, ""); // the abandoned `:del` is gone
    assert_eq!(e.palette_query, ""); // and didn't leak into the palette query
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.take_effects().is_empty()); // `:del` never ran
}

#[test]
fn cmd_p_mid_insert_aborts_the_dot_recording() {
    let mut e = typed("ab"); // `i a b` — a dot recording in progress
    e.handle(Key::Palette); // palette trip aborts it
    e.handle(Key::Escape); // back to Normal — would normally complete a recording
    e.handle(Key::Char('.'));
    assert_eq!(e.text, "ab"); // nothing to repeat: no re-insert, no reopened palette
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn a_completed_dot_survives_a_palette_round_trip() {
    let mut e = typed("abc");
    e.handle(Key::Escape); // caret on 'c'
    e.handle(Key::Char('x')); // dot = [x], text "ab"
    e.handle(Key::Palette);
    e.handle(Key::Escape); // palette round trip
    e.handle(Key::Char('.')); // still repeats the x
    assert_eq!(e.text, "a");
}

#[test]
fn typing_filters_and_enter_opens_the_picked_file() {
    let mut e = palette_editor(&[
        "/sd/repo/notes.md",
        "/sd/repo/todo.md",
        "/sd/local/journal.md",
    ]);
    e.handle(Key::Palette);
    for c in "todo".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Normal); // palette closed
    let effs = e.take_effects();
    assert_eq!(kinds(&effs), vec![Kind::Load]);
    match &effs[0] {
        Effect::Load { path, scope } => {
            assert_eq!(path, "/sd/repo/todo.md");
            assert_eq!(*scope, Scope::Tracked);
        }
        other => panic!("expected a Load, got {other:?}"),
    }
}

#[test]
fn opening_a_local_file_from_the_palette_carries_local_scope() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/local/journal.md"]);
    e.handle(Key::Palette);
    for c in "journal".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    match &e.take_effects()[0] {
        Effect::Load { path, scope } => {
            assert_eq!(path, "/sd/local/journal.md");
            assert_eq!(*scope, Scope::Local); // scope derived from the /sd/local path
        }
        other => panic!("expected a Local Load, got {other:?}"),
    }
}

#[test]
fn opening_the_active_file_from_the_palette_is_a_noop() {
    let mut e = palette_editor(&["/sd/repo/notes.md", "/sd/repo/todo.md"]);
    e.handle(Key::Palette);
    for c in "notes".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    // notes.md is already active: no Load, just a closed palette.
    assert!(e.take_effects().is_empty());
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn half_page_keys_move_the_selection_wrapping() {
    let mut e = palette_editor(&["/sd/repo/a.md", "/sd/repo/b.md", "/sd/repo/c.md"]);
    e.handle(Key::Palette);
    for ch in "md".chars() {
        e.handle(Key::Char(ch)); // reach the search threshold: all three match
    }
    assert_eq!(e.palette_sel, 0);
    e.handle(Key::HalfPageDown);
    assert_eq!(e.palette_sel, 1);
    e.handle(Key::HalfPageDown);
    e.handle(Key::HalfPageDown); // wraps past the last row to the top
    assert_eq!(e.palette_sel, 0);
    e.handle(Key::HalfPageUp); // wraps back to the last row
    assert_eq!(e.palette_sel, 2);
}

#[test]
fn ctrl_n_p_navigate_the_palette() {
    let mut e = palette_editor(&["/sd/repo/a.md", "/sd/repo/b.md", "/sd/repo/c.md"]);
    e.handle(Key::Palette);
    for ch in "md".chars() {
        e.handle(Key::Char(ch)); // reach the search threshold: all three match
    }
    e.handle(Key::Down); // Ctrl-n
    assert_eq!(e.palette_sel, 1);
    e.handle(Key::Down);
    e.handle(Key::Down); // wraps past the last row to the top
    assert_eq!(e.palette_sel, 0);
    e.handle(Key::Up); // Ctrl-p wraps back to the last row
    assert_eq!(e.palette_sel, 2);
}

#[test]
fn ctrl_n_p_move_by_a_line_in_normal_mode() {
    // Three lines; caret starts on the last char of line 3 (with_file posture).
    let mut e = Editor::with_file("/sd/repo/n.md".into(), Scope::Tracked, "aa\nbb\ncc".into());
    e.handle(Key::Char('g')); // gg → top (line 1)
    e.handle(Key::Char('g'));
    assert_eq!(e.caret, 0);
    e.handle(Key::Down); // Ctrl-n → line 2
    assert_eq!(e.caret, 3); // start of "bb"
    e.handle(Key::Down); // → line 3
    assert_eq!(e.caret, 6); // start of "cc"
    e.handle(Key::Up); // Ctrl-p → line 2
    assert_eq!(e.caret, 3);
}

#[test]
fn ctrl_n_takes_a_count_in_normal_mode() {
    let mut e = Editor::with_file("/sd/repo/n.md".into(), Scope::Tracked, "aa\nbb\ncc".into());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g')); // top
    e.handle(Key::Char('2'));
    e.handle(Key::Down); // 2<C-n> → down two lines
    assert_eq!(e.caret, 6); // start of "cc"
}

#[test]
fn ctrl_n_p_scroll_in_view_mode() {
    let mut e = Editor::with_file("/sd/repo/n.md".into(), Scope::Tracked, "a\nb\nc\nd".into());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('r')); // gr → View
    assert_eq!(e.mode(), Mode::View);
    let top = e.scroll_top();
    e.handle(Key::Down); // Ctrl-n scrolls like j
    assert_eq!(e.scroll_top(), top + 1);
    e.handle(Key::Up); // Ctrl-p scrolls like k
    assert_eq!(e.scroll_top(), top);
}

#[test]
fn editing_the_query_resets_the_selection_to_the_top() {
    let mut e = palette_editor(&["/sd/repo/a.md", "/sd/repo/b.md"]);
    e.handle(Key::Palette);
    for ch in "md".chars() {
        e.handle(Key::Char(ch)); // reach the search threshold: both match
    }
    e.handle(Key::HalfPageDown);
    assert_eq!(e.palette_sel, 1);
    e.handle(Key::Char('a')); // a query edit resets the selection
    assert_eq!(e.palette_sel, 0);
}

#[test]
fn backspace_on_an_empty_query_closes_the_palette() {
    let mut e = palette_editor(&["/sd/repo/a.md"]);
    e.handle(Key::Palette);
    e.handle(Key::Backspace);
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn short_query_lists_recents_only() {
    let mut e = palette_editor(&["/sd/repo/b.md", "/sd/repo/a.md", "/sd/repo/c.md"]);
    // No opens yet: below the search threshold there is nothing to show.
    assert!(palette_labels(&e).is_empty());
    // Open c.md through the palette; it becomes the recents-only result.
    e.handle(Key::Palette);
    for ch in "c.md".chars() {
        e.handle(Key::Char(ch));
    }
    e.handle(Key::Enter);
    e.take_effects(); // drop the queued Load; we only care about the MRU
    assert_eq!(palette_labels(&e), vec!["repo/c.md"]);
}

#[test]
fn two_char_query_reveals_the_full_file_list() {
    let mut e = palette_editor(&["/sd/repo/b.md", "/sd/repo/a.md", "/sd/repo/c.md"]);
    e.handle(Key::Palette);
    e.handle(Key::Char('m')); // one char: still recents-only (none yet)
    assert!(e.palette_matches().is_empty());
    e.handle(Key::Char('d')); // "md": the full list, fuzzy-ranked
    assert_eq!(palette_labels(&e), vec!["repo/a.md", "repo/b.md", "repo/c.md"]);
}

#[test]
fn recents_float_above_the_full_list_on_a_matching_query() {
    let mut e = palette_editor(&["/sd/repo/b.md", "/sd/repo/a.md", "/sd/repo/c.md"]);
    // Open c.md so it is the MRU head.
    e.handle(Key::Palette);
    for ch in "c.md".chars() {
        e.handle(Key::Char(ch));
    }
    e.handle(Key::Enter);
    e.take_effects();
    // "md" scores the three labels equally; the stable sort keeps the
    // recently-opened c.md in front of the sorted rest.
    e.handle(Key::Palette);
    for ch in "md".chars() {
        e.handle(Key::Char(ch));
    }
    assert_eq!(palette_labels(&e), vec!["repo/c.md", "repo/a.md", "repo/b.md"]);
}

#[test]
fn draw_in_palette_mode_does_not_panic() {
    let mut e = palette_editor(&["/sd/repo/a.md", "/sd/local/j.md"]);
    e.handle(Key::Palette);
    let _ = e.draw(true); // empty query, no recents: "(type to search)"
    e.handle(Key::Char('j')); // one char, still below the threshold
    let _ = e.draw(true);
    e.handle(Key::Char('m')); // at the threshold: the ranked list
    let _ = e.draw(true);
    // Empty file list: the "(no files on card)" path must also be safe.
    let mut empty = Editor::new();
    empty.handle(Key::Palette);
    let _ = empty.draw(true);
}

// ---- Preferences (.typoena.toml) ----

#[test]
fn prefs_default_matches_the_documented_defaults() {
    let p = Prefs::default();
    assert!(p.save_on_idle);
    assert!(p.format_on_save);
    assert!(p.line_numbers);
    assert!(p.open_last_on_boot);
    assert_eq!(p.theme, "light");
    assert_eq!(p.auto_sync, "10m");
}

#[test]
fn prefs_parse_falls_back_to_defaults_for_missing_keys() {
    // Only one key present; the rest stay at their defaults.
    let p = Prefs::parse("line_numbers = false\n");
    assert!(!p.line_numbers);
    assert!(p.save_on_idle); // untouched -> default
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
    assert!(p.save_on_idle); // "yes" isn't a TOML bool -> default (true)
}

#[test]
fn prefs_to_toml_round_trips_through_parse() {
    let p = Prefs {
        save_on_idle: false,
        format_on_save: true,
        line_numbers: false,
        open_last_on_boot: false,
        theme: "dark".into(),
        auto_sync: "5m".into(),
    };
    assert_eq!(Prefs::parse(&p.to_toml()), p);
}

#[test]
fn prefs_parse_reads_theme_and_auto_sync_strings() {
    let p = Prefs::parse("theme = \"dark\"\nauto_sync = \"15m\"\n");
    assert_eq!(p.theme, "dark");
    assert_eq!(p.auto_sync, "15m");
}

#[test]
fn empty_prefs_file_yields_defaults() {
    assert_eq!(Prefs::parse(""), Prefs::default());
}

// ---- line_numbers pref (live gutter toggle) ----

#[test]
fn line_numbers_off_reclaims_the_gutter_columns() {
    let mut e = Editor::with_text("one\ntwo\nthree".into());
    assert!(e.text_cols() < WRITE_COLS); // gutter present by default
    e.prefs.line_numbers = false;
    assert_eq!(e.gutter_cols(), 0);
    assert_eq!(e.text_cols(), WRITE_COLS); // full width reclaimed
}

#[test]
fn draw_with_line_numbers_off_does_not_panic() {
    // The `gutter - 1` field width would underflow if unguarded.
    let mut e = Editor::with_text("alpha\nbeta\ngamma".into());
    e.prefs.line_numbers = false;
    let _ = e.draw(true);
}

// ---- Palette command mode (`>`) ----

/// Open the palette and type `query` (so `>...` enters command mode).
fn palette_type(files: &[&str], query: &str) -> Editor {
    let mut e = palette_editor(files);
    e.handle(Key::Palette);
    for c in query.chars() {
        e.handle(Key::Char(c));
    }
    e
}

#[test]
fn leading_gt_switches_the_palette_to_command_mode() {
    let e = palette_type(&["/sd/repo/notes.md"], ">");
    assert!(e.palette_command_mode());
    // Every prefs command is offered on a bare `>`.
    assert_eq!(e.palette_command_matches().len(), PALETTE_CMDS.len());
}

#[test]
fn backspacing_the_gt_returns_to_file_mode() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">");
    assert!(e.palette_command_mode());
    e.handle(Key::Backspace);
    assert!(!e.palette_command_mode());
    assert_eq!(e.mode(), Mode::Palette); // still open, just file mode again
}

#[test]
fn command_filter_fuzzy_matches_the_label() {
    let e = palette_type(&["/sd/repo/notes.md"], ">line");
    let matches = e.palette_command_matches();
    assert_eq!(matches.len(), 1);
    assert_eq!(PALETTE_CMDS[matches[0]], PaletteCmd::LineNumbers);
}

#[test]
fn command_label_reflects_current_pref_state() {
    let e = palette_editor(&["/sd/repo/notes.md"]);
    assert_eq!(e.command_label(PaletteCmd::LineNumbers), "line numbers: on");
}

#[test]
fn running_a_command_toggles_the_pref_live_and_queues_a_save_prefs() {
    // >line<Enter> flips line_numbers off, in-core, and asks the host to persist.
    let mut e = palette_type(&["/sd/repo/notes.md"], ">line");
    assert!(e.prefs().line_numbers);
    e.handle(Key::Enter);
    assert!(!e.prefs().line_numbers); // applied live
    assert_eq!(e.mode(), Mode::Palette); // stays open for more toggles
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SavePrefs]);
}

#[test]
fn command_mode_stays_open_across_multiple_toggles() {
    // Flip line numbers, then retype the query for save-on-idle and flip that,
    // without the palette closing between them; each toggle persists. Navigate
    // by fuzzy filter, not registry position.
    let mut e = palette_type(&["/sd/repo/notes.md"], ">line");
    let before = e.prefs().line_numbers;
    e.handle(Key::Enter);
    assert_eq!(e.prefs().line_numbers, !before); // applied live
    assert_eq!(e.mode(), Mode::Palette); // stays open
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SavePrefs]);
    // Still open: retype the query for a different pref (">idle" is unique to
    // "save on idle").
    for _ in 0.."line".len() {
        e.handle(Key::Backspace);
    }
    for c in "idle".chars() {
        e.handle(Key::Char(c));
    }
    let before = e.prefs().save_on_idle;
    e.handle(Key::Enter);
    assert_eq!(e.prefs().save_on_idle, !before);
    assert_eq!(e.mode(), Mode::Palette);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SavePrefs]);
}

#[test]
fn save_prefs_carries_the_serialized_prefs() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">line");
    e.handle(Key::Enter);
    let effects = e.take_effects();
    let Effect::SavePrefs { contents } = &effects[0] else {
        panic!("expected SavePrefs, got {effects:?}");
    };
    // The written TOML reflects the toggled state and round-trips.
    assert!(!Prefs::parse(contents).line_numbers);
}

#[test]
fn running_a_command_confirms_the_new_state_on_the_snackbar() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">save");
    e.handle(Key::Enter);
    // save_on_idle default true -> off; the notice names the new state.
    assert_eq!(e.notice.as_deref(), Some("save on idle: off - saved"));
}

// ---- Preset (non-boolean) prefs: rotate through options on Enter ----

#[test]
fn next_option_rotates_and_wraps() {
    assert_eq!(next_option("light", &THEME_OPTIONS), "dark");
    assert_eq!(next_option("dark", &THEME_OPTIONS), "light"); // wraps
    assert_eq!(next_option("10m", &AUTO_SYNC_OPTIONS), "15m");
    assert_eq!(next_option("30m", &AUTO_SYNC_OPTIONS), "2m"); // wraps
}

#[test]
fn next_option_snaps_an_unknown_value_to_the_head() {
    // A hand-typed value outside the preset list lands on the first option.
    assert_eq!(next_option("sepia", &THEME_OPTIONS), "light");
    assert_eq!(next_option("7m", &AUTO_SYNC_OPTIONS), "2m");
}

#[test]
fn theme_command_label_reflects_the_current_value() {
    let e = palette_editor(&["/sd/repo/notes.md"]);
    assert_eq!(e.command_label(PaletteCmd::Theme), "theme: light");
    assert_eq!(e.command_label(PaletteCmd::AutoSync), "auto sync: 10m");
}

#[test]
fn running_the_theme_command_rotates_the_preset_and_queues_a_save_prefs() {
    // >theme<Enter> flips light -> dark, in-core, and asks the host to persist.
    let mut e = palette_type(&["/sd/repo/notes.md"], ">theme");
    assert_eq!(e.prefs().theme, "light");
    e.handle(Key::Enter);
    assert_eq!(e.prefs().theme, "dark"); // applied live
    assert_eq!(e.mode(), Mode::Palette); // stays open for more changes
    assert_eq!(kinds(&e.take_effects()), vec![Kind::SavePrefs]);
    e.handle(Key::Enter);
    assert_eq!(e.prefs().theme, "light"); // rotates back, wrapping
}

#[test]
fn running_the_auto_sync_command_walks_the_interval_presets() {
    // Default 10m; Enter rotates 10m -> 15m -> 30m -> 2m (wrap).
    let mut e = palette_type(&["/sd/repo/notes.md"], ">auto");
    assert_eq!(e.prefs().auto_sync, "10m");
    for expected in ["15m", "30m", "2m"] {
        e.handle(Key::Enter);
        assert_eq!(e.prefs().auto_sync, expected);
    }
}

#[test]
fn dark_theme_inverts_the_whole_frame() {
    // The dark frame is the exact bitwise inverse of the light one.
    let mut e = Editor::with_text("hello world".into());
    e.caret = 0;
    let light = e.draw(true).bytes().to_vec();
    e.prefs.theme = "dark".into();
    let dark = e.draw(true).bytes().to_vec();
    assert_eq!(light.len(), dark.len());
    assert!(light.iter().zip(&dark).all(|(l, d)| *l == !*d));
}

#[test]
fn a_no_match_command_query_runs_nothing() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">zzzzz");
    assert!(e.palette_command_matches().is_empty());
    let before = e.prefs().clone();
    e.handle(Key::Enter);
    assert_eq!(e.prefs(), &before); // nothing toggled
    assert!(e.take_effects().is_empty()); // nothing queued
    assert_eq!(e.mode(), Mode::Palette); // stays open so the query can be fixed
}

#[test]
fn ctrl_n_moves_the_command_selection_wrapping() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">");
    for _ in 0..PALETTE_CMDS.len() - 1 {
        e.handle(Key::Down); // Ctrl-N down to the last command
    }
    assert_eq!(e.palette_sel, PALETTE_CMDS.len() - 1);
    e.handle(Key::Down); // wraps back to the top
    assert_eq!(e.palette_sel, 0);
    e.handle(Key::Up); // and Ctrl-P wraps back to the bottom
    assert_eq!(e.palette_sel, PALETTE_CMDS.len() - 1);
}

#[test]
fn settings_command_opens_the_palette_in_command_mode() {
    let (e, _) = command("settings");
    assert_eq!(e.mode(), Mode::Palette);
    assert!(e.palette_command_mode()); // dropped straight into `>` mode
    assert_eq!(e.palette_command_matches().len(), PALETTE_CMDS.len());
}

#[test]
fn draw_in_command_mode_does_not_panic() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">");
    let _ = e.draw(true);
    let mut none = palette_type(&["/sd/repo/notes.md"], ">zzzzz"); // "(no command)"
    let _ = none.draw(true);
}

// ---- `>` command palette generalisation (v0.6) ----

#[test]
fn command_labels_for_the_new_actions() {
    let e = Editor::new();
    assert_eq!(e.command_label(PaletteCmd::NewFile), "new file...");
    assert_eq!(e.command_label(PaletteCmd::Format), "format");
    assert_eq!(e.command_label(PaletteCmd::Publish), "publish");
}

#[test]
fn format_command_runs_and_closes() {
    // A one-shot: it formats the buffer (extra trailing blanks collapse) and
    // closes the palette — and, being an action not a toggle, queues no
    // SavePrefs. `>format` ranks the action above `format on save` (registry
    // order breaks the tie).
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "a\n\n\n".into());
    e.handle(Key::Palette);
    for c in ">format".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.text(), "a\n"); // formatted → not the FormatOnSave toggle
    assert_eq!(e.mode(), Mode::Normal); // one-shot closes
    assert!(!kinds(&e.take_effects()).contains(&Kind::SavePrefs));
}

#[test]
fn publish_command_saves_and_pushes_then_closes() {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "hi".into());
    e.handle(Key::Palette);
    for c in ">publish".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(kinds(&e.take_effects()), vec![Kind::Save, Kind::Publish]);
}

#[test]
fn publish_command_is_unavailable_in_a_local_buffer() {
    let mut e = Editor::with_file("/sd/local/j.md".into(), Scope::Local, "hi".into());
    e.handle(Key::Palette);
    for c in ">publish".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.take_effects().is_empty()); // Local never reaches the remote
}

#[test]
fn new_file_command_opens_the_filename_input_step() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">new");
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Palette); // still open — now the input step
    assert_eq!(e.palette_step, PaletteStep::NewFile);
    assert!(e.palette_query.is_empty()); // the list filter gave way to the name
}

#[test]
fn new_file_step_creates_the_typed_file_and_switches() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">new");
    e.handle(Key::Enter); // → input step
    for c in "draft.md".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Normal); // committed and closed
    assert_eq!(e.path(), "/sd/repo/draft.md"); // scope inherited from notes.md
    assert!(e.dirty()); // a fresh, unsaved file
}

#[test]
fn new_file_step_backspace_returns_to_the_command_list() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">new");
    e.handle(Key::Enter); // input step, empty name
    e.handle(Key::Backspace); // nothing to erase → step back to the `>` list
    assert_eq!(e.palette_step, PaletteStep::List);
    assert!(e.palette_command_mode()); // query restored to ">"
    assert_eq!(e.mode(), Mode::Palette);
}

#[test]
fn new_file_step_empty_enter_stays_in_the_step() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">new");
    e.handle(Key::Enter); // input step
    e.handle(Key::Enter); // empty name → no-op, still awaiting one
    assert_eq!(e.palette_step, PaletteStep::NewFile);
    assert_eq!(e.mode(), Mode::Palette);
    assert_eq!(e.path(), "/sd/repo/notes.md"); // unchanged
}

#[test]
fn draw_in_new_file_step_does_not_panic() {
    let mut e = palette_type(&["/sd/repo/notes.md"], ">new");
    e.handle(Key::Enter);
    let _ = e.draw(true); // empty-name placeholder path
    for c in "draft.md".chars() {
        e.handle(Key::Char(c));
    }
    let _ = e.draw(true); // with a typed name
}

#[test]
fn e_command_is_retired() {
    // `:e` no longer opens files — bare Cmd-P does. `:e foo` is now a no-op.
    let mut e = Editor::with_file("/sd/repo/a.md".into(), Scope::Tracked, "A".into());
    ex(&mut e, "e /sd/repo/b.md");
    assert_eq!(e.path(), "/sd/repo/a.md"); // unchanged
    assert!(e.take_effects().is_empty()); // nothing queued
}

// ---- Snippets (v0.6) ----

fn with_snippets(json: &str) -> Editor {
    let mut e = Editor::new();
    e.set_snippets(Snippets::parse(json).unwrap());
    e
}

#[test]
fn strip_labels_reduces_placeholders_to_bare_stops() {
    assert_eq!(strip_stop_labels("# ${1:Titre}"), "# $1");
    assert_eq!(strip_stop_labels("${2}"), "$2");
    assert_eq!(strip_stop_labels("[$1]($2)$0"), "[$1]($2)$0"); // plain stops untouched
    assert_eq!(strip_stop_labels("price: $ and ${3:x}"), "price: $ and $3"); // lone $ kept
}

#[test]
fn parse_body_extracts_literal_and_visit_order() {
    let (lit, stops) = parse_snippet_body("[$1]($2)$0");
    assert_eq!(lit, "[]()");
    assert_eq!(stops, vec![1, 3, 4]); // $1, $2, then $0 (end) last
}

#[test]
fn parse_body_appends_implicit_final_stop_when_no_zero() {
    let (lit, stops) = parse_snippet_body("# $1\n## $2");
    assert_eq!(lit, "# \n## ");
    assert_eq!(stops, vec![2, 6, 6]); // $1, $2, implicit rest at the end
}

#[test]
fn parse_body_no_stops_has_empty_visit_list() {
    let (lit, stops) = parse_snippet_body("- [ ] ");
    assert_eq!(lit, "- [ ] ");
    assert!(stops.is_empty());
}

#[test]
fn parse_snippets_reads_zed_json_string_and_array_bodies() {
    // r###"…"### so the `"#`/`"##` in the heading bodies don't close the string.
    let json = r###"{
        "Link": { "prefix": "link", "body": "[$1]($2)$0", "description": "Inline link" },
        "Book notes": { "prefix": "booknotes", "body": ["# ${1:Titre}", "## $2"] }
    }"###;
    let s = Snippets::parse(json).unwrap().0;
    assert_eq!(s.len(), 2);
    // BTreeMap parse → sorted by display name ("Book notes" < "Link").
    assert_eq!(s[0].name, "Book notes");
    assert_eq!(s[0].prefix, "booknotes");
    assert_eq!(s[0].body, "# $1\n## $2"); // array joined with \n, label stripped
    assert_eq!(s[0].description, ""); // omitted → empty
    assert_eq!(s[1].name, "Link");
    assert_eq!(s[1].body, "[$1]($2)$0");
    assert_eq!(s[1].description, "Inline link");
}

#[test]
fn parse_snippets_empty_and_malformed() {
    assert!(Snippets::parse("{}").unwrap().0.is_empty());
    assert!(Snippets::parse("").unwrap().0.is_empty()); // empty file = no snippets
    assert!(Snippets::parse(" \n\t").unwrap().0.is_empty()); // whitespace-only too
    assert!(Snippets::parse("{ not json").is_err()); // host logs, boots with none
}

#[test]
fn tab_expands_prefix_and_lands_on_first_stop() {
    let mut e = with_snippets(r#"{ "Link": { "prefix": "link", "body": "[$1]($2)$0" } }"#);
    e.handle(Key::Char('i'));
    for c in "link".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Char('\t')); // expand
    assert_eq!(e.text, "[]()"); // trigger word replaced by the expansion
    assert_eq!(e.caret, 1); // caret on $1
    assert_eq!(e.mode(), Mode::Insert);
    assert_eq!(e.snippet_stops, vec![3, 4]); // $2 then $0 pending
}

#[test]
fn tab_advances_stops_and_typing_shifts_pending_ones() {
    let mut e = with_snippets(r#"{ "Link": { "prefix": "link", "body": "[$1]($2)$0" } }"#);
    e.handle(Key::Char('i'));
    for c in "link".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Char('\t'));
    for c in "url".chars() {
        e.handle(Key::Char(c)); // type at $1
    }
    assert_eq!(e.text, "[url]()");
    assert_eq!(e.snippet_stops, vec![6, 7]); // pending shifted by +3
    e.handle(Key::Char('\t')); // → $2
    assert_eq!(e.caret, 6);
    assert_eq!(e.snippet_stops, vec![7]);
    for c in "http".chars() {
        e.handle(Key::Char(c));
    }
    assert_eq!(e.text, "[url](http)");
    e.handle(Key::Char('\t')); // → $0 (end); session ends
    assert_eq!(e.caret, 11);
    assert!(e.snippet_stops.is_empty());
}

#[test]
fn esc_ends_the_snippet_session() {
    let mut e = with_snippets(r#"{ "Link": { "prefix": "link", "body": "[$1]($2)$0" } }"#);
    e.handle(Key::Char('i'));
    for c in "link".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Char('\t'));
    assert!(!e.snippet_stops.is_empty());
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.snippet_stops.is_empty(), "leaving Insert ends the session");
}

#[test]
fn tab_without_matching_prefix_inserts_spaces() {
    let mut e = with_snippets(r#"{ "Link": { "prefix": "link", "body": "[$1]($2)$0" } }"#);
    e.handle(Key::Char('i'));
    for c in "zzz".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Char('\t'));
    assert!(e.text.starts_with("zzz"));
    assert!(e.text.len() > 3, "tab inserted whitespace, not an expansion");
    assert!(e.snippet_stops.is_empty());
}

#[test]
fn no_stop_snippet_expands_without_a_session() {
    let mut e = with_snippets(r#"{ "Todo": { "prefix": "todo", "body": "- [ ] " } }"#);
    e.handle(Key::Char('i'));
    for c in "todo".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Char('\t'));
    assert_eq!(e.text, "- [ ] ");
    assert_eq!(e.caret, 6); // caret at the end, no session
    assert!(e.snippet_stops.is_empty());
}

#[test]
fn undo_after_expansion_restores_the_trigger_word() {
    let mut e = with_snippets(r#"{ "Link": { "prefix": "link", "body": "[$1]($2)$0" } }"#);
    e.handle(Key::Char('i'));
    for c in "link".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Char('\t'));
    assert_eq!(e.text, "[]()");
    e.handle(Key::Escape);
    e.handle(Key::Char('u')); // undo the whole expansion
    assert_eq!(e.text, "link");
}

// ---- `$` snippet palette ----

// r##"…"## so the `"#` in the `"# $1"` heading body doesn't close the string.
const TWO_SNIPPETS: &str = r##"{
    "Markdown link": { "prefix": "link", "body": "[$1]($2)$0", "description": "Inline link" },
    "Book notes": { "prefix": "booknotes", "body": "# $1", "description": "Reading fiche" }
}"##;

/// Open the palette on an editor with `json`'s snippets loaded, then type `q`.
fn snippet_palette(json: &str, q: &str) -> Editor {
    let mut e = with_snippets(json);
    e.handle(Key::Palette);
    for c in q.chars() {
        e.handle(Key::Char(c));
    }
    e
}

#[test]
fn dollar_switches_the_palette_to_snippet_mode() {
    let e = snippet_palette(TWO_SNIPPETS, "$");
    assert!(e.palette_snippet_mode());
    assert!(!e.palette_command_mode());
    // A bare `$` lists every snippet.
    assert_eq!(e.palette_snippet_matches().len(), 2);
}

#[test]
fn backspacing_the_dollar_returns_to_file_mode() {
    let mut e = snippet_palette(TWO_SNIPPETS, "$");
    assert!(e.palette_snippet_mode());
    e.handle(Key::Backspace);
    assert!(!e.palette_snippet_mode());
    assert_eq!(e.mode(), Mode::Palette); // still open, file mode again
}

#[test]
fn snippet_filter_fuzzy_matches_name_prefix_and_description() {
    // Query the prefix.
    let e = snippet_palette(TWO_SNIPPETS, "$link");
    let m = e.palette_snippet_matches();
    assert_eq!(m.len(), 1);
    assert_eq!(e.snippets[m[0]].name, "Markdown link");
    // Query a word only in the *description* ("fiche") finds the other one.
    let e = snippet_palette(TWO_SNIPPETS, "$fiche");
    let m = e.palette_snippet_matches();
    assert_eq!(m.len(), 1);
    assert_eq!(e.snippets[m[0]].name, "Book notes");
}

#[test]
fn enter_in_snippet_mode_inserts_and_starts_the_session() {
    let mut e = snippet_palette(TWO_SNIPPETS, "$link");
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Insert); // dropped into the buffer at $1
    assert_eq!(e.text, "[]()");
    assert_eq!(e.caret, 1); // on $1
    assert_eq!(e.snippet_stops, vec![3, 4]); // $2 then $0 pending
    // Inserting content closes the palette (unlike a `>` toggle, which stays).
    // A follow-up Esc must leave Insert, not reopen anything.
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn snippet_palette_insertion_is_one_undo_group() {
    let mut e = snippet_palette(TWO_SNIPPETS, "$link");
    e.handle(Key::Enter);
    e.handle(Key::Escape); // end the session, back to Normal
    e.handle(Key::Char('u')); // undo the whole insertion
    assert_eq!(e.text, ""); // buffer restored to its pre-insert state
}

#[test]
fn ctrl_n_wraps_around_the_snippet_result_list() {
    let mut e = snippet_palette(TWO_SNIPPETS, "$");
    e.handle(Key::Down);
    assert_eq!(e.palette_sel, 1); // two snippets → last index is 1
    e.handle(Key::Down); // past the end — wraps to the top
    assert_eq!(e.palette_sel, 0);
    e.handle(Key::Up); // before the top — wraps to the bottom
    assert_eq!(e.palette_sel, 1);
}

#[test]
fn empty_snippet_library_matches_nothing() {
    let mut e = Editor::new(); // no snippets set
    e.handle(Key::Palette);
    e.handle(Key::Char('$'));
    assert!(e.palette_snippet_mode());
    assert!(e.palette_snippet_matches().is_empty());
    e.handle(Key::Enter); // no-op, stays open
    assert_eq!(e.mode(), Mode::Palette);
    let _ = e.draw(true); // "(no snippets)" path must not panic
}

#[test]
fn draw_in_snippet_mode_does_not_panic() {
    let mut e = snippet_palette(TWO_SNIPPETS, "$");
    let _ = e.draw(true);
    let mut filtered = snippet_palette(TWO_SNIPPETS, "$link");
    let _ = filtered.draw(true);
}

// ---- hint-on-pause ----

/// Type `word` into a fresh Insert-mode buffer with the two snippets loaded.
fn typed_in_insert(word: &str) -> Editor {
    let mut e = with_snippets(TWO_SNIPPETS);
    e.handle(Key::Char('i'));
    for c in word.chars() {
        e.handle(Key::Char(c));
    }
    e
}

#[test]
fn pause_hint_names_the_snippet_a_prefix_would_expand() {
    let mut e = typed_in_insert("link");
    assert_eq!(e.snippet_hint, None); // not computed per keystroke
    e.refresh_stats(); // the typing-pause throttle
    assert_eq!(e.snippet_hint.as_deref(), Some("Markdown link"));
}

#[test]
fn pause_hint_is_absent_without_a_matching_prefix() {
    let mut e = typed_in_insert("zz");
    e.refresh_stats();
    assert_eq!(e.snippet_hint, None);
}

#[test]
fn pause_hint_clears_when_leaving_insert() {
    let mut e = typed_in_insert("link");
    e.refresh_stats();
    assert!(e.snippet_hint.is_some());
    e.handle(Key::Escape); // → Normal
    e.refresh_stats(); // the main loop refreshes on non-Insert actions
    assert_eq!(e.snippet_hint, None);
}

#[test]
fn pause_hint_is_absent_during_a_live_session() {
    let mut e = typed_in_insert("link");
    e.handle(Key::Char('\t')); // expand → session live, caret on $1
    assert!(!e.snippet_stops.is_empty());
    e.refresh_stats();
    assert_eq!(e.snippet_hint, None, "mid-session Tab advances, not expands");
}

#[test]
fn draw_with_a_pause_hint_does_not_panic() {
    let mut e = typed_in_insert("link");
    e.refresh_stats();
    assert!(e.snippet_hint.is_some());
    let _ = e.draw(true); // the `» name` panel row must render cleanly
}

// --- `/` search (v0.7) --------------------------------------------------

/// A fresh Normal-mode editor over `text`, caret normalized to 0 with `gg`
/// (a loaded file resumes at its end).
fn over(text: &str) -> Editor {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, text.into());
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    e
}

/// Run `/{pat}<Enter>` on `e`.
fn search(e: &mut Editor, pat: &str) {
    e.handle(Key::Char('/'));
    for c in pat.chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
}

#[test]
fn slash_opens_the_search_prompt_and_esc_cancels() {
    let mut e = over("alpha beta");
    e.handle(Key::Char('/'));
    assert_eq!(e.mode(), Mode::Command); // command-line mode, `/` prompt
    e.handle(Key::Char('b'));
    e.handle(Key::Escape);
    assert_eq!(e.mode(), Mode::Normal);
    assert_eq!(e.caret, 0); // cancelled search never moves the caret
}

#[test]
fn search_jumps_past_the_caret_to_the_next_match() {
    let mut e = over("alpha beta alpha");
    search(&mut e, "alpha");
    // The caret sits on the first "alpha"; search starts *after* it.
    assert_eq!(e.caret, 11);
    assert_eq!(e.mode(), Mode::Normal);
}

#[test]
fn search_wraps_to_the_top_with_a_notice() {
    let mut e = over("alpha beta");
    search(&mut e, "beta"); // caret → 6
    search(&mut e, "alpha"); // no match after 6 → wraps to 0
    assert_eq!(e.caret, 0);
    assert_eq!(e.notice.as_deref(), Some("wrapped"));
}

#[test]
fn search_not_found_keeps_the_caret_and_says_so() {
    let mut e = over("alpha beta");
    search(&mut e, "gamma");
    assert_eq!(e.caret, 0);
    assert_eq!(e.notice.as_deref(), Some("not found: gamma"));
}

#[test]
fn n_repeats_forward_and_wraps() {
    let mut e = over("ab x ab x ab");
    search(&mut e, "ab"); // → 5
    assert_eq!(e.caret, 5);
    e.handle(Key::Char('n')); // → 10
    assert_eq!(e.caret, 10);
    e.handle(Key::Char('n')); // wraps → 0
    assert_eq!(e.caret, 0);
    assert_eq!(e.notice.as_deref(), Some("wrapped"));
}

#[test]
fn capital_n_repeats_backward_and_wraps() {
    let mut e = over("ab x ab x ab");
    search(&mut e, "ab"); // → 5
    e.handle(Key::Char('N')); // back → 0
    assert_eq!(e.caret, 0);
    e.handle(Key::Char('N')); // wraps to the last match → 10
    assert_eq!(e.caret, 10);
    assert_eq!(e.notice.as_deref(), Some("wrapped"));
}

#[test]
fn count_applies_to_n() {
    let mut e = over("ab ab ab ab");
    search(&mut e, "ab"); // → 3
    e.handle(Key::Char('2'));
    e.handle(Key::Char('n')); // 2 matches forward → 9
    assert_eq!(e.caret, 9);
}

#[test]
fn n_without_a_previous_search_says_so() {
    let mut e = over("alpha");
    e.handle(Key::Char('n'));
    assert_eq!(e.caret, 0);
    assert_eq!(e.notice.as_deref(), Some("no previous search"));
}

#[test]
fn empty_slash_repeats_the_last_search() {
    let mut e = over("ab x ab x ab");
    search(&mut e, "ab"); // → 5
    search(&mut e, ""); // bare `/` Enter reuses "ab" → 10
    assert_eq!(e.caret, 10);
}

#[test]
fn lowercase_search_is_case_insensitive() {
    let mut e = over("x Alpha alpha");
    search(&mut e, "alpha");
    assert_eq!(e.caret, 2); // "Alpha" matches "alpha"
}

#[test]
fn smartcase_a_capital_makes_the_search_exact() {
    let mut e = over("x paris Paris");
    search(&mut e, "Paris"); // capital → case-sensitive
    assert_eq!(e.caret, 8); // skips the lowercase "paris"
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    search(&mut e, "paris"); // all-lowercase → insensitive again
    assert_eq!(e.caret, 2);
}

#[test]
fn search_folds_accents_both_ways() {
    let mut e = over("x Été bien"); // 'É' (2 bytes) folds to 'e'
    search(&mut e, "été");
    assert_eq!(e.caret, 2);
    assert_eq!(&e.text[e.caret..e.caret + 5], "Été");
    e.handle(Key::Char('g'));
    e.handle(Key::Char('g'));
    search(&mut e, "ete"); // bare ascii finds the accented word too
    assert_eq!(e.caret, 2);
}

#[test]
fn smartcase_still_folds_accents() {
    let mut e = over("x ete Ete");
    search(&mut e, "Été"); // capital É → case-sensitive, but é still = e
    assert_eq!(e.caret, 6); // matches "Ete", not "ete"
}

#[test]
fn backward_search_is_case_insensitive() {
    let mut e = over("Alpha x alpha");
    search(&mut e, "alpha"); // → 8 (past the caret on 'A')
    assert_eq!(e.caret, 8);
    e.handle(Key::Char('N')); // back → the capitalized one at 0
    assert_eq!(e.caret, 0);
}

#[test]
fn search_lands_on_char_boundaries_in_multibyte_text() {
    let mut e = over("héé ém"); // 'é' is 2 bytes
    search(&mut e, "ém");
    assert_eq!(e.caret, 6); // byte offset of the standalone "ém"
    assert_eq!(&e.text[e.caret..e.caret + 3], "ém");
}

#[test]
fn n_extends_a_visual_selection() {
    let mut e = over("ab x ab");
    search(&mut e, "ab"); // → 5
    e.handle(Key::Char('N')); // back → 0
    e.handle(Key::Char('v')); // Visual, anchor at 0
    e.handle(Key::Char('n')); // extend to the next match
    assert_eq!(e.mode(), Mode::Visual);
    assert_eq!(e.caret, 5);
    e.handle(Key::Char('y')); // yank the span (inclusive of the caret char)
    assert_eq!(e.register, "ab x a");
}

#[test]
fn last_search_survives_a_buffer_switch() {
    let mut e = over("ab x ab");
    search(&mut e, "ab"); // → 5
    e.handle(Key::Char(':'));
    for c in "enew /sd/repo/other.md".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    e.handle(Key::Char('i'));
    for c in "ab cd ab".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Escape);
    e.handle(Key::Char('0'));
    e.handle(Key::Char('n')); // the pattern is editor-global, like vim
    assert_eq!(e.caret, 6);
}

#[test]
fn slash_prompt_draws_without_panic() {
    let mut e = over("alpha");
    e.handle(Key::Char('/'));
    e.handle(Key::Char('a'));
    let _ = e.draw(true);
}

// --- `:gl` pull support (v0.7) ------------------------------------------

#[test]
fn refresh_active_replaces_text_and_resets_state() {
    let mut e = over("old text");
    e.handle(Key::Char('v')); // some transient state to reset
    e.refresh_active("pulled text".into());
    assert_eq!(e.text, "pulled text");
    assert_eq!(e.mode(), Mode::Normal);
    assert!(!e.dirty());
    assert!(e.undo.is_empty()); // old snapshots reference the old text
    assert_eq!(e.path(), "/sd/repo/notes.md"); // same file, new contents
    assert_eq!(e.caret, 10); // boot posture: caret on the last char
}

#[test]
fn joined_file_list_sorts_dedups_and_survives_blank_lines() {
    let mut e = Editor::new();
    e.set_file_list_joined("/sd/repo/b.md\n\n/sd/repo/a.md\n/sd/repo/b.md\n".into());
    assert_eq!(files_vec(&e), vec!["/sd/repo/a.md", "/sd/repo/b.md"]);
    e.add_to_file_list("/sd/repo/ab.md"); // lands between, sorted
    e.add_to_file_list("/sd/repo/a.md"); // already known — no dup
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
    e.handle(Key::Enter); // notes.md parked (clean); b.md active (dirty by design)
    e.handle(Key::Char(':'));
    for c in "enew /sd/repo/c.md".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter); // b.md parked (dirty); c.md active
    assert_eq!(e.parked.len(), 2);
    e.drop_clean_parked();
    let kept: Vec<&str> = e.parked.iter().map(|b| b.path.as_str()).collect();
    assert_eq!(kept, ["/sd/repo/b.md"]); // clean notes.md dropped, dirty b.md kept
}
