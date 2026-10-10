//! The multiplexer state and its pure handlers.

use crate::{Action, Command, Event, SheetId, Stack};

/// The index of the top of the stack: where a new sheet is pushed.
///
/// A new sheet goes on top, so the spawn index is always zero; it is named so
/// the commands that spawn a sheet do not each spell the same literal.
const STACK_TOP: usize = 0;

/// Returns the sheet that takes the place of the one that just left `index`.
///
/// The sheet that shifts down into the gap is the natural next focus, and the
/// new bottom is the fallback when the sheet that left was the last so nothing
/// shifted. `None` means the stack is now empty.
fn successor(sheets: &[(SheetId, crate::Sheet)], index: usize) -> Option<SheetId> {
    sheets
        .get(index)
        .or_else(|| sheets.last())
        .map(|(id, _)| *id)
}

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

    /// Whether the prefix menu is shown.
    ///
    /// Like full screen, it is a *view*: it changes no sheet's region, no
    /// session is resized, and the stack is untouched. It is painted over the
    /// composed screen, and its layout is a pure function of the size and the
    /// bindings -- so, like full screen, nothing has to be put back on the way
    /// out.
    pub menu: bool,

    /// Whether the focused sheet is shown full screen.
    ///
    /// Full screen is a *view*, not a rearrangement. It is a single flag rather
    /// than a remembered region, so nothing has to be put back on the way out:
    /// the other sheets were never moved and their regions are still a pure
    /// function of the size, the order, and the ratio. It is deliberately not
    /// transitive -- a resize keeps it, because the layout it hides is
    /// re-derived either way; see [`handle_resize`](State::handle_resize).
    pub full_screen: bool,

    /// Which sheet the commands act on.
    ///
    /// It is a [`SheetId`], not an index, so a sheet keeps its identity as the
    /// stack is reordered around it; [`focused_index`](State::focused_index)
    /// turns it into a position. It is cleared the moment it would name a sheet
    /// that is no longer in the stack -- see
    /// [`handle_rotate_sheets`](State::handle_rotate_sheets) and
    /// [`handle_session_exited`](State::handle_session_exited).
    pub focused: Option<SheetId>,

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
            menu: false,
            full_screen: false,
            focused: None,
            exit: false,
        }
    }

    /// Returns the axis the stack is laid out along at the current size.
    ///
    /// The renderer needs it to know which edge of a sheet carries the border:
    /// the last row when the stack runs vertically, the last column when it
    /// runs horizontally. It is the same function the stack splits its regions
    /// with
    /// ([`orientation_with_default_factor`](crate::orientation_with_default_factor))
    /// read from the same size, so the border's edge and the split's axis
    /// cannot disagree.
    pub fn orientation(&self) -> crate::Orientation {
        crate::orientation_with_default_factor(self.size)
    }

    /// Returns the menu's layout at the current size, or `None` when it does
    /// not fit.
    ///
    /// This is the one place the menu's geometry is derived, so the renderer and
    /// [`handle_mouse`](State::handle_mouse) test the same rectangles: a click
    /// lands on a command exactly when the row the renderer drew for that
    /// command was clicked.
    pub fn menu_layout(&self) -> Option<crate::MenuLayout> {
        crate::menu_layout(self.size, &crate::input::prefix_key_text())
    }

    /// Returns the index of the focused sheet, or `None` when there is none.
    ///
    /// The stack search is what keeps focus honest: a focus that names a sheet
    /// the stack no longer holds reads as nothing focused rather than as a
    /// stale index. The top of the stack is not a substitute for this, because
    /// a rotation moves a sheet out from under the top.
    pub fn focused_index(&self) -> Option<usize> {
        let focused = self.focused?;
        self.stack
            .sheets()
            .iter()
            .position(|(id, _)| *id == focused)
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
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(size) => self.handle_resize(size),
            Event::TitleChanged { index, title } => self.handle_title_changed(index, title),
            Event::SessionExited(id) => self.handle_session_exited(id),
        }
    }

    /// Records the title the child in the sheet at `index` set.
    ///
    /// Nothing on the layout moves: a title is a label, not a fact the regions
    /// are derived from, so no session is resized and no action is returned. The
    /// edge repaints because the frame it would build changed.
    ///
    /// An index with no sheet behind it is dropped rather than stored: the edge
    /// reads the title off a session it is about to reap, and the sheet can be
    /// gone by then.
    fn handle_title_changed(&mut self, index: usize, title: String) -> Vec<Action> {
        self.stack.set_title(index, title);
        Vec::new()
    }

    /// Moves focus to the sheet under the pointer.
    ///
    /// This is the first gesture mouse support answers, and the only one so far:
    /// a left press picks the sheet it landed on. The test is done against
    /// [`regions()`](State::regions) rather than the stack's own layout for one
    /// reason -- that is the method where full screen enters the layout. A sheet
    /// hidden behind a full-screen sheet has an empty region there, so a click
    /// on the space it would have had lands on nothing instead of stealing focus
    /// out from under the view. Reusing `Stack::hit_test` would test the
    /// unfolded layout and do exactly that.
    ///
    /// Every other kind -- a release, a drag, a wheel turn -- is dropped. None
    /// of them has a meaning yet, and a child never sees a mouse report while
    /// the edge keeps reporting on for itself, so there is nothing to forward.
    fn handle_mouse(&mut self, mouse: tuinix::MouseInput) -> Vec<Action> {
        if mouse.kind != tuinix::MouseInputKind::LeftPress {
            return Vec::new();
        }
        let position = mouse.position;

        // The menu, when it is up, gets first refusal: a click on a command row
        // runs it, and a click anywhere else in the box dismisses the menu. This
        // is tested before anything else so a menu drawn over a sheet does not
        // let a click fall through to the sheet under it.
        if self.menu {
            let layout = self.menu_layout();
            let command = layout
                .as_ref()
                .and_then(|layout| layout.command_at(position));
            self.menu = false;
            return match command {
                Some(command) => self.handle_command(command),
                None => Vec::new(),
            };
        }

        // The command label opens the menu. Only the focused sheet draws one, so
        // only its border line can be clicked, and the click does not move the
        // focus: the menu is about the focused sheet, and a click on the label
        // must not quietly change which sheet that is.
        if self.label_at(position) {
            self.menu = true;
            return Vec::new();
        }

        if let Some(index) = self.sheet_at(position)
            && let Some((id, _)) = self.stack.sheets().get(index)
        {
            self.focused = Some(*id);
        }
        // Focus is a pure state change: nothing has to be spawned, resized, or
        // written to a child. The edge repaints because the frame changed.
        Vec::new()
    }

    /// Returns whether `position` is on the focused sheet's command label.
    ///
    /// The label is the border line of the focused sheet: the last row of a
    /// vertical sheet, the last column of a horizontal one. Only the label's
    /// own part of the line is meant, but a click anywhere along the line
    /// accepts -- the border *is* the affordance, and separating the label's
    /// few cells from the rule's many would make the target smaller than the
    /// thing drawn.
    ///
    /// A hidden sheet (full screen elsewhere) has an empty region, and a stack
    /// with no focus has no label, so neither can be clicked.
    fn label_at(&self, position: tuinix::Position) -> bool {
        let orientation = self.orientation();
        self.regions()
            .iter()
            .zip(self.stack.sheets())
            .enumerate()
            .any(|(index, (region, _))| {
                if Some(index) != self.focused_index() {
                    return false;
                }
                if region.size.rows == 0 || region.size.cols == 0 {
                    return false;
                }
                match orientation {
                    crate::Orientation::Vertical => {
                        position.row == region.position.row + region.size.rows - 1
                            && position.col >= region.position.col
                            && position.col < region.position.col + region.size.cols
                    }
                    crate::Orientation::Horizontal => {
                        position.col == region.position.col + region.size.cols - 1
                            && position.row >= region.position.row
                            && position.row < region.position.row + region.size.rows
                    }
                }
            })
    }

    /// Returns the index of the sheet under `position`, or `None` when the
    /// position is outside every visible sheet.
    ///
    /// The first sheet in stack order wins, so an overlapping click -- only
    /// reachable by a size the split cannot give every sheet a cell of -- picks
    /// the one on top, the way a real stack would.
    pub fn sheet_at(&self, position: tuinix::Position) -> Option<usize> {
        self.regions()
            .iter()
            .position(|region| region.contains(position))
    }

    /// Turns a key press into actions, in the tmux prefix style.
    ///
    /// The rule, which is the prefix latch the README names:
    ///
    /// - Unlatched, the key is the prefix: arm the latch, open the
    ///   [`menu`](State::menu), and report [`Action::PrefixPending`].
    /// - Latched, the key is the prefix itself: clear the latch and forward the
    ///   prefix to the focused sheet, so a bare prefix can still be typed. This
    ///   is tmux's *send-prefix*.
    /// - Latched, the key is bound: clear the latch and carry the command out.
    /// - Latched, the key is unbound: clear the latch and emit nothing. The
    ///   prefix was a command that resolved to nothing, not a keystroke.
    /// - Unlatched, any other key: forward it to the focused sheet.
    ///
    /// The menu opens with the latch and closes with it, so what is on screen and
    /// what the next key means are never out of step. A forwarded key is dropped
    /// when the stack is empty, rather than sent to nowhere.
    fn handle_key(&mut self, key: tuinix::KeyInput) -> Vec<Action> {
        if !self.prefix {
            if crate::input::is_prefix(key) {
                self.prefix = true;
                // Arming the prefix also opens the menu, so the chords are
                // visible while one is being chosen, not just known by heart.
                // A screen too small for the box shows no menu -- the flag
                // tracks what is on screen, so a click never lands on a menu
                // that was not drawn.
                self.menu = self.menu_layout().is_some();
                return vec![Action::PrefixPending];
            }
            return self.send_key(key);
        }

        // From here the prefix is latched. Whichever key arrived, the latch
        // clears and the menu closes: the chord is one key, and the menu is the
        // face of that one key.
        self.prefix = false;
        self.menu = false;
        if crate::input::is_prefix(key) {
            return self.send_key(key);
        }
        match crate::input::resolve_command(key) {
            Some(command) => self.handle_command(command),
            // An unbound key dismisses the menu and does nothing else: the
            // prefix was a command that resolved to nothing, not a keystroke to
            // forward.
            None => Vec::new(),
        }
    }

    /// Forwards `key` to the focused sheet's session, or drops it.
    ///
    /// Which sheet receives it is named by focus, not by position: the key is
    /// delivered to the focused sheet even when it is not the top of the stack,
    /// so rotating the stack moves the keyboard to the sheet that came up. With
    /// no sheet to receive it the key is dropped.
    fn send_key(&self, key: tuinix::KeyInput) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            return Vec::new();
        };
        vec![Action::SendToFocused { index, key }]
    }

    /// Carries a resolved command out, returning the actions it expands to.
    fn handle_command(&mut self, command: Command) -> Vec<Action> {
        match command {
            Command::NewSheet => self.handle_new_sheet(),
            Command::CloseSheet => self.handle_close_sheet(),
            Command::RotateSheets => self.handle_rotate_sheets(),
            Command::ToggleFullScreen => self.handle_toggle_full_screen(),
            Command::EditScrollback => self.handle_edit_scrollback(),
            Command::PasteClipboard => self.handle_paste_clipboard(),
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
        let id = self.stack.push_top();
        self.focused = Some(id);
        let mut actions = vec![Action::SpawnSession { index: STACK_TOP }];
        actions.extend(self.resize_actions());
        actions
    }

    /// Closes the focused sheet and asks the edge to close its session.
    ///
    /// Does nothing when the stack is empty. The remaining sheets re-derive
    /// their regions, so closing one grows the rest.
    fn handle_close_sheet(&mut self) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            return Vec::new();
        };
        self.stack.remove(index);
        // The next sheet to show is the one that took the closed sheet's place,
        // or the new bottom when the closed one was last.
        self.focused = successor(self.stack.sheets(), index);
        let mut actions = vec![Action::CloseSession { index }];
        actions.extend(self.resize_actions());
        actions
    }

    /// Moves the focused sheet to the bottom and the next one to the top.
    ///
    /// Does nothing when there is at most one sheet, since rotating a single
    /// sheet is a no-op. The regions swap with the order, so every session is
    /// resized.
    fn handle_rotate_sheets(&mut self) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            return Vec::new();
        };
        if self.stack.len() <= 1 {
            return Vec::new();
        }
        // Whichever sheet takes the focused one's place is focused next, so the
        // rotation brings the stack up under the keyboard rather than moving the
        // keyboard away. The focused sheet goes to the bottom, so rotating the
        // top sheet leaves focus on the new top.
        self.stack.move_to_bottom(index);
        self.focused = successor(self.stack.sheets(), index)
            .or_else(|| self.stack.sheets().first().map(|(id, _)| *id));
        self.resize_actions()
    }

    /// Asks the edge to open the focused sheet's scrollback in an editor.
    ///
    /// The core names the sheet and stops there: the scrollback belongs to the
    /// `termnix` session the sheet hosts, which the core does not hold, so the
    /// edge reads it out and hands it on. Focus is resolved here, as for every
    /// other action, so a rotation moves the keyboard *and* the history a later
    /// `prefix l` opens together.
    ///
    /// Nothing in the state changes: opening the scrollback is not a view of
    /// tumu's own, so the stack, the focus, and the full-screen flag are left as
    /// they were. An empty stack has nothing to open, so it is a no-op.
    fn handle_edit_scrollback(&mut self) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            return Vec::new();
        };
        vec![Action::EditScrollback { index }]
    }

    /// Asks the edge to paste the host clipboard into the focused sheet.
    ///
    /// The core names the sheet and stops there: the clipboard belongs to the
    /// host terminal, which the core does not hold, so the edge reads it and
    /// enqueues it as input. Focus is resolved here, as for every other action,
    /// so a rotation moves the keyboard *and* where a later `prefix y` pastes
    /// together.
    ///
    /// Nothing in the state changes: pasting is not a view of tumu's own, so
    /// the stack, the focus, and the full-screen flag are left as they were. An
    /// empty stack has nowhere to paste, so it is a no-op.
    fn handle_paste_clipboard(&mut self) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            return Vec::new();
        };
        vec![Action::PasteClipboard { index }]
    }

    /// Flips full screen and resizes every sheet to match the new layout.
    ///
    /// Neither capture nor restore touches the stack: the focused sheet's
    /// region becomes the whole terminal, the other sheets keep their places and
    /// fold back in when the flag clears. The resize is what makes a full-screen
    /// sheet's session take the terminal over, and what hands the rows back to
    /// the hidden sheets on the way out.
    fn handle_toggle_full_screen(&mut self) -> Vec<Action> {
        self.full_screen = !self.full_screen;
        self.resize_actions()
    }

    /// Returns a `ResizeSession` for every sheet that ends up with a usable
    /// region.
    ///
    /// A region too small to give the child a non-zero grid is skipped, the
    /// same way a resize that produces one is.
    fn resize_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        for (index, region) in self.regions().into_iter().enumerate() {
            if let Some(grid) = crate::terminal_size_to_cell_size(region.size) {
                actions.push(Action::ResizeSession { index, size: grid });
            }
        }
        actions
    }

    /// Returns the region of every visible sheet, in stack order.
    ///
    /// This is where full screen enters the layout: with it on, the focused
    /// sheet takes the whole terminal and the others get no region at all,
    /// which both stops the edge from drawing them and stops
    /// [`resize_actions`](State::resize_actions) from resizing their sessions to
    /// a zero grid. The regions the other sheets would have had are not lost --
    /// they are the same pure function of the size, the order, and the ratio, so
    /// clearing the flag brings them straight back.
    ///
    /// The returned vector is always as long as the stack; a sheet the full
    /// screen hides has an empty region at its index rather than no entry, so an
    /// index still names the same sheet whether or not it is on show.
    pub fn regions(&self) -> Vec<tuinix::Region> {
        if !self.full_screen {
            return self.stack.regions(self.size);
        }
        let focused = self.focused_index();
        let full = crate::terminal_region(self.size);
        let empty = tuinix::Region {
            position: full.position,
            size: tuinix::Size { rows: 0, cols: 0 },
        };
        (0..self.stack.len())
            .map(|index| if Some(index) == focused { full } else { empty })
            .collect()
    }

    /// Returns the region of the sheet at `index`, in stack order.
    ///
    /// Returns `None` when there is no such sheet. A sheet the full screen
    /// hides has an empty region, so it is nothing to draw.
    pub fn region_of(&self, index: usize) -> Option<tuinix::Region> {
        self.regions().into_iter().nth(index)
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
            let id = self.stack.push_top();
            self.focused = Some(id);
            actions.push(Action::SpawnSession { index: STACK_TOP });
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
        let was_focused = self.focused == Some(id);
        self.stack.remove(index);
        if was_focused {
            self.focused = successor(self.stack.sheets(), index);
            // A sheet dying under full screen would leave nothing to show, so
            // the flag clears rather than zoom the sheet that took its place.
            self.full_screen = false;
        }
        let mut actions = vec![Action::CloseSession { index }];
        actions.extend(self.resize_actions());
        actions
    }
}
