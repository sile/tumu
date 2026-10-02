//! Examples of the prefix latch and the commands a latched key names.

mod helpers_tuinix;

use helpers_tuinix::size;
use tuinix::{KeyCode, KeyInput};

fn key(ctrl: bool, code: KeyCode) -> KeyInput {
    KeyInput {
        ctrl,
        alt: false,
        code,
    }
}

fn ch(c: char) -> KeyInput {
    key(false, KeyCode::Char(c))
}

fn ctrl(c: char) -> KeyInput {
    key(true, KeyCode::Char(c))
}

fn state_with(rows: usize, cols: usize, sheets: usize) -> tumu::State {
    let mut state = tumu::State::new(size(rows, cols));
    // A resize over an empty stack spawns the first sheet; pushing the rest
    // through the same event keeps the test off the internal push.
    state.update(tumu::Event::Resize(size(rows, cols)));
    for _ in 1..sheets {
        state.update(tumu::Event::Key(tumu::PREFIX));
        state.update(tumu::Event::Key(ch('n')));
    }
    state
}

#[test]
fn the_prefix_arms_the_latch_and_reports_it() {
    let mut state = state_with(24, 80, 1);

    let actions = state.update(tumu::Event::Key(tumu::PREFIX));

    assert_eq!(actions, vec![tumu::Action::PrefixPending]);
    assert!(state.prefix);
}

#[test]
fn only_the_exact_prefix_key_arms_the_latch() {
    let mut state = state_with(24, 80, 1);

    // `q` without Ctrl is ordinary typing, not the prefix.
    let actions = state.update(tumu::Event::Key(ch('q')));

    assert_eq!(actions, vec![tumu::Action::SendToFocused { key: ch('q') }]);
    assert!(!state.prefix);
}

#[test]
fn the_prefix_is_the_configured_chord() {
    // The one constant the whole prefix hangs on; the test pins it so the
    // `c-q` -> `c-o` change later is a deliberate edit.
    assert_eq!(tumu::PREFIX, ctrl('q'));
}

#[test]
fn an_unlatched_key_is_forwarded_to_the_focused_sheet() {
    let mut state = state_with(24, 80, 1);

    let actions = state.update(tumu::Event::Key(ch('a')));

    assert_eq!(actions, vec![tumu::Action::SendToFocused { key: ch('a') }]);
    assert!(!state.prefix);
}

#[test]
fn a_bare_prefix_is_sent_to_the_focused_sheet() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(tumu::PREFIX));

    assert_eq!(
        actions,
        vec![tumu::Action::SendToFocused { key: tumu::PREFIX }]
    );
    assert!(!state.prefix);
}

#[test]
fn an_unbound_latched_key_does_nothing() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(ch('z')));

    assert!(actions.is_empty());
    assert!(!state.prefix);
}

#[test]
fn an_alt_modified_latched_key_is_unbound() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(KeyInput {
        ctrl: false,
        alt: true,
        code: KeyCode::Char('n'),
    }));

    assert!(actions.is_empty());
    assert!(!state.prefix);
}

#[test]
fn the_new_sheet_command_stacks_a_sheet_on_top() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(ch('n')));

    assert_eq!(state.stack.len(), 2);
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::SpawnSession { index: 0 })
    );
}

#[test]
fn the_close_sheet_command_drops_the_focused_sheet() {
    let mut state = state_with(24, 80, 2);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(ch('x')));

    assert_eq!(state.stack.len(), 1);
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::CloseSession { index: 0 })
    );
}

#[test]
fn the_rotate_command_moves_the_focused_sheet_to_the_bottom() {
    let mut state = state_with(24, 80, 3);
    let top = state.stack.sheets()[0].0;
    state.update(tumu::Event::Key(tumu::PREFIX));

    state.update(tumu::Event::Key(ch('o')));

    assert_eq!(state.stack.sheets().last().map(|(id, _)| *id), Some(top));
}

#[test]
fn rotating_a_single_sheet_does_nothing() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(ch('o')));

    assert!(actions.is_empty());
    assert_eq!(state.stack.len(), 1);
}

#[test]
fn the_quit_command_sets_exit_and_returns_it() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(ch('q')));

    assert!(state.exit);
    assert_eq!(actions, vec![tumu::Action::Exit]);
}

#[test]
fn a_forwarded_key_is_dropped_when_there_is_no_sheet() {
    let mut state = tumu::State::new(size(24, 80));

    let actions = state.update(tumu::Event::Key(ch('a')));

    assert!(actions.is_empty());
}
