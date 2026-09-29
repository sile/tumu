//! Helpers for building `tuinix` display geometry in tests.

/// A [`tuinix::Size`] from plain row and column counts.
pub fn size(rows: usize, cols: usize) -> tuinix::Size {
    tuinix::Size { rows, cols }
}
