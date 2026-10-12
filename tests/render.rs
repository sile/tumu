//! The renderer paints the stack, and never paints outside the frame.
//!
//! A sheet is drawn as its own frame and composed onto the screen, so the
//! cases that matter are the two edges of that composition: a border clipped at
//! its own sheet's edge (it stops there and does not reach the neighbour) and a
//! grid clipped at the sheet's edge. The whole-screen frame is still the public
//! thing, so these assert on `tumu::frame`.
//!
//! The border runs along the stack's axis -- the last row when the stack is
//! vertical, the last column when it is horizontal -- and the axis is a
//! setting, so a test names the one it means rather than picking a size that
//! used to imply it.

mod helpers_tuinix;

use helpers_tuinix::size;

/// Vertical, for the tests whose border is a row.
const VERTICAL: tumu::Orientation = tumu::Orientation::Vertical;

/// Horizontal, for the tests whose border is a column.
const HORIZONTAL: tumu::Orientation = tumu::Orientation::Horizontal;

/// Builds a state of `rows`x`cols` stacked along `orientation`, whose first
/// sheet is focused, as startup leaves it.
fn started(rows: usize, cols: usize, orientation: tumu::Orientation) -> tumu::State {
    let mut state = tumu::State::new(size(rows, cols)).with_orientation(orientation);
    state.update(tumu::Event::Resize(size(rows, cols)));
    state
}

/// Returns a [`tumu::SheetGrids`] holding no grids: the chrome and nothing
/// under it.
///
/// This is what a test that asserts on the layout -- borders, titles, the menu
/// -- passes: the grids under the borders are not what it is looking at.
fn no_grids() -> tumu::SheetGrids<'static> {
    tumu::SheetGrids::new()
}

/// Returns a [`tumu::SheetGrids`] holding one grid, for sheet 0.
///
/// There is no trait to implement and nothing to clone: the collection is
/// filled in by hand, exactly as the edge fills it in from its sessions.
fn grids_of(grid: tumu::SheetGrid<'_>) -> tumu::SheetGrids<'_> {
    let mut grids = tumu::SheetGrids::new();
    grids.push(Some(grid));
    grids
}

/// Returns the text of one row of a frame, left to right.
///
/// Blanks are kept: a border is written with spaces in it (`└ [C-q ...]`), and
/// a test that reads the line back must see those spaces as part of it.
fn row_text(frame: &tuinix::Frame, row: usize) -> String {
    frame
        .chars()
        .filter(|(position, _)| position.row == row)
        .map(|(_, ch)| ch.value())
        .collect()
}

/// Returns the text of one column of a frame, top to bottom.
///
/// Blanks are kept for the reason [`row_text`] keeps them.
fn col_text(frame: &tuinix::Frame, col: usize) -> String {
    frame
        .chars()
        .filter(|(position, _)| position.col == col)
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

            let frame = tumu::frame(&state, &no_grids());

            for (position, _) in frame.chars() {
                assert!(position.row < rows, "row {position:?} within {rows}");
                assert!(position.col < cols, "col {position:?} within {cols}");
            }
            Ok(())
        })
        .expect("no property failed");
}

#[test]
fn a_vertical_border_names_the_focused_sheet_with_the_label() {
    // A tall, narrow terminal stacks vertically, so the border is the last row.
    let state = started(24, 40, VERTICAL);

    let frame = tumu::frame(&state, &no_grids());
    let bottom = row_text(&frame, 23);

    // The one sheet is focused, so its border carries the command label and
    // reads from the corner outwards, the rest of the row being rule.
    assert!(
        bottom.starts_with("└ [C-q ...]"),
        "focused border was {bottom:?}"
    );
    assert!(
        bottom.contains('─'),
        "the rest of the border is rule: {bottom:?}"
    );
}

