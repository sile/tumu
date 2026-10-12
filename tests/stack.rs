//! Properties and examples of the stack's size derivation.
//!
//! The axis is an argument now, not something read off the size, so every test
//! here names the one it means.

mod helpers_tuinix;

use helpers_tuinix::size;

/// Vertical, for the tests that stack down the rows.
const VERTICAL: tumu::Orientation = tumu::Orientation::Vertical;

/// Horizontal, for the tests that stack across the columns.
const HORIZONTAL: tumu::Orientation = tumu::Orientation::Horizontal;

#[test]
fn an_empty_stack_has_no_regions() {
    let stack = tumu::Stack::new();

    assert!(stack.is_empty());
    assert!(stack.regions(size(24, 80), VERTICAL).is_empty());
}

#[test]
fn a_single_sheet_fills_the_terminal() {
    let mut stack = tumu::Stack::new();
    stack.push_top();

    let regions = stack.regions(size(24, 80), VERTICAL);

    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].position.row, 0);
    assert_eq!(regions[0].position.col, 0);
    assert_eq!(regions[0].size.rows, 24);
    assert_eq!(regions[0].size.cols, 80);
}

#[test]
fn the_regions_fill_the_terminal_along_the_axis() {
    let seed = noprop::seed_from_env_or_time("TUMU_SEED").expect("seed");
    let mut runner = noprop::Runner::new(seed);

    runner
        .run(256, |ctx| {
            let rows = noprop::sample_usize_in(ctx, 1..=40);
            let cols = noprop::sample_usize_in(ctx, 1..=120);
            let size = size(rows, cols);
            let axis = if noprop::sample_bool(ctx) {
                VERTICAL
            } else {
                HORIZONTAL
            };
            let extent = match axis {
                tumu::Orientation::Vertical => rows,
                tumu::Orientation::Horizontal => cols,
            };
            // Every sheet needs at least one cell along the axis, so there
            // can be no more sheets than the axis is long.
            let count = noprop::sample_usize_in(ctx, 1..=extent);
            let num = noprop::sample_usize_in(ctx, 0..=8) as u32;
            let den = noprop::sample_usize_in(ctx, 0..=8) as u32;
            let mut stack = tumu::Stack::new();
            if let Some(ratio) = tumu::Ratio::new(num, den) {
                stack.set_ratio(ratio);
            }
            for _ in 0..count {
                stack.push_top();
            }

            let regions = stack.regions(size, axis);
            let total: usize = regions
                .iter()
                .map(|r| match axis {
                    tumu::Orientation::Vertical => r.size.rows,
                    tumu::Orientation::Horizontal => r.size.cols,
                })
                .sum();
            assert_eq!(total, extent, "extent for {count} sheets at {rows}x{cols}");
            // Each region spans the whole of the other side.
            for region in &regions {
                match axis {
                    tumu::Orientation::Vertical => {
                        assert_eq!(region.position.col, 0);
                        assert_eq!(region.size.cols, cols);
                        assert!(region.size.rows >= 1);
                    }
                    tumu::Orientation::Horizontal => {
                        assert_eq!(region.position.row, 0);
                        assert_eq!(region.size.rows, rows);
                        assert!(region.size.cols >= 1);
                    }
                }
            }
            Ok(())
        })
        .expect("no property failed");
}

/// Fills a stack with `count` sheets and returns the extent of each region
/// along `axis`, in stack order.
fn extents(
    rows: usize,
    cols: usize,
    count: usize,
    ratio: tumu::Ratio,
    axis: tumu::Orientation,
) -> Vec<usize> {
    let size = size(rows, cols);
    let mut stack = tumu::Stack::new();
    stack.set_ratio(ratio);
    for _ in 0..count {
        stack.push_top();
    }
    stack
        .regions(size, axis)
        .iter()
        .map(|r| match axis {
            tumu::Orientation::Vertical => r.size.rows,
            tumu::Orientation::Horizontal => r.size.cols,
        })
        .collect()
}

#[test]
fn the_default_odds_halve_the_remaining_space() {
    let rows = extents(24, 10, 3, tumu::Ratio::DEFAULT, VERTICAL);
    assert_eq!(rows, vec![12, 6, 6]);

    // The same odds read across a horizontal stack divide the columns instead.
    let cols = extents(10, 24, 3, tumu::Ratio::DEFAULT, HORIZONTAL);
    assert_eq!(cols, vec![12, 6, 6]);
}

#[test]
fn a_ratio_of_two_to_one_grows_every_sheet_but_the_last() {
    let rows = extents(24, 10, 3, tumu::Ratio::new(2, 1).expect("2:1"), VERTICAL);

    // 2:1 of 24 is 16, then 2:1 of the 8 left is 6 (rounded up), then 2.
    assert_eq!(rows, vec![16, 6, 2]);
}

#[test]
fn a_ratio_of_zero_hands_everything_to_the_sheets_below() {
    let rows = extents(24, 10, 3, tumu::Ratio::new(0, 1).expect("0:1"), VERTICAL);

    // The two top sheets are squeezed to the one row they must keep.
    assert_eq!(rows, vec![1, 1, 22]);
}

#[test]
fn a_new_sheet_goes_on_top() {
    let mut stack = tumu::Stack::new();
    let first = stack.push_top();
    let second = stack.push_top();

    let ids: Vec<tumu::SheetId> = stack.sheets().iter().map(|(id, _)| *id).collect();

    assert_eq!(ids, vec![second, first]);
}

#[test]
fn moving_a_sheet_to_the_top_reorders_the_stack() {
    let mut stack = tumu::Stack::new();
    let first = stack.push_top();
    let second = stack.push_top();
    let third = stack.push_top();

    // The bottom sheet (index 2) moves to the top.
    stack.move_to_top(2);

    let ids: Vec<tumu::SheetId> = stack.sheets().iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, vec![first, third, second]);
}

#[test]
fn hit_test_finds_the_topmost_sheet() {
    let mut stack = tumu::Stack::new();
    stack.push_top();
    stack.push_top();

    // A horizontal stack, so the split runs down the columns.
    let screen = size(24, 80);

    // The top sheet owns columns 0..40.
    assert_eq!(
        stack.hit_test(tuinix::Position { row: 0, col: 0 }, screen, HORIZONTAL),
        Some(0)
    );
    // The next owns columns 40..80.
    assert_eq!(
        stack.hit_test(tuinix::Position { row: 23, col: 79 }, screen, HORIZONTAL),
        Some(1)
    );
    // A row outside the terminal matches nothing.
    assert_eq!(
        stack.hit_test(tuinix::Position { row: 24, col: 0 }, screen, HORIZONTAL),
        None
    );
}
