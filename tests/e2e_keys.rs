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
    tumu.send_char('n');

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
    tumu.send_char('n');
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    // Close the focused sheet: the one on top.
    tumu.send_prefix();
    tumu.send_char('x');

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
fn the_rotate_command_brings_the_stack_up_under_the_keyboard() {
    let mut tumu = harness("rotate");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    tumu.send_prefix();
    tumu.send_char('o');

    // Rotating moves the top sheet to the bottom, so the sheet that was at
    // index 1 comes up to index 0 and takes the focus with it. Focus is on the
    // sheet that came to the *top*, so the label stays in column 39: what moved
    // is which sheet wears it, which the frame alone does not show. The layout
    // is unchanged -- two 40-column sheets -- so the observable result is that
    // the label is still exactly once, on the top sheet.
    tumu.wait_until("the label is back on the top sheet", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└─")
    });
    assert!(!tumu.screen_column(79).contains("[C-q ...]"));
}

#[test]
fn the_full_screen_command_fills_the_terminal_and_folds_back() {
    let mut tumu = harness("full-screen");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });

    tumu.send_prefix();
    tumu.send_char('z');

    // The focused sheet takes the whole terminal; the other is hidden, so the
    // border it used to show (at column 39) is gone and the focused sheet's
    // border is the screen's last column.
    tumu.wait_until("the hidden sheet's border is gone", |h| {
        h.screen_column(79).starts_with("└ [C-q ...]")
            && h.screen_column(39).chars().all(|c| !"└─".contains(c))
    });

    tumu.send_prefix();
    tumu.send_char('z');

    // Folding back restores both, with focus where it was.
    tumu.wait_until("both sheet borders return", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]") && h.screen_column(79).starts_with("└")
    });
}

#[test]
fn the_quit_command_exits() {
    let mut tumu = harness("quit");
    tumu.wait_for_column_text("[C-q ...]");

    tumu.send_prefix();
    tumu.send_char('q');

    tumu.wait_for_exit();
}
