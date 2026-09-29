//! Unit tests for the geometry conversions and the split primitive.

mod helpers_tuinix;

use helpers_tuinix::size;

#[test]
fn the_terminal_region_starts_at_the_origin() {
    let region = tumu::terminal_region(size(24, 80));

    assert_eq!(region.position.row, 0);
    assert_eq!(region.position.col, 0);
    assert_eq!(region.size.rows, 24);
    assert_eq!(region.size.cols, 80);
}

#[test]
fn a_split_divides_the_rows_and_keeps_the_columns() {
    let region = tumu::terminal_region(size(24, 80));

    let (top, bottom) = tumu::split_region(region, 10);

    assert_eq!(top.position.row, 0);
    assert_eq!(top.size.rows, 10);
    assert_eq!(bottom.position.row, 10);
    assert_eq!(bottom.size.rows, 14);
    assert_eq!(top.size.cols, 80);
    assert_eq!(bottom.size.cols, 80);
}

#[test]
fn a_split_clamps_so_neither_side_is_empty() {
    let region = tumu::terminal_region(size(10, 80));

    let (top, bottom) = tumu::split_region(region, 0);
    assert_eq!(top.size.rows, 1);
    assert_eq!(bottom.size.rows, 9);

    let (top, bottom) = tumu::split_region(region, 99);
    assert_eq!(top.size.rows, 9);
    assert_eq!(bottom.size.rows, 1);
}
