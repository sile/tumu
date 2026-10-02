//! Examples of the prefix latch and the commands a latched key names.

mod helpers_tuinix;

use helpers_tuinix::size;
use tuinix::{KeyCode, KeyInput};

/// A `SendToFocused` aimed at the top of the stack, which is where a fresh
/// state's focus sits.
fn send_to_top(key: KeyInput) -> tumu::Action {
    tumu::Action::SendToFocused { index: 0, key }
}

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

    assert_eq!(actions, vec![send_to_top(ch('q'))]);
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

    assert_eq!(actions, vec![send_to_top(ch('a'))]);
    assert!(!state.prefix);
}

#[test]
fn a_bare_prefix_is_sent_to_the_focused_sheet() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    let actions = state.update(tumu::Event::Key(tumu::PREFIX));

    assert_eq!(actions, vec![send_to_top(tumu::PREFIX)]);
    assert!(!state.prefix);
}

#[test]
fn an_unbound_latched_key_does_nothing() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(tumu::PREFIX));

    // `w` is bound to nothing (`z` is the full-screen toggle now, so the test
    // needs a key that stays free).
    let actions = state.update(tumu::Event::Key(ch('w')));

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

/// Drives a latched command: arm the prefix, then press `key`.
fn run_command(state: &mut tumu::State, key: char) -> Vec<tumu::Action> {
    state.update(tumu::Event::Key(tumu::PREFIX));
    state.update(tumu::Event::Key(ch(key)))
}

#[test]
fn the_first_sheet_is_focused_and_the_next_ones_take_it_over() {
    let mut state = state_with(24, 80, 1);
    let first = state.stack.sheets()[0].0;
    assert_eq!(state.focused, Some(first));

    // A new sheet goes on top and takes the keyboard with it, so the key that
    // follows lands on the sheet the user just made.
    run_command(&mut state, 'n');
    let second = state.stack.sheets()[0].0;
    assert_eq!(state.focused, Some(second));
    assert_eq!(
        state.update(tumu::Event::Key(ch('a'))),
        vec![tumu::Action::SendToFocused {
            index: 0,
            key: ch('a'),
        }]
    );
}

#[test]
fn rotating_moves_focus_to_the_sheet_that_comes_up() {
    let mut state = state_with(24, 80, 3);
    let top = state.stack.sheets()[0].0;
    let next = state.stack.sheets()[1].0;

    run_command(&mut state, 'o');

    // The old top went to the bottom; the sheet that was second is now first
    // and is what the keyboard talks to.
    assert_eq!(state.stack.sheets()[0].0, next);
    assert_eq!(state.stack.sheets().last().map(|(id, _)| *id), Some(top));
    assert_eq!(state.focused, Some(next));
    assert_eq!(
        state.update(tumu::Event::Key(ch('a'))),
        vec![tumu::Action::SendToFocused {
            index: 0,
            key: ch('a'),
        }]
    );
}

#[test]
fn rotating_the_bottom_sheet_keeps_the_keyboard_on_it() {
    // Focus follows the rank, not the sheet: the stack comes up under the
    // keyboard. Start with the *bottom* sheet focused instead of the top.
    let mut state = state_with(24, 80, 3);
    let bottom = state.stack.sheets()[2].0;
    state.focused = Some(bottom);

    run_command(&mut state, 'o');

    assert_eq!(state.stack.sheets()[2].0, bottom);
    assert_eq!(state.focused, Some(bottom));
}

#[test]
fn closing_a_sheet_moves_focus_to_the_one_that_took_its_place() {
    let mut state = state_with(24, 80, 3);
    let second = state.stack.sheets()[1].0;
    // Focus the middle sheet and close it.
    state.focused = Some(second);

    run_command(&mut state, 'x');

    // The sheet that was third slid into the middle and is focused.
    let now_second = state.stack.sheets()[1].0;
    assert_eq!(state.focused, Some(now_second));
    assert_ne!(state.focused, Some(second));
}

#[test]
fn closing_the_bottom_sheet_moves_focus_to_the_new_bottom() {
    let mut state = state_with(24, 80, 3);
    let bottom = state.stack.sheets()[2].0;
    state.focused = Some(bottom);

    run_command(&mut state, 'x');

    assert_eq!(state.stack.len(), 2);
    assert_eq!(state.focused, Some(state.stack.sheets()[1].0));
}

