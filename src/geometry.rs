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
/// The origin is `(0, 0)`, so this is the display a stack of one sheet fills.
pub fn terminal_region(size: tuinix::Size) -> tuinix::Region {
    tuinix::Region {
        position: tuinix::Position { row: 0, col: 0 },
        size,
    }
}

/// Splits `region` into a leading part and a trailing part along `axis`,
/// giving the leading part `share` cells.
///
/// Verically, `share` rows go to the first region and the rest to the second;
/// horizontally, `share` columns go to the first and the rest to the second,
/// so the second region is the *right* one. In
/// either case the two sizes sum to the original along the axis, and the split
/// is clamped so neither part is empty: a split that would leave a side with no
/// cells keeps one cell for it and gives the rest to the other. A region one
/// cell long therefore yields two one-cell regions that overlap the original.
///
/// This is the one primitive the stack derives its layout from, so the whole
/// arrangement is expressible as a left fold of it.
pub fn split_region(
    region: tuinix::Region,
    share: usize,
    axis: crate::Orientation,
) -> (tuinix::Region, tuinix::Region) {
    match axis {
        crate::Orientation::Vertical => {
            let total = region.size.rows;
            let lead = share.clamp(1, total.saturating_sub(1).max(1));
            let rest = total.saturating_sub(lead);
            let lead_region = tuinix::Region {
                position: region.position,
                size: tuinix::Size {
                    rows: lead,
                    cols: region.size.cols,
                },
            };
            let rest_region = tuinix::Region {
                position: tuinix::Position {
                    row: region.position.row + lead,
                    col: region.position.col,
                },
                size: tuinix::Size {
                    rows: rest,
                    cols: region.size.cols,
                },
            };
            (lead_region, rest_region)
        }
        crate::Orientation::Horizontal => {
            let total = region.size.cols;
            let lead = share.clamp(1, total.saturating_sub(1).max(1));
            let rest = total.saturating_sub(lead);
            let lead_region = tuinix::Region {
                position: region.position,
                size: tuinix::Size {
                    rows: region.size.rows,
                    cols: lead,
                },
            };
            let rest_region = tuinix::Region {
                position: tuinix::Position {
                    row: region.position.row,
                    col: region.position.col + lead,
                },
                size: tuinix::Size {
                    rows: region.size.rows,
                    cols: rest,
                },
            };
            (lead_region, rest_region)
        }
    }
}

/// Returns the number of cells `region` has along `axis`.
pub fn region_extent(region: tuinix::Region, axis: crate::Orientation) -> usize {
    match axis {
        crate::Orientation::Vertical => region.size.rows,
        crate::Orientation::Horizontal => region.size.cols,
    }
}

/// Moves a screen position into the grid of the sheet occupying `region`.
///
/// This is the arithmetic a forwarded mouse report needs: the host says where the
/// pointer is on *the screen*, and the child knows only its own grid, which
/// starts at the sheet's content origin and is one line shorter than the sheet
/// when the stack runs vertically or one column narrower when it runs
/// horizontally (see [`sheet_content_size`](crate::sheet_content_size)).
///
/// `grid` is the child's own size -- what the session was spawned or resized to
/// -- and is the authority on where the grid ends: a report outside it would be a
/// position the child cannot have, so it is clamped into the grid rather than
/// dropped. Clamping is the honest answer for the same reason it is for a click:
/// the mouse protocols have no way to say "outside", and a drag that leaves the
/// sheet and comes back should keep working, which is exactly what a clamped
/// trail of reports does.
///
/// The result is in the child's coordinates, so the edge can hand it straight to
/// `termnix` to re-encode for that child.
pub fn screen_position_to_grid(
    position: tuinix::Position,
    region: tuinix::Region,
    origin: tuinix::Position,
    grid: termnix::Size,
) -> termnix::Position {
    let row = position
        .row
        .saturating_sub(region.position.row + origin.row);
    let col = position
        .col
        .saturating_sub(region.position.col + origin.col);
    termnix::Position {
        row: row.min(usize::from(grid.rows.get()) - 1) as u16,
        col: col.min(usize::from(grid.cols.get()) - 1) as u16,
    }
}
