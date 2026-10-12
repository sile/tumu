//! End-to-end tests for key handling: the prefix latch, key forwarding, and
//! the commands the first keymap binds.
//!
//! These drive the real binary through keys, the way a terminal would, and
//! read the resulting grid back. They prove the prefix works end to end, not
//! just in the core's unit tests. Only the focused sheet's border carries the
//! command label (`[C-q ...]`), so the assertions below double as a check on
//! where focus is.
//!
//! These run their sheets *horizontally* (`--orientation horizontal`), so each
//! sheet's border runs down its right column and the border text is read
//! column by column (`wait_for_column_text`), not row by row. A sheet's border
//! column is its own last column, so the top sheet's is at column 39 and the
//! sheet below it at column 79. The axis is a setting now, so the vertical case
//! is exercised on its own in `e2e_orientation.rs`; choosing it here keeps the
//! assertions below about *where* a sheet ends, not which way the stack runs.
//!
//! Every test pins `$SHELL` to a silent script, so a sheet's border shows only
//! tumu's own text. A real login shell sets a window title of its own (fish
//! does, on every prompt), which would land in the border and make assertions
//! about *tumu's* text flaky.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// Writes a silent shell script -- one that never sets a title -- and returns
/// its path. The script just sleeps, so the sheet stays alive and quiet.
fn silent_shell(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-keys-{}-{}.sh", std::process::id(), name));
    fs::write(&path, "#!/bin/sh\nexec sleep 30\n").expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// Opens a harness whose sheets run a silent shell, stacked horizontally.
fn harness(name: &str) -> e2e::TumuHarness {
    let shell = silent_shell(name);
    e2e::TumuHarness::with_shell_and_orientation(
        24,
        80,
        shell.to_str().expect("utf-8 path"),
        "horizontal",
    )
}

#[test]
fn a_new_sheet_stacks_on_top() {
    // 24x80 is wider than tall, so the stack lays out horizontally and two
    // sheets split the columns 40/40. The new sheet goes on top, so it is at
    // index 0 and takes the focus; the old sheet 0 slides down to index 1.
    let mut tumu = harness("stacks-on-top");
    // One sheet, focused: exactly one border carries the label.
    tumu.wait_for_column_text("[C-q ...]");

    tumu.send_prefix();
    tumu.send_char('c');

    // Two sheets: the label moves to the new top sheet (column 39) and the
    // sheet that was 0 loses it. Both borders repaint in one frame, so wait for
    // both rather than read a frame that may still hold the previous one.
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });
    assert!(
        !tumu.screen_column(79).contains("[C-q ...]"),
        "only the focused sheet shows the label: {}",
        tumu.screen_column(79)
    );
}

#[test]
fn closing_the_focused_sheet_grows_the_rest() {
    let mut tumu = harness("closing-grows");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    // Close the focused sheet: the one on top. `x` does not close it -- it
    // opens the extended menu -- so the close is `x` then `k`.
    tumu.send_prefix();
    tumu.send_char('x');
    tumu.send_char('k');

    // Back to one sheet filling the terminal, so its border is now the screen's
    // *last* column (79) and the column it used to end at (39) is content.
    tumu.wait_until("one sheet border", |h| {
        h.screen_column(79).starts_with("└ [C-q ...]")
    });
    assert!(
        tumu.screen_column(39).chars().all(|c| !"└─".contains(c)),
        "the closed sheet's border column is gone: {:?}",
        tumu.screen_column(39)
    );
}

#[test]
fn the_move_to_top_command_brings_the_sheet_under_the_keyboard_to_the_top() {
    let mut tumu = harness("move-to-top");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    // Step down to the lower sheet, so the keyboard is on the one that is *not*
    // on top.
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_until("the label moves down", |h| {
        h.screen_column(79).starts_with("└ [C-q ...]") && h.screen_column(39).starts_with("└─")
    });

    // `t` lifts that sheet to the top. The sheet moves under the keyboard, so
    // the label moves to the top sheet's border -- which is now the sheet that
    // wears it -- while the layout stays two 40-column sheets.
    tumu.send_prefix();
    tumu.send_char('t');
    tumu.wait_until("the label is back on the top sheet", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└─")
    });
    assert!(!tumu.screen_column(79).contains("[C-q ...]"));
}

