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
//! binding's label, which is what makes the two the same fact.//! Below the command rows sits the *send-prefix* row, which stands for the
//! prefix pressed twice rather than a [`Command`]. Nothing here is chosen with
//! the pointer: the menu is a list a key picks from, so the layout answers only
//! *where* each row is drawn. The rectangles are still here -- the renderer
//! paints each row's text across its own rectangle, so the drawing and the
//! layout cannot drift.

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
/// A row is one line of a box's interior, as wide as the interior, which is what
/// the renderer writes its text across. Nothing tests the rectangle against a
/// pointer any more, but the two halves still belong together: the line is drawn
/// where the layout says, so the box cannot be described one way and painted
/// another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    /// The key column's text, left-aligned in its column.
    pub key: String,

    /// The label column's text, left-aligned after the gap.
    pub label: String,

    /// The command the row stands for, or `None` for the header and the
    /// send-prefix row, which stand for nothing.
    ///
    /// It is not a click target; it is what makes the row's key and its meaning
    /// one fact, and what `tests/menu.rs` checks the list against.
    pub command: Option<Command>,

    /// Where the row sits on the screen: the box's interior width, one row
    /// tall.
    pub region: tuinix::Region,
}

/// The menu as it is laid out on a screen of a given size.
///
/// The rectangles are in screen coordinates, so the renderer can paint each row
/// without knowing how the box was centred, and a test can read where a row went
/// without recomputing the arithmetic. The column widths are here too, so the
/// renderer pads the two columns to line them up rather than measuring the text
/// a second time.
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
    /// Returns whether `position` is anywhere inside the box.
    ///
    /// It is the one geometric question left to ask about the menu: nothing is
    /// *chosen* with the pointer, so the rows need no hit test. A caller that
    /// wants to know whether a click landed on the box at all -- a test, or a
    /// future gesture -- asks this.
    pub fn contains(&self, position: tuinix::Position) -> bool {
        self.region.contains(position)
    }
}

/// Lays the prefix menu out on a screen of `size`, or returns `None` when there
/// is not enough room for it.
///
/// The box is centred on the screen. Its width is the widest row (header, every
/// binding, the send-prefix row) plus the two border columns and the padding;
/// its height is the header, one row per binding, the send-prefix row, and the
/// two border rows. When the screen is too small to hold the box whole the menu
/// is not shown at all, rather than shown clipped: a half-visible menu is not a
/// thing a click can be answered from honestly.
pub fn menu_layout(size: tuinix::Size, prefix_key: &str) -> Option<MenuLayout> {
    // The send-prefix row is the one row that is not a binding: it stands for
    // the prefix pressed twice, so its key is the prefix written out and its
    // label is the fixed word. It is appended after every binding, so it reads
    // as "and then more" past the last command.
    let send_prefix = Some(MenuRow {
        key: prefix_key.to_string(),
        label: SEND_PREFIX_LABEL.to_string(),
        command: None,
        // Filled in by `layout` once the box is placed; the row is always the
        // last, so it can be located then.
        region: EMPTY_REGION,
    });
    layout(BINDINGS, size, send_prefix)
}

/// Lays the extended menu out on a screen of `size`, or returns `None` when
/// there is not enough room for it.
///
/// The extended menu is the same box as [`menu_layout`], built from
/// [`EXT_BINDINGS`](crate::EXT_BINDINGS) instead: the commands kept out of the
/// prefix because their effect cannot be taken back. It has no send-prefix row,
/// because the prefix is not part of this menu -- it is *opened* by the prefix,
/// and the keys it offers are a fresh set of chords, not the prefix's own.
///
/// The two menus share [`layout`], so a box is centred and measured by one
/// routine whether it holds ten commands or two.
pub fn ext_menu_layout(size: tuinix::Size) -> Option<MenuLayout> {
    layout(crate::EXT_BINDINGS, size, None)
}

/// Lays a menu of `bindings` out on a screen of `size`, with `trailer` as an
/// optional last row.
///
/// This is the one place the box is measured and placed, so the two menus (and
/// any later one) cannot disagree about how a menu is centred or how wide a
/// column is. `trailer` is the send-prefix row for the prefix menu and `None`
/// for the extended menu; its region is filled in here once the box is placed,
/// since the row is always the last.
///
/// Returns `None` when the box does not fit, rather than a clipped layout: a
/// menu a row short is not something a key can be answered from honestly. That
/// matters most for the extended menu, which is the only way to reach its
/// commands -- a menu that cannot be shown is safer than a command that fires
/// without one, but it means the commands are unreachable until the screen
/// grows.
fn layout(
    bindings: &[crate::Binding],
    size: tuinix::Size,
    trailer: Option<MenuRow>,
) -> Option<MenuLayout> {
    // The key column holds the header word and every chord, so it is as wide as
    // the widest of those. The trailer's key (the prefix, written out) counts
    // too, so the column fits it when there is one.
    let key_width = bindings
        .iter()
        .map(|binding| chord_text(binding).chars().count())
        .chain([KEY_HEADER.chars().count()])
        .chain(trailer.iter().map(|row| row.key.chars().count()))
        .max()
        .unwrap_or(0);
    let label_width = bindings
        .iter()
        .map(|binding| binding.label.chars().count())
        .chain([LABEL_HEADER.chars().count()])
        .chain(trailer.iter().map(|row| row.label.chars().count()))
        .max()
        .unwrap_or(0);

    // `| key  label |`: two border columns, one space of padding each side of
    // the text, and the gap between the columns.
    let inner = key_width + GAP + label_width;
    let width = inner + 4;
    // One header row, one row per binding, the trailer if there is one, and the
    // two border rows. The 3 is the header plus the two borders, so the box is
    // as tall as the rows that go in it -- one short and the bottom border would
    // be pushed off the box (or a content row would land on it).
    let height = bindings.len() + 3 + usize::from(trailer.is_some());

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

    let mut rows = Vec::with_capacity(height - 2);
    rows.push(MenuRow {
        key: KEY_HEADER.to_string(),
        label: LABEL_HEADER.to_string(),
        command: None,
        region: row_region(0),
    });
    for (offset, binding) in bindings.iter().enumerate() {
        rows.push(MenuRow {
            key: chord_text(binding),
            label: binding.label.to_string(),
            command: Some(binding.command),
            region: row_region(1 + offset),
        });
    }
    if let Some(mut trailer) = trailer {
        trailer.region = row_region(1 + bindings.len());
        rows.push(trailer);
    }

    Some(MenuLayout {
        region,
        key_width,
        label_width,
        rows,
    })
}

/// A placeholder rectangle for a row whose region is filled in by the caller.
///
/// Only the trailer is built this way: its row is made before the box is
/// measured (its key is one of the things the width depends on), and its region
/// is set once the box is placed. The value never reaches a [`MenuLayout`].
const EMPTY_REGION: tuinix::Region = tuinix::Region {
    position: tuinix::Position { row: 0, col: 0 },
    size: tuinix::Size { rows: 0, cols: 0 },
};

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
