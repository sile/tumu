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

/// The default prefix key, `C-q`.
///
/// The prefix is a setting now, so there is no one key to name; the tests that
/// exercise the latch use the built-in default and the helper keeps them off
/// the character's spelling.
fn prefix() -> KeyInput {
    tumu::prefix_key(tumu::DEFAULT_PREFIX_CHAR)
}

/// Builds a session id from a plain number, for tests that only need an id.
///
/// A real id is a `NonZeroU64`, which is what makes "no session" unrepresentable
/// rather than a sentinel; tests here do not care about the value, only that
/// two ids can be told apart.
fn session(n: u64) -> tumu::TermnixSessionId {
    tumu::TermnixSessionId::new(std::num::NonZeroU64::new(n).expect("non-zero"))
}

/// Builds a `rows`x`cols` state stacked *horizontally*, with `sheets` sheets.
///
/// Horizontal because the click tests below name columns: at 24x80 two sheets
/// split the columns 40/40. The axis is a setting now, so it is asked for here
/// rather than implied by the size.
fn state_with(rows: usize, cols: usize, sheets: usize) -> tumu::State {
    let mut state =
        tumu::State::new(size(rows, cols)).with_orientation(tumu::Orientation::Horizontal);
    // A resize over an empty stack spawns the first sheet; pushing the rest
    // through the same event keeps the test off the internal push.
    state.update(tumu::Event::Resize(size(rows, cols)));
    for _ in 1..sheets {
        run_command(&mut state, 'c');
    }
    state
}

/// A one-sheet state whose prefix is `Ctrl+c` rather than the default.
fn state_with_prefix_char(c: char) -> tumu::State {
    let mut state = tumu::State::new(size(24, 80)).with_prefix_char(c);
    state.update(tumu::Event::Resize(size(24, 80)));
    state
}

#[test]
fn the_prefix_arms_the_latch_and_reports_it() {
    let mut state = state_with(24, 80, 1);

    let actions = state.update(tumu::Event::Key(prefix()));

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
fn the_default_prefix_is_ctrl_q() {
    // The built-in default; the test pins it so changing it is a deliberate
    // edit. It is only the default -- `--prefix` overrides it at run time.
    assert_eq!(prefix(), ctrl('q'));
    assert_eq!(tumu::DEFAULT_PREFIX_CHAR, 'q');
}

#[test]
fn the_prefix_can_be_set_to_another_character() {
    let mut state = state_with_prefix_char('o');

    // `Ctrl+o` arms the latch...
    let actions = state.update(tumu::Event::Key(ctrl('o')));
    assert_eq!(actions, vec![tumu::Action::PrefixPending]);
    assert!(state.prefix);

    // ...and `Ctrl+q`, the default, is now ordinary typing: it is forwarded to
    // the focused sheet and does not arm anything.
    let mut state = state_with_prefix_char('o');
    let actions = state.update(tumu::Event::Key(ctrl('q')));
    assert_eq!(actions, vec![send_to_top(ctrl('q'))]);
    assert!(!state.prefix);
}

#[test]
fn a_configured_prefix_still_resolves_chords() {
    let mut state = state_with_prefix_char('o');

    state.update(tumu::Event::Key(ctrl('o')));
    let actions = state.update(tumu::Event::Key(ch('c')));

    // `prefix c` makes a sheet whatever the prefix character is: the chords are
    // the unmodified keys and the prefix only arms the latch.
    assert!(
        matches!(
            actions.first(),
            Some(&tumu::Action::SpawnSession {
                index: 0,
                program: tumu::Program::Shell,
                // The existing sheet has no session in a unit test, so there is
                // nothing to inherit a directory from.
                cwd_from: None
            })
        ),
        "the chord resolved through the configured prefix, got {actions:?}"
    );
}

#[test]
fn the_prefix_character_does_not_shadow_a_chord() {
    // With the prefix on `q`, the latch and a chord that uses `q` would share a
    // character; they are still different keys, because the prefix is `Ctrl+q`
    // and a chord is bare `q`. This is the namespace the two live in.
    //
    // The chord here is `c` (new sheet), because `q` is no longer a *prefix*
    // chord -- it moved behind the extended menu -- but the point is the same:
    // the latch and the chord are told apart by the Ctrl, whatever character
    // each carries.
    let mut state = state_with_prefix_char('q');

    state.update(tumu::Event::Key(ctrl('q')));
    let actions = state.update(tumu::Event::Key(ch('c')));

    assert!(
        matches!(actions.first(), Some(&tumu::Action::SpawnSession { .. })),
        "the bare chord reached the table, got {actions:?}"
    );
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
    state.update(tumu::Event::Key(prefix()));

    let actions = state.update(tumu::Event::Key(prefix()));

    assert_eq!(actions, vec![send_to_top(prefix())]);
    assert!(!state.prefix);
}

#[test]
fn an_unbound_latched_key_does_nothing() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(prefix()));

    // `w` is bound to nothing (`z` is the full-screen toggle now, so the test
    // needs a key that stays free).
    let actions = state.update(tumu::Event::Key(ch('w')));

    assert!(actions.is_empty());
    assert!(!state.prefix);
}

#[test]
fn an_alt_modified_latched_key_is_unbound() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(prefix()));

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

    let actions = run_command(&mut state, 'c');

    assert_eq!(state.stack.len(), 2);
    assert!(
        matches!(
            actions.first(),
            Some(&tumu::Action::SpawnSession {
                index: 0,
                program: tumu::Program::Shell,
                // No session is attached to the existing sheet in a unit test,
                // so it has no directory to inherit.
                cwd_from: None
            })
        ),
        "the first action spawns a shell in the new top sheet, got {actions:?}"
    );
}

