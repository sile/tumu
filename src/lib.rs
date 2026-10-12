//! `tumu`'s Sans I/O core.
//!
//! A terminal multiplexer that arranges its sheets in a stack: a new sheet
//! goes on top, and every sheet's size is derived from the stack rather than
//! set by hand, so a change to the terminal's size -- a rotation, most of all --
//! re-lays the stack out instead of moving it.
//!
//! This crate holds everything that does not touch a file descriptor, a child
//! process, or a clock: the stack model ([`Stack`]) with its size derivation,
//! the state ([`State`]) with its pure handlers -- including which sheet the
//! commands act on and whether it is shown full screen -- and the renderer
//! that paints a [`Frame`](tuinix::Frame) from that state. The one thing the
//! core decides but does not perform is I/O: spawning children, reading and
//! writing the terminal, and resizing the PTYs all happen at the edge in the
//! binary's `app` module.
//! The core hands back plain values -- the region a sheet should occupy, the
//! frame to draw -- and is driven entirely by calls from the edge.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod action;
mod command;
mod event;
mod geometry;
mod id;
mod input;
mod menu;
mod orientation;
mod ratio;
mod render;
mod scrollback;
mod sheet;
mod stack;
mod state;

pub use action::{Action, Program};
pub use command::Command;
pub use event::Event;
pub use geometry::{
    cell_size_to_terminal_size, region_extent, screen_position_to_grid, split_region,
    terminal_region, terminal_size_to_cell_size,
};
pub use id::{SheetId, TermnixSessionId};
pub use input::{
    BINDINGS, Binding, DEFAULT_PREFIX_CHAR, EXT_BINDINGS, SCROLLBACK_FILE_PREFIX, is_prefix,
    prefix_key, prefix_key_text,
};
pub use menu::{MenuLayout, MenuRow, SEND_PREFIX_LABEL, chord_text, ext_menu_layout, menu_layout};
pub use orientation::Orientation;
// The ratio's bounds are re-exported under names that say what they bound, so a
// reader at the crate root does not have to know which module is the only one
// with numbers in it. `MIN_RATIO_NUM` is exported for symmetry, though the
// binary does not read it: `--ratio` clamps through `Ratio::from_num`, so the
// floor is not spelled out there.
pub use ratio::{
    DEFAULT_NUM as DEFAULT_RATIO_NUM, MAX_NUM as MAX_RATIO_NUM, MIN_NUM as MIN_RATIO_NUM, Ratio,
};
pub use render::{
    SheetGrid, SheetGrids, draw_sheet_grid, frame, menu_frame, sheet_content_origin,
    sheet_content_size, sheet_frame,
};
pub use scrollback::{DEFAULT_CELLS, DEFAULT_LINES, ScrollbackLimit};
pub use sheet::Sheet;
pub use stack::Stack;
pub use state::State;
