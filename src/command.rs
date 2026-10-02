//! The commands a latched key can name.
//!
//! A [`Command`] is what a key *means* -- the intent behind a prefix chord --
//! kept separate from the [`Action`](crate::Action) the edge carries out. One
//! command may expand to several actions, and the expansion happens against the
//! state as it is at the moment of the press, so a command never names a sheet.

/// One command a prefix chord can trigger.
///
/// The set is deliberately small: it is the least that makes the stack
/// touchable by hand (add, close, rotate, leave). Navigation, full-screen, and
/// the ratio are the next additions and will extend this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// Add a new sheet on top of the stack.
    NewSheet,

    /// Close the focused sheet.
    CloseSheet,

    /// Move the focused sheet to the bottom, bringing the next one up.
    RotateSheets,

    /// Leave the multiplexer.
    Quit,
}
