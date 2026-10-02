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

    /// Leave the multiplexer.
    Exit,
}
