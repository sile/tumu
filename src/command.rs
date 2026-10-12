//! The commands a latched key can name.
//!
//! A [`Command`] is what a key *means* -- the intent behind a prefix chord --
//! kept separate from the [`Action`](crate::Action) the edge carries out. One
//! command may expand to several actions, and the expansion happens against the
//! state as it is at the moment of the press, so a command never names a sheet.

/// One command a prefix chord can trigger.
///
/// The set is deliberately small: it is the least that makes the stack
/// touchable by hand (add -- a shell, an editor, a tool --, close, step around,
/// move, resize the split, rotate, zoom, open the scrollback, leave).
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
    /// the *right* when it runs horizontally. The step wraps: at the bottom (or
    /// the right) end of the stack it comes around to the top, so repeated
    /// steps cycle through every sheet rather than stopping at an end.
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
    /// It wraps for the same reason -- from the top it comes around to the
    /// bottom -- and resizes nothing.
    FocusPrevious,

    /// Move the focused sheet to the top of the stack.
    ///
    /// The focused sheet is lifted out of the stack and put back at the top,
    /// the sheets that were above it closing ranks behind it. It is a move of
    /// that one sheet, not a rotation: the keyboard stays where it was -- on the
    /// sheet that moved -- so the sheet comes to the top *under the keyboard*,
    /// which is what makes it useful (pinning the sheet in hand at the place
    /// new sheets appear). A sheet already at the top has nowhere to go, and a
    /// lone sheet has nothing to move past, so both are no-ops.
    ///
    /// This is a rearrangement, not a view: the stack's order changes, so every
    /// region is re-derived and the sessions are resized to match.
    MoveSheetToTop,

    /// Give the sheets at hand a wider share of the split.
    ///
    /// A stack splits what is left at each step in the proportion `num : 1`,
    /// the sheet at hand against every sheet below it put together; this moves
    /// `num` one part up, so each sheet that is not the bottom takes more and
    /// the bottom takes what is left. It is `prefix +`, the counterpart of
    /// [`NarrowSheet`](Command::NarrowSheet).
    ///
    /// This is a *layout* change, not a view one: every region is derived from
    /// the ratio, so the sessions are resized to match. The ratio is bounded
    /// ([`Ratio::MAX_NUM`](crate::Ratio::MAX_NUM)), so at the widest share the
    /// command is a no-op rather than a share that grows without end.
    WidenSheet,

    /// Give the sheets at hand a narrower share of the split.
    ///
    /// The inverse of [`WidenSheet`](Command::WidenSheet): it moves `num` one
    /// part down, so each sheet that is not the bottom takes less and the
    /// bottom takes more. It is `prefix -`, and it is bounded below the same
    /// way, so the odds never go past `1 : 1`.
    NarrowSheet,

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

    /// Add a sheet whose session is `mamediff`, in place of a shell.
    ///
    /// This is `prefix d`. It is the editor sheet's shape one step further: a
    /// new sheet on top, under the keyboard, whose program is not the shell but
    /// an external tool -- here the diff browser. The sheet is a sheet like any
    /// other, so its child is painted by tumu and its exit closes it through the
    /// ordinary [`SessionExited`](crate::Event::SessionExited) path; nothing is
    /// handed over to it.
    ///
    /// The command names *which* tool, not *how* to run it: the program and its
    /// arguments are the edge's, which is also the side that may read the
    /// environment for an override. The core only decides that a sheet is added
    /// and what kind of child it hosts.
    NewDiffSheet,

    /// Add a sheet whose session is `mamegrep`, in place of a shell.
    ///
    /// This is `prefix s`, the sibling of [`NewDiffSheet`](Command::NewDiffSheet):
    /// the same new sheet on top, running the grep browser instead of the diff
    /// one. The split of work is the same -- the core names the tool, the edge
    /// builds the command.
    NewGrepSheet,

    /// Leave the multiplexer.
    Quit,
}