#[test]
fn a_new_sheet_inherits_the_focused_sheets_session() {
    // The sheet that is focused when the new one is asked for is where the new
    // child starts. The session is named, not the index: the push moves the
    // focused sheet off the top, so an index taken before it would name the new
    // sheet by the time the edge spawned anything.
    let mut state = state_with(24, 80, 1);
    let source = session(7);
    state.stack.attach_session(0, source);

    let actions = run_command(&mut state, 'c');

    let spawn = actions
        .iter()
        .find_map(|action| match action {
            tumu::Action::SpawnSession { cwd_from, .. } => Some(cwd_from),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a new sheet spawns a session, got {actions:?}"));
    assert_eq!(
        *spawn,
        Some(source),
        "the new sheet inherits the focused sheet's session, got {actions:?}"
    );
}

#[test]
fn a_new_sheet_with_nothing_focused_has_nothing_to_inherit() {
    // With no sheet focused there is no session to start from, and the new
    // child keeps the edge's own working directory.
    let mut state = tumu::State::new(size(24, 80));
    state.update(tumu::Event::Resize(size(24, 80)));
    // The resize spawned the first sheet but no session is attached in a unit
    // test; focus is on that sheet and it holds no session.
    assert_eq!(state.focused_index(), Some(0));

    let actions = run_command(&mut state, 'c');

    let spawn = actions
        .iter()
        .find_map(|action| match action {
            tumu::Action::SpawnSession { cwd_from, .. } => Some(cwd_from),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a new sheet spawns a session, got {actions:?}"));
    assert_eq!(*spawn, None, "nothing to inherit, got {actions:?}");
}

#[test]
fn the_diff_sheet_command_runs_the_diff_browser_in_a_new_top_sheet() {
    let mut state = state_with(24, 80, 1);

    let actions = run_command(&mut state, 'd');

    // The tool sheet is a sheet like any other: pushed on top, focused, and
    // spawned through the one `SpawnSession` action. Only the Program differs
    // from `prefix c`.
    assert_eq!(state.stack.len(), 2);
    assert_eq!(state.focused, Some(state.stack.sheets()[0].0));
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::SpawnSession {
            index: 0,
            program: tumu::Program::Diff,
            cwd_from: None
        }),
        "the first action spawns the diff browser in the new top sheet, got {actions:?}"
    );
    // The sheets under it re-derive their regions, exactly as for a shell sheet.
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, tumu::Action::ResizeSession { .. })),
        "the stack is laid out around the new sheet, got {actions:?}"
    );
}

#[test]
fn the_grep_sheet_command_runs_the_grep_browser_in_a_new_top_sheet() {
    let mut state = state_with(24, 80, 1);

    let actions = run_command(&mut state, 's');

    assert_eq!(state.stack.len(), 2);
    assert_eq!(state.focused, Some(state.stack.sheets()[0].0));
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::SpawnSession {
            index: 0,
            program: tumu::Program::Grep,
            cwd_from: None
        }),
        "the first action spawns the grep browser in the new top sheet, got {actions:?}"
    );
}

#[test]
fn the_tool_sheets_are_told_apart_from_each_other_and_from_the_shell() {
    // `d`, `s`, and `c` are three ways to add a sheet and differ only in what
    // runs in it, so the three programs must stay distinct: a shared variant
    // would make `prefix s` open `mamediff`.
    assert_ne!(tumu::Program::Diff, tumu::Program::Grep);
    assert_ne!(tumu::Program::Diff, tumu::Program::Shell);
    assert_ne!(tumu::Program::Grep, tumu::Program::Shell);
}

#[test]
fn a_tool_sheet_can_be_opened_on_an_empty_stack() {
    // The tool commands add a sheet like `prefix c` does, so -- unlike the
    // commands that act on the focused sheet -- they work when there is none.
    let mut state = tumu::State::new(size(24, 80));

    let actions = run_command(&mut state, 'd');

    assert_eq!(state.stack.len(), 1);
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::SpawnSession {
            index: 0,
            program: tumu::Program::Diff,
            cwd_from: None
        }),
        "an empty stack still gets a diff sheet, got {actions:?}"
    );
}

#[test]
fn the_close_sheet_command_drops_the_focused_sheet() {
    let mut state = state_with(24, 80, 2);
    // Attach a session to the focused (top) sheet, so closing it has a session
    // to name. A sheet whose session has not been spawned yet closes silently.
    let top = session(7);
    state.stack.attach_session(0, top);
    state.update(tumu::Event::Key(prefix()));

    // Closing is behind the extended menu: `x` opens it, `k` picks close.
    state.update(tumu::Event::Key(ch('x')));
    let actions = state.update(tumu::Event::Key(ch('k')));

    assert_eq!(state.stack.len(), 1);
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::CloseSession { session: top })
    );
}

#[test]
fn closing_a_sheet_names_the_session_it_closed_not_the_one_that_shifted_up() {
    // The bug this guards: the session used to be named by the *index* of the
    // sheet, resolved after the sheet was removed. Closing the top sheet would
    // then name index 0, which is now the sheet that shifted up -- and the edge
    // would close *that* sheet's session instead. Naming the session directly is
    // what keeps the two apart.
    let mut state = state_with(24, 80, 2);
    let top = session(1);
    let below = session(2);
    state.stack.attach_session(0, top);
    state.stack.attach_session(1, below);
    state.update(tumu::Event::Key(prefix()));

    // The close is `x` then `k`, as above: the menu is what keeps the session
    // named before the sheet is removed.
    state.update(tumu::Event::Key(ch('x')));
    let actions = state.update(tumu::Event::Key(ch('k')));

    assert_eq!(
        actions.first(),
        Some(&tumu::Action::CloseSession { session: top }),
        "the closed session is the one that was focused, not the survivor"
    );
    assert_eq!(
        state.stack.sheets()[0].1.session,
        Some(below),
        "the survivor keeps its session"
    );
}

#[test]
fn the_focus_next_command_steps_down_the_stack() {
    let mut state = state_with(24, 80, 3);
    // Focus is on the top sheet to start with.
    let middle = state.stack.sheets()[1].0;
    let bottom = state.stack.sheets()[2].0;

    let actions = run_command(&mut state, 'n');
    assert_eq!(state.focused, Some(middle), "one step lands on the middle");
    assert!(
        actions.is_empty(),
        "stepping moves the focus, not the layout: nothing to resize"
    );

    run_command(&mut state, 'n');
    assert_eq!(
        state.focused,
        Some(bottom),
        "a second step lands on the bottom"
    );
}

#[test]
fn the_focus_previous_command_steps_up_the_stack() {
    let mut state = state_with(24, 80, 3);
    // Start at the bottom, so both steps go somewhere.
    let bottom = state.stack.sheets()[2].0;
    let middle = state.stack.sheets()[1].0;
    let top = state.stack.sheets()[0].0;
    state.focused = Some(bottom);

    run_command(&mut state, 'p');
    assert_eq!(state.focused, Some(middle));

    run_command(&mut state, 'p');
    assert_eq!(state.focused, Some(top));
}

#[test]
fn a_focus_step_wraps_at_the_ends_of_the_stack() {
    // The step wraps, so the keyboard cycles through the stack: `p` on the top
    // sheet comes around to the bottom, and `n` on the bottom comes around to
    // the top. Nothing stops at an end, so no sheet is unreachable by stepping.
    let mut state = state_with(24, 80, 3);
    let top = state.stack.sheets()[0].0;
    let bottom = state.stack.sheets()[2].0;

    // `p` from the top wraps to the bottom.
    assert_eq!(state.focused, Some(top), "start on the top");
    run_command(&mut state, 'p');
    assert_eq!(state.focused, Some(bottom), "`p` from the top wraps");

    // `n` from the bottom wraps to the top.
    run_command(&mut state, 'n');
    assert_eq!(state.focused, Some(top), "`n` from the bottom wraps");
}

