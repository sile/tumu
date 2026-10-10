//! Painting the stack into a [`Frame`](tuinix::Frame).
//!
//! The layout comes from [`State::regions`](crate::State::regions), never from
//! the stack directly: that one call is where full screen narrows the drawing to
//! the focused sheet, so the renderer needs no rule of its own for it.
//!
//! Drawing is free of I/O: it reads a [`State`](crate::State) and writes into a
//! frame, so a test can assert on the frame as data. What the edge does with the
//! frame -- comparing it to the last one and writing the terminal -- is the
//! edge's business.
//!
//! A sheet is drawn as its own small frame -- the size of its region -- and the
//! screen is those frames composed with
//! [`Frame::put_frame()`](tuinix::Frame::put_frame). That keeps a sheet's drawing
//! free of screen coordinates: everything inside a sheet is written from
//! `(0, 0)`, and the one place a sheet's position enters the screen is the
//! `put_frame` call that places it. Clipping falls out of the same call: a
//! character past the sheet's right edge is past the *sheet's* edge and is
//! dropped there, so a title can never land on a neighbour.
//!
//! A sheet's *contents* are the grid of a live [`Session`](termnix::Session),
//! which the core does not hold, so [`frame()`] takes them as a parameter: a
//! [`SheetContents`] the edge implements over its sessions. The core asks which
//! grid a sheet shows and paints it; it never reads a session itself. `()`, which
//! answers `None` for every index, draws the chrome alone: that is what a test
//! that only cares about the titles passes.

use crate::State;

/// One sheet's terminal grid, ready to be painted into a frame.
///
/// This is the edge's view of the [`Session`](termnix::Session) a sheet shows:
/// the visible rows, the cursor, and whether that cursor is shown. It borrows
/// values the edge already owns, so the core stays free of `termnix` types of
/// its own -- the struct is defined here only so both sides agree on the shape,
/// and the rows it borrows are `termnix` cells.
///
/// Rows are top to bottom, each a left-to-right slice of cells as wide as the
/// frame it is painted into. A row shorter than the frame leaves the rest of
/// the row as it was; a longer row is clipped.
pub struct SheetGrid<'a> {
    /// The visible rows, top to bottom.
    ///
    /// This is owned rather than a slice because [`Session`](termnix::Session)
    /// exposes its rows as an *iterator* of borrowed slices, not as a slice: the
    /// edge collects them, and the collection has to live somewhere for the
    /// duration of the paint. The cells themselves are still borrowed, so a
    /// grid is cheap to build and the session's memory is never copied.
    pub rows: Vec<&'a [termnix::Cell]>,

    /// The cursor position, relative to the grid's top-left corner.
    pub cursor: termnix::Position,

    /// Whether the cursor should be drawn. A hidden cursor is not shown even
    /// when the region has room for it.
    pub cursor_visible: bool,
}

/// A source of sheet grids, one per sheet index.
///
/// The grid of a sheet is the visible state of a live
/// [`Session`](termnix::Session), which the core does not own, so [`frame()`]
/// takes one of these instead of reaching for the sessions itself. The edge
/// answers from its session table; a test can answer from a literal. Answering
/// `None` for every index is a valid contents source, and draws the chrome
/// with no contents under it.
pub trait SheetContents {
    /// Returns the grid shown by the sheet at `index`, or `None` if it has none.
    fn grid(&self, index: usize) -> Option<SheetGrid<'_>>;
}

impl SheetContents for () {
    /// Answers `None` for every sheet: the chrome with nothing under it.
    fn grid(&self, _index: usize) -> Option<SheetGrid<'_>> {
        None
    }
}

/// Paints one sheet's grid into a sheet frame, from its top-left corner.
///
/// The frame is the sheet's *content* frame, one row shorter than the sheet, so
/// the title row above it is left alone. Cells past the frame's right edge or
/// below its last row are dropped, so a grid larger than its frame is clipped
/// rather than spilling onto a neighbour.
pub fn draw_sheet_grid(frame: &mut tuinix::Frame, grid: &SheetGrid) {
    let size = frame.size();
    for (row, cells) in grid.rows.iter().enumerate() {
        if row >= size.rows {
            break;
        }
        for (col, cell) in cells.iter().enumerate() {
            if col >= size.cols {
                break;
            }
            // A width-0 cell is the trailing half of a wide glyph; it stands
            // for no character of its own, so the leading cell's width covers
            // it and there is nothing to write here.
            if cell.width == 0 {
                continue;
            }
            let Some(ch) =
                tuinix::Char::new(cell.ch, usize::from(cell.width), convert_style(cell.style))
            else {
                continue;
            };
            frame.put_char(tuinix::Position { row, col }, ch);
        }
    }
}

