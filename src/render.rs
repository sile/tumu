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
/// The content sits one row below the top of the sheet, leaving that row for a
/// title. A sheet one row tall therefore has no content rows, which is how a
/// collapsed sheet is drawn: its single row is the title alone.
///
/// The result is a size, not a region, because inside a sheet the content's
/// top-left corner is always `(0, 1)`: the sheet's own frame starts at `(0, 0)`
/// and the screen position of the sheet is applied when the sheet is composed
/// into the screen, not when it is drawn.
pub fn sheet_content_size(size: tuinix::Size) -> tuinix::Size {
    tuinix::Size {
        rows: size.rows.saturating_sub(1),
        cols: size.cols,
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
    for (index, ((_, sheet), region)) in
        state.stack.sheets().iter().zip(state.regions()).enumerate()
    {
        // A sheet the full screen hides has no region, so there is nothing to
        // draw or place.
        if region.size.rows == 0 || region.size.cols == 0 {
            continue;
        }
        let mut sheet_frame = sheet_frame(index, sheet, region.size, focused == Some(index));
        if let Some(grid) = contents.grid(index) {
            let mut content = tuinix::Frame::new(sheet_content_size(region.size));
            draw_sheet_grid(&mut content, &grid);
            // The content frame is one row shorter than the sheet and sits one
            // row down, under the title.
            sheet_frame.put_frame(tuinix::Position { row: 1, col: 0 }, &content);
        }
        screen.put_frame(region.position, &sheet_frame);
    }
    screen
}

/// Draws one sheet's title into a fresh frame of size `size`.
///
/// The title is: the position in the stack, the title the child set (if any),
/// and the derived size. The first two are what the sheet *is* -- which one it
/// is, what the child calls itself -- and lead; the size trails, because it is
/// the part that can be spared when the sheet is too narrow to show everything.
/// A focused sheet -- the one the keyboard is aimed at -- is marked with a `>`
/// after the index, so a rotation or a full-screen toggle is visible in the
/// frame alone.
///
/// The frame is the sheet's own, so its right edge is the sheet's right edge:
/// a title that would cross it is clipped here and cannot reach a neighbour.
/// A title is the child's, so it may hold any character at all; its width is
/// measured rather than assumed (see [`push_str()`] below).
pub fn sheet_frame(
    index: usize,
    sheet: &crate::Sheet,
    size: tuinix::Size,
    focused: bool,
) -> tuinix::Frame {
    let mut frame = tuinix::Frame::new(size);
    // Focus leads the title, so it survives a narrow sheet that clips the rest:
    // a `[1]` there may be all that is left after the marker.
    let marker = if focused { ">" } else { " " };
    let right = size.cols;
    let mut position = tuinix::Position::ORIGIN;
    position = push_str(&mut frame, position, right, &format!("[{index}]{marker}"));
    position = push_str(&mut frame, position, right, &sheet.title);
    // A separator only when a title was written, so an untitled sheet does not
    // read as `[0]> 24x40` with a stray space.
    if !sheet.title.is_empty() {
        position = push_str(&mut frame, position, right, " ");
    }
    push_str(
        &mut frame,
        position,
        right,
        &format!("{}x{}", size.rows, size.cols),
    );
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
