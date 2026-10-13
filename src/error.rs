//! The error window: its layout as data, derived from a message.
//!
//! When a sheet's program fails to launch -- a `$TUMU_EDITOR` that is missing, a
//! `$TUMU_MAMEDIFF` that is not installed -- the failure is not a reason to end
//! the whole run. The sheet that could not start is closed and the failure is
//! shown in a small box centred on the terminal, the same shape the menu is: the
//! user reads what went wrong and dismisses it with a key or a click.
//!
//! Like the menu, it is a *view*: it changes no sheet's region, so it lives
//! outside [`State::regions`](crate::State::regions) and is painted over the
//! composed screen afterwards. Nothing here draws: the layout is the box and the
//! lines of text to put in it, and the renderer paints what it says.

/// The error window as it is laid out on a screen of a given size.
///
/// The rectangle is in screen coordinates, so the renderer can paint the box
/// without knowing how it was centred, and a test can read where it went. Unlike
/// [`MenuLayout`](crate::MenuLayout) there are no columns and no per-row
/// rectangles: the window is a message, not a table, so the lines are just the
/// text to write, one per interior row, left-aligned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorLayout {
    /// The whole box, border included.
    pub region: tuinix::Region,

    /// The interior lines, top to bottom: the message, already wrapped to the
    /// box's width. There is one entry per interior row, so the box is always as
    /// tall as the lines it holds plus its two borders.
    pub lines: Vec<String>,
}

/// How many blank columns sit between the box's border and its text on each
/// side.
const PADDING: usize = 1;

/// Lays an error window holding `message` out on a screen of `size`, or returns
/// `None` when there is not enough room for it.
///
/// The box is centred on the screen. Its width is the longest line of the
/// message plus the border columns and the padding, and its height is the
/// number of lines plus the two border rows. A message is wrapped so the box is
/// never wider than the screen, and when even a wrapped box does not fit the
/// layout is `None` and the window is not shown at all -- the same rule the menu
/// follows.
///
/// The wrapping is on characters, not words: an error names a program and a
/// cause (`failed to start \"mamediff\"`), and those tokens are what a reader
/// copies out, so breaking a line mid-word would make the one useful thing in
/// the box harder to read than a slightly ragged edge.
///
/// The box is capped in height too. A message is short by construction -- the
/// edge builds it from one program and one `io::Error` -- but the cap keeps a
/// pathological one from filling the screen and hiding every sheet behind it,
/// which is where the user has to go to read the box's advice.
pub fn error_layout(size: tuinix::Size, message: &str) -> Option<ErrorLayout> {
    // The widest the box may be: the screen, so a wrapped line always fits.
    // The interior is that much narrower by the border and the padding, and a
    // box narrower than a single character plus its chrome has no room for any
    // text at all.
    let max_width = size.cols.min(MAX_WIDTH);
    let interior_width = max_width.checked_sub(2 + 2 * PADDING)?;
    if interior_width == 0 {
        return None;
    }

    // Wrap every line of the message to the interior width, then cap the number
    // of rows: a box taller than the screen could not be centred on it, and a
    // box that fills the screen is not a window over the sheets any more.
    let mut lines: Vec<String> = Vec::new();
    for raw in message.lines() {
        wrap(raw, interior_width, &mut lines);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    let max_interior_rows = size.rows.saturating_sub(2);
    if lines.len() > max_interior_rows {
        lines.truncate(max_interior_rows);
    }
    if lines.is_empty() {
        return None;
    }

    let line_width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let width = (line_width + 2 + 2 * PADDING).min(max_width);
    let height = lines.len() + 2;

    if width > size.cols || height > size.rows {
        return None;
    }

    let top = (size.rows - height) / 2;
    let left = (size.cols - width) / 2;
    Some(ErrorLayout {
        region: tuinix::Region {
            position: tuinix::Position {
                row: top,
                col: left,
            },
            size: tuinix::Size {
                rows: height,
                cols: width,
            },
        },
        lines,
    })
}

/// The widest an error box may be, however wide the screen is.
///
/// A message is a sentence, not a paragraph, so a box as wide as a wide terminal
/// would put a line of text in a great deal of empty space. The cap keeps the box
/// a box; a screen narrower than it decides the width instead.
const MAX_WIDTH: usize = 60;

/// Wraps `text` at `width` characters and appends the pieces to `out`.
///
/// A blank line is a line: `text` being empty appends one empty string, so a
/// message's own blank line between paragraphs is preserved rather than
/// swallowed. The split is on characters -- see [`error_layout`] for why.
fn wrap(text: &str, width: usize, out: &mut Vec<String>) {
    if text.is_empty() {
        out.push(String::new());
        return;
    }
    let mut current = String::new();
    for ch in text.chars() {
        if current.chars().count() == width {
            out.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }
    out.push(current);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(rows: usize, cols: usize) -> tuinix::Size {
        tuinix::Size { rows, cols }
    }

    #[test]
    fn a_short_message_is_one_interior_row_with_borders_around_it() {
        let layout = error_layout(size(24, 80), "boom").expect("it fits");
        assert_eq!(layout.lines, ["boom"]);
        // The interior plus the two borders.
        assert_eq!(layout.region.size.rows, 3);
        // The line, two border columns, and one padding column each side.
        assert_eq!(layout.region.size.cols, 4 + 4);
    }

    #[test]
    fn a_message_with_two_lines_is_two_interior_rows() {
        let layout = error_layout(size(24, 80), "failed to start\nnot found").expect("it fits");
        assert_eq!(layout.lines, ["failed to start", "not found"]);
        assert_eq!(layout.region.size.rows, 4);
    }

    #[test]
    fn a_long_line_is_wrapped_to_the_box_width() {
        // A line longer than the cap is split, so no interior line is wider
        // than the cap's interior width.
        let long = "x".repeat(MAX_WIDTH * 2);
        let layout = error_layout(size(24, 200), &long).expect("it fits");
        assert!(layout.lines.len() > 2, "a long line wraps");
        for line in &layout.lines {
            assert!(line.chars().count() <= MAX_WIDTH - 4);
        }
        assert_eq!(layout.region.size.cols, MAX_WIDTH);
    }

    #[test]
    fn the_box_is_centred_on_the_screen() {
        let size = size(24, 80);
        let layout = error_layout(size, "boom").expect("it fits");
        let left = layout.region.position.col;
        let right = size.cols - (left + layout.region.size.cols);
        assert!(right <= left + 1 && left <= right + 1, "centred");
        let top = layout.region.position.row;
        let bottom = size.rows - (top + layout.region.size.rows);
        assert!(bottom <= top + 1 && top <= bottom + 1, "centred");
    }

    #[test]
    fn an_empty_message_still_lays_out_a_box() {
        // The window is shown even for an empty message -- the failure is the
        // point, not the words -- so it is never a `None` the edge has to guard.
        // A zero-height interior is not a box, though, so an empty message still
        // gets one blank interior row.
        let layout = error_layout(size(24, 80), "").expect("it fits");
        assert_eq!(layout.lines, [""]);
        assert_eq!(layout.region.size.rows, 3);
    }

    #[test]
    fn a_small_screen_gives_no_layout() {
        assert!(error_layout(size(2, 80), "boom").is_none(), "too short");
        assert!(error_layout(size(24, 3), "boom").is_none(), "too narrow");
    }
}
