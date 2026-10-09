//! The prefix menu: its layout as data, derived from the bindings.
//!
//! The menu is a small box centred on the terminal that lists every prefix
//! chord. It is a *view*, like full screen: it changes no sheet's region, so it
//! lives outside [`State::regions`](crate::State::regions) and is painted over
//! the composed screen afterwards. Nothing here draws: the layout is the box,
//! the rows of text to put in it, and the rectangle each row occupies, and the
//! renderer paints what it says.
//!
//! The rows come from [`BINDINGS`](crate::BINDINGS), so a chord cannot exist
//! without appearing in the menu and the menu cannot list a chord that does not
//! exist. The key column is the chord's own text and the label column is the
//! binding's label, which is what makes the two the same fact.
//!
//! Below the command rows sits the *send-prefix* row, which stands for the
//! prefix pressed twice rather than a [`Command`]. A click on a command row
//! selects it; a click anywhere else in the box -- the border, the header, the
//! send-prefix row -- dismisses the menu.

use crate::{BINDINGS, Command};

/// The label the send-prefix row shows, next to the prefix key.
///
/// It stands for "and then more", the ellipsis the README draws on both the
/// border and the menu. It is not a [`Command`] and is not in [`BINDINGS`], but
/// it belongs in the menu for the same reason every binding does: it is
/// something a prefix chord can do.
pub const SEND_PREFIX_LABEL: &str = "send prefix";

/// The word shown above the key column.
const KEY_HEADER: &str = "key";

/// The word shown above the label column.
const LABEL_HEADER: &str = "command";

/// How many blank columns sit between the key column and the label column.
///
/// It is public because the renderer pads the two columns with it: the layout
/// measures the columns, the renderer writes them, and both must agree on the
/// space between them for a click to land on the row the renderer drew.
pub const GAP: usize = 3;

/// One line of the menu: the text it shows, and where it sits.
///
/// A row is one line of a box's interior, as wide as the interior, so a click
/// anywhere across it lands on the row and not on the character under the
/// pointer. That is what makes a row a target rather than the text in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    /// The key column's text, left-aligned in its column.
    pub key: String,

    /// The label column's text, left-aligned after the gap.
    pub label: String,

    /// The command the row selects, or `None` for the header and the
    /// send-prefix row, which select nothing.
    pub command: Option<Command>,

    /// Where the row sits on the screen: the box's interior width, one row
    /// tall.
    pub region: tuinix::Region,
}

/// The menu as it is laid out on a screen of a given size.
///
/// The rectangles are in screen coordinates, so a click can be hit-tested
/// against them without knowing how the box was centred, and the renderer can
/// paint each row without recomputing the arithmetic. The column widths are
/// here too, so the renderer pads the two columns to line them up rather than
/// measuring the text a second time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuLayout {
    /// The whole box, border included.
    pub region: tuinix::Region,

    /// The width of the key column.
    pub key_width: usize,

    /// The width of the label column.
    pub label_width: usize,

    /// Every row inside the box, top to bottom: the header, one row per
    /// binding, then the send-prefix row. The top and bottom border rows are
    /// not here; the renderer draws them from `region`.
    pub rows: Vec<MenuRow>,
}

impl MenuLayout {
    /// Returns the command the click at `position` selects, or `None` when the
    /// click is not on a command row.
    ///
    /// A click outside the box, on the border, on the header, or on the
    /// send-prefix row all answer `None`: they are the dismiss cases, and the
    /// caller treats every `None` the same way.
    pub fn command_at(&self, position: tuinix::Position) -> Option<Command> {
        self.rows
            .iter()
            .find(|row| row.region.contains(position))
            .and_then(|row| row.command)
    }

    /// Returns whether `position` is anywhere inside the box.
    pub fn contains(&self, position: tuinix::Position) -> bool {
        self.region.contains(position)
    }
}

/// Lays the menu out on a screen of `size`, or returns `None` when there is not
/// enough room for it.
///
/// The box is centred on the screen. Its width is the widest row (header, every
/// binding, the send-prefix row) plus the two border columns and the padding;
/// its height is the header, one row per binding, the send-prefix row, and the
/// two border rows. When the screen is too small to hold the box whole the menu
/// is not shown at all, rather than shown clipped: a half-visible menu is not a
/// thing a click can be answered from honestly.
pub fn menu_layout(size: tuinix::Size, prefix_key: &str) -> Option<MenuLayout> {
    // The key column holds the header word and, on the send-prefix row, the
    // prefix twice (what the chord *is*), so it is as wide as the widest of
    // those and the single-character chords.
    let send_prefix_key = format!("{prefix_key} {prefix_key}");
    let key_width = BINDINGS
        .iter()
        .map(|binding| chord_text(binding).chars().count())
        .chain([KEY_HEADER.chars().count(), send_prefix_key.chars().count()])
        .max()
        .unwrap_or(0);
    let label_width = BINDINGS
        .iter()
        .map(|binding| binding.label.chars().count())
        .chain([
            LABEL_HEADER.chars().count(),
            SEND_PREFIX_LABEL.chars().count(),
        ])
        .max()
        .unwrap_or(0);

    // `| key  label |`: two border columns, one space of padding each side of
    // the text, and the gap between the columns.
    let inner = key_width + GAP + label_width;
    let width = inner + 4;
    let height = BINDINGS.len() + 4;

    if width > size.cols || height > size.rows {
        return None;
    }

    let top = (size.rows - height) / 2;
    let left = (size.cols - width) / 2;
    let region = tuinix::Region {
        position: tuinix::Position {
            row: top,
            col: left,
        },
        size: tuinix::Size {
            rows: height,
            cols: width,
        },
    };

    // A row's rectangle: the box's interior width, `index` rows below the top
    // border. The header is row 0, so the first binding is row 1.
    let row_region = |index: usize| tuinix::Region {
        position: tuinix::Position {
            row: top + 1 + index,
            col: left + 1,
        },
        size: tuinix::Size {
            rows: 1,
            cols: width - 2,
        },
    };

    let mut rows = Vec::with_capacity(BINDINGS.len() + 2);
    rows.push(MenuRow {
        key: KEY_HEADER.to_string(),
        label: LABEL_HEADER.to_string(),
        command: None,
        region: row_region(0),
    });
    for (offset, binding) in BINDINGS.iter().enumerate() {
        rows.push(MenuRow {
            key: chord_text(binding),
            label: binding.label.to_string(),
            command: Some(binding.command),
            region: row_region(1 + offset),
        });
    }
    rows.push(MenuRow {
        key: prefix_key.to_string(),
        label: SEND_PREFIX_LABEL.to_string(),
        command: None,
        region: row_region(1 + BINDINGS.len()),
    });

    Some(MenuLayout {
        region,
        key_width,
        label_width,
        rows,
    })
}

/// Returns the text shown in the key column for `binding`.
///
/// A chord is a plain character today, so the text is that character; the
/// function exists so a chord that is not one character (a control key, a
/// function key) has one place to gain its spelling. The modifiers are folded
/// in the way they are pressed, so a future `C-a` reads as `C-a` and not `a`.
pub fn chord_text(binding: &crate::Binding) -> String {
    let key = binding.key.code;
    let base = match key {
        tuinix::KeyCode::Char(c) => c.to_string(),
        other => format!("{other:?}"),
    };
    match (binding.key.ctrl, binding.key.alt) {
        (false, false) => base,
        (true, false) => format!("C-{base}"),
        (false, true) => format!("M-{base}"),
        (true, true) => format!("C-M-{base}"),
    }
}
