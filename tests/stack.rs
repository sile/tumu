//! Properties and examples of the stack's size derivation.

mod helpers_tuinix;

use helpers_tuinix::size;

#[test]
fn an_empty_stack_has_no_regions() {
    let stack = tumu::Stack::new();

    assert!(stack.is_empty());
    assert!(stack.regions(size(24, 80)).is_empty());
}

#[test]
fn a_single_window_fills_the_terminal() {
    let mut stack = tumu::Stack::new();
    stack.push_top();

    let regions = stack.regions(size(24, 80));

    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].position.row, 0);
    assert_eq!(regions[0].position.col, 0);
    assert_eq!(regions[0].size.rows, 24);
    assert_eq!(regions[0].size.cols, 80);
}

#[test]
fn the_regions_sum_to_the_terminal_rows() {
    let seed = noprop::seed_from_env_or_time("TUMU_SEED").expect("seed");
    let mut runner = noprop::Runner::new(seed);

    runner
        .run(256, |ctx| {
            let rows = noprop::sample_usize_in(ctx, 1..=40);
            let cols = noprop::sample_usize_in(ctx, 1..=120);
            // Every window needs at least one row, so there can be no more
            // windows than the terminal has rows.
            let count = noprop::sample_usize_in(ctx, 1..=rows);
            let mut stack = tumu::Stack::new();
            for _ in 0..count {
                stack.push_top();
            }

            let regions = stack.regions(size(rows, cols));
            let total: usize = regions.iter().map(|r| r.size.rows).sum();
            assert_eq!(total, rows, "rows for {count} windows at {rows}");
            for region in &regions {
                assert_eq!(region.position.col, 0);
                assert_eq!(region.size.cols, cols);
                assert!(region.size.rows >= 1);
            }
            Ok(())
        })
        .expect("no property failed");
}

#[test]
fn equal_weights_halve_the_remaining_space() {
    let mut stack = tumu::Stack::new();
    stack.push_top();
    stack.push_top();
    stack.push_top();

    // 1:1 then 1:1 of the rest, as the README spells the odds: 12, 6, 6 for 24.
    let rows: Vec<usize> = stack
        .regions(size(24, 80))
        .iter()
        .map(|r| r.size.rows)
        .collect();

    assert_eq!(rows, vec![12, 6, 6]);
}

#[test]
fn a_new_window_goes_on_top() {
    let mut stack = tumu::Stack::new();
    let first = stack.push_top();
    let second = stack.push_top();

    let ids: Vec<tumu::WindowId> = stack.windows().iter().map(|(id, _)| *id).collect();

    assert_eq!(ids, vec![second, first]);
}

#[test]
fn moving_a_window_to_the_top_reorders_the_stack() {
    let mut stack = tumu::Stack::new();
    let first = stack.push_top();
    let second = stack.push_top();
    let third = stack.push_top();

    // The bottom window (index 2) moves to the top.
    stack.move_to_top(2);

    let ids: Vec<tumu::WindowId> = stack.windows().iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, vec![first, third, second]);
}

#[test]
fn hit_test_finds_the_topmost_window() {
    let mut stack = tumu::Stack::new();
    stack.push_top();
    stack.push_top();

    let screen = size(24, 80);

    // The top window owns rows 0..12.
    assert_eq!(
        stack.hit_test(tuinix::Position { row: 0, col: 0 }, screen),
        Some(0)
    );
    // The next owns rows 12..24.
    assert_eq!(
        stack.hit_test(tuinix::Position { row: 23, col: 79 }, screen),
        Some(1)
    );
    // A column outside the terminal matches nothing.
    assert_eq!(
        stack.hit_test(tuinix::Position { row: 0, col: 80 }, screen),
        None
    );
}