#[test]
fn an_unfocused_border_has_no_label() {
    // Two sheets stacked vertically: the focused one (on top) carries the
    // label, the other (below) does not.
    let mut state = started(24, 40, VERTICAL);
    state.stack.push_top();
    // Move focus off the top sheet, so the one below it is the unfocused case.
    let second = state.stack.sheets()[1].0;
    state.focused = Some(second);

    let frame = tumu::frame(&state, &no_grids());

    // Every border starts with the corner, and the label appears exactly once.
    let screen: String = frame.chars().map(|(_, ch)| ch.value()).collect();
    assert_eq!(screen.matches("[C-q ...]").count(), 1);
    assert!(screen.matches('└').count() >= 2, "one corner per sheet");
}

#[test]
fn a_horizontal_border_is_written_vertically_in_the_last_column() {
    // A wide, short terminal stacks horizontally, so the border is the last
    // column of the sheet, the text read top to bottom.
    let state = started(24, 80, HORIZONTAL);

    let frame = tumu::frame(&state, &no_grids());
    let right = col_text(&frame, 79);

    assert!(
        right.starts_with("└ [C-q ...]"),
        "the last column holds the border written vertically: {right:?}"
    );
    assert!(
        col_text(&frame, 0).trim().is_empty(),
        "the sheet's first column is content, not border"
    );
}

#[test]
fn a_narrow_sheet_does_not_leak_its_border_onto_its_neighbour() {
    // Two sheets side by side (`24x80` stacks horizontally), each 40 columns,
    // so the sheet at index 1 owns columns 40..80. The border is the sheet's
    // last column, so index 1's is column 79 and index 0's is column 39. The
    // focused sheet (index 1) sets a title longer than 40 rows: the border must
    // be clipped at its own bottom edge and leave sheet 0 alone.
    let mut state = started(24, 80, HORIZONTAL);
    state.stack.push_top();
    state.update(tumu::Event::TitleChanged {
        index: 1,
        title: "T".repeat(200),
    });

    let frame = tumu::frame(&state, &no_grids());

    // Sheet 0 (columns 0..40) is unfocused: its own last column is a bare
    // corner and rule, with no label and no leaked `T`.
    let neighbour = col_text(&frame, 39);
    assert_eq!(
        neighbour,
        format!("└{}", "─".repeat(23)),
        "sheet 0 is unfocused: a bare corner and rule, no label and no leak"
    );
    let focused_col = col_text(&frame, 79);
    assert!(
        focused_col.starts_with("└ [C-q ...] T"),
        "the focused border and its title survive in its own column: {focused_col:?}"
    );
    // The title is clipped at the focused sheet's own bottom edge, so no `T`
    // appears anywhere in the other sheet's columns.
    let leaked = frame
        .chars()
        .filter(|(position, ch)| ch.value() == 'T' && position.col < 40)
        .count();
    assert_eq!(
        leaked, 0,
        "the long title leaked into its neighbour's columns"
    );
}

#[test]
fn a_sheet_with_no_contents_shows_only_its_border() {
    // Tall enough to stack vertically and wide enough that the border's text
    // fits on its row (a narrower sheet clips the label itself, which is its
    // own case elsewhere).
    let state = started(24, 40, VERTICAL);

    let frame = tumu::frame(&state, &no_grids());

    assert!(
        row_text(&frame, 23).starts_with("└ [C-q ...]"),
        "the border is the only thing drawn"
    );
    // Composing a sheet frame paints its whole rectangle, so the rows above the
    // border are blank cells rather than absent ones.
    assert!(
        row_text(&frame, 0).trim().is_empty(),
        "the content rows carry no text"
    );
}

