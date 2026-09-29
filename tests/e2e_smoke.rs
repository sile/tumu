//! End-to-end smoke test: the real `tumu` binary starts and paints.
//!
//! This is the smallest test that proves the harness works at all. If the
//! binary cannot take over a PTY, or the emulator cannot read its output back,
//! this test fails and every other test in the suite is meaningless.

mod e2e;

#[test]
fn tumu_starts_and_paints_a_window_title() {
    let mut tumu = e2e::TumuHarness::open();

    // The single startup window is window 0, and the derived size is the whole
    // terminal.
    tumu.wait_for_text("[0] 24x80");
}
