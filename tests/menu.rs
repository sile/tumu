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
fn the_box_is_tall_enough_for_both_borders_and_every_row() {
    // The box is the header, every binding, the trailer if there is one, and two
    // border rows. A box one row short hides the bottom border, so the height is
    // checked against the rows it holds rather than a literal.
    let prefix = tumu::menu_layout(size(40, 80), PREFIX).expect("the menu fits 40x80");
    assert_eq!(
        prefix.region.size.rows,
        prefix.rows.len() + 2,
        "one row per listed row, plus the two borders"
    );

    let ext = tumu::ext_menu_layout(size(40, 80)).expect("the ext menu fits 40x80");
    assert_eq!(
        ext.region.size.rows,
        ext.rows.len() + 2,
        "the ext menu measures its box the same way"
    );
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

#[test]
fn the_ext_menu_lists_the_ext_bindings_and_no_trailer() {
    let layout = tumu::ext_menu_layout(size(40, 80)).expect("the ext menu fits 40x80");

    // A header, the ext bindings, and no send-prefix row: the prefix is not part
    // of this menu.
    assert_eq!(layout.rows.len(), tumu::EXT_BINDINGS.len() + 1);
    assert_eq!(layout.rows[0].command, None, "the header selects nothing");
    for (row, binding) in layout.rows[1..].iter().zip(tumu::EXT_BINDINGS) {
        assert_eq!(row.command, Some(binding.command));
        assert_eq!(row.label, binding.label);
        assert_eq!(row.key, tumu::chord_text(binding));
    }
}

#[test]
fn the_ext_menu_holds_the_commands_the_prefix_menu_does_not() {
    // The two menus are disjoint: a command is reachable either at the prefix or
    // in the menu, never both, which is what makes the menu a real second row
    // rather than a copy.
    let prefix = tumu::menu_layout(size(40, 80), PREFIX).expect("the menu fits 40x80");
    let ext = tumu::ext_menu_layout(size(40, 80)).expect("the ext menu fits 40x80");

    let prefix_commands: Vec<tumu::Command> =
        prefix.rows.iter().filter_map(|row| row.command).collect();
    for row in &ext.rows {
        if let Some(command) = row.command {
            assert!(
                !prefix_commands.contains(&command),
                "{command:?} is in both menus"
            );
        }
    }
}

#[test]
fn the_ext_menu_is_not_laid_out_when_the_screen_is_too_small() {
    assert!(tumu::ext_menu_layout(size(40, 4)).is_none());
    assert!(tumu::ext_menu_layout(size(2, 80)).is_none());
}
