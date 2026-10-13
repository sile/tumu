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
//! [`SheetGrids`] the edge fills in from its sessions. The core asks which grid
//! a sheet shows and paints it; it never reads a session itself. An empty
//! [`SheetGrids`] draws the chrome alone, which is what a test that only cares
//! about the layout passes.
//!
//! [`SheetGrids`] is a plain collection rather than a trait because there is
//! only ever one source of grids -- the edge's sessions -- so an interface to
//! abstract over would have no second implementation to justify it. The edge
//! fills the collection in and hands it over; a test fills it in by hand.

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
    /// The rows to paint, top to bottom.
    ///
    /// These are the rows the sheet shows, most recent at the bottom. When the
    /// sheet's content region is shorter than the session's visible grid -- the
    /// session was just shrunk and its older visible rows moved into history --
    /// the rows here begin with the tail of that history (see `history_rows`),
    /// so the region stays filled and a row that scrolled out of the grid is
    /// still shown rather than left blank. There are therefore never more rows
    /// here than the content region has lines: the edge trims to that before
    /// handing the grid over, bottom-aligned, so the top of the region is the
    /// oldest row still shown and the bottom is the newest.
    ///
    /// This is owned rather than a slice because [`Session`](termnix::Session)
    /// exposes its rows as an *iterator* of borrowed slices, not as a slice: the
    /// edge collects them, and the collection has to live somewhere for the
    /// duration of the paint. The cells themselves are still borrowed, so a
    /// grid is cheap to build and the session's memory is never copied.
    pub rows: Vec<&'a [termnix::Cell]>,

    /// How many of the leading entries in `rows` came from scrollback rather
    /// than from the session's visible grid.
    ///
    /// The visible grid's own top row is therefore at index `history_rows`, and
    /// the cursor -- whose position is in visible-grid coordinates -- sits that
    /// many rows further down the painted region. A grid handed over with no
    /// history in front of it sets this to `0`.
    pub history_rows: usize,

    /// The cursor position, relative to the visible grid's top-left corner.
    ///
    /// This is the session's own cursor, not a position in `rows`: the edge
    /// that fills a grid in adds `history_rows` when it needs the cursor's place
    /// in the painted region.
    pub cursor: termnix::Position,

    /// Whether the cursor should be drawn. A hidden cursor is not shown even
    /// when the region has room for it.
    pub cursor_visible: bool,
}

/// What every sheet shows, one entry per sheet, in stack order.
///
/// The contents of a sheet are the visible state of a live
/// [`Session`](termnix::Session), which the core does not own, so [`frame()`]
/// takes one of these instead of reaching for the sessions itself. The edge
/// builds it from its session table; a test can build it from literals.
///
/// It is positional -- entry `i` is what sheet `i` shows -- rather than a
/// lookup keyed by session, because the caller that has the grids is the caller
/// that painted the stack and so already walked it in stack order. A `None`
/// entry is a sheet with nothing to draw under its border, which is what a
/// sheet with no session looks like.
#[derive(Default)]
pub struct SheetGrids<'a> {
    /// One entry per sheet, in stack order.
    grids: Vec<Option<SheetGrid<'a>>>,
}

impl<'a> SheetGrids<'a> {
    /// Returns an empty set: the chrome and nothing under it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the grid for the next sheet, in stack order.
    ///
    /// Call once per sheet, top to bottom, so the position of the call is the
    /// sheet the grid belongs to and no index has to be passed or checked.
    pub fn push(&mut self, grid: Option<SheetGrid<'a>>) {
        self.grids.push(grid);
    }

    /// Returns the grid the sheet at `index` shows, if there is one.
    pub fn grid(&self, index: usize) -> Option<&SheetGrid<'a>> {
        self.grids.get(index)?.as_ref()
    }
}

/// Paints one sheet's grid into a sheet frame, from its top-left corner.
///
/// The frame is the sheet's *content* frame, one line shorter than the sheet, so
/// the border after it is left alone. Cells past the frame's right edge or below
/// its last row are dropped, so a grid larger than its frame is clipped rather
/// than spilling onto a neighbour.
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

