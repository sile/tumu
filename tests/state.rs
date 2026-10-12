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

fn state_with(rows: usize, cols: usize, sheets: usize) -> tumu::State {
    let mut state = tumu::State::new(size(rows, cols));
    // A resize over an empty stack spawns the first sheet; pushing the rest
    // through the same event keeps the test off the internal push.
    state.update(tumu::Event::Resize(size(rows, cols)));
    for _ in 1..sheets {
        state.update(tumu::Event::Key(prefix()));
        state.update(tumu::Event::Key(ch('n')));
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
    let actions = state.update(tumu::Event::Key(ch('q')));

    // `prefix q` quits whatever the prefix character is: the chords are the
    // unmodified keys and the prefix only arms the latch.
    assert_eq!(actions, vec![tumu::Action::Exit]);
    assert!(state.exit);
}

#[test]
fn the_prefix_character_does_not_shadow_a_chord() {
    // With the prefix on `q`, the latch and the quit chord share a character;
    // they are still different keys, because the prefix is `Ctrl+q` and the
    // chord is `q`. This is the namespace the two live in.
    let mut state = state_with_prefix_char('q');

    state.update(tumu::Event::Key(ctrl('q')));
    let actions = state.update(tumu::Event::Key(ch('q')));

    assert_eq!(actions, vec![tumu::Action::Exit]);
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
    state.update(tumu::Event::Key(prefix()));

    let actions = state.update(tumu::Event::Key(ch('n')));

    assert_eq!(state.stack.len(), 2);
    assert!(
        matches!(
            actions.first(),
            Some(&tumu::Action::SpawnSession {
                index: 0,
                program: tumu::Program::Shell
            })
        ),
        "the first action spawns a shell in the new top sheet, got {actions:?}"
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

    let actions = state.update(tumu::Event::Key(ch('x')));

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

    let actions = state.update(tumu::Event::Key(ch('x')));

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
fn the_rotate_command_moves_the_focused_sheet_to_the_bottom() {
    let mut state = state_with(24, 80, 3);
    let top = state.stack.sheets()[0].0;
    state.update(tumu::Event::Key(prefix()));

    state.update(tumu::Event::Key(ch('o')));

    assert_eq!(state.stack.sheets().last().map(|(id, _)| *id), Some(top));
}

#[test]
fn rotating_a_single_sheet_does_nothing() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(prefix()));

    let actions = state.update(tumu::Event::Key(ch('o')));

    assert!(actions.is_empty());
    assert_eq!(state.stack.len(), 1);
}

#[test]
fn the_quit_command_sets_exit_and_returns_it() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(prefix()));

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
    assert!(
        actions.iter().any(|action| matches!(
            action,
            tumu::Action::SpawnSession {
                index: 0,
                program: tumu::Program::Editor { .. }
            }
        )),
        "the editor sheet spawns the editor, got {actions:?}"
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
fn editing_the_scrollback_on_an_empty_stack_does_nothing() {
    let mut state = state_with(24, 80, 1);
    run_command(&mut state, 'x');

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
    run_command(&mut state, 'x');

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

    // The border is the top sheet's first row, and the row below it is already
    // its content; the bottom half belongs to the sheet below.
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
    state.update(tumu::Event::Key(ch('x')));
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
fn the_prefix_again_sends_the_prefix_and_closes_the_menu() {
    let mut state = state_with(24, 80, 1);
    state.update(tumu::Event::Key(prefix()));

    let actions = state.update(tumu::Event::Key(prefix()));

    assert_eq!(actions, vec![send_to_top(prefix())]);
    assert!(!state.menu);
}

#[test]
fn clicking_a_menu_row_runs_its_command_and_closes_the_menu() {
    let mut state = state_with(40, 80, 2);
    // The focused sheet needs a session for closing it to name one.
    state.stack.attach_session(0, session(1));
    state.update(tumu::Event::Key(prefix()));
    let layout = state.menu_layout().expect("the menu fits 40x80");

    // Click the row that closes a sheet.
    let row = layout
        .rows
        .iter()
        .find(|row| row.command == Some(tumu::Command::CloseSheet))
        .expect("a close-sheet row");
    let actions = state.update(tumu::Event::Mouse(click(
        row.region.position.row,
        row.region.position.col + 1,
    )));

    assert!(!state.menu, "selecting a row closes the menu");
    assert_eq!(state.stack.len(), 1, "the focused sheet was closed");
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, tumu::Action::CloseSession { .. })),
        "closing a sheet asks the edge to close its session"
    );
}

#[test]
fn clicking_the_menu_border_dismisses_it() {
    let mut state = state_with(40, 80, 2);
    state.update(tumu::Event::Key(prefix()));
    let layout = state.menu_layout().expect("the menu fits 40x80");
    let corner = layout.region.position;

    let actions = state.update(tumu::Event::Mouse(click(corner.row, corner.col)));

    assert!(!state.menu, "a border click dismisses");
    assert!(actions.is_empty(), "and runs nothing");
    assert_eq!(state.stack.len(), 2, "the stack is untouched");
}

#[test]
fn clicking_the_focused_sheets_border_label_opens_the_menu() {
    // A tall terminal stacks vertically, so the focused border is its last row.
    let mut state = state_with(40, 30, 1);
    let region = state.region_of(0).expect("the sheet's region");

    state.update(tumu::Event::Mouse(click(
        region.position.row + region.size.rows - 1,
        region.position.col + 2,
    )));

    assert!(state.menu, "a click on the label opens the menu");
}

#[test]
fn clicking_the_focused_sheets_border_opens_the_menu_when_horizontal() {
    // A wide terminal stacks horizontally, so the border is the sheet's own
    // *last* column -- the edge it shares with nothing, because the neighbour is
    // the sheet to its left.
    let mut state = state_with(24, 80, 1);
    let region = state.region_of(0).expect("the sheet's region");
    let border_col = region.position.col + region.size.cols - 1;

    state.update(tumu::Event::Mouse(click(5, border_col)));
    assert!(state.menu, "a click on the border opens the menu");

    // A click one column to its left is content, not border.
    state.update(tumu::Event::Mouse(click(5, border_col - 1)));
    // The menu was open, so that click dismisses it; the point is that it did
    // not land on the border and re-open it.
    assert!(!state.menu);
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
