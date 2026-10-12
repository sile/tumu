//! The commands a latched key can name.
//!
//! A [`Command`] is what a key *means* -- the intent behind a prefix chord --
//! kept separate from the [`Action`](crate::Action) the edge carries out. One
//! command may expand to several actions, and the expansion happens against the
//! state as it is at the moment of the press, so a command never names a sheet.

/// One command a prefix chord can trigger.
///
/// The set is deliberately small: it is the least that makes the stack
/// touchable by hand (add, close, step around, rotate, zoom, open the
/// scrollback, leave). The ratio is the next addition and will extend this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// Add a new sheet on top of the stack.
    NewSheet,

    /// Close the focused sheet.
    CloseSheet,

    /// Move the focus to the sheet that comes next in the stack.
    ///
    /// "Next" is the step along the stacking axis, in the direction the stack
    /// advances: the sheet *below* when the stack runs vertically, the sheet to
    /// the *right* when it runs horizontally. The step is one place, not a
    /// wrap: at the bottom (or the right) end of the stack it does nothing,
    /// rather than run around to the top, so focus never moves somewhere the
    /// user was not looking.
    ///
    /// Focus is what this moves, not the sheets: the stack keeps its order and
    /// every region keeps its shape, so there is nothing to resize and no
    /// session to wake. The command carries nothing because the direction is
    /// the command -- the axis it is read along belongs to the state, which is
    /// why the two steps are two commands rather than one with a sign.
    FocusNext,

    /// Move the focus to the sheet that comes before in the stack.
    ///
    /// The inverse step of [`FocusNext`](Command::FocusNext): the sheet
    /// *above* in a vertical stack, the one to the *left* in a horizontal one.
    /// It is a plain step for the same reason -- one place, no wrap, nothing
    /// resized.
    FocusPrevious,

    /// Move the focused sheet to the bottom, bringing the next one up.
    RotateSheets,

    /// Stack the sheets the other way: top to bottom, or left to right.
    ///
    /// This is a *layout* change, not a view one: every sheet's region takes a
    /// different shape, so the sessions are resized to match. It replaces the
    /// automatic flip a device rotation used to cause, so the axis now changes
    /// only when it is asked to.
    RotateOrientation,

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

    /// Open the extended menu: the second row of commands, where the ones whose
    /// effect cannot be taken back live.
    ///
    /// Closing a sheet and quitting both end something the user may still want,
    /// so they are one step removed from the prefix. The step is a menu rather
    /// than a confirmation prompt because a prompt asks the *same* question
    /// every time and can be answered by muscle memory, while a menu makes the
    /// user read a different row before choosing -- the shape `kk`'s ext mode
    /// has. The command carries nothing: the menu holds its own bindings.
    OpenExtMenu,

    /// Leave the multiplexer.
    Quit,
}
