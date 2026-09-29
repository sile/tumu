//! A window: a terminal session displayed in one slot of the stack.

use crate::TermnixSessionId;

/// One window of the stack.
///
/// A window names the session it displays. It does *not* store a region -- nor
/// the odds that size it: the rectangle a window occupies is computed by the
/// [`Stack`](crate::Stack) from the terminal size, the order, and the stack's
/// [ratio](crate::Ratio), so that a resize re-derives every region instead of
/// leaving stale ones behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    /// The session this window displays, or `None` until one is attached.
    pub session: Option<TermnixSessionId>,
}

impl Window {
    /// Makes a window with no session attached.
    pub const fn new() -> Self {
        Self { session: None }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}
