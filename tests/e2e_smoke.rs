//! End-to-end smoke test: the real `tumu` binary starts and paints.
//!
//! This is the smallest test that proves the harness works at all. If the
//! binary cannot take over a PTY, or the emulator cannot read its output back,
//! this test fails and every other test in the suite is meaningless.

mod e2e;

#[test]
fn tumu_starts_and_paints_a_sheet_border() {
    // A tall terminal stacks vertically, so the border is a row the test can
    // read back as text; the default 24x80 stacks horizontally and writes the
    // border down a column instead.
    let mut tumu = e2e::TumuHarness::with_size(40, 24);

    // The single startup sheet is focused, so its border carries the command
    // label, and the corner and rule run the rest of the row.
    tumu.wait_for_text("└ [C-q ...]");
    assert!(tumu.screen_contains('─'.to_string().as_str()));
}
