//! The renderer paints the stack, and never paints outside the frame.

mod helpers_tuinix;

use helpers_tuinix::size;

#[test]
fn the_renderer_never_paints_outside_the_frame() {
    let seed = noprop::seed_from_env_or_time("TUMU_SEED").expect("seed");
    let mut runner = noprop::Runner::new(seed);

    runner
        .run(256, |ctx| {
            let rows = noprop::sample_usize_in(ctx, 1..=40);
            let cols = noprop::sample_usize_in(ctx, 1..=120);
            let count = noprop::sample_usize_in(ctx, 0..=5);
            let mut state = tumu::State::new(size(rows, cols));
            for _ in 0..count {
                state.stack.push_top();
            }

            let frame = tumu::frame(&state);

            for (position, _) in frame.chars() {
                assert!(position.row < rows, "row {position:?} within {rows}");
                assert!(position.col < cols, "col {position:?} within {cols}");
            }
            Ok(())
        })
        .expect("no property failed");
}

#[test]
fn a_title_row_names_each_sheet() {
    let mut state = tumu::State::new(size(24, 80));
    state.stack.push_top();

    let frame = tumu::frame(&state);
    let first_row: String = frame
        .chars()
        .filter(|(position, _)| position.row == 0)
        .map(|(_, ch)| ch.value())
        .collect();

    assert!(first_row.contains("[0]"), "title row was {first_row:?}");
}
