//! Input events fed into the Sans I/O core.
//!
//! An [`Event`] is a plain value. The edge builds one from whatever it read --
//! a decoded key, a terminal resize, a session exiting -- and hands it to
//! [`State::update()`](crate::State::update). The core never reads a file
//! descriptor or checks a clock to produce an event.

use crate::SheetId;

/// Something that happened and that the core must react to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Mouse(tuinix::MouseInput),

    /// The host terminal changed size.
    ///
    /// This is what a device rotation arrives as, and it is the event the whole
    /// auto-derivation exists for: the stack re-lays-out and every sheet's
    /// region follows.
    Resize(tuinix::Size),

    /// The session hosted by the given sheet exited.
    SessionExited(SheetId),
}
