//! A window: a terminal session displayed in one slot of the stack.

use crate::TermnixSessionId;

/// One window of the stack.
///
/// A window names the session it displays and the weight it holds in the
/// size derivation. It does *not* store a region: the rectangle a window
/// occupies is computed by the [`Stack`](crate::Stack) from the terminal size
/// and the other windows, so that a resize re-derives every region instead of
/// leaving stale ones behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    /// The session this window displays, or `None` until one is attached.
    pub session: Option<TermnixSessionId>,

    /// How much of the space left to it this window claims, against the rest
    /// of the stack below it.
    ///
    /// The value is a weight, not a row count: the derivation splits the rows
    /// that remain after the windows above, in the proportion this weight bears
    /// to a single unit standing for every window below. The default of one
    /// therefore halves the space at each step, which is the arrangement the
    /// README describes with `1:1` odds.
    pub weight: u16,
}

impl Window {
    /// Makes a window with the default weight and no session attached.
    pub const fn new() -> Self {
        Self {
            session: None,
            weight: 1,
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}