#[test]
fn stepping_around_the_stack_reaches_every_sheet() {
    // Three sheets, three `n` steps: each lands on a different sheet, and the
    // third returns to the one the first started from. That is what the wrap is
    // for -- the whole stack is reachable by holding the step key.
    let mut state = state_with(24, 80, 3);
    let start = state.focused;
    assert_eq!(state.stack.len(), 3);

    let mut seen = std::collections::BTreeSet::new();
    let mut third = None;
    for step in 0..3 {
        run_command(&mut state, 'n');
        seen.insert(state.focused);
        if step == 2 {
            third = state.focused;
        }
    }

    assert_eq!(seen.len(), 3, "three steps visit three sheets");
    assert_eq!(third, start, "the third step returns to the start");
}

#[test]
fn a_focus_step_on_one_sheet_lands_back_on_it() {
    // A single sheet is a ring of one, so the wrap keeps focus where it is
    // rather than stepping off the stack.
    let mut state = state_with(24, 80, 1);
    let only = state.stack.sheets()[0].0;

    run_command(&mut state, 'n');
    assert_eq!(state.focused, Some(only));
    run_command(&mut state, 'p');
    assert_eq!(state.focused, Some(only));
}

#[test]
fn a_focus_step_on_an_empty_stack_does_nothing() {
    let mut state = tumu::State::new(size(24, 80));

    let actions = run_command(&mut state, 'n');

    assert!(actions.is_empty());
    assert_eq!(state.focused, None);
}

#[test]
fn the_move_to_top_command_lifts_the_focused_sheet_to_the_top() {
    // Focus the bottom sheet and move it to the top: it leaves its place and
    // the two above it slide down, so the order is turned so the moved sheet is
    // first and the rest keep their relative order.
    let mut state = state_with(24, 80, 3);
    let top = state.stack.sheets()[0].0;
    let middle = state.stack.sheets()[1].0;
    let bottom = state.stack.sheets()[2].0;
    state.focused = Some(bottom);

    run_command(&mut state, 't');

    let order: Vec<_> = state.stack.sheets().iter().map(|(id, _)| *id).collect();
    assert_eq!(
        order,
        vec![bottom, top, middle],
        "the focused sheet is first"
    );
    assert_eq!(
        state.focused,
        Some(bottom),
        "the keyboard stays on the sheet that moved"
    );
}

#[test]
fn moving_a_sheet_already_on_top_does_nothing() {
    // The resulting order would be the order already in hand, so the command is
    // a no-op: nothing is resized and the order is unchanged.
    let mut state = state_with(24, 80, 3);
    let order: Vec<_> = state.stack.sheets().iter().map(|(id, _)| *id).collect();
    assert_eq!(
        state.focused,
        Some(order[0]),
        "focus starts on the top sheet"
    );

    let actions = run_command(&mut state, 't');

    assert!(actions.is_empty());
    let after: Vec<_> = state.stack.sheets().iter().map(|(id, _)| *id).collect();
    assert_eq!(after, order);
}

#[test]
fn moving_a_single_sheet_does_nothing() {
    let mut state = state_with(24, 80, 1);

    let actions = run_command(&mut state, 't');

    assert!(actions.is_empty());
    assert_eq!(state.stack.len(), 1);
}

#[test]
fn a_state_carries_the_split_ratio_it_was_given() {
    let state = state_with(24, 80, 1).with_ratio(tumu::Ratio::from_num(3));

    assert_eq!(state.ratio(), tumu::Ratio::from_num(3));
    assert_eq!(state.stack.ratio(), tumu::Ratio::from_num(3));
}

#[test]
fn the_widen_split_command_grows_the_top_sheets_share_and_resizes() {
    // Two sheets at 24 rows: with the default `1 : 1` they are 12/12. A step
    // wider is `2 : 1`, so the top sheet takes 16 and the rest keeps 8.
    let mut state = state_with(24, 80, 2).with_orientation(tumu::Orientation::Vertical);
    state.stack.attach_session(0, session(1));
    state.stack.attach_session(1, session(2));
    let before = state.regions();
    assert_eq!(before[0].size.rows, 12);

    let actions = run_command(&mut state, '+');

    assert_eq!(state.ratio(), tumu::Ratio::from_num(2));
    let after = state.regions();
    assert_eq!(after[0].size.rows, 16, "the top sheet takes more");
    assert_eq!(after[1].size.rows, 8, "the sheet below takes the rest");
    // Every region changed shape, so every session is resized to its content.
    assert_eq!(
        actions,
        vec![
            tumu::Action::ResizeSession {
                index: 0,
                size: termnix::Size {
                    rows: std::num::NonZeroU16::new(15).expect("non-zero"),
                    cols: std::num::NonZeroU16::new(80).expect("non-zero"),
                },
            },
            tumu::Action::ResizeSession {
                index: 1,
                size: termnix::Size {
                    rows: std::num::NonZeroU16::new(7).expect("non-zero"),
                    cols: std::num::NonZeroU16::new(80).expect("non-zero"),
                },
            },
        ]
    );
}

#[test]
fn the_narrow_split_command_shrinks_the_top_sheets_share() {
    // Start wide (`3 : 1`) and step narrower: `2 : 1`, so the top sheet gives
    // rows back to the sheet below.
    let mut state = state_with(24, 80, 2)
        .with_orientation(tumu::Orientation::Vertical)
        .with_ratio(tumu::Ratio::from_num(3));
    assert_eq!(state.regions()[0].size.rows, 18);

    run_command(&mut state, '-');

    assert_eq!(state.ratio(), tumu::Ratio::from_num(2));
    assert_eq!(state.regions()[0].size.rows, 16);
}

#[test]
fn the_split_steps_are_their_own_inverse_in_the_middle_of_the_range() {
    let mut state = state_with(24, 80, 2).with_ratio(tumu::Ratio::from_num(3));
    let before = state.regions();

    run_command(&mut state, '+');
    run_command(&mut state, '-');

    assert_eq!(state.ratio(), tumu::Ratio::from_num(3));
    assert_eq!(state.regions(), before, "a step back restores the layout");
}

#[test]
fn widening_at_the_ceiling_does_nothing() {
    // The share is already as wide as it may go, so the step asks for what is
    // in hand: nothing is resized and the ratio is unchanged.
    let mut state = state_with(24, 80, 2).with_ratio(tumu::Ratio::from_num(tumu::MAX_RATIO_NUM));

    let actions = run_command(&mut state, '+');

    assert!(actions.is_empty());
    assert_eq!(state.ratio(), tumu::Ratio::from_num(tumu::MAX_RATIO_NUM));
}

