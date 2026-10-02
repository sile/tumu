//! Which way a stack stacks: a pure function of the terminal's shape.
//!
//! The axis is not a setting. A stack is laid out along the longer side of the
//! display, so the two arrangements the README draws are the same arrangement
//! seen before and after a rotation: the terminal's size change flips the axis,
//! and every region is re-derived from it. Keeping the choice out of the state
//! is what lets the rotation be handled by the one `Resize` event.
//!
//! Comparing the rows to the columns is not quite comparing the sides, because
//! a cell is taller than it is wide. A `factor` stands for that cell aspect: the
//! effective height is `rows * factor` cells against `cols`. It is a plain
//! argument so that a terminal which reports its pixel size can pass the real
//! ratio in, while the default assumes the usual cell.

/// The direction a stack grows in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Windows are laid out top to bottom, each taking rows.
    Vertical,

    /// Windows are laid out left to right, each taking columns.
    Horizontal,
}

/// The height of one cell as a multiple of its width.
///
/// Cells are about twice as tall as they are wide, so a terminal whose rows
/// and columns are equal is really taller than it is wide -- about `2 : 1`.
/// This is the default `factor` used when nothing better is known.
pub const DEFAULT_CELL_ASPECT: u32 = 2;

/// Returns the axis a terminal of `size` should stack along.
///
/// The taller side wins, counting the rows at `factor` times their number so
/// that the comparison is between the sides as they look. A tie is vertical, so
/// a square display stacks the way the README's main figure does. The rows are
/// scaled in `u64` so a very large terminal cannot overflow the product.
pub fn orientation(size: tuinix::Size, factor: u32) -> Orientation {
    let height = (size.rows as u64) * u64::from(factor);
    let width = size.cols as u64;
    if height >= width {
        Orientation::Vertical
    } else {
        Orientation::Horizontal
    }
}

/// Returns the axis a terminal of `size` should stack along, using the default
/// [cell aspect](DEFAULT_CELL_ASPECT).
pub fn orientation_with_default_factor(size: tuinix::Size) -> Orientation {
    orientation(size, DEFAULT_CELL_ASPECT)
}
