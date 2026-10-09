//! End-to-end tests for mouse handling: a click moves focus.
//!
//! These drive the real binary through mouse reports, the way a terminal would,
//! and read the resulting grid back. Focus is visible in the border -- only the
//! focused sheet carries the command label (`[C-q ...]`) -- so "the click moved
//! focus" is an assertion about the frame, not about the core's internals.
//!
//! The default 24x80 is wider than tall, so two sheets split the columns 40/40
//! and each border runs down its left column; focus is read from those columns.
//!
//! The reports go through `termnix`, which only encodes one when the child has
//! asked for mouse reporting; that `tumu` has (see `App::new`) is part of what
//! these tests pin down.
//!
//! Every test pins `$SHELL` to a silent script, so a sheet's border shows only
//! tumu's own text. A real login shell sets a window title of its own (fish
//! does, on every prompt), which would land in the border and make these
//! assertions about *tumu's* text flaky.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// The column the lower sheet of a 80-column terminal starts at.
const LOWER_SHEET_COL: u16 = 60;

/// The column inside the upper sheet of a 80-column terminal.
const UPPER_SHEET_COL: u16 = 10;

/// The left column of the upper sheet, where its border is written.
const UPPER_BORDER_COL: u16 = 0;

/// The left column of the lower sheet, where its border is written.
const LOWER_BORDER_COL: u16 = 40;

/// Waits until the two sheets are laid out and one of their borders carries the
/// focus label. `focused_column` is 0 for the upper sheet, 40 for the lower.
fn wait_for_focus(tumu: &mut e2e::TumuHarness, upper_focused: bool) {
    let (focused, other) = if upper_focused {
        (UPPER_BORDER_COL, LOWER_BORDER_COL)
    } else {
        (LOWER_BORDER_COL, UPPER_BORDER_COL)
    };
    tumu.wait_until("focus to settle", |h| {
        h.screen_column(focused).starts_with("└ [C-q ...]")
            && h.screen_column(other).starts_with("└─")
    });
}

/// Writes a silent shell script -- one that never sets a title -- and returns
/// its path. The script just sleeps, so the sheet stays alive and quiet.
fn silent_shell(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-mouse-{}-{}.sh", std::process::id(), name));
    fs::write(&path, "#!/bin/sh\nexec sleep 30\n").expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// Opens a harness whose sheets run a silent shell.
fn harness(name: &str) -> e2e::TumuHarness {
    let shell = silent_shell(name);
    e2e::TumuHarness::with_shell(24, 80, shell.to_str().expect("utf-8 path"))
}
#[test]
fn clicking_a_sheet_moves_focus_to_it() {
    // 24x80 is wider than tall, so two sheets split the columns 40/40. The new
    // sheet goes on top, at index 0, and takes the focus.
    let mut tumu = harness("moves-focus");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('n');
    wait_for_focus(&mut tumu, true);

    // Click the sheet below: it takes the focus, and the label moves with it.
    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);
}

#[test]
fn clicking_back_returns_focus_to_the_top_sheet() {
    let mut tumu = harness("click-back");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('n');
    wait_for_focus(&mut tumu, true);

    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);

    tumu.click(5, UPPER_SHEET_COL);
    wait_for_focus(&mut tumu, true);
}

#[test]
fn a_click_does_not_disturb_the_layout() {
    let mut tumu = harness("layout-unchanged");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('n');
    wait_for_focus(&mut tumu, true);

    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);

    // Focus is all a click changes: both sheets' borders are still there, in the
    // same two columns, so neither sheet was moved or resized.
    assert!(tumu.screen_column(UPPER_BORDER_COL).starts_with("└"));
    assert!(tumu.screen_column(LOWER_BORDER_COL).starts_with("└"));
}

#[test]
fn the_keyboard_follows_a_click() {
    let mut tumu = harness("keyboard-follows");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('n');
    wait_for_focus(&mut tumu, true);

    // Click the lower sheet, then close it. `x` acts on the focused sheet, so
    // the lower one must go and the upper one must take the whole terminal.
    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);
    tumu.send_prefix();
    tumu.send_char('x');

    // One sheet is left, focused, filling the terminal, with no border in the
    // lower sheet's old column.
    tumu.wait_until("the lower sheet is gone", |h| {
        h.screen_column(UPPER_BORDER_COL).starts_with("└ [C-q ...]")
            && h.screen_column(LOWER_BORDER_COL).trim().is_empty()
    });
}
