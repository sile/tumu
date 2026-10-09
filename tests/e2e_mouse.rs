//! End-to-end tests for mouse handling: a click moves focus.
//!
//! These drive the real binary through mouse reports, the way a terminal would,
//! and read the resulting grid back. Focus is visible in the title row -- `>`
//! after the index marks the focused sheet -- so "the click moved focus" is an
//! assertion about the frame, not about the core's internals.
//!
//! The reports go through `termnix`, which only encodes one when the child has
//! asked for mouse reporting; that `tumu` has (see `App::new`) is part of what
//! these tests pin down.
//!
//! Every test pins `$SHELL` to a silent script, so a sheet's title row shows
//! only tumu's own `[index] marker size` text. A real login shell sets a window
//! title of its own (fish does, on every prompt), which would land in the title
//! row and make these assertions about *tumu's* text flaky.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// The column the lower sheet of a 80-column terminal starts at.
const LOWER_SHEET_COL: u16 = 60;

/// The column inside the upper sheet of a 80-column terminal.
const UPPER_SHEET_COL: u16 = 10;

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
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");
    tumu.wait_for_text("[1] 24x40");

    // Click the sheet below: it takes the focus, and the marker moves with it.
    tumu.click(5, LOWER_SHEET_COL);

    tumu.wait_for_text("[1]>24x40");
    tumu.wait_for_text("[0] 24x40");
}

#[test]
fn clicking_back_returns_focus_to_the_top_sheet() {
    let mut tumu = harness("click-back");
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");
    tumu.wait_for_text("[1] 24x40");

    tumu.click(5, LOWER_SHEET_COL);
    tumu.wait_for_text("[1]>24x40");

    tumu.click(5, UPPER_SHEET_COL);

    tumu.wait_for_text("[0]>24x40");
    // The two titles repaint in one frame, but `wait_for_text` returns as soon
    // as the first matches: wait for the second too rather than asserting on a
    // frame that may still hold the previous title.
    tumu.wait_for_text("[1] 24x40");
}

#[test]
fn a_click_does_not_disturb_the_layout() {
    let mut tumu = harness("layout-unchanged");
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");
    tumu.wait_for_text("[1] 24x40");

    tumu.click(5, LOWER_SHEET_COL);
    tumu.wait_for_text("[1]>24x40");
    tumu.wait_for_text("[0] 24x40");

    // Focus is all a click changes: neither sheet has been moved or resized,
    // so both titles still report the same derived size.
    let screen = tumu.screen_text();
    assert!(screen.contains("[0] 24x40"), "screen was:\n{screen}");
    assert!(screen.contains("[1]>24x40"), "screen was:\n{screen}");
}

#[test]
fn the_keyboard_follows_a_click() {
    let mut tumu = harness("keyboard-follows");
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");
    tumu.wait_for_text("[1] 24x40");

    // Click the lower sheet, then close it. `x` acts on the focused sheet, so
    // the lower one must go and the upper one must take the whole terminal.
    tumu.click(5, LOWER_SHEET_COL);
    tumu.wait_for_text("[1]>24x40");
    tumu.send_prefix();
    tumu.send_char('x');

    tumu.wait_for_text("[0]>24x80");
    assert!(!tumu.screen_contains("[1]"));
}
