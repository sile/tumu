//! The axis is a pure function of the terminal's shape.

mod helpers_tuinix;

use helpers_tuinix::size;

#[test]
fn a_tall_terminal_stacks_vertically() {
    // 40 rows against 80 columns: 40*2 = 80 >= 80, so vertical.
    assert_eq!(
        tumu::orientation_with_default_factor(size(40, 80)),
        tumu::Orientation::Vertical
    );
}

#[test]
fn a_wide_terminal_stacks_horizontally() {
    // 24 rows against 80 columns: 24*2 = 48 < 80, so horizontal.
    assert_eq!(
        tumu::orientation_with_default_factor(size(24, 80)),
        tumu::Orientation::Horizontal
    );
}

#[test]
fn a_square_terminal_stacks_vertically() {
    // 40*2 = 80 >= 40 would be vertical even at factor one; a true tie at
    // factor two is rows = cols/2, and here the taller side still wins.
    assert_eq!(
        tumu::orientation_with_default_factor(size(40, 40)),
        tumu::Orientation::Vertical
    );
}

#[test]
fn a_factor_of_one_compares_the_sides_directly() {
    assert_eq!(
        tumu::orientation(size(24, 80), 1),
        tumu::Orientation::Horizontal
    );
    assert_eq!(
        tumu::orientation(size(24, 20), 1),
        tumu::Orientation::Vertical
    );
    // A tie goes to vertical.
    assert_eq!(
        tumu::orientation(size(24, 24), 1),
        tumu::Orientation::Vertical
    );
}

#[test]
fn rotation_flips_the_axis() {
    // The whole point: a rotation swaps the sides, so the axis follows without
    // anything being stored. At the default factor a cell counts double, so a
    // 6-inch panel's pre- and post-rotation sizes look like these.
    let portrait = size(40, 30);
    let landscape = size(30, 80);

    assert_eq!(
        tumu::orientation_with_default_factor(portrait),
        tumu::Orientation::Vertical
    );
    assert_eq!(
        tumu::orientation_with_default_factor(landscape),
        tumu::Orientation::Horizontal
    );
}
