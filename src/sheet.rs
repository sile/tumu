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

/// What the core opened a sheet for.
///
/// The core chooses a sheet's role when it adds the sheet, and the role is what
/// tells one kind of sheet from another even after the session behind it has
/// attached -- a distinction the session itself cannot make, since the editor is
/// just a child like any other once it is running.
///
/// It exists for the wheel: a scroll up from an ordinary sheet opens the
/// history in a copy-mode sheet, and a scroll up *from* that sheet must not open
/// another one on top of it. Without the role, a copy-mode sheet is
/// indistinguishable from the shell it was opened over, and a wheel gesture
/// stacks one history per turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetRole {
    /// A sheet the user asked for: a shell, or an external tool in its place.
    #[default]
    Normal,

    /// A sheet showing another sheet's history -- `prefix l`, or the wheel.
    CopyMode,
}

/// One sheet: a window of the stack showing a terminal session.
///
/// A sheet is deliberately neither `Copy` nor comparable. It is a place a
/// session is shown, not a value to be duplicated: the stack owns each sheet
/// exactly once, and two sheets are told apart by their [`SheetId`], not by
/// their contents. Skipping those derives now saves every later field -- a
/// title, a scroll position -- from having to be clonable-for-free and
/// meaningfully comparable for the whole struct to keep compiling.
///
/// [`SheetId`]: crate::SheetId
#[derive(Debug, Clone)]
pub struct Sheet {
    /// The session this sheet displays, or `None` until one is attached.
    pub session: Option<TermnixSessionId>,

    /// What the core opened this sheet for.
    ///
    /// It is set when the sheet is pushed and is not changed afterwards: a
    /// sheet stays the copy-mode sheet it was opened as, for as long as it is
    /// in the stack.
    pub role: SheetRole,

    /// The window title the child last set, or empty when it set none.
    ///
    /// The child owns this string: it arrives from OSC 0 / OSC 2 and is stored
    /// as it came, empty included. The core does not interpret it -- it is a
    /// label the child chose for itself, kept here so the renderer can show it
    /// without reaching into a live session, which only the edge holds.
    pub title: String,
}

impl Sheet {
    /// Makes a sheet with no session attached and no title.
    pub const fn new() -> Self {
        Self::with_role(SheetRole::Normal)
    }

    /// Makes a sheet with no session, no title, and the given role.
    pub const fn with_role(role: SheetRole) -> Self {
        Self {
            session: None,
            role,
            title: String::new(),
        }
    }
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}
