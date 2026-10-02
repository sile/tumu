//! End-to-end tests for key handling: the prefix latch and key forwarding.
//!
//! These drive the real binary through keys, the way a terminal would, and
//! read the resulting grid back. They prove the prefix works end to end, not
//! just in the core's unit tests.

mod e2e;

#[test]
fn a_new_sheet_stacks_on_top() {
    // 24x80 is wider than tall, so the stack lays out horizontally and two
    // sheets split the columns 40/40. The new sheet goes on top: sheet 1.
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0] 24x80");

    tumu.send_prefix();
    tumu.send_char('n');

    tumu.wait_for_text("[1] 24x40");
    // The first sheet takes the other half, so both titles are on screen.
    assert!(tumu.screen_contains("[0] 24x40"));
}

#[test]
fn closing_the_focused_sheet_grows_the_rest() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0] 24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[1] 24x40");

    tumu.send_prefix();
    tumu.send_char('x');

    // Back to one sheet, filling the terminal again.
    tumu.wait_for_text("[0] 24x80");
    assert!(!tumu.screen_contains("[1] "));
}

#[test]
fn the_quit_command_exits() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0] 24x80");

    tumu.send_prefix();
    tumu.send_char('q');

    tumu.wait_for_exit();
}
