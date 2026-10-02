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
fn a_vertical_split_divides_the_rows_and_keeps_the_columns() {
    let region = tumu::terminal_region(size(24, 80));

    let (lead, rest) = tumu::split_region(region, 10, tumu::Orientation::Vertical);

    assert_eq!(lead.position.row, 0);
    assert_eq!(lead.size.rows, 10);
    assert_eq!(rest.position.row, 10);
    assert_eq!(rest.size.rows, 14);
    assert_eq!(lead.size.cols, 80);
    assert_eq!(rest.size.cols, 80);
}

#[test]
fn a_horizontal_split_divides_the_columns_and_keeps_the_rows() {
    let region = tumu::terminal_region(size(24, 80));

    let (lead, rest) = tumu::split_region(region, 30, tumu::Orientation::Horizontal);

    assert_eq!(lead.position.col, 0);
    assert_eq!(lead.size.cols, 30);
    assert_eq!(rest.position.col, 30);
    assert_eq!(rest.size.cols, 50);
    assert_eq!(lead.size.rows, 24);
    assert_eq!(rest.size.rows, 24);
}

#[test]
fn a_vertical_split_clamps_so_neither_side_is_empty() {
    let region = tumu::terminal_region(size(10, 80));

    let (lead, rest) = tumu::split_region(region, 0, tumu::Orientation::Vertical);
    assert_eq!(lead.size.rows, 1);
    assert_eq!(rest.size.rows, 9);

    let (lead, rest) = tumu::split_region(region, 99, tumu::Orientation::Vertical);
    assert_eq!(lead.size.rows, 9);
    assert_eq!(rest.size.rows, 1);
}

#[test]
fn a_horizontal_split_clamps_so_neither_side_is_empty() {
    let region = tumu::terminal_region(size(10, 80));

    let (lead, rest) = tumu::split_region(region, 0, tumu::Orientation::Horizontal);
    assert_eq!(lead.size.cols, 1);
    assert_eq!(rest.size.cols, 79);

    let (lead, rest) = tumu::split_region(region, 99, tumu::Orientation::Horizontal);
    assert_eq!(lead.size.cols, 79);
    assert_eq!(rest.size.cols, 1);
}