#[test]
fn a_full_screen_sheet_draws_no_border_and_contents_fill_the_terminal() {
    use termnix::{Cell, Style};

    /// Builds a single-width cell holding `ch`.
    fn cell(ch: char) -> Cell {
        Cell {
            ch,
            width: 1,
            style: Style::default(),
        }
    }

    // Two sheets stacked vertically, the top one taken full screen. Without a
    // border the top sheet fills the whole 24 rows, so a grid row that would
    // have been clipped at the border is shown all the way down.
    let mut state = started(24, 40, VERTICAL);
    state.stack.push_top();
    state.focused = Some(state.stack.sheets()[0].0);
    // `prefix z` takes the focused sheet full screen, the same chord the user
    // presses; the state's own handler is what flips the flag.
    state.update(tumu::Event::Key(tumu::prefix_key(
        tumu::DEFAULT_PREFIX_CHAR,
    )));
    state.update(tumu::Event::Key(tuinix::KeyInput {
        code: tuinix::KeyCode::Char('z'),
        ctrl: false,
        alt: false,
    }));
    assert!(state.full_screen, "the top sheet is full screen");

    // A grid deep enough to reach the last row the border used to own.
    let deep: Vec<Vec<Cell>> = (0..24).map(|_| vec![cell('x'), cell('y')]).collect();
    let rows: Vec<&[Cell]> = deep.iter().map(|row| row.as_slice()).collect();
    let grid = tumu::SheetGrid {
        rows,
        history_rows: 0,
        cursor: termnix::Position { row: 0, col: 0 },
        cursor_visible: false,
    };
    let contents = grids_of(grid);

    let frame = tumu::frame(&state, &contents);
    let text: String = frame.chars().map(|(_, ch)| ch.value()).collect();

    // No border character anywhere: the full-screen sheet has no line to spend.
    assert!(!text.contains('└'), "no border corner is drawn: {text:?}");
    assert!(!text.contains('─'), "no border rule is drawn: {text:?}");
    // The last row is the grid's, not a border's. The rest of the row is blank
    // padding to the sheet's full width, which is what the border used to make
    // room for.
    assert_eq!(
        row_text(&frame, 23).trim_end(),
        "xy",
        "the last row carries content, not a border"
    );
}

#[test]
fn the_border_label_follows_the_configured_prefix_character() {
    // The label names the key that is actually in force, so a state built with
    // another prefix character must show that character and not the default.
    let mut state = tumu::State::new(size(24, 40)).with_prefix_char('o');
    state.update(tumu::Event::Resize(size(24, 40)));

    let frame = tumu::frame(&state, &no_grids());

    assert!(
        row_text(&frame, 23).starts_with("└ [C-o ...]"),
        "the label names the configured prefix"
    );
    assert!(
        !row_text(&frame, 23).contains("C-q"),
        "the default does not appear once the prefix is set"
    );
}

#[test]
fn contents_are_painted_to_the_left_of_the_border_and_clipped_at_the_sheet_edge() {
    use termnix::{Cell, Style};

    /// Builds a single-width cell holding `ch`.
    fn cell(ch: char) -> Cell {
        Cell {
            ch,
            width: 1,
            style: Style::default(),
        }
    }

    // One sheet, stacked vertically so the border owns the last *row* while the
    // sheet is still wide enough for the border's text, with a grid whose first
    // row is longer than the sheet is wide. The grid starts at row 0 and must
    // stop at the sheet's right edge.
    const COLS: usize = 20;
    let state = started(24, COLS, VERTICAL);
    let long: Vec<Cell> = "abcdefghijklmnopqrstuvwxyz".chars().map(cell).collect();
    let rows = vec![long.as_slice()];
    let grid = tumu::SheetGrid {
        rows,
        history_rows: 0,
        cursor: termnix::Position { row: 0, col: 0 },
        cursor_visible: false,
    };
    let contents = grids_of(grid);

    let frame = tumu::frame(&state, &contents);

    assert!(
        row_text(&frame, 23).starts_with("└ [C-q ...]"),
        "the border owns the last row"
    );
    assert_eq!(
        row_text(&frame, 0),
        "abcdefghijklmnopqrst",
        "the grid is clipped to {COLS} columns"
    );

    for (position, _) in frame.chars() {
        assert!(position.col < COLS, "col {position:?} within {COLS}");
    }
}