/// Returns the size of a sheet's content when a *bordered* sheet is `size`.
///
/// The border takes one line off the sheet along the stack's axis: one of the
/// rows when the stack runs vertically, one of the columns when it runs
/// horizontally. The rest is the content. A sheet one cell long on that axis
/// therefore has no content cells, which is how a collapsed sheet is drawn: its
/// single line is the border alone.
///
/// The border sits on the edge the sheet advances towards along the axis -- the
/// **last** row of a vertical sheet, the **last** column of a horizontal one --
/// because a sheet owns the line that closes it off from the sheet after it in
/// the stack (see [`sheet_content_origin`]).
///
/// The result is a size, not a region, because inside a sheet the content's
/// top-left corner is always the sheet's own `(0, 0)`: the content is drawn
/// from there and the border is placed after it, so the sheet's own frame never
/// moves. The screen position of the sheet is applied when the sheet is composed
/// into the screen, not when it is drawn.
///
/// A full-screen sheet draws no border (see [`State::content_region_of`]), so
/// this size is only the bordered case; the full-screen case is the region
/// itself.
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
/// The border closes a sheet off at the far end of the axis, so the content
/// starts at the near end: the sheet's own top-left corner, `(0, 0)`, whichever
/// way the stack runs. The border is placed *after* the content, not before it,
/// so the grid never moves when the axis flips -- only the line does.
pub fn sheet_content_origin(_orientation: crate::Orientation) -> tuinix::Position {
    tuinix::Position::ORIGIN
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
/// own, so they arrive through `contents`, keyed by the session a sheet hosts.
/// A sheet the caller has no grid for is drawn as its border alone, which is
/// what a sheet with no session looks like.
pub fn frame(state: &State, contents: &SheetGrids) -> tuinix::Frame {
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
        // A full-screen sheet draws no border: it fills its region whole, so
        // there is no line to reserve and no `sheet_frame` to compose under it.
        let bordered = state.sheet_is_bordered(index);
        let mut sheet_frame = if bordered {
            sheet_frame(
                sheet,
                region.size,
                focused == Some(index),
                orientation,
                state.prefix_char,
            )
        } else {
            tuinix::Frame::new(region.size)
        };
        if let Some(grid) = contents.grid(index) {
            let content_size = if bordered {
                sheet_content_size(region.size, orientation)
            } else {
                region.size
            };
            let mut content = tuinix::Frame::new(content_size);
            draw_sheet_grid(&mut content, grid);
            // The content frame sits at the sheet's own corner; the border is
            // the line left over at the far end of the axis.
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

    // The extended menu uses the same box and is painted at the same time. The
    // two are never shown together -- opening one dismisses the other -- so the
    // order between them does not matter; they are drawn one after the other for
    // the one frame a key could ask for either.
    if state.ext_menu
        && let Some(layout) = state.ext_menu_layout()
    {
        screen.put_frame(layout.region.position, &menu_frame(&layout));
    }

    // The error window is painted last of all, over everything including the
    // menus: it is modal, so while it is up no key reaches the prefix and no
    // menu can be open at the same time. It lives outside the layout the same
    // way the menu does, which is why it is composed here rather than by
    // `regions()`.
    if let Some(layout) = state.error_layout() {
        screen.put_frame(layout.region.position, &error_frame(&layout));
    }

    screen
}

/// The corner a sheet's border starts at.
///
/// The border runs along the sheet's bottom edge when the stack is vertical and
/// its right edge when horizontal, and the line comes away from the corner
/// rightwards or (turned a quarter turn) downwards. `└` is the corner that joins
/// a line coming down from above to one going right, which is the shape a sheet
/// opening off a divider has.
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
/// The key text is derived from the configured prefix character
/// ([`prefix_key_text`]), not written here again, so a `--prefix o` shows `C-o`
/// on the label too.
///
/// [`prefix_key_text`]: crate::input::prefix_key_text
fn border_label(prefix_char: char) -> String {
    format!("[{} ...]", crate::input::prefix_key_text(prefix_char))
}

/// Draws one sheet's border into a fresh frame of size `size`.
///
/// The border is a single line along the stack's axis -- the last row when the
/// stack is vertical, the last column when it is horizontal -- that reads, from
/// the corner outwards: a corner (`└`), the command label (only on the focused
/// sheet), the title the child set (if any); the rest of the line up to the
/// sheet's edge is rule (`─`). The corner and the rule are what make the sheet
/// read as a sheet -- a line the stack is divided by -- and the label and the
/// title are what distinguish one from another.
///
/// The border is the line a sheet ends on, so it sits at the far end of the axis
/// from where the sheet starts: vertically it is the **last** row and
/// horizontally the **last** column. Stacked the other way round, the sheet that
/// starts at row 1 or column 1 draws its own line at row 0 or column 0, so the
/// lines still come one per sheet and each sheet keeps its own title.
///
/// When the stack is horizontal the same text is written *down the column*, one
/// character per row, so the border reads top to bottom the way it reads left to
/// right in a bottom edge.
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
    prefix_char: char,
) -> tuinix::Frame {
    // The line, corner to edge, as the text it reads. A separator follows the
    // corner and the label only when something comes after them, so an untitled
    // unfocused sheet is a bare `└` and then rule.
    let mut line = String::new();
    line.push(BORDER_CORNER);
    if focused {
        line.push(' ');
        line.push_str(&border_label(prefix_char));
        line.push(' ');
    }
    if !sheet.title.is_empty() {
        line.push_str(&sheet.title);
        line.push(' ');
    }

    let mut frame = tuinix::Frame::new(size);
    match orientation {
        crate::Orientation::Vertical => {
            // The border is the last row; the text runs left to right from the
            // sheet's left edge and stops at its right edge.
            let row = size.rows.saturating_sub(1);
            let right = size.cols;
            let start = tuinix::Position { row, col: 0 };
            let after_text = push_str(&mut frame, start, right, &line);
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
            // The border is the last column and reads downwards, one character
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

/// Paints a menu into a fresh frame the size of the box.
///
/// The box is drawn where [`MenuLayout`](crate::MenuLayout) says: a top border, a
/// header row naming the two columns, one row per binding, the box's trailer row
/// if it has one (the prefix menu's send-prefix row), and a bottom border. Each
/// row is painted across the box's interior width, so what is on screen is where
/// the layout said it would be.
///
/// The frame is the size of the box, so the renderer can compose it over the
/// sheet frames with [`Frame::put_frame`](tuinix::Frame::put_frame) at the box's
/// position; the rest of the screen is not part of this frame at all.
///
/// It draws any [`MenuLayout`](crate::MenuLayout) the same way, so the prefix
/// menu and the extended menu share one painting routine and the two cannot
/// drift in how a box reads -- only in which rows it holds.
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
    put_line(0, &border('┌', '┐'));

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

    // The bottom border goes on last: a content row is as wide as the interior,
    // so it can never reach the border row, but writing the border after them
    // leaves no ordering in which one row could erase it.
    put_line(layout.region.size.rows - 1, &border('└', '┘'));

    frame
}

/// Paints an error window into a fresh frame the size of the box.
///
/// The box is drawn where [`ErrorLayout`](crate::ErrorLayout) says: a top border,
/// one row per line of the message, and a bottom border. The lines are already
/// wrapped to the interior width by the layout, so each one is written as it
/// stands, left-aligned after the padding. The frame is the size of the box, so
/// the renderer composes it over the sheet frames exactly as it composes a
/// menu.
///
/// It is a box rather than a bare message so the failure reads as tumu's own
/// note -- the same chrome the menu has -- rather than as output of a sheet
/// underneath it.
pub fn error_frame(layout: &crate::ErrorLayout) -> tuinix::Frame {
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
    put_line(0, &border('┌', '┐'));

    // A content row: the padding, the line, and the room left over. The line is
    // already wrapped to the interior, so it can never reach the border column;
    // the padding is what keeps it off the box's edge. The top and bottom border
    // rows are the layout's first and last, so the lines start one row down.
    for (offset, text) in layout.lines.iter().enumerate() {
        put_line(offset + 1, &format!("│ {text}"));
    }

    put_line(layout.region.size.rows - 1, &border('└', '┘'));

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
