//! End-to-end tests for key handling: the prefix latch, key forwarding, and
//! the commands the first keymap binds.
//!
//! These drive the real binary through keys, the way a terminal would, and
//! read the resulting grid back. They prove the prefix works end to end, not
//! just in the core's unit tests. A title carries a marker after the index --
//! `>` on the focused sheet, a space on the rest -- so the assertions below
//! double as a check on where focus is.

mod e2e;

#[test]
fn a_new_sheet_stacks_on_top() {
    // 24x80 is wider than tall, so the stack lays out horizontally and two
    // sheets split the columns 40/40. The new sheet goes on top, so it is at
    // index 0 and takes the focus; the old sheet 0 slides down to index 1.
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");

    tumu.send_prefix();
    tumu.send_char('n');

    tumu.wait_for_text("[0]>24x40");
    // The sheet that was 0 took the other half, and lost the focus marker.
    assert!(tumu.screen_contains("[1] 24x40"));
}

#[test]
fn closing_the_focused_sheet_grows_the_rest() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");

    // Close the focused sheet: the one on top.
    tumu.send_prefix();
    tumu.send_char('x');

    // Back to one sheet, filling the terminal again.
    tumu.wait_for_text("[0]>24x80");
    assert!(!tumu.screen_contains("[1]"));
}

#[test]
fn the_rotate_command_brings_the_stack_up_under_the_keyboard() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");

    tumu.send_prefix();
    tumu.send_char('o');

    // Rotating moves the top sheet to the bottom, so the sheet that was at
    // index 1 comes up to index 0 and takes the focus with it.
    tumu.wait_for_text("[0]>24x40");
    assert!(tumu.screen_contains("[1] 24x40"));
}

#[test]
fn the_full_screen_command_fills_the_terminal_and_folds_back() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");

    tumu.send_prefix();
    tumu.send_char('z');

    // The focused sheet takes the whole terminal; the other is hidden.
    tumu.wait_for_text("[0]>24x80");
    assert!(!tumu.screen_contains("[1]"));

    tumu.send_prefix();
    tumu.send_char('z');

    // Folding back restores both, with focus where it was.
    tumu.wait_for_text("[0]>24x40");
    assert!(tumu.screen_contains("[1] 24x40"));
}

#[test]
fn the_quit_command_exits() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");

    tumu.send_prefix();
    tumu.send_char('q');

    tumu.wait_for_exit();
}
