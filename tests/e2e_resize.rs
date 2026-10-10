//! End-to-end resize behavior: the stack re-lays-out on a terminal resize.
//!
//! A resize is what a device rotation arrives as, so these drive the real bytes
//! a terminal sends on SIGWINCH and read back the sheet the core laid out. A
//! resize that flips the stack's axis moves the border from a row to a column
//! and back, which is what these assert on.

mod e2e;

#[test]
fn a_resize_relays_out_the_single_sheet() {
    // Tall: the stack is vertical and the border is the last row.
    let mut tumu = e2e::TumuHarness::with_size(40, 24);
    tumu.wait_for_text("└ [C-q ...]");

    // Rotate to a short, wide terminal: the stack flips to horizontal and the
    // border moves to the last column, written vertically.
    tumu.resize(12, 200);

    tumu.wait_for_column_text("[C-q ...]");
    assert!(
        !tumu.screen_contains("└ [C-q ...]"),
        "the border is no longer a row: {}",
        tumu.screen_text()
    );
}