#[test]
fn narrowing_at_the_floor_does_nothing() {
    let mut state = state_with(24, 80, 2).with_ratio(tumu::Ratio::from_num(tumu::MIN_RATIO_NUM));

    let actions = run_command(&mut state, '-');

    assert!(actions.is_empty());
    assert_eq!(state.ratio(), tumu::Ratio::from_num(tumu::MIN_RATIO_NUM));
}

#[test]
fn resizing_the_split_on_an_empty_stack_does_nothing() {
    let mut state = tumu::State::new(size(24, 80));

    let widen = run_command(&mut state, '+');
    let narrow = run_command(&mut state, '-');

    assert!(widen.is_empty());
    assert!(narrow.is_empty());
    assert_eq!(state.ratio(), tumu::Ratio::DEFAULT);
}

#[test]
fn the_rotate_layout_command_flips_the_axis_and_resizes() {
    let mut state = state_with(24, 80, 1);
    state.stack.attach_session(0, session(1));
    assert_eq!(state.orientation(), tumu::Orientation::Horizontal);

    let actions = run_command(&mut state, 'r');

    assert_eq!(state.orientation(), tumu::Orientation::Vertical);
    // The region's shape changed, so every session is resized to it -- and to
    // the *content* region, not the whole sheet: the border line is not the
    // child's, so a vertical sheet of 24 rows offers its child 23. That is the
    // same size the spawn is given, which is what keeps a resize from disagreeing
    // with the spawn about how tall the grid is.
    assert_eq!(
        actions,
        vec![tumu::Action::ResizeSession {
            index: 0,
            size: termnix::Size {
                rows: std::num::NonZeroU16::new(23).expect("non-zero"),
                cols: std::num::NonZeroU16::new(80).expect("non-zero"),
            },
        }]
    );
}

#[test]
fn rotating_the_layout_is_its_own_inverse() {
    let mut state = state_with(24, 80, 2);
    let before = state.regions();

    run_command(&mut state, 'r');
    run_command(&mut state, 'r');

    assert_eq!(state.orientation(), tumu::Orientation::Horizontal);
    assert_eq!(state.regions(), before, "two flips restore the layout");
}

#[test]
fn a_resize_keeps_the_axis() {
    // The shape no longer decides: a resize re-lays the stack out along the axis
    // it already has, which is the behaviour that replaced the auto-flip.
    let mut state = state_with(24, 80, 2);
    assert_eq!(state.orientation(), tumu::Orientation::Horizontal);

    state.update(tumu::Event::Resize(size(80, 24)));

    assert_eq!(state.orientation(), tumu::Orientation::Horizontal);
    // And the split still runs down the columns of the *narrow* terminal: 24
    // columns halve into 12 each, and each sheet still spans all 80 rows.
    let regions = state.regions();
    assert_eq!(regions[0].size.cols, 12);
    assert_eq!(regions[0].size.rows, 80);
}

#[test]
fn the_quit_command_sets_exit_and_returns_it() {
    let mut state = state_with(24, 80, 1);
    // Quitting is behind the extended menu: `x` opens it, `q` picks quit.
    run_command(&mut state, 'x');

    let actions = state.update(tumu::Event::Key(ch('q')));

    assert!(state.exit);
    assert_eq!(actions, vec![tumu::Action::Exit]);
}

#[test]
fn the_editor_opens_on_the_focused_sheets_session() {
    let mut state = state_with(24, 80, 2);
    let session = session(7);
    state.stack.attach_session(0, session);

    let actions = run_command(&mut state, 'l');

    // The core names the session whose history to show and stops there: reading
    // the text out of a terminal is the edge's, so the action carries the
    // session and not the bytes. The editor sheet goes on top, so the action
    // names row 0 as where its session belongs.
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::OpenEditor { index: 0, session })
    );
}

#[test]
fn opening_the_editor_on_a_sheet_with_no_session_does_nothing() {
    let mut state = state_with(24, 80, 2);
    let before = state.stack.len();

    // A sheet with no session is one whose child has not been spawned yet, or
    // has just exited. There is no history to read and no sheet worth adding,
    // so the command is a no-op rather than an editor on an empty file.
    let actions = run_command(&mut state, 'l');

    assert!(actions.is_empty());
    assert_eq!(state.stack.len(), before, "no sheet was added");
}

#[test]
fn editing_the_scrollback_adds_a_focused_editor_sheet_on_top() {
    let mut state = state_with(24, 80, 2);
    let source = session(7);
    state.stack.attach_session(0, source);
    let before: Vec<tumu::SheetId> = state.stack.sheets().iter().map(|(id, _)| *id).collect();

    let actions = run_command(&mut state, 'l');

    // The editor opens in a sheet of its own, pushed on top like any new sheet,
    // and it takes the focus so the keyboard follows the editor in.
    assert_eq!(
        state.stack.len(),
        before.len() + 1,
        "the editor adds a sheet"
    );
    let editor_sheet = state.stack.sheets()[0].0;
    assert!(
        !before.contains(&editor_sheet),
        "the top sheet is the new editor sheet"
    );
    assert_eq!(state.focused, Some(editor_sheet));
    // The editor sheet names the scrollback to read and the program to run.
    assert_eq!(
        actions.first(),
        Some(&tumu::Action::OpenEditor {
            index: 0,
            session: source
        })
    );
    // The editor opens where the sheet it reads is, so its spawn inherits the
    // focused sheet's session -- the one the test attached above -- not a
    // directory picked out of tumu's own launch place.
    let spawn = actions
        .iter()
        .find_map(|action| match action {
            tumu::Action::SpawnSession {
                index: 0,
                program: tumu::Program::Editor { .. },
                cwd_from,
            } => Some(cwd_from),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the editor sheet spawns the editor, got {actions:?}"));
    assert_eq!(
        *spawn,
        Some(source),
        "the editor inherits the sheet it reads, got {actions:?}"
    );
}

#[test]
fn editing_the_scrollback_follows_focus() {
    let mut state = state_with(24, 80, 3);
    // Focus the bottom sheet, so the session named is not the top of the stack.
    let bottom = state.stack.sheets()[2].0;
    state.focused = Some(bottom);
    let session = session(9);
    state.stack.attach_session(2, session);

    let actions = run_command(&mut state, 'l');

    assert_eq!(
        actions.first(),
        Some(&tumu::Action::OpenEditor { index: 0, session })
    );
}

#[test]
fn a_copy_mode_sheet_is_marked_as_such() {
    let mut state = state_with(24, 80, 2);
    state.stack.attach_session(0, session(7));

    run_command(&mut state, 'l');

    let editor = state.stack.sheets()[0].0;
    assert_eq!(state.focused, Some(editor));
    assert_eq!(
        state.stack.role_of(0),
        Some(tumu::SheetRole::CopyMode),
        "the sheet `prefix l` pushes is a copy-mode sheet"
    );
    // The sheet it was read from stays an ordinary sheet.
    assert_eq!(state.stack.role_of(1), Some(tumu::SheetRole::Normal));
}

#[test]
fn a_scroll_up_from_a_copy_mode_sheet_does_not_open_another() {
    // A wheel *gesture* is many turns, and the turns after the first land on the
    // editor sheet. Until the editor asks for the mouse those turns have nothing
    // to page, so the copy-mode door is the only thing they could do -- and it
    // must stay shut, or the gesture stacks one history per turn.
    let mut state = state_with(24, 80, 2);
    state.stack.attach_session(0, session(7));
    run_command(&mut state, 'l');
    let editor = state.focused;
    let before = state.stack.len();

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollUp,
        5,
        10,
    )));

    assert!(actions.is_empty(), "nothing is opened: {actions:?}");
    assert_eq!(state.stack.len(), before, "no second sheet is added");
    assert_eq!(state.focused, editor, "the focus stays on the editor sheet");
}

