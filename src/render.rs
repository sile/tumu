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
//! The two layers a frame is built from are painted in two places. The chrome
//! -- the title row of every visible sheet -- is drawn by [`frame()`] here, from
//! the state alone. A sheet's *contents* are the grid of a live
//! [`Session`](termnix::Session), which the core does not hold, so the edge
//! paints them with [`draw_sheet_grid()`] after the chrome: the two occupy
//! disjoint regions (a title is one row above the content), so the order does
//! not matter.

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
/// region it is painted into. A row shorter than the region leaves the rest of
/// the row as it was; a longer row is clipped.
pub struct SheetGrid<'a> {
    /// The visible rows, top to bottom.
    pub rows: &'a [&'a [termnix::Cell]],

    /// The cursor position, relative to the grid's top-left corner.
    pub cursor: termnix::Position,

    /// Whether the cursor should be drawn. A hidden cursor is not shown even
    /// when the region has room for it.
    pub cursor_visible: bool,
}

/// Paints one sheet's grid into `frame`, with its top-left corner at `region`.
///
/// Pass the sheet's *content* region (see [`sheet_content_region()`]), not its
/// full region: the title is painted one row above, and this must not cover it.
/// Cells past the region's right edge or below its last row are dropped, so a
/// grid larger than its region is clipped rather than spilling onto a
/// neighbour.
///
/// This is the contents painter, so unlike the rest of the core it takes
/// `termnix` cells: the grid belongs to a live session, which only the edge
/// holds. The rest of the core still deals in `tuinix` geometry alone.
pub fn draw_sheet_grid(frame: &mut tuinix::Frame, region: tuinix::Region, grid: &SheetGrid) {
    for (row, cells) in grid.rows.iter().enumerate() {
        if row >= region.size.rows {
            break;
        }
        for (col, cell) in cells.iter().enumerate() {
            if col >= region.size.cols {
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
            frame.put_char(
                tuinix::Position {
                    row: region.position.row + row,
                    col: region.position.col + col,
                },
                ch,
            );
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

/// Returns the region of a sheet's content inside `region`.
///
/// The content sits one row below the top of the region, leaving that row for a
/// title. A region one row tall therefore has no content rows, which is how a
/// collapsed sheet is drawn: its single row is the title alone.
pub fn sheet_content_region(region: tuinix::Region) -> tuinix::Region {
    tuinix::Region {
        position: tuinix::Position {
            row: region.position.row + 1,
            col: region.position.col,
        },
        size: tuinix::Size {
            rows: region.size.rows.saturating_sub(1),
            cols: region.size.cols,
        },
    }
}

/// Paints `state`'s chrome into a fresh frame of the state's size.
///
/// Each sheet is drawn as its title row: the position in the stack and the
/// derived size, so the ordering and the layout are visible on their own -- and
/// a test can assert on them without a live session behind them. The title
/// marks the focused sheet -- the one the keyboard is aimed at -- with a `>`
/// after the index, so a rotation or a full-screen toggle is visible in the
/// frame alone.
///
/// The sheets' *contents* are not drawn here: they are the grids of the live
/// sessions, which the core does not hold. The edge paints them afterwards with
/// [`draw_sheet_grid()`], into the content regions this leaves for them.
pub fn frame(state: &State) -> tuinix::Frame {
    let mut frame = tuinix::Frame::new(state.size);
    let focused = state.focused_index();
    for (index, region) in state.regions().into_iter().enumerate() {
        // A sheet the full screen hides has no region, so there is no row to
        // title.
        if region.size.rows == 0 || region.size.cols == 0 {
            continue;
        }
        // Focus leads the title, so it survives a narrow region that clips the
        // rest: a `[1]` there may be all that is left after the marker.
        let marker = if Some(index) == focused { ">" } else { " " };
        let title = format!("[{index}]{marker}{}x{}", region.size.rows, region.size.cols);
        let right = region.position.col + region.size.cols;
        let mut position = region.position;
        for ch in title.chars() {
            if position.col >= right {
                break;
            }
            // Titles are ASCII, so each character advances one column. A title
            // that held a wide character would need its real width here.
            let Some(cell) = tuinix::Char::new(ch, 1, tuinix::Style::RESET) else {
                continue;
            };
            position = frame.put_char(position, cell);
        }
    }
    frame
}
