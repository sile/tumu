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

mod e2e;

/// The column the lower sheet of a 80-column terminal starts at.
const LOWER_SHEET_COL: u16 = 60;

/// The column inside the upper sheet of a 80-column terminal.
const UPPER_SHEET_COL: u16 = 10;

#[test]
fn clicking_a_sheet_moves_focus_to_it() {
    // 24x80 is wider than tall, so two sheets split the columns 40/40. The new
    // sheet goes on top, at index 0, and takes the focus.
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");
    assert!(tumu.screen_contains("[1] 24x40"));

    // Click the sheet below: it takes the focus, and the marker moves with it.
    tumu.click(5, LOWER_SHEET_COL);

    tumu.wait_for_text("[1]>24x40");
    assert!(tumu.screen_contains("[0] 24x40"));
}

#[test]
fn clicking_back_returns_focus_to_the_top_sheet() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");

    tumu.click(5, LOWER_SHEET_COL);
    tumu.wait_for_text("[1]>24x40");

    tumu.click(5, UPPER_SHEET_COL);

    tumu.wait_for_text("[0]>24x40");
    assert!(tumu.screen_contains("[1] 24x40"));
}

#[test]
fn a_click_does_not_disturb_the_layout() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");

    tumu.click(5, LOWER_SHEET_COL);
    tumu.wait_for_text("[1]>24x40");

    // Focus is all a click changes: neither sheet has been moved or resized,
    // so both titles still report the same derived size.
    let screen = tumu.screen_text();
    assert!(screen.contains("[0] 24x40"), "screen was:\n{screen}");
    assert!(screen.contains("[1]>24x40"), "screen was:\n{screen}");
}

#[test]
fn the_keyboard_follows_a_click() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0]>24x80");
    tumu.send_prefix();
    tumu.send_char('n');
    tumu.wait_for_text("[0]>24x40");

    // Click the lower sheet, then close it. `x` acts on the focused sheet, so
    // the lower one must go and the upper one must take the whole terminal.
    tumu.click(5, LOWER_SHEET_COL);
    tumu.wait_for_text("[1]>24x40");
    tumu.send_prefix();
    tumu.send_char('x');

    tumu.wait_for_text("[0]>24x80");
    assert!(!tumu.screen_contains("[1]"));
}
