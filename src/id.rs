//! Identifiers for windows and the sessions they host.
//!
//! Each id wraps a [`NonZeroU64`](std::num::NonZeroU64) so an id is never zero
//! and the `Option<Id>` niche is free.

use std::num::NonZeroU64;

/// Addresses a window within a [`Stack`](crate::Stack).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(NonZeroU64);

impl WindowId {
    /// Builds an id from a non-zero number.
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Returns the underlying number.
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }
}

/// Names the `termnix` session a window hosts.
///
/// The name carries the dependency it comes from, because a session is not a
/// concept of the stack: it is the live child process a window happens to
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
