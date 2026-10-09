//! The renderer paints the stack, and never paints outside the frame.
//!
//! A sheet is drawn as its own frame and composed onto the screen, so the
//! cases that matter are the two edges of that composition: a title clipped at
//! its own sheet's edge (it stops there and does not reach the neighbour) and a
//! grid clipped at the sheet's right edge. The whole-screen frame is still the
//! public thing, so these assert on `tumu::frame`.

mod helpers_tuinix;

use helpers_tuinix::size;

/// Builds a state whose first sheet is focused, as startup leaves it.
fn started(rows: usize, cols: usize) -> tumu::State {
    let mut state = tumu::State::new(size(rows, cols));
    state.update(tumu::Event::Resize(size(rows, cols)));
    state
}

/// A [`tumu::SheetContents`] that answers with one fixed grid.
///
/// The closure form is not usable for the trait -- a closure cannot return an
/// owned grid borrowing the caller's lifetime -- so tests spell the answer out.
/// This is the smallest thing that can: one grid for one index.
struct FixedGrid<'a> {
    index: usize,
    grid: tumu::SheetGrid<'a>,
}

impl tumu::SheetContents for FixedGrid<'_> {
    fn grid(&self, index: usize) -> Option<tumu::SheetGrid<'_>> {
        (index == self.index).then(|| tumu::SheetGrid {
            // `SheetGrid`'s rows are borrowed for `'_`, which here is the
            // caller's borrow of `self`; rebuilding the `Vec` each call is what
            // keeps the borrow honest as the trait's `'_` is per call.
            rows: self.grid.rows.clone(),
            cursor: self.grid.cursor,
            cursor_visible: self.grid.cursor_visible,
        })
    }
}

/// Returns the text of one row of a frame, blanks skipped.
fn row_text(frame: &tuinix::Frame, row: usize) -> String {
    frame
        .chars()
        .filter(|(position, ch)| position.row == row && !ch.is_blank())
        .map(|(_, ch)| ch.value())
        .collect()
}

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

            let frame = tumu::frame(&state, &());

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
    let state = started(24, 80);

    let frame = tumu::frame(&state, &());
    let first_row = row_text(&frame, 0);

    assert!(first_row.contains("[0]"), "title row was {first_row:?}");
}

#[test]
fn a_narrow_sheet_does_not_leak_its_title_onto_its_neighbour() {
    // Two sheets side by side, each 40 columns (`80x24` is a horizontal stack),
    // so the sheet at index 1 owns columns 40..80. It sets a title longer than
    // 40 columns: the title must be clipped at its own left edge (column 40) and
    // leave the sheet at columns 0..40 -- the other one -- untouched.
    let mut state = started(24, 80);
    state.stack.push_top();
    state.update(tumu::Event::TitleChanged {
        index: 1,
        title: "T".repeat(200),
    });

    let frame = tumu::frame(&state, &());
    let first_row = row_text(&frame, 0);

    // Sheet 0's own title survives at the left, and sheet 1's overflowing `T`s
    // did not run back over it.
    assert!(
        first_row.starts_with("[0]24x40"),
        "the neighbour's own title must survive: {first_row:?}"
    );
    let t_count = first_row.chars().filter(|&ch| ch == 'T').count();
    assert!(
        t_count <= 40,
        "the long title leaked past its own 40 columns ({t_count} T's): {first_row:?}"
    );
}

#[test]
fn a_sheet_with_no_contents_shows_only_its_title() {
    let state = started(24, 80);

    let frame = tumu::frame(&state, &());

    assert_eq!(row_text(&frame, 0), "[0]>24x80");
    assert_eq!(row_text(&frame, 1), "", "the content rows are blank");
}

#[test]
fn contents_are_painted_below_the_title_and_clipped_at_the_sheet_edge() {
    use termnix::{Cell, Style};

    /// Builds a single-width cell holding `ch`.
    fn cell(ch: char) -> Cell {
        Cell {
            ch,
            width: 1,
            style: Style::default(),
        }
    }

    // One sheet, 4 rows tall and `cols` wide, with a grid whose first row is
    // longer than the sheet is wide. The title owns row 0; the grid starts at
    // row 1 and must stop at the sheet's right edge.
    const COLS: usize = 8;
    let state = started(4, COLS);
    let long: Vec<Cell> = "abcdefghij".chars().map(cell).collect();
    let rows = vec![long.as_slice()];
    let grid = tumu::SheetGrid {
        rows,
        cursor: termnix::Position { row: 0, col: 0 },
        cursor_visible: false,
    };
    let contents = FixedGrid { index: 0, grid };

    let frame = tumu::frame(&state, &contents);

    assert_eq!(row_text(&frame, 0), format!("[0]>4x{COLS}"));
    assert_eq!(
        row_text(&frame, 1),
        "abcdefgh",
        "the grid is clipped to {COLS} columns"
    );

    for (position, _) in frame.chars() {
        assert!(position.col < COLS, "col {position:?} within {COLS}");
    }
}
