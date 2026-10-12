//! Input events fed into the Sans I/O core.
//!
//! An [`Event`] is a plain value. The edge builds one from whatever it read --
//! a decoded key, a terminal resize, a session exiting -- and hands it to
//! [`State::update()`](crate::State::update). The core never reads a file
//! descriptor or checks a clock to produce an event.

use crate::SheetId;

/// Something that happened and that the core must react to.
///
/// An event is not `Copy`: a title is a `String` the child chose, so the word
/// a sheet is labelled with costs an allocation to hand over. Everything else
/// is small and `Copy`, but the derive has to follow the widest variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A key was pressed on the host terminal.
    ///
    /// The key arrives already decoded, so deciding what it means -- whether it
    /// arms the prefix, names a command, or belongs to the focused sheet --
    /// stays in the core. Reading the bytes and turning them into this value is
    /// the edge's work.
    Key(tuinix::KeyInput),

    /// A mouse event happened on the host terminal.
    ///
    /// The position is what makes this event worth carrying to the core: it is
    /// already in terminal cells, so deciding *which sheet was clicked* stays in
    /// the core, next to the layout that produced the regions. Reading the
    /// report and decoding it is the edge's work.
    ///
    /// The whole [`tuinix::MouseInput`] rides along -- kind, position, and
    /// modifiers -- so a later gesture (a scroll, a modifier-click) does not
    /// need another event variant. Only the position and the press kind are read
    /// today; see [`handle_mouse`](crate::State::handle_mouse).
    ///
    /// Moving the focus is *all* the core does with a report, and the only thing
    /// a report can do: a click is not shown in the frame and nothing about it is
    /// remembered. The menu is chosen by keys, so a click can never run a
    /// command -- see [`handle_mouse`](crate::State::handle_mouse).
    Mouse(tuinix::MouseInput),

    /// Whether the focused sheet's child is asking to be sent the mouse.
    ///
    /// A program that wants the mouse -- a pager, an editor, `tmux` itself --
    /// turns reporting on with a DECSET mode, and `termnix` records the ask in
    /// the session's modes. The host terminal, though, is *tumu's* to configure,
    /// and the mouse can only be reported to one layer: with reporting on at the
    /// host a report reaches tumu and nowhere else, so a child that asked for it
    /// would never see one. The edge therefore tracks the focused session's ask
    /// and tells the core when it changes.
    ///
    /// It is *the focused sheet's* ask, not a per-session fact, because the
    /// mouse follows the focus the way the keyboard does: one pointer, one
    /// focused sheet, so one bit decides where a report goes. The bit travels
    /// with the state so that a later change (handing the mouse on to the child)
    /// needs no new event, and so a test can pin what happens in each case
    /// without a PTY.
    MouseWanted(bool),

    /// The host terminal changed size.
    ///
    /// This is what a device rotation arrives as, and it is the event the whole
    /// auto-derivation exists for: the stack re-lays-out and every sheet's
    /// region follows.
    Resize(tuinix::Size),

    /// The child in the sheet at `index` set its window title.
    ///
    /// The title is the string the child sent with OSC 0 or OSC 2, carried
    /// here so the core can keep it on the sheet: the renderer shows it in the
    /// title row, and the core does not hold the live session it came from.
    /// An empty title is a real value -- a terminal reset clears the title --
    /// so it is reported the same way rather than skipped.
    TitleChanged {
        /// The sheet whose session set the title.
        index: usize,
        /// The title as the child sent it, or empty when it cleared it.
        title: String,
    },

    /// The session hosted by the given sheet exited.
    SessionExited(SheetId),
}
