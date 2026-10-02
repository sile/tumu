//! Painting the stack into a [`Frame`](tuinix::Frame).
//!
//! Drawing is free of I/O: it reads a [`State`](crate::State) and writes into a
//! frame, so a test can assert on the frame as data. What the edge does with the
//! frame -- comparing it to the last one and writing the terminal -- is the
//! edge's business.

use crate::State;

/// Returns the region of a sheet's content inside `region`.
///
/// The content sits one row below the top of the region, leaving that row for a
/// title. A region one row tall therefore has no content rows, which is how a
/// collapsed sheet is drawn: its single row is the title alone.
pub fn sheet_content_region(region: tuinix::Region) -> tuinix::Region {
    tuinix::Region {
        position: tuinix::Position {
            row: region.position.row + 1,
            col: region.position.col,
        },
        size: tuinix::Size {
            rows: region.size.rows.saturating_sub(1),
            cols: region.size.cols,
        },
    }
}

/// Paints `state` into a fresh frame of the state's size.
///
/// For the PoC, each sheet is drawn as its title row: the position in the
/// stack, so the ordering and the derived sizes are visible without a live
/// session behind them. Painting the sessions' grids into the content regions is
/// the edge's job, because only it holds them.
pub fn frame(state: &State) -> tuinix::Frame {
    let mut frame = tuinix::Frame::new(state.size);
    for (index, region) in state.stack.regions(state.size).into_iter().enumerate() {
        let title = format!("[{}] {}x{}", index, region.size.rows, region.size.cols);
        let right = region.position.col + region.size.cols;
        let mut position = region.position;
        for ch in title.chars() {
            if position.col >= right {
                break;
            }
            // Titles are ASCII, so each character advances one column. A title
            // that held a wide character would need its real width here.
            let Some(cell) = tuinix::Char::new(ch, 1, tuinix::Style::RESET) else {
                continue;
            };
            position = frame.put_char(position, cell);
        }
    }
    frame
}
