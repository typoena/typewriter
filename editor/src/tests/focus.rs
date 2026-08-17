//! Focus mode (Pomodoro) state machine: the `:focus` toggle, the Rest curtain,
//! and the `:focusdebug` time-base. The block *timer* is host-side (a monotonic
//! clock the pure core can't read), so these drive the Focus→Rest transition
//! directly via `enter_rest` the way the host does; only the editor state is
//! asserted here.

use super::*;

#[test]
fn focus_toggles_session_and_effects() {
    let (mut e, effects) = command("focus");
    assert!(e.pomodoro_on());
    assert_eq!(kinds(&effects), vec![Kind::FocusStart]);
    // Toggling again ends the session.
    ex(&mut e, "focus");
    assert!(!e.pomodoro_on());
    assert_eq!(kinds(&e.take_effects()), vec![Kind::FocusStop]);
}

#[test]
fn enter_rest_masks_and_swallows_keys() {
    let mut e =
        Editor::with_file("/sd/repo/notes.md".into(), Scope::Tracked, "hello world".into());
    ex(&mut e, "focus");
    let _ = e.take_effects();
    e.enter_rest(12, 25);
    assert_eq!(e.mode(), Mode::Rest);
    // Every single key is swallowed — including a bare `c`/`q` and `Esc` (both
    // exits are Ctrl chords now) — so the hidden buffer can't be edited, no key
    // ends the break, and nothing is recorded as an effect.
    let before = e.text().to_string();
    for k in ['c', 'q', 'x', 'i', 'z', 'd', 'd'] {
        e.handle(Key::Char(k));
    }
    e.handle(Key::Escape);
    assert_eq!(e.text(), before);
    assert_eq!(e.mode(), Mode::Rest);
    assert!(e.pomodoro_on());
    assert!(e.take_effects().is_empty());
}

#[test]
fn rest_ctrl_c_starts_next_block() {
    let (mut e, _) = command("focus");
    e.enter_rest(40, 25);
    e.handle(Key::Char('c'));
    assert_eq!(e.mode(), Mode::Rest);
    e.handle(Key::FocusContinue);
    assert_eq!(e.mode(), Mode::Normal);
    assert!(e.pomodoro_on());
    assert_eq!(kinds(&e.take_effects()), vec![Kind::FocusStart]);
}

#[test]
fn rest_ctrl_q_ends_session() {
    let (mut e, _) = command("focus");
    assert!(e.pomodoro_on());
    e.enter_rest(0, 25);
    e.handle(Key::Char('q'));
    assert_eq!(e.mode(), Mode::Rest);
    e.handle(Key::FocusQuit);
    assert_eq!(e.mode(), Mode::Normal);
    assert!(!e.pomodoro_on());
    assert_eq!(kinds(&e.take_effects()), vec![Kind::FocusStop]);
}

#[test]
fn focusdebug_flips_time_base_without_effect() {
    let (mut e, effects) = command("focusdebug");
    assert!(e.focus_debug());
    assert!(effects.is_empty());
    ex(&mut e, "focusdebug");
    assert!(!e.focus_debug());
}