#[test]
fn editing_the_scrollback_on_an_empty_stack_does_nothing() {
    let mut state = state_with(24, 80, 1);
    run_ext_close(&mut state);

    let actions = run_command(&mut state, 'l');

    assert!(actions.is_empty());
}

#[test]
fn the_paste_clipboard_command_names_the_focused_sheet() {
    let mut state = state_with(24, 80, 2);

    let actions = run_command(&mut state, 'y');

    // The core names the sheet and stops: the clipboard belongs to the host
    // terminal, which only the edge holds, so the action carries an index and
    // not the bytes.
    assert_eq!(actions, vec![tumu::Action::PasteClipboard { index: 0 }]);
}

#[test]
fn pasting_the_clipboard_leaves_the_stack_alone() {
    let mut state = state_with(24, 80, 2);
    let before: Vec<tumu::SheetId> = state.stack.sheets().iter().map(|(id, _)| *id).collect();
    let focused = state.focused;

    run_command(&mut state, 'y');

    // Pasting is not a view of tumu's own: nothing on the stack moved, focus
    // stayed put, and the loop is not asked to stop.
    let after: Vec<tumu::SheetId> = state.stack.sheets().iter().map(|(id, _)| *id).collect();
    assert_eq!(after, before);
    assert_eq!(state.focused, focused);
    assert!(!state.full_screen);
    assert!(!state.exit);
}

#[test]
fn pasting_the_clipboard_follows_focus() {
    let mut state = state_with(24, 80, 3);
    // Focus the bottom sheet, so the index named is not the top of the stack.
    let bottom = state.stack.sheets()[2].0;
    state.focused = Some(bottom);

    let actions = run_command(&mut state, 'y');

    assert_eq!(actions, vec![tumu::Action::PasteClipboard { index: 2 }]);
}

#[test]
fn pasting_the_clipboard_on_an_empty_stack_does_nothing() {
    let mut state = state_with(24, 80, 1);
    run_ext_close(&mut state);

    let actions = run_command(&mut state, 'y');

    assert!(actions.is_empty());
}

#[test]
fn a_forwarded_key_is_dropped_when_there_is_no_sheet() {
    let mut state = tumu::State::new(size(24, 80));

    let actions = state.update(tumu::Event::Key(ch('a')));

    assert!(actions.is_empty());
}

/// Drives a latched command: arm the prefix, then press `key`.
fn run_command(state: &mut tumu::State, key: char) -> Vec<tumu::Action> {
    state.update(tumu::Event::Key(prefix()));
    state.update(tumu::Event::Key(ch(key)))
}

/// Drives a close: arm the prefix, open the extended menu with `x`, press `k`.
///
/// Closing a sheet used to be one chord (`prefix x`); it is behind the extended
/// menu now, so a test that wants a sheet closed has to go through the menu.
/// The two keys are pressed with no prefix between them: the menu is modal.
fn run_ext_close(state: &mut tumu::State) -> Vec<tumu::Action> {
    state.update(tumu::Event::Key(prefix()));
    state.update(tumu::Event::Key(ch('x')));
    state.update(tumu::Event::Key(ch('k')))
}