#[test]
fn the_menu_is_painted_over_the_sheets_when_it_is_open() {
    let mut state = started(24, 40, VERTICAL);
    assert!(!state.menu, "the menu is closed at startup");

    state.menu = true;
    let frame = tumu::frame(&state, &no_grids());
    let text: String = frame.chars().map(|(_, ch)| ch.value()).collect();

    // The box's corner and the header are on screen.
    assert!(text.contains('┌'), "a menu box is drawn");
    assert!(
        text.contains("command"),
        "the header names the label column"
    );

    // Every binding has a row naming its key and its label.
    for binding in tumu::BINDINGS {
        let key = tumu::chord_text(binding);
        assert!(text.contains(&key), "the menu lists the key {key}");
        assert!(
            text.contains(binding.label),
            "the menu lists {}",
            binding.label
        );
    }
}

#[test]
fn a_closed_menu_paints_nothing_over_the_sheets() {
    let mut state = started(24, 40, VERTICAL);
    state.menu = false;

    let frame = tumu::frame(&state, &no_grids());
    let text: String = frame.chars().map(|(_, ch)| ch.value()).collect();

    assert!(!text.contains('┌'), "no box when the menu is closed");
    assert!(!text.contains("command"), "no header either");
}

#[test]
fn the_menu_paints_the_rows_the_layout_names() {
    let mut state = started(24, 40, VERTICAL);
    state.menu = true;
    let layout = state.menu_layout().expect("the menu fits 24x40");
    let frame = tumu::frame(&state, &no_grids());

    // Every command row's own screen row shows its key and its label, so the
    // renderer and the hit-test use the same rectangles.
    for row in &layout.rows {
        let line = row_text(&frame, row.region.position.row);
        assert!(line.contains(&row.key), "row {:?} shows its key", row.key);
        assert!(
            line.contains(&row.label),
            "row {:?} shows its label",
            row.label
        );
    }
}

#[test]
fn an_open_menu_is_closed_by_a_border_on_all_four_sides() {
    // The box is a rectangle: a top border, a bottom border, and a vertical line
    // down each side. A box that is a row short loses its bottom border while its
    // top and sides stay, so the bottom is checked as well as the top.
    for name in ["prefix", "ext"] {
        let mut state = started(24, 40, VERTICAL);
        let layout = if name == "prefix" {
            state.menu = true;
            state.menu_layout()
        } else {
            state.ext_menu = true;
            state.ext_menu_layout()
        }
        .expect("the open menu fits 24x40");
        let frame = tumu::frame(&state, &no_grids());

        // The box's interior row, as it lands on screen: the box is composed
        // over the sheets, so a row is sliced to the box's columns before it is
        // read -- the sheet border underneath is not the menu's.
        let box_row = |row: usize| -> String {
            let col = layout.region.position.col;
            (col..col + layout.region.size.cols)
                .map(|col| {
                    frame
                        .chars()
                        .find(|(p, _)| p.row == row && p.col == col)
                        .map(|(_, ch)| ch.value())
                        .unwrap_or(' ')
                })
                .collect()
        };

        let top = box_row(layout.region.position.row);
        assert!(top.starts_with('┌'), "{name}: a top border: {top:?}");
        assert!(top.ends_with('┐'), "{name}: the top border closes: {top:?}");

        // The box's last row is the bottom border; it is the row the box ends
        // on, so a content row never reaches past it.
        let last = layout.region.position.row + layout.region.size.rows - 1;
        let bottom = box_row(last);
        assert!(
            bottom.starts_with('└'),
            "{name}: a bottom border: {bottom:?}"
        );
        assert!(
            bottom.ends_with('┘'),
            "{name}: the bottom border closes: {bottom:?}"
        );
    }
}