#[test]
fn closing_the_last_sheet_leaves_nothing_focused() {
    let mut state = state_with(24, 80, 1);

    run_command(&mut state, 'x');

    assert!(state.stack.is_empty());
    assert_eq!(state.focused, None);
}

#[test]
fn exiting_a_focused_sheet_moves_the_focus() {
    let mut state = state_with(24, 80, 2);
    let top = state.stack.sheets()[0].0;

    state.update(tumu::Event::SessionExited(top));

    assert_eq!(state.stack.len(), 1);
    assert_eq!(state.focused, Some(state.stack.sheets()[0].0));
    assert_ne!(state.focused, Some(top));
}

#[test]
fn full_screen_gives_the_focused_sheet_the_whole_terminal() {
    let mut state = state_with(24, 80, 2);

    let actions = run_command(&mut state, 'z');

    assert!(state.full_screen);
    let regions = state.regions();
    assert_eq!(regions.len(), 2);
    // The focused sheet is on top and takes everything.
    assert_eq!(regions[0].size.rows, 24);
    assert_eq!(regions[0].size.cols, 80);
    // The sheet below is hidden: it has no region at its index.
    assert_eq!(regions[1].size.rows, 0);
    assert_eq!(regions[1].size.cols, 0);
    // Only the visible sheet is resized, and it takes the terminal's grid.
    let grid = termnix::Size {
        rows: std::num::NonZeroU16::new(24).expect("non-zero"),
        cols: std::num::NonZeroU16::new(80).expect("non-zero"),
    };
    assert!(actions.contains(&tumu::Action::ResizeSession {
        index: 0,
        size: grid
    }));
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, tumu::Action::ResizeSession { index: 1, .. }))
    );
}

#[test]
fn full_screen_is_its_own_inverse() {
    let mut state = state_with(24, 80, 2);
    let before = state.regions();

    run_command(&mut state, 'z');
    run_command(&mut state, 'z');

    assert!(!state.full_screen);
    // The layout comes back *derived*, not restored: the same size, order and
    // ratio give the same regions as before.
    assert_eq!(state.regions(), before);
}

#[test]
fn full_screen_zooms_whichever_sheet_is_focused() {
    let mut state = state_with(24, 80, 3);
    let bottom = state.stack.sheets()[2].0;
    state.focused = Some(bottom);

    run_command(&mut state, 'z');

    let regions = state.regions();
    assert_eq!(regions[0].size.rows, 0);
    assert_eq!(regions[1].size.rows, 0);
    assert_eq!(regions[2].size.rows, 24);
    assert_eq!(regions[2].size.cols, 80);
}

#[test]
fn a_resize_keeps_full_screen_and_re_lays_it_out() {
    let mut state = state_with(24, 80, 2);
    run_command(&mut state, 'z');

    // A rotation is a resize: the full-screen view is not a position that a
    // resize could invalidate, so it simply fills the new terminal.
    state.update(tumu::Event::Resize(size(40, 30)));

    assert!(state.full_screen);
    let regions = state.regions();
    assert_eq!(regions[0].size.rows, 40);
    assert_eq!(regions[0].size.cols, 30);
    assert_eq!(regions[1].size.rows, 0);
}

#[test]
fn a_new_sheet_under_full_screen_takes_the_whole_terminal() {
    let mut state = state_with(24, 80, 1);
    run_command(&mut state, 'z');

    run_command(&mut state, 'n');

    // The new sheet is focused, so full screen follows it: one sheet on show,
    // and the other hidden.
    let regions = state.regions();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].size.rows, 24);
    assert_eq!(regions[1].size.rows, 0);
}

#[test]
fn exiting_the_full_screen_sheet_folds_the_stack_back() {
    let mut state = state_with(24, 80, 2);
    run_command(&mut state, 'z');
    let top = state.stack.sheets()[0].0;

    state.update(tumu::Event::SessionExited(top));

    // The sheet that died was the only one on show, so the view folds back
    // rather than zoom the sheet that replaced it.
    assert!(!state.full_screen);
    assert_eq!(state.stack.len(), 1);
    assert_eq!(state.regions()[0].size.rows, 24);
    assert_eq!(state.regions()[0].size.cols, 80);
}

#[test]
fn an_empty_stack_under_full_screen_has_no_regions() {
    let mut state = state_with(24, 80, 1);
    run_command(&mut state, 'z');
    run_command(&mut state, 'x');

    assert!(state.regions().is_empty());
    assert_eq!(state.region_of(0), None);
}