#[test]
fn the_first_sheet_is_focused_and_the_next_ones_take_it_over() {
    let mut state = state_with(24, 80, 1);
    let first = state.stack.sheets()[0].0;
    assert_eq!(state.focused, Some(first));

    // A new sheet goes on top and takes the keyboard with it, so the key that
    // follows lands on the sheet the user just made.
    run_command(&mut state, 'c');
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
fn moving_a_sheet_to_the_top_sends_the_keyboard_with_it() {
    // After the move the focused sheet is at index 0, and the next key goes to
    // *it* -- the keyboard moved with the sheet, not to a rank.
    let mut state = state_with(24, 80, 3);
    let bottom = state.stack.sheets()[2].0;
    state.focused = Some(bottom);

    run_command(&mut state, 't');

    assert_eq!(state.stack.sheets()[0].0, bottom);
    assert_eq!(state.focused, Some(bottom));
    assert_eq!(
        state.update(tumu::Event::Key(ch('a'))),
        vec![tumu::Action::SendToFocused {
            index: 0,
            key: ch('a'),
        }]
    );
}

#[test]
fn moving_a_sheet_to_the_top_resizes_the_stack() {
    // The order changed, so every region is re-derived and the sessions are
    // resized to match -- unlike a focus step, which moves only the keyboard.
    let mut state = state_with(24, 80, 2);
    state.stack.attach_session(0, session(1));
    state.stack.attach_session(1, session(2));
    state.focused = Some(state.stack.sheets()[1].0);

    let actions = run_command(&mut state, 't');

    assert!(
        !actions.is_empty(),
        "a reorder resizes: the regions changed"
    );
    assert!(
        actions
            .iter()
            .all(|action| matches!(action, tumu::Action::ResizeSession { .. })),
        "the only work a move causes is resizing: {actions:?}"
    );
}

#[test]
fn closing_a_sheet_moves_focus_to_the_one_that_took_its_place() {
    let mut state = state_with(24, 80, 3);
    let second = state.stack.sheets()[1].0;
    // Focus the middle sheet and close it.
    state.focused = Some(second);

    run_ext_close(&mut state);

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

    run_ext_close(&mut state);

    assert_eq!(state.stack.len(), 2);
    assert_eq!(state.focused, Some(state.stack.sheets()[1].0));
}

#[test]
fn closing_the_last_sheet_leaves_nothing_focused() {
    let mut state = state_with(24, 80, 1);

    run_ext_close(&mut state);

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

    run_command(&mut state, 'c');

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
    run_ext_close(&mut state);

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

#[test]
fn a_bordered_sheet_offers_its_child_one_row_less_than_its_region() {
    // Horizontal, so the border is the last *column*: the content is one column
    // narrower than the sheet, at the sheet's own corner.
    let state = state_with(24, 80, 1);

    let region = state.region_of(0).expect("sheet 0");
    let content = state.content_region_of(0).expect("sheet 0");

    assert_eq!(region.size.cols, 80);
    assert_eq!(content.size.cols, 79);
    assert_eq!(content.size.rows, region.size.rows);
    // The content starts at the sheet's own corner; the border is the column
    // left over at the far end.
    assert_eq!(content.position, region.position);
    assert!(state.sheet_is_bordered(0));
}

#[test]
fn a_full_screen_sheet_offers_its_child_its_whole_region() {
    let mut state = state_with(24, 80, 2);
    run_command(&mut state, 'z');

    // A full-screen sheet draws no border, so its content is the whole region:
    // the child gets every row and column, exactly as it would with no
    // multiplexer in front of it.
    let region = state.region_of(0).expect("sheet 0");
    let content = state.content_region_of(0).expect("sheet 0");
    assert_eq!(content, region);
    assert!(!state.sheet_is_bordered(0));

    // The hidden sheet is still bordered, and its content region is still empty
    // -- nothing is drawn for it either way.
    assert_eq!(state.content_region_of(1).map(|r| r.size.rows), Some(0));
    assert!(state.sheet_is_bordered(1));
}

#[test]
fn a_resize_and_a_spawn_agree_on_a_sheets_content_size() {
    // The seed is what a resize returns, and it names the content region -- the
    // rows above the border -- not the whole sheet. That is deliberate: it is
    // the very size the edge spawns a session at, so a later resize cannot
    // shrink the child by a line the spawn did not take.
    let mut state = state_with(24, 80, 1);
    let grid = termnix::Size {
        rows: std::num::NonZeroU16::new(24).expect("non-zero"),
        cols: std::num::NonZeroU16::new(79).expect("non-zero"),
    };

    let actions = state.update(tumu::Event::Resize(size(24, 80)));

    assert_eq!(
        actions,
        vec![tumu::Action::ResizeSession {
            index: 0,
            size: grid
        }]
    );
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
        tuinix::MouseInputKind::ScrollDown,
    ] {
        state.update(tumu::Event::Mouse(mouse(kind, 5, 60)));
        assert_eq!(state.focused, focused, "{kind:?} moved focus");
    }
    // `ScrollUp` is deliberately absent: with nothing asking for the mouse it
    // opens copy mode rather than moving the focus. That it leaves the focus on
    // the sheet the history was read from is asserted on its own, below.
}

#[test]
fn a_scroll_up_with_nothing_asking_opens_copy_mode() {
    // With no child asking for the mouse, a scroll up is the copy-mode door:
    // the same command `prefix l` runs, so the wheel and the key are two ways
    // into one room. What proves it is the editor sheet the command adds -- and
    // the `OpenEditor` that names the history to read.
    let mut state = state_with(24, 80, 2);
    let session = session(7);
    state.stack.attach_session(0, session);
    let before = state.stack.len();

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollUp,
        5,
        10,
    )));

    assert_eq!(
        actions.first(),
        Some(&tumu::Action::OpenEditor { index: 0, session }),
        "the wheel opens the focused sheet's history: {actions:?}"
    );
    assert_eq!(state.stack.len(), before + 1, "an editor sheet is added");
    // The editor sheet goes on top and takes the focus, exactly as `prefix l`
    // leaves it: the keyboard follows the editor in, so a gesture of many turns
    // does not open a sheet per turn.
    assert_eq!(state.focused, Some(state.stack.sheets()[0].0));
}

#[test]
fn a_scroll_up_with_a_child_asking_is_forwarded_not_turned_into_copy_mode() {
    // The ask decides the meaning of the turn: a pager that wanted the mouse
    // pages on the wheel, and the history is *not* opened on top of it. The
    // report is the child's, and no sheet is added.
    let mut state = state_with(24, 80, 2);
    let session = session(7);
    state.stack.attach_session(0, session);
    let before = state.stack.len();
    state.update(tumu::Event::MouseWanted(true));

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollUp,
        5,
        10,
    )));

    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused { index: 0, .. }]
        ),
        "the turn is the child's: {actions:?}"
    );
    assert_eq!(state.stack.len(), before, "no editor sheet is added");
}

#[test]
fn a_scroll_down_with_nothing_asking_does_nothing() {
    // Only *up* opens copy mode: there is no "below the bottom" to go to, so a
    // down turn with nothing asking is dropped like any other report with nothing
    // behind it. The distinction is the point -- a wheel that opened the history
    // on either direction would add a sheet to every scroll.
    let mut state = state_with(24, 80, 2);
    let session = session(7);
    state.stack.attach_session(0, session);
    let before = state.stack.len();
    let focused = state.focused;

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollDown,
        5,
        10,
    )));

    assert!(actions.is_empty(), "nothing is done: {actions:?}");
    assert_eq!(state.stack.len(), before, "no sheet is added");
    assert_eq!(state.focused, focused, "the focus does not move");
}

#[test]
fn a_scroll_up_with_no_session_to_read_does_nothing() {
    // A sheet whose child has not started yet has no history to open, so the
    // turn is a no-op -- the same answer `prefix l` gives on such a sheet. The
    // wheel must not add an editor sheet on an empty file.
    let mut state = state_with(24, 80, 2);
    let before = state.stack.len();

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollUp,
        5,
        10,
    )));

    assert!(actions.is_empty(), "nothing is done: {actions:?}");
    assert_eq!(state.stack.len(), before, "no sheet is added");
}

#[test]
fn clicking_a_stacked_sheet_uses_the_derived_layout() {
    // A vertical stack, so the split runs down the rows: the top sheet owns the
    // upper half, the sheet below the lower half.
    let mut state = tumu::State::new(size(40, 30)).with_orientation(tumu::Orientation::Vertical);
    state.update(tumu::Event::Resize(size(40, 30)));
    run_command(&mut state, 'c');
    let top = state.stack.sheets()[0].0;
    let bottom = state.stack.sheets()[1].0;

    // A click in the lower half lands on the sheet below; one in the upper half
    // lands on the top sheet.
    state.update(tumu::Event::Mouse(click(30, 5)));
    assert_eq!(state.focused, Some(bottom));

    state.update(tumu::Event::Mouse(click(1, 5)));
    assert_eq!(state.focused, Some(top));
}

#[test]
fn arming_the_prefix_opens_the_menu_and_the_next_key_closes_it() {
    let mut state = state_with(24, 80, 2);
    assert!(!state.menu, "the menu starts closed");

    state.update(tumu::Event::Key(prefix()));
    assert!(state.menu, "the prefix opens the menu");

    // A bound key runs its command and closes the menu.
    state.update(tumu::Event::Key(ch('c')));
    assert!(!state.menu, "the chord closes the menu");
    assert!(!state.prefix, "and the latch too");
}

