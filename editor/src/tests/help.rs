//! The `:help` card: the paged on-device command reference.

use super::*;

/// Screen columns at the card's font — the widest a help line may be.
const SCREEN_COLS: usize = (display::WIDTH / CW as u16) as usize;

fn helped() -> Editor {
    let mut e = Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "hello".into());
    ex(&mut e, "help");
    e
}

#[test]
fn help_command_opens_the_card_at_the_first_page() {
    let mut e = helped();
    assert_eq!(e.mode(), Mode::Help);
    assert_eq!(e.help_page, 0);
    assert!(e.take_effects().is_empty(), ":help must queue nothing");
}

#[test]
fn help_alias_h_opens_the_same_card() {
    let mut e = Editor::new();
    ex(&mut e, "h");
    assert_eq!(e.mode(), Mode::Help);
}

#[test]
fn help_reopens_at_the_first_page() {
    // Leaving on page 3 and coming back must not resume there — the card reads
    // the same every time it is raised.
    let mut e = helped();
    e.handle(Key::Char(' '));
    e.handle(Key::Char('q'));
    ex(&mut e, "help");
    assert_eq!(e.help_page, 0);
}

#[test]
fn help_pages_fit_the_card() {
    // The paging is authored, so nothing computes the fit at runtime: this test
    // is what keeps an added row from falling off the bottom (or the right).
    for (i, page) in HELP_PAGES.iter().enumerate() {
        assert!(page.len() <= HELP_ROWS, "page {i} has {} rows", page.len());
        for &(keys, gloss) in page.iter() {
            let width = if gloss.is_empty() {
                keys.chars().count()
            } else {
                assert!(keys.chars().count() < HELP_GLOSS_COL, "{keys:?} overruns the gloss column");
                HELP_GLOSS_COL + gloss.chars().count()
            };
            assert!(width <= SCREEN_COLS, "{keys:?} / {gloss:?} is {width} cols wide");
        }
    }
}

#[test]
fn help_paging_walks_forward_and_back_and_clamps() {
    let mut e = helped();
    let last = HELP_PAGES.len() - 1;
    for key in [Key::Char(' '), Key::Char('j'), Key::Char('n'), Key::Down] {
        e.handle(key);
    }
    assert_eq!(e.help_page, last, "paging past the end clamps on the last page");
    for key in [Key::Char('k'), Key::Char('p'), Key::Up] {
        e.handle(key);
    }
    assert_eq!(e.help_page, 0, "paging back past the start clamps on the first");
}

#[test]
fn help_card_leaves_on_enter_q_or_esc() {
    for leave in [Key::Enter, Key::Char('q'), Key::Escape] {
        let mut e = helped();
        e.handle(leave);
        assert_eq!(e.mode(), Mode::Normal, "{leave:?} should leave the card");
    }
}

#[test]
fn help_card_swallows_other_keys() {
    let mut e = helped();
    e.handle(Key::Char('x')); // would delete a char in Normal
    assert_eq!(e.mode(), Mode::Help, "a stray key must not leave the card");
    assert_eq!(e.text, "hello", "the buffer stays untouched behind the card");
}

#[test]
fn help_pages_each_paint_something_different() {
    let mut e = helped();
    let mut frames = vec![e.draw(true).bytes().to_vec()];
    for _ in 1..HELP_PAGES.len() {
        e.handle(Key::Char(' '));
        frames.push(e.draw(true).bytes().to_vec());
    }
    for (i, f) in frames.iter().enumerate() {
        for (j, g) in frames.iter().enumerate().skip(i + 1) {
            assert_ne!(f, g, "pages {i} and {j} paint the same card");
        }
    }
}

#[test]
fn help_command_is_in_the_palette() {
    let mut e = palette_editor(&["/sd/repo/notes.md"]);
    e.handle(Key::CommandPalette);
    for c in "help".chars() {
        e.handle(Key::Char(c));
    }
    e.handle(Key::Enter);
    assert_eq!(e.mode(), Mode::Help, "> help must raise the card");
}

