//! The commands a latched key can name.
//!
//! A [`Command`] is what a key *means* -- the intent behind a prefix chord --
//! kept separate from the [`Action`](crate::Action) the edge carries out. One
//! command may expand to several actions, and the expansion happens against the
//! state as it is at the moment of the press, so a command never names a sheet.

/// One command a prefix chord can trigger.
///
/// The set is deliberately small: it is the least that makes the stack
/// touchable by hand (add, close, rotate, zoom, open the scrollback, leave).
/// Navigation and the ratio are the next additions and will extend this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// Add a new sheet on top of the stack.
    NewSheet,

    /// Close the focused sheet.
    CloseSheet,

    /// Move the focused sheet to the bottom, bringing the next one up.
    RotateSheets,

    /// Show the focused sheet full screen, or fold it back into the stack.
    ///
    /// This is tmux's zoom: a view, not a rearrangement. The stack keeps its
    /// order, so the toggle is its own inverse and nothing is remembered.
    ToggleFullScreen,

    /// Hand the focused sheet's scrollback to an external editor.
    ///
    /// This is the copy-mode entry point of the PoC: the scrollback is read
    /// out and passed to an editor, which is where a selection and a copy are
    /// made for now. The *content* is not part of the command -- it belongs to
    /// the focused sheet's live session, which only the edge holds -- so the
    /// command carries nothing and the edge assembles the text.
    EditScrollback,

    /// Paste the last text a sheet copied into the focused sheet.
    ///
    /// This is the other half of copy mode: a child that copies with OSC 52
    /// hands its text to the host terminal's clipboard, and this puts that
    /// same text back into the focused sheet. The *content* is not part of the
    /// command -- the clipboard belongs to the host terminal, which only the
    /// edge holds -- so the command carries nothing and the edge supplies the
    /// bytes.
    PasteClipboard,

    /// Leave the multiplexer.
    Quit,
}