#[test]
fn an_unbound_key_closes_the_menu_and_does_nothing_else() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(prefix()));
    assert!(state.menu);

    let actions = state.update(tumu::Event::Key(ch('Z')));

    assert!(actions.is_empty(), "an unbound chord emits nothing");
    assert!(!state.menu, "but it dismisses the menu");
    assert!(!state.prefix);
}

#[test]
fn the_ext_menu_opens_on_its_chord_and_stays_open_past_it() {
    let mut state = state_with(24, 80, 2);

    // `x` at the prefix does not close anything: it opens the extended menu.
    run_command(&mut state, 'x');
    assert!(state.ext_menu, "`x` opens the extended menu");
    assert_eq!(state.stack.len(), 2, "and closes no sheet");

    // The menu outlives the key that opened it, unlike the prefix menu which is
    // the face of one latched key.
    assert!(state.ext_menu, "the menu is still open");
}

#[test]
fn a_key_the_ext_menu_does_not_name_dismisses_it_without_running_anything() {
    let mut state = state_with(24, 80, 2);
    run_command(&mut state, 'x');
    assert!(state.ext_menu);

    // `c` is bound *at the prefix* but not in the menu, so in the menu it is a
    // "no": the menu closes and no sheet is made.
    let actions = state.update(tumu::Event::Key(ch('c')));

    assert!(actions.is_empty(), "an unbound menu key emits nothing");
    assert!(!state.ext_menu, "but it dismisses the menu");
    assert_eq!(state.stack.len(), 2, "and makes no sheet");
}

#[test]
fn the_ext_menu_close_runs_and_dismisses_the_menu() {
    let mut state = state_with(24, 80, 2);
    let top = session(9);
    state.stack.attach_session(0, top);
    run_command(&mut state, 'x');

    let actions = state.update(tumu::Event::Key(ch('k')));

    assert_eq!(
        actions.first(),
        Some(&tumu::Action::CloseSession { session: top })
    );
    assert!(!state.ext_menu, "the command closes the menu");
}

#[test]
fn the_ext_menu_quit_sets_exit_and_dismisses_the_menu() {
    let mut state = state_with(24, 80, 1);
    run_command(&mut state, 'x');

    let actions = state.update(tumu::Event::Key(ch('q')));

    assert_eq!(actions, vec![tumu::Action::Exit]);
    assert!(state.exit);
    assert!(!state.ext_menu);
}

#[test]
fn the_prefix_is_inert_while_the_ext_menu_is_open() {
    // The menu is modal: pressing the prefix again does not latch it, it is just
    // another key the menu does not name, so it dismisses the menu and nothing
    // else.
    let mut state = state_with(24, 80, 1);
    run_command(&mut state, 'x');

    let actions = state.update(tumu::Event::Key(prefix()));

    assert!(actions.is_empty());
    assert!(!state.prefix, "the prefix does not latch inside the menu");
    assert!(!state.ext_menu, "and dismisses it");
    assert!(state.focused.is_some(), "no sheet was touched");
}

#[test]
fn the_prefix_again_sends_the_prefix_and_closes_the_menu() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(prefix()));

    let actions = state.update(tumu::Event::Key(prefix()));

    assert_eq!(actions, vec![send_to_top(prefix())]);
    assert!(!state.menu);
}

#[test]
fn clicking_a_menu_row_does_not_run_it() {
    // The menu is chosen with keys, so a click anywhere -- a command row
    // included -- only moves the focus, and the menu stays for the next key.
    // The click lands on the sheet under the box, which is the point: the box is
    // not a target, it is a list.
    let mut state = state_with(40, 80, 2);
    let top = state.stack.sheets()[0].0;
    state.update(tumu::Event::Key(prefix()));
    assert!(state.menu);

    // A row of the box, which is centred over both sheets.
    let row = state
        .menu_layout()
        .expect("the menu fits 40x80")
        .rows
        .first()
        .expect("a header row")
        .region
        .position;
    state.update(tumu::Event::Mouse(click(row.row, row.col)));

    assert!(state.menu, "a click does not dismiss the menu");
    assert_eq!(state.focused, Some(top), "and runs no command either");
    assert_eq!(state.stack.len(), 2, "the stack is untouched");
}

#[test]
fn a_click_on_the_focused_sheets_border_is_a_no_op() {
    // The border carries the command label, but the label is not a target: the
    // menu is the prefix's, so a click on the line only lands on the sheet.
    // A tall terminal stacks vertically, so the focused border is its last row.
    let mut state = state_with(40, 30, 1);
    let region = state.region_of(0).expect("the sheet's region");
    let focused = state.focused;

    state.update(tumu::Event::Mouse(click(
        region.position.row + region.size.rows - 1,
        region.position.col + 2,
    )));

    assert!(!state.menu, "a border click does not open the menu");
    assert_eq!(state.focused, focused, "and does not move the focus");
}

#[test]
fn the_focused_sheets_child_asking_for_the_mouse_is_recorded() {
    let mut state = state_with(24, 80, 1);
    assert!(!state.mouse_wanted(), "nothing asked at startup");

    state.update(tumu::Event::MouseWanted(true));
    assert!(state.mouse_wanted());

    // And back, because a child that stops asking is as much a fact as one that
    // starts. Nothing about the layout moves either way.
    let actions = state.update(tumu::Event::MouseWanted(false));
    assert!(actions.is_empty(), "the ask implies no I/O");
    assert!(!state.mouse_wanted());
}

#[test]
fn a_child_that_asked_for_the_mouse_gets_a_report_inside_its_own_region() {
    let mut state = state_with(24, 80, 2);
    state.update(tumu::Event::MouseWanted(true));

    // The click is inside the focused sheet (the top one, columns 0..40). With
    // the child asking, it is the child's report and the focus stays where it
    // is: a program tracking the mouse is tracking it inside its own grid, so a
    // click on the sheet it runs in is a click on the program.
    let actions = state.update(tumu::Event::Mouse(click(5, 10)));

    assert_eq!(state.focused, Some(state.stack.sheets()[0].0));
    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused {
                index: 0,
                held: None,
                ..
            }]
        ),
        "the report goes to the focused sheet: {actions:?}"
    );
}

#[test]
fn a_click_outside_a_mouse_tracking_child_moves_the_focus_instead() {
    let mut state = state_with(24, 80, 2);
    state.update(tumu::Event::MouseWanted(true));

    // The click is in the sheet *below* (columns 40..80), which is outside the
    // focused child's region. The child's grid is the whole of what it can have
    // a report about, so a click on its neighbour is tumu's: the focus moves and
    // nothing is forwarded. This is how the user leaves a tracking program.
    let actions = state.update(tumu::Event::Mouse(click(5, 60)));

    assert_eq!(state.focused, Some(state.stack.sheets()[1].0));
    assert!(actions.is_empty(), "nothing is forwarded: {actions:?}");
}