#[test]
fn a_sheet_hidden_by_full_screen_has_an_empty_region() {
    let mut state = state_with(24, 80, 2);
    run_command(&mut state, 'z');

    // The index still names the same sheet whether or not it is on show, so
    // `Some(empty)` means hidden and `None` means no such sheet.
    assert_eq!(state.region_of(1).map(|r| r.size.rows), Some(0));
    assert_eq!(state.region_of(2), None);
}

/// A mouse report of `kind` at a zero-based `position`.
fn mouse(kind: tuinix::MouseInputKind, row: usize, col: usize) -> tuinix::MouseInput {
    tuinix::MouseInput {
        kind,
        position: tuinix::Position { row, col },
        ctrl: false,
        alt: false,
        shift: false,
    }
}

/// A left press at a zero-based `position`.
fn click(row: usize, col: usize) -> tuinix::MouseInput {
    mouse(tuinix::MouseInputKind::LeftPress, row, col)
}

#[test]
fn clicking_a_sheet_moves_focus_to_it() {
    let mut state = state_with(24, 80, 2);
    // An 80-column terminal splits into two 40-column sheets; the top one is
    // focused to start with.
    let top = state.stack.sheets()[0].0;
    let bottom = state.stack.sheets()[1].0;
    assert_eq!(state.focused, Some(top));

    state.update(tumu::Event::Mouse(click(5, 60)));

    assert_eq!(state.focused, Some(bottom));

    // And back again, so the move is not one-way.
    state.update(tumu::Event::Mouse(click(5, 10)));

    assert_eq!(state.focused, Some(top));
}

#[test]
fn clicking_the_focused_sheet_is_a_no_op() {
    let mut state = state_with(24, 80, 2);
    let top = state.stack.sheets()[0].0;
    state.update(tumu::Event::Mouse(click(5, 10)));

    let actions = state.update(tumu::Event::Mouse(click(5, 10)));

    assert_eq!(state.focused, Some(top));
    // Focus moves the keyboard, but it spawns nothing and writes to no child:
    // the frame is the only thing a click changes.
    assert!(actions.is_empty());
}

#[test]
fn clicking_outside_every_sheet_changes_nothing() {
    let mut state = state_with(24, 80, 2);
    let focused = state.focused;

    // One row past the bottom of the terminal.
    let actions = state.update(tumu::Event::Mouse(click(24, 0)));

    assert_eq!(state.focused, focused);
    assert!(actions.is_empty());
}

#[test]
fn a_sheet_hidden_by_full_screen_cannot_be_clicked() {
    let mut state = state_with(24, 80, 2);
    let top = state.stack.sheets()[0].0;
    run_command(&mut state, 'z');

    // This column belongs to the sheet below, which the full-screen view has
    // taken off the screen: the clamp that empties its region must also make it
    // unhittable, or a click on blank rows would steal focus out of the view.
    state.update(tumu::Event::Mouse(click(5, 60)));

    assert_eq!(state.focused, Some(top));
}

#[test]
fn only_a_left_press_moves_focus() {
    let mut state = state_with(24, 80, 2);
    let focused = state.focused;

    for kind in [
        tuinix::MouseInputKind::LeftRelease,
        tuinix::MouseInputKind::RightPress,
        tuinix::MouseInputKind::MiddlePress,
        tuinix::MouseInputKind::Drag,
        tuinix::MouseInputKind::ScrollUp,
        tuinix::MouseInputKind::ScrollDown,
    ] {
        state.update(tumu::Event::Mouse(mouse(kind, 5, 60)));
        assert_eq!(state.focused, focused, "{kind:?} moved focus");
    }
}

#[test]
fn clicking_a_stacked_sheet_uses_the_derived_layout() {
    // A tall terminal stacks vertically, so the split runs down the rows.
    let mut state = state_with(40, 30, 2);
    let top = state.stack.sheets()[0].0;
    let bottom = state.stack.sheets()[1].0;

    // The first two rows are a title and a line of the top sheet's content;
    // the bottom half belongs to the sheet below.
    state.update(tumu::Event::Mouse(click(30, 5)));
    assert_eq!(state.focused, Some(bottom));

    state.update(tumu::Event::Mouse(click(1, 5)));
    assert_eq!(state.focused, Some(top));
}
