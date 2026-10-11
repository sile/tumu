//! How much scrollback history a session is allowed to keep.
//!
//! `termnix` keeps no bound of its own: a session's scrollback grows until the
//! caller trims it with [`Session::trim_scrollback()`](termnix::Session). The
//! edge does that trimming, and this is the bound it trims to. It is a plain
//! value -- the binary builds one from its arguments and hands it to the edge --
//! so the core never reads the environment or the command line itself.
//!
//! The history is bounded on two axes, lines and cells, because either can run
//! away on its own: a program can print an unbounded number of short lines, and
//! a single line can be arbitrarily wide once long output wraps. Whichever
//! limit is reached first decides, so both are needed.

/// The scrollback bound used when nothing overrides it.
///
/// One thousand lines is a screenful or so on the displays tumu targets, and
/// enough history to look back through without holding a session's whole life
/// in memory. The cell bound is the headroom that keeps a handful of very wide
/// lines from dwarfing the line bound; it is the same pair `termnix`'s own
/// documentation uses as its example.
pub const DEFAULT_LINES: usize = 1000;

/// The retained-cell bound used when nothing overrides it.
///
/// See [`DEFAULT_LINES`] for why the two live together.
pub const DEFAULT_CELLS: usize = 100_000;

/// A bound on a session's retained scrollback.
///
/// Both limits are applied at once: a session is trimmed until it holds at most
/// [`lines`](Self::lines) history lines *and* at most [`cells`](Self::cells)
/// retained cells. A limit of zero clears the history, which is
/// [`trim_scrollback`](termnix::Session::trim_scrollback)'s own meaning for
/// zero and is kept here rather than reinterpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbackLimit {
    lines: usize,
    cells: usize,
}

impl ScrollbackLimit {
    /// Builds a bound from a line count and a cell count.
    ///
    /// The values are taken as they are: a zero is a real limit that clears the
    /// history, not a request for the default. Building a defaulted bound is
    /// [`Default::default()`], which the binary uses when the user names no
    /// limit.
    pub const fn new(lines: usize, cells: usize) -> Self {
        Self { lines, cells }
    }

    /// The most history lines that may be retained.
    pub const fn lines(self) -> usize {
        self.lines
    }

    /// The most cells that may be retained across the history.
    pub const fn cells(self) -> usize {
        self.cells
    }
}

impl Default for ScrollbackLimit {
    fn default() -> Self {
        Self::new(DEFAULT_LINES, DEFAULT_CELLS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_documented_pair() {
        let limit = ScrollbackLimit::default();
        assert_eq!(limit.lines(), DEFAULT_LINES);
        assert_eq!(limit.cells(), DEFAULT_CELLS);
    }

    #[test]
    fn new_keeps_zero_as_a_real_limit() {
        // Zero means "keep no history", not "use the default": the accessors
        // must report it as given.
        let limit = ScrollbackLimit::new(0, 0);
        assert_eq!(limit.lines(), 0);
        assert_eq!(limit.cells(), 0);
    }
}