/// Converts a [`termnix`] cell style into a [`tuinix`] style.
///
/// The two crates describe the same thing under different names: `termnix`
/// spells "the terminal's default" as `None`, which is what `tuinix` uses too,
/// so the two colors map across unchanged. The flags `tuinix` has no field for
/// (italic, blink, dim, strikethrough) are dropped rather than guessed at.
fn convert_style(style: termnix::Style) -> tuinix::Style {
    tuinix::Style {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        reverse: style.reverse,
        fg_color: convert_color(style.foreground),
        bg_color: convert_color(style.background),
        ..tuinix::Style::new()
    }
}

/// Converts a [`termnix`] color into a [`tuinix`] color.
///
/// `None` is "the terminal's default" in both crates, so it passes straight
/// through; an indexed or RGB color is the same shape on both sides.
fn convert_color(color: Option<termnix::Color>) -> Option<tuinix::Color> {
    match color? {
        termnix::Color::Indexed(index) => Some(tuinix::Color::Indexed(index)),
        termnix::Color::Rgb(rgb) => Some(tuinix::Color::Rgb(rgb.r, rgb.g, rgb.b)),
    }
}

/// Returns the size of a sheet's content when the sheet is `size`.
///
/// The border takes one line off the sheet along the stack's axis: one of the
/// rows when the stack runs vertically, one of the columns when it runs
/// horizontally. The rest is the content. A sheet one cell long on that axis
/// therefore has no content cells, which is how a collapsed sheet is drawn: its
/// single line is the border alone.
///
/// Which line it is follows the axis, and which *side* of the sheet it sits on
/// follows the axis too: the border divides a sheet from the one below it or the
/// one to its right, so it is the top row of a vertical sheet and the last
/// column of a horizontal one (see [`sheet_content_origin`]).
///
/// The result is a size, not a region, because inside a sheet the content's
/// top-left corner is always the sheet's own `(0, 0)`: the content is drawn
/// from there and the border is placed around it, so the sheet's own frame never
/// moves. The screen position of the sheet is applied when the sheet is composed
/// into the screen, not when it is drawn.
pub fn sheet_content_size(size: tuinix::Size, orientation: crate::Orientation) -> tuinix::Size {
    match orientation {
        crate::Orientation::Vertical => tuinix::Size {
            rows: size.rows.saturating_sub(1),
            cols: size.cols,
        },
        crate::Orientation::Horizontal => tuinix::Size {
            rows: size.rows,
            cols: size.cols.saturating_sub(1),
        },
    }
}

/// Returns where a sheet's content starts inside the sheet's own frame.
///
/// The border sits on the edge that divides the sheet from its neighbour along
/// the axis: the **top** row when the stack runs vertically, because the sheet
/// below it is the one the line is drawn for, and the **right** column when it
/// runs horizontally, for the sheet to its left. The content therefore starts
/// one cell in from the opposite edge -- down one row vertically, and at the
/// sheet's own left edge horizontally, which is the origin itself.
///
/// That is what keeps a sheet's content at its own `(0, 0)`: the frame is built
/// at the content's corner and the border is placed around it, so flipping the
/// axis moves the border, not the grid.
pub fn sheet_content_origin(orientation: crate::Orientation) -> tuinix::Position {
    match orientation {
        crate::Orientation::Vertical => tuinix::Position { row: 1, col: 0 },
        crate::Orientation::Horizontal => tuinix::Position::ORIGIN,
    }
}

