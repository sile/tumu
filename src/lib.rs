//! `tumu`'s Sans I/O core.
//!
//! A terminal multiplexer that arranges its sheets in a stack: a new sheet
//! goes on top, and every sheet's size is derived from the stack rather than
//! set by hand, so a change to the terminal's size -- a rotation, most of all --
//! re-lays the stack out instead of moving it.
//!
//! This crate holds everything that does not touch a file descriptor, a child
//! process, or a clock: the stack model ([`Stack`]) with its size derivation,
//! the state ([`State`]) with its pure handlers, and the renderer that paints a
//! [`Frame`](tuinix::Frame) from that state. The one thing the core decides but
//! does not perform is I/O: spawning children, reading and writing the terminal,
//! and resizing the PTYs all happen at the edge in the binary's `app` module.
//! The core hands back plain values -- the region a sheet should occupy, the
//! frame to draw -- and is driven entirely by calls from the edge.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod action;
mod event;
mod geometry;
mod id;
mod orientation;
mod ratio;
mod render;
mod sheet;
mod stack;
mod state;

pub use action::Action;
pub use event::Event;
pub use geometry::{
    cell_size_to_terminal_size, region_extent, split_region, terminal_region,
    terminal_size_to_cell_size,
};
pub use id::{SheetId, TermnixSessionId};
pub use orientation::{
    DEFAULT_CELL_ASPECT, Orientation, orientation, orientation_with_default_factor,
};
pub use ratio::Ratio;
pub use render::{frame, sheet_content_region};
pub use sheet::Sheet;
pub use stack::Stack;
pub use state::State;