#[test]
fn the_ratio_flag_sets_the_starting_split() {
    // `$SHELL` is pinned to the silent script and `--ratio 2` asks for the
    // `2 : 1` split at startup, so the first sheet takes two thirds of the
    // columns and the one made after it takes the rest -- a layout a test can
    // read off the border columns without ever pressing `+`.
    let shell = silent_shell("ratio-flag");
    let mut tumu = e2e::TumuHarness::with_args(24, 80, |command| {
        command.env("SHELL", &shell);
        command.arg("--orientation").arg("horizontal");
        command.arg("--ratio").arg("2");
    });
    tumu.wait_for_column_text("[C-q ...]");

    tumu.send_prefix();
    tumu.send_char('c');
    // Two thirds of 80 rounds up to 54, so the top sheet's border is at column
    // 53 and the sheet below it at column 79.
    tumu.wait_until("the starting split is two thirds", |h| {
        h.screen_column(53).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });
}

#[test]
fn the_split_steps_redraw_the_border_where_the_share_lands() {
    let mut tumu = harness("split-steps");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    // Two sheets at the default `1 : 1` split the 80 columns 40/40, so the top
    // sheet's border is at column 39.
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    // `+` widens the top sheet's share to `2 : 1`: two thirds of 80 rounds up
    // to 54 columns, so its border moves to column 53 and the sheet below keeps
    // the remaining 26.
    tumu.send_prefix();
    tumu.send_char('+');
    tumu.wait_until("the top sheet grows to two thirds", |h| {
        h.screen_column(53).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    // `-` steps back to the halving, so the border returns to column 39.
    tumu.send_prefix();
    tumu.send_char('-');
    tumu.wait_until("the split returns to half", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]")
    });
}

#[test]
fn the_full_screen_command_fills_the_terminal_and_folds_back() {
    let mut tumu = harness("full-screen");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    tumu.send_prefix();
    tumu.send_char('z');

    // The focused sheet takes the whole terminal and draws *no* border: a
    // full-screen sheet has no neighbour to be closed off from, so the line it
    // showed at column 39 is gone and the one it would have shown at column 79
    // is not drawn either. Nothing of a border is left on the screen.
    tumu.wait_until("no sheet border at all", |h| {
        h.screen_column(39).chars().all(|c| !"└─".contains(c))
            && h.screen_column(79).chars().all(|c| !"└─".contains(c))
    });

    tumu.send_prefix();
    tumu.send_char('z');

    // Folding back restores both, with focus where it was.
    tumu.wait_until("both sheet borders return", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });
}

#[test]
fn the_focus_step_moves_the_keyboard_between_the_sheets() {
    let mut tumu = harness("focus-step");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    // Two sheets split the columns 40/40 and the new one is on top, so the
    // label sits on the top sheet's border at column 39.
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    // `n` steps *down* the stack: focus lands on the sheet below, so the label
    // moves to its border at column 79.
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_until("the label moves down", |h| {
        h.screen_column(79).starts_with("└ [C-q ...]") && h.screen_column(39).starts_with("└─")
    });

    // `p` steps back up, so the label returns to the top sheet. The layout
    // never moved: only which sheet wears the label did.
    tumu.send_prefix();
    tumu.send_char('p');
    tumu.wait_until("the label moves back up", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└─")
    });
}

#[test]
fn the_quit_command_exits() {
    let mut tumu = harness("quit");
    tumu.wait_for_column_text("[C-q ...]");

    // Quitting is behind the extended menu too, so `x` opens it and `q` quits.
    tumu.send_prefix();
    tumu.send_char('x');
    tumu.send_char('q');

    tumu.wait_for_exit();
}

#[test]
fn the_extended_menu_opens_and_a_stray_key_dismisses_it() {
    let mut tumu = harness("ext-menu");
    tumu.wait_for_column_text("[C-q ...]");

    // `x` does not close the sheet: it opens the extended menu, whose two rows
    // name the commands the prefix no longer carries.
    tumu.send_prefix();
    tumu.send_char('x');
    tumu.wait_for_text("close sheet");
    assert!(tumu.screen_contains("quit"), "the menu lists quit");

    // Any key the menu does not name dismisses it and does nothing else, so a
    // stray press cannot run the command the menu was guarding.
    tumu.send_char('y');
    tumu.wait_until("the menu is gone", |h| !h.screen_contains("close sheet"));
    // The sheet is still there: the frame still carries its border label.
    tumu.wait_for_column_text("[C-q ...]");
}
