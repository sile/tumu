//! The multiplexer state and its pure handlers.

use crate::{Action, Command, Event, Stack};

/// Everything the core knows: the stack, the terminal size it fills, and
/// whether the prefix latch is armed.
///
/// The fields are public so the renderer and the edge can read them directly;
/// every change that affects layout goes through [`update()`](State::update) so
/// the regions the edge acts on stay consistent with the stack.
#[derive(Debug, Clone)]
pub struct State {
    /// The sheets, top to bottom.
    pub stack: Stack,

    /// The current terminal size.
    ///
    /// Every sheet's region is derived from this on demand, so it is the one
    /// stored fact a resize changes.
    pub size: tuinix::Size,

    /// Whether the prefix latch is armed.
    ///
    /// It is a latch, not a mode: arming it does not change what a sheet shows,
    /// and it clears on the next key whatever that key is. See
    /// [`handle_key`](State::handle_key) for the rule.
    pub prefix: bool,

    /// Whether the loop should stop.
    pub exit: bool,
}

impl State {
    /// Builds a state over an empty stack at `size`.
    pub const fn new(size: tuinix::Size) -> Self {
        Self {
            stack: Stack::new(),
            size,
            prefix: false,
            exit: false,
        }
    }

    /// Applies `event` and returns the actions the edge should carry out.
    ///
    /// The handler is pure with respect to the outside world: it mutates the
    /// state and returns plain values. The events that imply I/O -- spawning a
    /// session for a new sheet, resizing one after a change, forwarding a key to
    /// a child -- return the intents to do so rather than performing them.
    pub fn update(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize(size) => self.handle_resize(size),
            Event::SessionExited(id) => self.handle_session_exited(id),
        }
    }

    /// Turns a key press into actions, in the tmux prefix style.
    ///
    /// The rule, which is the prefix latch the README names:
    ///
    /// - Unlatched, the key is the prefix: arm the latch and report
    ///   [`Action::PrefixPending`] so the edge can show a command is pending.
    /// - Latched, the key is the prefix itself: clear the latch and forward the
    ///   prefix to the focused sheet, so a bare prefix can still be typed. This
    ///   is tmux's *send-prefix*.
    /// - Latched, the key is bound: clear the latch and carry the command out.
    /// - Latched, the key is unbound: clear the latch and emit nothing. The
    ///   prefix was a command that resolved to nothing, not a keystroke.
    /// - Unlatched, any other key: forward it to the focused sheet.
    ///
    /// A forwarded key is dropped when the stack is empty, rather than sent to
    /// nowhere.
    fn handle_key(&mut self, key: tuinix::KeyInput) -> Vec<Action> {
        if !self.prefix {
            if crate::input::is_prefix(key) {
                self.prefix = true;
                return vec![Action::PrefixPending];
            }
            return self.send_key(key);
        }

        self.prefix = false;
        if crate::input::is_prefix(key) {
            return self.send_key(key);
        }
        match crate::input::resolve_command(key) {
            Some(command) => self.handle_command(command),
            None => Vec::new(),
        }
    }

    /// Forwards `key` to the focused sheet's session, or drops it.
    ///
    /// The focused sheet is the top of the stack; a separate focus field is a
    /// later addition. With no sheet to receive it the key is dropped.
    fn send_key(&self, key: tuinix::KeyInput) -> Vec<Action> {
        if self.stack.is_empty() {
            return Vec::new();
        }
        vec![Action::SendToFocused { key }]
    }

    /// Carries a resolved command out, returning the actions it expands to.
    fn handle_command(&mut self, command: Command) -> Vec<Action> {
        match command {
            Command::NewSheet => self.handle_new_sheet(),
            Command::CloseSheet => self.handle_close_sheet(),
            Command::RotateSheets => self.handle_rotate_sheets(),
            Command::Quit => {
                self.exit = true;
                vec![Action::Exit]
            }
        }
    }

    /// Adds a sheet on top and asks the edge to spawn its session.
    ///
    /// The existing sheets keep their sessions and re-derive their regions, so
    /// the new sheet costs each of them rows; the spawn is named by index
    /// because the new sheet is on top.
    fn handle_new_sheet(&mut self) -> Vec<Action> {
        self.stack.push_top();
        let mut actions = vec![Action::SpawnSession { index: 0 }];
        actions.extend(self.resize_actions());
        actions
    }

    /// Closes the focused sheet and asks the edge to close its session.
    ///
    /// Does nothing when the stack is empty. The remaining sheets re-derive
    /// their regions, so closing one grows the rest.
    fn handle_close_sheet(&mut self) -> Vec<Action> {
        if self.stack.is_empty() {
            return Vec::new();
        }
        self.stack.remove(0);
        let mut actions = vec![Action::CloseSession { index: 0 }];
        actions.extend(self.resize_actions());
        actions
    }

    /// Moves the focused sheet to the bottom and the next one to the top.
    ///
    /// Does nothing when there is at most one sheet, since rotating a single
    /// sheet is a no-op. The regions swap with the order, so every session is
    /// resized.
    fn handle_rotate_sheets(&mut self) -> Vec<Action> {
        if self.stack.len() <= 1 {
            return Vec::new();
        }
        self.stack.move_to_bottom(0);
        self.resize_actions()
    }

    /// Returns a `ResizeSession` for every sheet that ends up with a usable
    /// region.
    ///
    /// A region too small to give the child a non-zero grid is skipped, the
    /// same way a resize that produces one is.
    fn resize_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        for (index, region) in self.stack.regions(self.size).into_iter().enumerate() {
            if let Some(grid) = crate::terminal_size_to_cell_size(region.size) {
                actions.push(Action::ResizeSession { index, size: grid });
            }
        }
        actions
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

        actions.extend(self.resize_actions());
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
        actions.extend(self.resize_actions());
        actions
    }
}
