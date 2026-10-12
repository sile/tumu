//! The prefix menu's layout: it fits, it lists every binding, and a click
//! selects the row it landed on.
//!
//! The layout is pure data, so these are direct assertions: the box's rectangle,
//! the rows' rectangles, and the command each one selects. Whether the renderer
//! paints what the layout says is `tests/render.rs`'s business.

mod helpers_tuinix;

use helpers_tuinix::size;

/// The prefix text the layout is built with; the real one comes from
/// `tumu::PREFIX`, and no test here depends on which key it is.
const PREFIX: &str = "C-q";

#[test]
fn the_menu_lists_one_row_per_binding_plus_a_header_and_send_prefix() {
    let layout = tumu::menu_layout(size(40, 80), PREFIX).expect("the menu fits 40x80");

    // The header, every binding, and the send-prefix row.
    assert_eq!(layout.rows.len(), tumu::BINDINGS.len() + 2);
    assert_eq!(layout.rows[0].command, None, "the header selects nothing");
    for (row, binding) in layout.rows[1..=tumu::BINDINGS.len()]
        .iter()
        .zip(tumu::BINDINGS)
    {
        assert_eq!(row.command, Some(binding.command));
        assert_eq!(row.label, binding.label);
    }
    let last = layout.rows.last().expect("a last row");
    assert_eq!(last.command, None, "send-prefix selects nothing");
    assert_eq!(last.key, PREFIX);
}

#[test]
fn the_box_is_centred_and_holds_every_row() {
    let size = size(40, 80);
    let layout = tumu::menu_layout(size, PREFIX).expect("the menu fits 40x80");

    // Centred: the space left of the box and right of it differ by at most one
    // column (the same for rows), and the box is inside the screen.
    let left = layout.region.position.col;
    let right = size.cols - (left + layout.region.size.cols);
    assert!(
        right <= left + 1 && left <= right + 1,
        "centred horizontally"
    );
    let top = layout.region.position.row;
    let bottom = size.rows - (top + layout.region.size.rows);
    assert!(bottom <= top + 1 && top <= bottom + 1, "centred vertically");

    // Every row is inside the box's interior.
    for row in &layout.rows {
        assert!(
            layout.contains(row.region.position),
            "row starts inside the box"
        );
        assert_eq!(row.region.size.rows, 1);
        assert_eq!(
            row.region.size.cols,
            layout.region.size.cols - 2,
            "a row spans the interior"
        );
    }
}

#[test]
fn a_click_selects_the_command_of_the_row_it_lands_on() {
    let layout = tumu::menu_layout(size(40, 80), PREFIX).expect("the menu fits 40x80");

    for row in &layout.rows {
        // A click anywhere across the row selects it, not just on its text.
        let start = row.region.position;
        let end = tuinix::Position {
            row: start.row,
            col: start.col + row.region.size.cols - 1,
        };
        assert_eq!(layout.command_at(start), row.command);
        assert_eq!(layout.command_at(end), row.command);
    }
}

#[test]
fn a_click_on_the_border_or_outside_dismisses() {
    let layout = tumu::menu_layout(size(40, 80), PREFIX).expect("the menu fits 40x80");

    // The four corners of the box, on the border, select nothing.
    let top = layout.region.position.row;
    let left = layout.region.position.col;
    let bottom = top + layout.region.size.rows - 1;
    let right = left + layout.region.size.cols - 1;
    for position in [
        tuinix::Position {
            row: top,
            col: left,
        },
        tuinix::Position {
            row: top,
            col: right,
        },
        tuinix::Position {
            row: bottom,
            col: left,
        },
        tuinix::Position {
            row: bottom,
            col: right,
        },
    ] {
        assert_eq!(
            layout.command_at(position),
            None,
            "{position:?} is on the border"
        );
    }

    // A position outside the box entirely.
    let outside = tuinix::Position {
        row: layout.region.position.row.saturating_sub(1),
        col: layout.region.position.col,
    };
    assert_eq!(layout.command_at(outside), None);
    assert!(!layout.contains(outside));
}

#[test]
fn the_menu_is_not_laid_out_when_the_screen_is_too_small() {
    // Narrower than the box, and shorter than the box.
    assert!(tumu::menu_layout(size(40, 4), PREFIX).is_none());
    assert!(tumu::menu_layout(size(2, 80), PREFIX).is_none());
}

#[test]
fn the_send_prefix_row_shows_the_configured_prefix() {
    // The row's key is whatever prefix the layout is built with, so a menu
    // built for `C-o` says so and not `C-q`.
    let layout = tumu::menu_layout(size(40, 80), "C-o").expect("the menu fits 40x80");

    let last = layout.rows.last().expect("a last row");
    assert_eq!(last.key, "C-o");
    assert_eq!(last.label, tumu::SEND_PREFIX_LABEL);
}

#[test]
fn a_longer_prefix_key_widens_the_key_column() {
    // Two characters instead of three would make the column narrower, so the
    // width is not a constant: it follows the key the layout is given.
    let short = tumu::menu_layout(size(40, 80), "C-o").expect("the menu fits 40x80");
    let long = tumu::menu_layout(size(40, 80), "C-M-o").expect("the menu fits 40x80");

    assert!(
        long.key_width > short.key_width,
        "a longer key widens its column"
    );
    assert!(long.region.size.cols > short.region.size.cols);
}

#[test]
fn the_row_commands_are_every_binding_exactly_once() {
    let layout = tumu::menu_layout(size(40, 80), PREFIX).expect("the menu fits 40x80");

    let mut listed: Vec<tumu::Command> = layout.rows.iter().filter_map(|row| row.command).collect();
    let mut bound: Vec<tumu::Command> = tumu::BINDINGS
        .iter()
        .map(|binding| binding.command)
        .collect();
    listed.sort_by_key(|command| format!("{command:?}"));
    bound.sort_by_key(|command| format!("{command:?}"));

    assert_eq!(listed, bound, "the menu shows exactly the bound commands");
}
