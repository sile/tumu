//! End-to-end resize behavior: the stack re-lays-out on a terminal resize,
//! and the axis does *not* change with it.
//!
//! A resize is what a device rotation arrives as, so these drive the real bytes
//! a terminal sends on SIGWINCH and read back the sheet the core laid out. The
//! axis used to be derived from the shape, so a resize flipped it; now it is a
//! setting, and these pin that a resize leaves it alone.

mod e2e;

#[test]
fn a_resize_keeps_the_axis_it_was_given() {
    // A vertical stack, whatever the terminal's shape: the border is the last
    // row before and after the resize.
    let mut tumu = e2e::TumuHarness::with_orientation(40, 24, "vertical");
    tumu.wait_for_text("└ [C-q ...]");

    // Turn to a short, wide terminal. The sheets take the new shape -- one row
    // of border at the bottom -- but stay stacked the same way, because nothing
    // asked for the other axis.
    tumu.resize(12, 200);

    tumu.wait_until("the border is still a row", |h| {
        h.screen_rows()
            .last()
            .is_some_and(|row| row.starts_with("└ [C-q ...]"))
    });
    assert!(
        !tumu.screen_column_contains("[C-q ...]"),
        "the border did not move into a column: {}",
        tumu.screen_text()
    );
}

#[test]
fn a_horizontal_stack_survives_a_resize_too() {
    // The mirror image: the axis is horizontal, so the border is the last
    // column both before and after.
    let mut tumu = e2e::TumuHarness::with_orientation(24, 80, "horizontal");
    tumu.wait_for_column_text("[C-q ...]");

    tumu.resize(200, 12);

    tumu.wait_until("the border is still a column", |h| {
        h.screen_column(11).starts_with("└ [C-q ...]")
    });
}

#[test]
fn the_rotate_layout_command_moves_the_border_to_the_other_edge() {
    // The axis changes when it is asked to: `prefix r` on a vertical stack
    // makes the border a column instead of a row.
    let mut tumu = e2e::TumuHarness::with_orientation(40, 24, "vertical");
    tumu.wait_for_text("└ [C-q ...]");

    tumu.send_prefix();
    tumu.send_char('r');

    // The stack is horizontal now, so the border is written down the last
    // column, and no row carries the line any more.
    tumu.wait_for_column_text("[C-q ...]");
    assert!(
        !tumu.screen_contains("└ [C-q ...]"),
        "the border is no longer a row: {}",
        tumu.screen_text()
    );
}
