//! A sheet: one window of the stack.
//!
//! Each sheet holds one terminal session and is drawn as a rectangular pane.
//! The name comes from the way the windows are arranged -- thin, stackable
//! panes, one laid over the next -- and from the display the multiplexer
//! targets, an e-ink tablet where each window reads like a sheet of paper. The
//! stack is built around this idea, so it earns a name of its own rather than a
//! borrowed one.
//!
//! A sheet names the session it displays. It does *not* store a region -- nor
//! the odds that size it: the area a sheet occupies is computed by the
//! [`Stack`](crate::Stack) from the terminal size, the order, and the stack's
//! [ratio](crate::Ratio), so that a resize re-derives every region instead of
//! leaving stale ones behind.

use crate::TermnixSessionId;

/// One sheet: a window of the stack showing a terminal session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sheet {
    /// The session this sheet displays, or `None` until one is attached.
    pub session: Option<TermnixSessionId>,
}

impl Sheet {
    /// Makes a sheet with no session attached.
    pub const fn new() -> Self {
        Self { session: None }
    }
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}