/// Paints `state` and `contents` into a fresh frame of the state's size.
///
/// Each visible sheet is drawn into a frame of its own region (see
/// [`sheet_frame()`]) and the sheet frames are composed onto the screen with
/// [`Frame::put_frame()`](tuinix::Frame::put_frame). Composing a sheet frame
/// paints its whole rectangle, blanks included, but a sheet's region does not
/// overlap its neighbours', so no sheet can erase another; a hidden sheet (full
/// screen on another) has an empty region and is skipped whole.
///
/// The contents are the grids of the live sessions, which the core does not
/// own, so they arrive through `contents`. A sheet the source answers `None`
/// for is drawn as its title row alone, which is what a sheet with no session
/// looks like.
pub fn frame(state: &State, contents: &impl SheetContents) -> tuinix::Frame {
    let mut screen = tuinix::Frame::new(state.size);
    let focused = state.focused_index();
    let orientation = state.orientation();
    for (index, ((_, sheet), region)) in
        state.stack.sheets().iter().zip(state.regions()).enumerate()
    {
        // A sheet the full screen hides has no region, so there is nothing to
        // draw or place.
        if region.size.rows == 0 || region.size.cols == 0 {
            continue;
        }
        let mut sheet_frame = sheet_frame(sheet, region.size, focused == Some(index), orientation);
        if let Some(grid) = contents.grid(index) {
            let mut content = tuinix::Frame::new(sheet_content_size(region.size, orientation));
            draw_sheet_grid(&mut content, &grid);
            // The content frame sits one cell along the axis, past the border.
            sheet_frame.put_frame(sheet_content_origin(orientation), &content);
        }
        screen.put_frame(region.position, &sheet_frame);
    }

    // The menu is painted last, over everything: it is a view laid on top of
    // the stack, so it may cover parts of several sheets. It is not part of the
    // layout, so it is composed here rather than by `regions()`.
    if state.menu
        && let Some(layout) = state.menu_layout()
    {
        screen.put_frame(layout.region.position, &menu_frame(&layout));
    }

    screen
}

/// The corner a sheet's border starts at.
///
/// The border runs along the sheet's top edge when the stack is vertical and
/// its left edge when horizontal, and the line comes away from the corner
/// downwards or rightwards. `└` is the corner that joins a border below it to
/// one going right, which is exactly this shape.
const BORDER_CORNER: char = '└';

/// The rule the rest of a border is drawn with.
const BORDER_RULE: char = '─';

/// Returns the label the focused sheet shows where a command can be started.
///
/// It is the prefix key written the way it is pressed, followed by a mark that
/// more can follow -- the ellipsis the README draws -- so a sheet says that the
/// keyboard is aimed at it and that a command is one key away. Only the focused
/// sheet shows it; every other sheet's border starts straight into its title,
/// which is what makes the sheet under the keyboard obvious at a glance.
///
/// The key text is derived from the prefix ([`prefix_key_text`]), not written
/// here again, so restoring the prefix to `C-o` changes this label with it.
///
/// [`prefix_key_text`]: crate::input::prefix_key_text
fn border_label() -> String {
    format!("[{} ...]", crate::input::prefix_key_text())
}

/// Draws one sheet's border into a fresh frame of size `size`.
///
/// The border is a single line along the stack's axis -- the top row when the
/// stack is vertical, the right column when it is horizontal -- that reads, from
/// the corner outwards: a corner (`└`), the command label (only on the focused
/// sheet), the title the child set (if any); the rest of the line up to the
/// sheet's edge is rule (`─`). The corner and the rule are what make the sheet
/// read as a sheet -- a line the stack is divided by -- and the label and the
/// title are what distinguish one from another.
///
/// The border divides a sheet from its neighbour along the axis, so it sits on
/// the edge nearest that neighbour: vertically both sheets span the width and
/// the one below is the one the line separates off, so it is the **top** row;
/// horizontally the stack runs the other way, so the neighbour is the one to the
/// **left** and the line is the **right** column. The corner therefore reads at
/// the end of the line the sheets advance towards.
///
/// When the stack is horizontal the same text is written *down the column*, one
/// character per row, so the border reads top to bottom the way it reads left to
/// right in a top edge.
///
/// The frame is the sheet's own, so a border that would cross its far edge is
/// clipped here and cannot reach a neighbour. A title is the child's, so it may
/// hold any character at all; in a vertical stack its width is measured rather
/// than assumed (see [`push_str()`] below), while a horizontal border is one
/// cell per character by construction.
pub fn sheet_frame(
    sheet: &crate::Sheet,
    size: tuinix::Size,
    focused: bool,
    orientation: crate::Orientation,
) -> tuinix::Frame {
    // The line, corner to edge, as the text it reads. A separator follows the
    // corner and the label only when something comes after them, so an untitled
    // unfocused sheet is a bare `└` and then rule.
    let mut line = String::new();
    line.push(BORDER_CORNER);
    if focused {
        line.push(' ');
        line.push_str(&border_label());
        line.push(' ');
    }
    if !sheet.title.is_empty() {
        line.push_str(&sheet.title);
        line.push(' ');
    }

    let mut frame = tuinix::Frame::new(size);
    match orientation {
        crate::Orientation::Vertical => {
            // The border is the top row; the text runs left to right and stops
            // at the sheet's right edge.
            let right = size.cols;
            let after_text = push_str(&mut frame, tuinix::Position::ORIGIN, right, &line);
            // The rule fills the rest of the row, corner to edge.
            let mut position = after_text;
            while position.col < right {
                let Some(cell) = tuinix::Char::new(BORDER_RULE, 1, tuinix::Style::RESET) else {
                    break;
                };
                position = frame.put_char(position, cell);
            }
        }
        crate::Orientation::Horizontal => {
            // The border is the right column and reads downwards, one character
            // per row, so a line longer than the sheet is clipped at the bottom
            // edge just as a vertical one is at the right edge.
            let col = size.cols.saturating_sub(1);
            let bottom = size.rows;
            let mut row = 0;
            for ch in line.chars() {
                if row >= bottom {
                    break;
                }
                let Some(cell) = tuinix::Char::new(ch, 1, tuinix::Style::RESET) else {
                    continue;
                };
                frame.put_char(tuinix::Position { row, col }, cell);
                row += 1;
            }
            while row < bottom {
                let Some(cell) = tuinix::Char::new(BORDER_RULE, 1, tuinix::Style::RESET) else {
                    break;
                };
                frame.put_char(tuinix::Position { row, col }, cell);
                row += 1;
            }
        }
    }
    frame
}