#[test]
fn a_drag_that_leaves_a_mouse_tracking_child_still_reaches_it() {
    let mut state = state_with(24, 80, 2);
    state.update(tumu::Event::MouseWanted(true));

    // A press inside the focused region arms the child's button; the drag that
    // follows may wander outside the region, and it is still the child's -- a
    // gesture that started on the program has to keep going to it.
    state.update(tumu::Event::Mouse(click(5, 10)));
    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::Drag,
        5,
        60,
    )));

    assert_eq!(state.focused, Some(state.stack.sheets()[0].0));
    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused {
                index: 0,
                held: Some(tuinix::MouseInputKind::LeftPress),
                ..
            }]
        ),
        "the drag stays with the child that saw the press: {actions:?}"
    );
}

#[test]
fn a_forwarded_drag_names_the_button_that_was_pressed() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::MouseWanted(true));

    // Press, then drag: the drag carries the button the press armed, because the
    // host's drag report does not name it and the child has to be told.
    state.update(tumu::Event::Mouse(click(5, 10)));
    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::Drag,
        5,
        12,
    )));

    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused {
                held: Some(tuinix::MouseInputKind::LeftPress),
                ..
            }]
        ),
        "the drag carries the held button: {actions:?}"
    );

    // The release clears it, so a later drag is a bare move.
    state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::LeftRelease,
        5,
        12,
    )));
    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::Drag,
        5,
        13,
    )));
    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused { held: None, .. }]
        ),
        "a drag after a release has no button: {actions:?}"
    );
}

#[test]
fn a_child_that_stops_asking_leaves_its_held_button_behind() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::MouseWanted(true));
    state.update(tumu::Event::Mouse(click(5, 10)));
    // The child stops asking before releasing. The press was already forwarded,
    // so the button it armed would otherwise be carried by a drag the next time
    // the child asks -- a button that child never saw pressed.
    state.update(tumu::Event::MouseWanted(false));
    state.update(tumu::Event::MouseWanted(true));

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::Drag,
        5,
        12,
    )));

    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused { held: None, .. }]
        ),
        "the stale button is forgotten: {actions:?}"
    );
}

#[test]
fn the_wheel_is_forwarded_but_never_held() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::MouseWanted(true));

    state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollUp,
        5,
        10,
    )));
    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::Drag,
        5,
        11,
    )));

    // The wheel has no release, so it can never be the button a drag holds.
    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused { held: None, .. }]
        ),
        "the wheel is not a held button: {actions:?}"
    );
}

#[test]
fn the_wheel_goes_to_the_focused_sheet_from_outside_its_region() {
    // Two 40-column sheets; focus the left one, then turn the wheel over the
    // *right* one. A pager scrolls by the line and does not read the cell the
    // pointer was over, so the turn is still the focused sheet's -- and routing
    // it by region would drop it, because the pointer is not on that sheet.
    let mut state = state_with(24, 80, 2);
    state.update(tumu::Event::MouseWanted(true));
    let left = state.stack.sheets()[0].0;
    state.update(tumu::Event::Mouse(click(5, 10)));
    assert_eq!(state.focused, Some(left), "the left sheet is focused");

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollDown,
        5,
        60,
    )));

    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused {
                index: 0,
                held: None,
                ..
            }]
        ),
        "the wheel reaches the focused sheet from outside it: {actions:?}"
    );
}

#[test]
fn the_wheel_over_another_sheet_does_not_move_the_focus() {
    // The same turn as the test above, asserted on the focus: the pointer says
    // where the turn happened, not which sheet the user wants next, and a
    // scroll that moved the focus would move the sheet out from under the
    // reader.
    let mut state = state_with(24, 80, 2);
    state.update(tumu::Event::MouseWanted(true));
    let left = state.stack.sheets()[0].0;
    state.update(tumu::Event::Mouse(click(5, 10)));

    state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollUp,
        5,
        60,
    )));

    assert_eq!(state.focused, Some(left), "the wheel moved the focus");
}

#[test]
fn the_wheel_past_every_sheet_still_reaches_the_focused_one() {
    // A turn on the terminal's last row, below both sheets, is outside every
    // region; the wheel still belongs to the focused sheet.
    let mut state = state_with(24, 80, 2);
    state.update(tumu::Event::MouseWanted(true));

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollDown,
        24,
        0,
    )));

    assert!(
        matches!(
            actions.as_slice(),
            [tumu::Action::SendMouseToFocused { index: 0, .. }]
        ),
        "a turn past every sheet still goes to the focused one: {actions:?}"
    );
}

#[test]
fn the_wheel_with_nothing_asking_is_dropped() {
    // The exception the wheel gets is only to the *region* test: a report of
    // any kind still needs a child that asked for the mouse. A down turn has no
    // command behind it either -- only up opens the history -- so both the
    // forward and the copy-mode door are closed and the turn is dropped.
    let mut state = state_with(24, 80, 2);
    assert!(!state.mouse_wanted());
    let focused = state.focused;

    let actions = state.update(tumu::Event::Mouse(mouse(
        tuinix::MouseInputKind::ScrollDown,
        5,
        10,
    )));

    assert!(actions.is_empty(), "nothing is forwarded: {actions:?}");
    assert_eq!(state.focused, focused, "the wheel must not move the focus");
}

#[test]
fn with_no_child_asking_a_report_still_only_moves_the_focus() {
    // The default, restated as the pair of the first test: with nothing asking,
    // every report takes the focus path and none is forwarded.
    let mut state = state_with(24, 80, 2);
    assert!(!state.mouse_wanted());

    let actions = state.update(tumu::Event::Mouse(click(5, 60)));

    assert!(actions.is_empty(), "nothing is forwarded: {actions:?}");
    assert_eq!(state.focused, Some(state.stack.sheets()[1].0));
}

#[test]
fn a_forwarded_report_needs_a_focused_sheet() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::MouseWanted(true));
    // The child exits and the sheet goes, but the ask is still recorded -- the
    // edge only clears it on the next pass.
    let id = state.stack.sheets()[0].0;
    state.update(tumu::Event::SessionExited(id));

    let actions = state.update(tumu::Event::Mouse(click(5, 10)));

    assert!(actions.is_empty(), "nowhere to forward to: {actions:?}");
}

#[test]
fn a_screen_too_small_for_the_box_shows_no_menu() {
    // Four columns cannot hold the box, so the prefix arms the latch but opens
    // nothing -- the flag tracks what is on screen, so a click can never land on
    // a menu that was not drawn.
    let mut state = state_with(40, 4, 1);

    state.update(tumu::Event::Key(prefix()));

    assert!(state.prefix, "the latch still arms");
    assert!(!state.menu, "but there is no menu to show");
    assert!(state.menu_layout().is_none());
}
