//! The intents the core returns for the edge to carry out.

/// What a new session in a sheet should run.
///
/// The core decides *which* program a sheet hosts; it does not build the
/// command. A sheet is usually a shell, but `prefix l` opens one whose session
/// is an editor reading a file the core also named -- so the choice has to
/// travel with the spawn rather than be fixed at the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// The user's shell, as every ordinary sheet runs.
    Shell,

    /// The external editor, opened on the file named `name`.
    ///
    /// The core names the file because the core is what wrote the text: the
    /// scrollback is a pure function of the sessions' terminal states, so the
    /// core assembles it and the edge writes it out. The name is bare -- a file
    /// name, not a path -- because *where* files may be written is the edge's to
    /// know; the directory is added there, once, so the spawn and the write
    /// cannot disagree about it.
    Editor {
        /// The file to open, without a directory.
        name: String,
    },
}

/// One thing the edge should do.
///
/// An action names *what* to do, not *how*: the core decides that a sheet was
/// added or a session must be resized, and the edge performs the PTY work an
/// action implies. The core never performs an action itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Spawn a session and attach it to the sheet at `index`.
    SpawnSession {
        /// The sheet that should host the new session.
        index: usize,

        /// What to run in it.
        program: Program,
    },

    /// Write the scrollback of `session` to a file and open it in an editor in
    /// the sheet at `index`.
    ///
    /// This is `prefix l`, and it is deliberately *not* a terminal handover:
    /// the editor runs as the session of a new sheet, so it is painted like any
    /// other child and the multiplexer keeps the terminal. The core has already
    /// added the sheet and named the index it wants the editor in, so the edge
    /// only has to put the bytes on disk and spawn.
    ///
    /// The session is named rather than its text because reading the text --
    /// cells to characters, trailing blanks trimmed -- needs the session, which
    /// the edge holds and the core does not. The core picks *whose* history to
    /// show; the edge has the bytes.
    OpenEditor {
        /// Where the editor session should go.
        index: usize,

        /// The session whose scrollback the editor opens.
        session: crate::TermnixSessionId,
    },

    /// Resize the session hosted by the sheet at `index` to `size`.
    ResizeSession {
        /// The sheet that hosts the session.
        index: usize,
        /// The grid size the session should take.
        size: termnix::Size,
    },

    /// Close the session with this id.
    ///
    /// The session is named by id rather than by the index of the sheet that
    /// hosts it, because the core has already removed that sheet from the stack
    /// by the time the edge carries the action out: an index would name whatever
    /// sheet shifted into the gap, and closing *that* sheet's session is the bug
    /// this shape exists to avoid.
    CloseSession {
        /// The session to close.
        session: crate::TermnixSessionId,
    },

    /// Send a key to the session hosted by the focused sheet.
    ///
    /// The key is carried undecoded, exactly as it arrived, because re-encoding
    /// it for the child is the edge's job and belongs to `termnix`, which knows
    /// the child's terminal modes. The core only decides *that* the key is the
    /// focused sheet's to receive.
    ///
    /// The sheet is named by index, like every other action, so focus is
    /// resolved in the core -- which is what lets the keyboard follow a
    /// rotation -- rather than guessed at the edge.
    SendToFocused {
        /// The sheet whose session should receive the key.
        index: usize,

        /// The key to forward.
        key: tuinix::KeyInput,
    },

    /// Paste the host clipboard into the session of the sheet at `index`.
    ///
    /// The action names *which* sheet, not *what* to paste: the clipboard
    /// belongs to the host terminal, which only the edge holds, so the edge
    /// reads it out and enqueues it as input. The core only decides that the
    /// focused sheet is where it goes, exactly as it resolves focus for every
    /// other action.
    PasteClipboard {
        /// The sheet whose session should receive the paste.
        index: usize,
    },

    /// The prefix latch is now armed and waiting for the next key.
    ///
    /// Nothing has to be done with it yet; it is reported so the edge can show
    /// that a command is pending, the way tmux marks the prefix.
    PrefixPending,

    /// Leave the multiplexer.
    Exit,
}