/// Paints the prefix menu into a fresh frame the size of the screen.
///
/// The box is drawn where [`MenuLayout`](crate::MenuLayout) says: a top border, a
/// header row naming the two columns, one row per binding, the send-prefix row,
/// and a bottom border. Each row is the box's interior width, so a click
/// anywhere across a row selects it, matching what
/// [`MenuLayout::command_at`](crate::MenuLayout::command_at) tests.
///
/// The frame is the size of the screen so the renderer can compose it over the
/// sheet frames with [`Frame::put_frame`](tuinix::Frame::put_frame) at the
/// origin; only the box's rectangle is written, so the rest is blank and, being
/// blank, leaves the sheets under it alone when composed.
///
/// [`MenuLayout`]: crate::MenuLayout
pub fn menu_frame(layout: &crate::MenuLayout) -> tuinix::Frame {
    let mut frame = tuinix::Frame::new(layout.region.size);
    let width = layout.region.size.cols;

    // Writes `text` at the start of row `row`, one character per cell.
    let mut put_line = |row: usize, text: &str| {
        let mut position = tuinix::Position { row, col: 0 };
        for ch in text.chars() {
            if let Some(cell) = tuinix::Char::new(ch, 1, tuinix::Style::RESET) {
                position = frame.put_char(position, cell);
            }
        }
    };

    // The top and bottom borders: a corner, a rule, and the opposite corner.
    let inner_width = width.saturating_sub(2);
    let border = |left_corner: char, right_corner: char| {
        let mut line = String::new();
        line.push(left_corner);
        for _ in 0..inner_width {
            line.push(BORDER_RULE);
        }
        line.push(right_corner);
        line
    };
    let bottom_row = layout.region.size.rows - 1;
    put_line(0, &border('┌', '┐'));
    put_line(bottom_row, &border('└', '┘'));

    // A content row: `| key  label |`, the two columns padded to their fixed
    // widths so the labels line up across rows. The row's screen row is turned
    // into a frame row by subtracting the box's top, since the frame is the box
    // alone and is composed at the box's position.
    for row in &layout.rows {
        let text = format!(
            "│ {key:<key_width$}{gap}{label:<label_width$} │",
            key = row.key,
            key_width = layout.key_width,
            gap = " ".repeat(crate::menu::GAP),
            label = row.label,
            label_width = layout.label_width,
        );
        put_line(row.region.position.row - layout.region.position.row, &text);
    }

    frame
}

/// Writes `text` at `position`, stopping at column `right`, and returns the
/// position just past what was written.
///
/// Each character's width is measured with `unicode-width`, the same UAX #11
/// table `termnix` lays its grid out with, so a title and the content under it
/// agree on how wide a character is. A character that would end past `right`
/// is dropped, and so is one whose full width does not fit in the columns left
/// -- a width-2 character with a single column to go is skipped rather than
/// half-painted over the sheet's right edge.
fn push_str(
    frame: &mut tuinix::Frame,
    mut position: tuinix::Position,
    right: usize,
    text: &str,
) -> tuinix::Position {
    use unicode_width::UnicodeWidthChar;
    for ch in text.chars() {
        let Some(width) = ch.width() else {
            continue;
        };
        if width == 0 || position.col + width > right {
            break;
        }
        let Some(cell) = tuinix::Char::new(ch, width, tuinix::Style::RESET) else {
            continue;
        };
        position = frame.put_char(position, cell);
    }
    position
}
