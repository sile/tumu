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
fn a_screen_position_moves_into_the_sheets_grid() {
    // A sheet at the top of a horizontal stack: its border is its last column,
    // so the content starts at the sheet's own column 0 and the grid is one
    // column narrower than the sheet.
    let region = tuinix::Region {
        position: tuinix::Position { row: 0, col: 40 },
        size: size(24, 40),
    };
    let grid = termnix::Size {
        rows: std::num::NonZeroU16::new(24).expect("non-zero"),
        cols: std::num::NonZeroU16::new(39).expect("non-zero"),
    };

    // A point five rows down and three columns into the sheet lands at the same
    // offset in the grid.
    let position = tumu::screen_position_to_grid(
        tuinix::Position { row: 5, col: 43 },
        region,
        tuinix::Position::ORIGIN,
        grid,
    );
    assert_eq!(position.row, 5);
    assert_eq!(position.col, 3);
}

#[test]
fn a_position_outside_the_grid_is_clamped_into_it() {
    // The mouse protocols have no way to say "outside", and a drag that leaves
    // the sheet and comes back has to keep working, so a position past the grid
    // is clamped to its last cell rather than dropped.
    let region = tuinix::Region {
        position: tuinix::Position::ORIGIN,
        size: size(10, 20),
    };
    let grid = termnix::Size {
        rows: std::num::NonZeroU16::new(10).expect("non-zero"),
        cols: std::num::NonZeroU16::new(10).expect("non-zero"),
    };

    let position = tumu::screen_position_to_grid(
        tuinix::Position { row: 9, col: 19 },
        region,
        tuinix::Position::ORIGIN,
        grid,
    );
    assert_eq!(position.row, 9, "the last grid row");
    assert_eq!(position.col, 9, "the last grid column");

    // A position *before* the region -- the pointer above or left of the sheet
    // -- saturates at the grid's origin rather than wrapping.
    let position = tumu::screen_position_to_grid(
        tuinix::Position { row: 0, col: 0 },
        tuinix::Region {
            position: tuinix::Position { row: 5, col: 5 },
            size: size(10, 20),
        },
        tuinix::Position::ORIGIN,
        grid,
    );
    assert_eq!(position.row, 0);
    assert_eq!(position.col, 0);
}

#[test]
fn the_content_origin_is_taken_into_account() {
    // Vertically the border is the last row, so the content starts at the
    // sheet's own row 0 -- but a future border on the near edge would start it
    // further in, and the conversion must follow `origin` rather than assume.
    let region = tuinix::Region {
        position: tuinix::Position { row: 10, col: 0 },
        size: size(10, 20),
    };
    let grid = termnix::Size {
        rows: std::num::NonZeroU16::new(8).expect("non-zero"),
        cols: std::num::NonZeroU16::new(20).expect("non-zero"),
    };

    let position = tumu::screen_position_to_grid(
        tuinix::Position { row: 12, col: 3 },
        region,
        tuinix::Position { row: 1, col: 0 },
        grid,
    );
    assert_eq!(position.row, 1, "row 11 is the sheet's own row 1");
    assert_eq!(position.col, 3);
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
