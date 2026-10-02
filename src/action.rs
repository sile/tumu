//! The intents the core returns for the edge to carry out.

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
    },

    /// Resize the session hosted by the sheet at `index` to `size`.
    ResizeSession {
        /// The sheet that hosts the session.
        index: usize,
        /// The grid size the session should take.
        size: termnix::Size,
    },

    /// Close the session hosted by the sheet at `index`.
    CloseSession {
        /// The sheet whose session should be closed.
        index: usize,
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

    /// The prefix latch is now armed and waiting for the next key.
    ///
    /// Nothing has to be done with it yet; it is reported so the edge can show
    /// that a command is pending, the way tmux marks the prefix.
    PrefixPending,

    /// Leave the multiplexer.
    Exit,
}
