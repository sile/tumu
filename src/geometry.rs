//! Conversions between `termnix`'s PTY/grid size and `tuinix`'s display
//! geometry.
//!
//! Both crates define a `Size`, but they are different types with different
//! ranges: [`termnix::Size`] counts rows and columns as [`NonZeroU16`], while
//! [`tuinix::Size`] counts them as `usize` and allows zero. The two are kept as
//! distinct types on purpose -- the bridge is the functions here, called
//! explicitly at the edge.
//!
//! [`NonZeroU16`]: std::num::NonZeroU16

use std::num::NonZeroU16;

/// Converts a [`termnix`] PTY/grid size into a [`tuinix`] display size.
///
/// The conversion is total: a [`termnix::Size`] is always non-zero, so it always
/// maps to a valid, non-empty [`tuinix::Size`].
pub fn cell_size_to_terminal_size(size: termnix::Size) -> tuinix::Size {
    tuinix::Size {
        rows: usize::from(size.rows.get()),
        cols: usize::from(size.cols.get()),
    }
}

/// Converts a [`tuinix`] display size into a [`termnix`] PTY/grid size.
///
/// Returns `None` when either dimension is zero or does not fit in a [`u16`],
/// so the result is always a valid, non-empty [`termnix::Size`].
pub fn terminal_size_to_cell_size(size: tuinix::Size) -> Option<termnix::Size> {
    let rows = NonZeroU16::new(u16::try_from(size.rows).ok()?)?;
    let cols = NonZeroU16::new(u16::try_from(size.cols).ok()?)?;
    Some(termnix::Size { rows, cols })
}

/// Returns the region covering a whole terminal of `size`.
///
/// The origin is `(0, 0)`, so this is the display a stack of one window fills.
pub fn terminal_region(size: tuinix::Size) -> tuinix::Region {
    tuinix::Region {
        position: tuinix::Position { row: 0, col: 0 },
        size,
    }
}

/// Splits `region` into a top part and a bottom part along the requested
/// number of rows.
///
/// `top_rows` rows go to the first region and the rest to the second. The two
/// heights sum to the original, and the split is clamped so neither part is
/// empty: a split that would leave a side with no rows keeps one row for it and
/// gives the rest to the other. A region with a single row therefore yields two
/// one-row regions that overlap the original's row.
///
/// This is the one primitive the stack derives its layout from, so the whole
/// arrangement is expressible as a left fold of it.
pub fn split_region(region: tuinix::Region, top_rows: usize) -> (tuinix::Region, tuinix::Region) {
    let total = region.size.rows;
    let top = top_rows.clamp(1, total.saturating_sub(1).max(1));
    let bottom = total.saturating_sub(top);
    let top_region = tuinix::Region {
        position: region.position,
        size: tuinix::Size {
            rows: top,
            cols: region.size.cols,
        },
    };
    let bottom_region = tuinix::Region {
        position: tuinix::Position {
            row: region.position.row + top,
            col: region.position.col,
        },
        size: tuinix::Size {
            rows: bottom,
            cols: region.size.cols,
        },
    };
    (top_region, bottom_region)
}
