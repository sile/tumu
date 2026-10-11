//! Identifiers for sheets and the sessions they host.
//!
//! Each id wraps a [`NonZeroU64`](std::num::NonZeroU64) so an id is never zero
//! and the `Option<Id>` niche is free.

use std::num::NonZeroU64;

/// Addresses a [`Sheet`](crate::Sheet) within a [`Stack`](crate::Stack).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SheetId(NonZeroU64);

impl SheetId {
    /// Builds an id from a non-zero number.
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Returns the underlying number.
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }
}

impl std::fmt::Display for SheetId {
    /// Writes the bare number.
    ///
    /// An id is only ever shown to a person in a file name -- the one the
    /// scrollback editor is opened on -- where the wrapper's name would be
    /// noise. The number itself is what makes the name unique.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Names the `termnix` session a sheet hosts.
///
/// The name carries the dependency it comes from, because a session is not a
/// concept of the stack: it is the live child process a sheet happens to
/// display, and the edge owns the [`termnix::Session`] this id refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TermnixSessionId(NonZeroU64);

impl TermnixSessionId {
    /// Builds an id from a non-zero number.
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Returns the underlying number.
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }
}
