//! End-to-end resize behavior: the stack re-lays-out on a terminal resize.
//!
//! A resize is what a device rotation arrives as, so these drive the real bytes
//! a terminal sends on SIGWINCH and read back the sheet sizes the core derived.

mod e2e;

#[test]
fn a_resize_relays_out_the_single_sheet() {
    let mut tumu = e2e::TumuHarness::open();
    tumu.wait_for_text("[0] 24x80");

    // Rotate to a short, wide terminal.
    tumu.resize(12, 200);

    tumu.wait_for_text("[0] 12x200");
}
