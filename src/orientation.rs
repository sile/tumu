//! Which way a stack stacks: a setting, not a guess.
//!
//! The axis used to be derived from the terminal's shape, so a rotation flipped
//! it by itself. That is gone: the axis is chosen by the user (`prefix r`, or
//! `--orientation` at startup) and the terminal's size no longer has an opinion
//! about it. The shape was a bad proxy for the intent -- a phone in landscape
//! and a laptop in landscape are the same shape and want different stacks --
//! and flipping the layout out from under a running program because the device
//! turned is surprising rather than helpful.
//!
//! What is left here is the setting itself: the two values, the one a run
//! starts from, and how the setting is spelled on the command line, in the
//! environment, and in the menu.

/// The direction a stack grows in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Sheets are laid out top to bottom, each taking rows.
    Vertical,

    /// Sheets are laid out left to right, each taking columns.
    Horizontal,
}

impl Orientation {
    /// The orientation a run starts from when nothing overrides it: vertical,
    /// which is how the README's main figure stacks.
    pub const DEFAULT: Self = Self::Vertical;

    /// Returns the other orientation.
    ///
    /// This is the whole of `prefix r`: the toggle is its own inverse, so
    /// nothing has to be remembered and there are no two values to keep in
    /// step.
    pub const fn flipped(self) -> Self {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::Vertical,
        }
    }

    /// Returns how the orientation is spelled on the command line and in the
    /// environment: `vertical` or `horizontal`.
    ///
    /// [`parse`](Self::parse) accepts the same word, so the spelling a user
    /// types and the spelling that comes back out cannot drift.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
        }
    }

    /// Parses the name of an orientation, in the spelling
    /// [`name`](Self::name) produces.
    ///
    /// Returns `None` for anything else, so a mistyped `--orientation` is
    /// reported by the caller rather than silently falling back to the default.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "vertical" => Some(Self::Vertical),
            "horizontal" => Some(Self::Horizontal),
            _ => None,
        }
    }
}
