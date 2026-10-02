//! The multiplexer state and its pure handlers.

use crate::{Action, Event, Stack};

/// Everything the core knows: the stack and the terminal size it fills.
///
/// The fields are public so the renderer and the edge can read them directly;
/// every change that affects layout goes through [`update()`](State::update) so
/// the regions the edge acts on stay consistent with the stack.
#[derive(Debug, Clone)]
pub struct State {
    /// The sheet, top to bottom.
    pub stack: Stack,

    /// The current terminal size.
    ///
    /// Every sheet's region is derived from this on demand, so it is the one
    /// stored fact a resize changes.
    pub size: tuinix::Size,

    /// Whether the loop should stop.
    pub exit: bool,
}

impl State {
    /// Builds a state over an empty stack at `size`.
    pub const fn new(size: tuinix::Size) -> Self {
        Self {
            stack: Stack::new(),
            size,
            exit: false,
        }
    }

    /// Applies `event` and returns the actions the edge should carry out.
    ///
    /// The handler is pure with respect to the outside world: it mutates the
    /// state and returns plain values. The two events that imply I/O -- spawning
    /// a session for a new sheet and resizing one after a change -- return the
    /// intents to do so rather than performing them.
    pub fn update(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::Resize(size) => self.handle_resize(size),
            Event::SessionExited(id) => self.handle_session_exited(id),
        }
    }

    /// Records the new size and lays every sheet out again.
    ///
    /// A new sheet is spawned when the stack was empty, so startup and the
    /// first resize share one path. Every sheet's session is resized to the
    /// region the new size gives it, which is how a rotation reaches the
    /// children.
    fn handle_resize(&mut self, size: tuinix::Size) -> Vec<Action> {
        self.size = size;
        let mut actions = Vec::new();

        if self.stack.is_empty() {
            self.stack.push_top();
            actions.push(Action::SpawnSession { index: 0 });
        }

        for (index, region) in self.stack.regions(size).into_iter().enumerate() {
            if let Some(grid) = crate::terminal_size_to_cell_size(region.size) {
                actions.push(Action::ResizeSession { index, size: grid });
            }
        }

        actions
    }

    /// Drops the sheet whose session exited and closes the session.
    ///
    /// A sheet with no session that has exited is just a sheet that stopped;
    /// the remaining sheets re-derive their regions as the stack closes the
    /// gap.
    fn handle_session_exited(&mut self, id: crate::SheetId) -> Vec<Action> {
        let Some(index) = self.stack.sheets().iter().position(|(s, _)| *s == id) else {
            return Vec::new();
        };
        self.stack.remove(index);
        let mut actions = vec![Action::CloseSession { index }];
        for (index, region) in self.stack.regions(self.size).into_iter().enumerate() {
            if let Some(grid) = crate::terminal_size_to_cell_size(region.size) {
                actions.push(Action::ResizeSession { index, size: grid });
            }
        }
        actions
    }
}
