//! The multiplexer state and its pure handlers.

use crate::{Action, Command, Event, Program, SheetId, Stack};

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

    /// Which way the sheets stack: top to bottom, or left to right.
    ///
    /// It is a setting, not a function of the terminal's shape. It used to be
    /// derived from the size, which is why a rotation flipped the stack by
    /// itself; now the axis changes only when `prefix r` (or `--orientation`)
    /// asks for it, so a resize re-lays the stack out along the axis it already
    /// has. Every region is a pure function of this and the size, so nothing
    /// has to be recomputed or stored when it flips.
    pub orientation: crate::Orientation,

    /// The character the prefix key carries, `Ctrl` fixed.
    ///
    /// Only the character is configurable -- see
    /// [`DEFAULT_PREFIX_CHAR`](crate::DEFAULT_PREFIX_CHAR) and
    /// [`prefix_key`](crate::prefix_key) -- so the key that arms the latch, the
    /// label on the border, and the menu's send-prefix row all read it from
    /// here and cannot disagree. It also means a test can run a state with any
    /// prefix without touching a global.
    pub prefix_char: char,

    /// Whether the prefix menu is shown.
    ///
    /// Like full screen, it is a *view*: it changes no sheet's region, no
    /// session is resized, and the stack is untouched. It is painted over the
    /// composed screen, and its layout is a pure function of the size and the
    /// bindings -- so, like full screen, nothing has to be put back on the way
    /// out.
    pub menu: bool,

    /// Whether the focused sheet's child is asking for the mouse.
    ///
    /// It is *the focused sheet's* ask, not a per-session fact, because the mouse
    /// follows the focus the way the keyboard does: one pointer, one focused
    /// sheet, so one bit decides where a report goes. The edge recomputes it when
    /// the focus changes and reports the change as
    /// [`Event::MouseWanted`](crate::Event::MouseWanted).
    ///
    /// While it is set, a mouse report that lands in the focused sheet's region
    /// is the child's and is forwarded to it -- as is any report once a forwarded
    /// press has armed a button, so a drag keeps going to the child that saw the
    /// press. A report that lands outside the region is tumu's own and only moves
    /// the focus, whether or not the child asked. That is the whole of the
    /// handover: the host terminal's reporting never changes, because the one
    /// thing on the other side of it -- the focused sheet's child -- changes what
    /// it does with a report instead.
    pub mouse_wanted: bool,

    /// The mouse button the core believes is held down, if any.
    ///
    /// A drag report from the host does not name the button being held, but the
    /// child has to be told which one it is, so it is tracked here from the
    /// presses and releases that were forwarded. It only ever counts buttons
    /// tumu sent: a press that arrived while the child was not asking for the
    /// mouse was tumu's, so no drag can be a continuation of it.
    pub held_button: Option<tuinix::MouseInputKind>,

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
            prefix_char: crate::input::DEFAULT_PREFIX_CHAR,
            orientation: crate::Orientation::DEFAULT,
            menu: false,
            mouse_wanted: false,
            held_button: None,
            full_screen: false,
            focused: None,
            exit: false,
        }
    }

    /// Sets the character the prefix key carries and returns the state.
    ///
    /// The prefix is `Ctrl` plus this character; the `Ctrl` is fixed, so the
    /// setting is one character and nothing else. It is set once, from what the
    /// edge read off the command line and the environment -- see the binary's
    /// `main` -- so the latch, the border label, and the menu's send-prefix row
    /// all agree on the key the user actually chose.
    pub fn with_prefix_char(mut self, c: char) -> Self {
        self.prefix_char = c;
        self
    }

    /// Sets the axis the sheets stack along and returns the state.
    ///
    /// Like the prefix character, it is set once, from what the edge read off
    /// the command line and the environment -- see the binary's `main` -- and
    /// from then on it is `prefix r` that changes it. It is the one source of
    /// the axis: the split, the border's edge, and the menu's label all read it
    /// back, so they cannot disagree with each other or with the setting.
    pub fn with_orientation(mut self, orientation: crate::Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Returns the axis the stack is laid out along.
    ///
    /// The renderer needs it to know which edge of a sheet carries the border:
    /// the last row when the stack runs vertically, the last column when it
    /// runs horizontally. It is the stored setting, so the border's edge and
    /// the split's axis cannot disagree: [`regions`](State::regions) splits by
    /// this value and the border is drawn by it.
    pub fn orientation(&self) -> crate::Orientation {
        self.orientation
    }

    /// Returns the menu's layout at the current size, or `None` when it does
    /// not fit.
    ///
    /// This is the one place the menu's geometry is derived, so the renderer and
    /// [`handle_mouse`](State::handle_mouse) test the same rectangles: a click
    /// lands on a command exactly when the row the renderer drew for that
    /// command was clicked.
    pub fn menu_layout(&self) -> Option<crate::MenuLayout> {
        crate::menu_layout(self.size, &crate::input::prefix_key_text(self.prefix_char))
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
    ///
    pub fn update(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::MouseWanted(wanted) => self.handle_mouse_wanted(wanted),
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

    /// Routes a mouse report: to the focused sheet's child, or to the focus.
    ///
    /// Which one it is is [`mouse_wanted`](State::mouse_wanted): a child that
    /// asked for the mouse with a DECSET mode gets a report that lands in its
    /// own region, and such a report does not move the focus -- a program that
    /// tracks the mouse is tracking it inside its own grid, and a click on the
    /// sheet it runs in is a click on *it*, not a request to focus something
    /// else. A child that has not asked gets nothing; there, the one gesture
    /// tumu answers is a left press, which moves the focus to the sheet it
    /// landed on.
    ///
    /// A report that lands *outside* the focused sheet's region is tumu's own,
    /// even while the focused child is asking for the mouse: the child's grid is
    /// the whole of what it can have a report about, and a click on another
    /// sheet is a click on tumu's chrome, not on the child. So a click on a
    /// neighbour moves the focus to it, which is what lets the user leave a
    /// mouse-tracking program (a pager, an editor) by clicking the sheet they
    /// want instead. The one exception is a report that continues a press the
    /// child already saw: a held button keeps the report, wherever it lands, so
    /// a drag that starts in the region and leaves it is still the child's.
    ///
    /// Inside the region, a report is forwarded to the *focused* sheet: the
    /// keyboard's rule, and the only one that makes sense for a child, since the
    /// pointer it sees is its own and a sheet that is not focused has no child
    /// waiting to hear about one. A position outside the child's grid -- the
    /// border row a full-screen sheet does not have, say -- has no coordinates in
    /// a terminal mouse protocol, so the edge clips it into the grid rather than
    /// dropping it.
    ///
    /// The held button is tracked here rather than at the edge because it is
    /// state: a drag report does not name its button, so the child can only be
    /// told which one it is if someone remembers the press. Only presses that
    /// were *forwarded* count, so a click that moved the focus leaves nothing
    /// behind for a later drag to be a continuation of.
    ///
    /// The hit test for the focus path is done against
    /// [`regions()`](State::regions) rather than the stack's own layout for one
    /// reason -- that is the method where full screen enters the layout. A sheet
    /// hidden behind a full-screen sheet has an empty region there, so a click on
    /// the space it would have had lands on nothing instead of stealing focus out
    /// from under the view. Reusing `Stack::hit_test` would test the unfolded
    /// layout and do exactly that.
    fn handle_mouse(&mut self, mouse: tuinix::MouseInput) -> Vec<Action> {
        // The focused child's report is the one that lands in its region, or any
        // report at all once it has a button held from such a report: a drag may
        // leave the region and has to keep going to the child that saw its press.
        if self.mouse_wanted
            && (self.held_button.is_some() || self.lands_on_focused_region(mouse.position))
        {
            return self.forward_mouse(mouse);
        }
        if mouse.kind != tuinix::MouseInputKind::LeftPress {
            return Vec::new();
        }
        if let Some(index) = self.sheet_at(mouse.position)
            && let Some((id, _)) = self.stack.sheets().get(index)
        {
            self.focused = Some(*id);
        }
        // Focus is a pure state change: nothing has to be spawned, resized, or
        // written to a child. The edge repaints because the frame changed.
        Vec::new()
    }

    /// Returns whether `position` is inside the focused sheet's region.
    ///
    /// The test is against [`regions()`](State::regions), the same layout the
    /// hit test uses, so it agrees with what is on screen: a full-screen sheet's
    /// region is the whole terminal there, and a hidden sheet's is empty. An
    /// empty stack has no region, so nothing lands on it.
    fn lands_on_focused_region(&self, position: tuinix::Position) -> bool {
        let Some(index) = self.focused_index() else {
            return false;
        };
        self.region_of(index)
            .is_some_and(|region| region.contains(position))
    }

    /// Sends a mouse report to the focused sheet's child and tracks the button.
    ///
    /// The button is what makes this more than a pass-through: a press arms it, a
    /// release clears it, and a drag carries whatever is armed so the child can
    /// tell a drag from a bare move. The wheel is not tracked -- it has no
    /// release, so it can never be held -- which is why only the press and
    /// release kinds touch the field.
    ///
    /// A report with no focused sheet has nowhere to go, and is dropped: the
    /// ask that put us in this path can only have come from a focused session,
    /// but the stack can empty between the two -- a child can exit and be reaped
    /// before its own report is read back.
    fn forward_mouse(&mut self, mouse: tuinix::MouseInput) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            self.held_button = None;
            return Vec::new();
        };
        let held = match mouse.kind {
            tuinix::MouseInputKind::LeftPress
            | tuinix::MouseInputKind::MiddlePress
            | tuinix::MouseInputKind::RightPress => {
                self.held_button = Some(mouse.kind);
                None
            }
            tuinix::MouseInputKind::LeftRelease
            | tuinix::MouseInputKind::MiddleRelease
            | tuinix::MouseInputKind::RightRelease => {
                self.held_button = None;
                None
            }
            tuinix::MouseInputKind::Drag => self.held_button,
            tuinix::MouseInputKind::ScrollUp | tuinix::MouseInputKind::ScrollDown => None,
        };
        vec![Action::SendMouseToFocused { index, mouse, held }]
    }

    /// Records what the focused sheet's child wants from the mouse.
    ///
    /// Turning the ask *off* also forgets any held button: the press that armed
    /// it was the child's, and a child that has stopped asking cannot be waiting
    /// for its release. Forgetting it here is what stops a later drag to the same
    /// child -- after it asks again -- from claiming a button it never saw.
    ///
    /// Nothing about the layout moves, so no action is returned: what the ask
    /// changes is where a *report* goes, which the next report answers for
    /// itself.
    fn handle_mouse_wanted(&mut self, wanted: bool) -> Vec<Action> {
        self.mouse_wanted = wanted;
        if !wanted {
            self.held_button = None;
        }
        Vec::new()
    }

    /// Returns whether the focused sheet's child is asking for the mouse.
    ///
    /// An empty stack has no child to ask, so it answers `false`. This is what
    /// the edge compares against the session's own ask to decide whether the
    /// focus has just moved to or away from a sheet that wants the mouse, so a
    /// sheet that starts a pager records the ask and the edge can act on it.
    pub fn mouse_wanted(&self) -> bool {
        self.mouse_wanted
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
            if crate::input::is_prefix(self.prefix_char, key) {
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
        if crate::input::is_prefix(self.prefix_char, key) {
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
            Command::RotateOrientation => self.handle_rotate_orientation(),
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
        let mut actions = vec![Action::SpawnSession {
            index: STACK_TOP,
            program: Program::Shell,
        }];
        actions.extend(self.resize_actions());
        actions
    }

    /// Closes the focused sheet and asks the edge to close its session.
    ///
    /// Does nothing when the stack is empty. The remaining sheets re-derive
    /// their regions, so closing one grows the rest.
    ///
    /// The session is named *before* the sheet is removed: removing first would
    /// leave an index that names whatever sheet shifted into the gap, and the
    /// edge carries the action out after the removal, so it would close the
    /// wrong session. A sheet whose session has not been spawned yet has none to
    /// close, which is why the id is read as an `Option`.
    fn handle_close_sheet(&mut self) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            return Vec::new();
        };
        let session = self
            .stack
            .sheets()
            .get(index)
            .and_then(|(_, sheet)| sheet.session);
        self.stack.remove(index);
        // The next sheet to show is the one that took the closed sheet's place,
        // or the new bottom when the closed one was last.
        self.focused = successor(self.stack.sheets(), index);
        let mut actions = Vec::new();
        if let Some(session) = session {
            actions.push(Action::CloseSession { session });
        }
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

    /// Opens the focused sheet's scrollback in an editor, in a sheet of its own.
    ///
    /// This is `prefix l`, and it is the one command that adds a sheet *and*
    /// names a program for it. Two things decide that shape:
    ///
    /// - The editor is a child like any other, so it lives in a session, and a
    ///   session lives in a sheet. Running it there rather than over the whole
    ///   terminal is what keeps the multiplexer in control: the driver is never
    ///   dropped, the frames of the other sheets keep their meaning, and the
    ///   editor's exit closes its sheet through the ordinary
    ///   [`SessionExited`](crate::Event::SessionExited) path rather than through
    ///   a hand-over with its own rules.
    /// - The text it opens is the focused sheet's scrollback, which the core
    ///   does not hold. The core names the *sheet* and the edge reads the
    ///   scrollback out of the session behind it: turning cells into characters
    ///   is a `termnix` transcription with no decision of tumu's in it, so it
    ///   belongs with the session, not in the model.
    ///
    /// Nothing is opened when there is no sheet to read: an empty stack is the
    /// one no-op. A sheet whose session has no scrollback to offer still gets an
    /// editor, on an empty file.
    ///
    /// Focus moves to the editor sheet, so closing it (with `prefix x`, or by
    /// quitting the editor) leaves the keyboard on the sheet the editor was
    /// opened *from*.
    fn handle_edit_scrollback(&mut self) -> Vec<Action> {
        let Some(index) = self.focused_index() else {
            return Vec::new();
        };
        self.handle_new_editor_sheet(index)
    }

    /// Adds a sheet whose session is the editor, opened on the scrollback of
    /// the sheet at `from`.
    ///
    /// The sheet the editor is opened *from* is named rather than the text it
    /// holds, because the text is the edge's to read: the action that asks for
    /// the editor carries the sheet, and the edge resolves it to a session the
    /// same way it resolves every other action's sheet.
    ///
    /// The sheet is pushed and focused exactly as [`handle_new_sheet`] pushes
    /// one, so the editor appears where any new sheet appears -- on top, under
    /// the keyboard -- and the sheets under it re-derive their regions. The
    /// spawn is named by [`STACK_TOP`] for the same reason.
    ///
    /// The editor's file is named here -- a bare name, not a path -- so the
    /// whole story is one action: "this sheet, this file". The name is derived
    /// from the sheet's id, which is unique for the life of the run, so two
    /// editor sheets cannot collide; where the file goes is the edge's, which is
    /// the only side that may touch the file system.
    ///
    /// [`handle_new_sheet`]: State::handle_new_sheet
    fn handle_new_editor_sheet(&mut self, from: usize) -> Vec<Action> {
        let Some(session) = self
            .stack
            .sheets()
            .get(from)
            .and_then(|(_, sheet)| sheet.session)
        else {
            return Vec::new();
        };
        let id = self.stack.push_top();
        self.focused = Some(id);
        let name = format!("{}-{id}.txt", crate::input::SCROLLBACK_FILE_PREFIX);
        vec![
            Action::OpenEditor {
                index: STACK_TOP,
                session,
            },
            Action::SpawnSession {
                index: STACK_TOP,
                program: Program::Editor { name },
            },
        ]
        .into_iter()
        .chain(self.resize_actions())
        .collect()
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

    /// Stacks the sheets the other way and resizes every sheet to match.
    ///
    /// It is *layout*, not a view: every region takes a different shape, so the
    /// sessions are resized to it. It replaces the automatic flip a device
    /// rotation used to cause, so the axis changes only when it is asked to; a
    /// [`Resize`](Event::Resize) re-lays the stack out along the axis it already
    /// has.
    ///
    /// The toggle is its own inverse, so there is no second state to keep in
    /// step and nothing to remember.
    fn handle_rotate_orientation(&mut self) -> Vec<Action> {
        self.orientation = self.orientation.flipped();
        self.resize_actions()
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
    /// content region.
    ///
    /// The size is the sheet's *content* region, not the sheet's region: the
    /// border line is not the child's, so the grid is sized to the rows or
    /// columns the content actually has (see
    /// [`content_region_of`](State::content_region_of)). That is the same size
    /// the edge spawns a session at, so a later resize agrees with the spawn
    /// rather than shrinking the child by a line the spawn did not take.
    ///
    /// A region too small to give the child a non-zero grid is skipped, the
    /// same way a resize that produces one is.
    fn resize_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        for index in 0..self.stack.len() {
            let Some(region) = self.content_region_of(index) else {
                continue;
            };
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
    /// they are the same pure function of the size, the order, the ratio, and
    /// the orientation, so clearing the flag brings them straight back.
    ///
    /// The returned vector is always as long as the stack; a sheet the full
    /// screen hides has an empty region at its index rather than no entry, so an
    /// index still names the same sheet whether or not it is on show.
    pub fn regions(&self) -> Vec<tuinix::Region> {
        if !self.full_screen {
            return self.stack.regions(self.size, self.orientation);
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

    /// Returns the *content* region of the sheet at `index`: the part of its
    /// region the child's grid covers, border excluded.
    ///
    /// A bordered sheet -- every sheet but the one shown full screen -- loses
    /// one line to the border along the stack's axis, so the content is a
    /// [`sheet_content_size`](crate::sheet_content_size) inside the region. The
    /// full-screen sheet draws **no** border: it fills the whole region, so its
    /// content is the region itself. That one exception is why this lives on the
    /// state -- full screen is the state's fact, and the two sides that size a
    /// child ([`resize_actions`](State::resize_actions) here and the edge's
    /// spawn) have to agree with the renderer, which is the third reader.
    ///
    /// Returns `None` when there is no such sheet. A sheet the full screen
    /// hides has an empty region, so an empty content region.
    pub fn content_region_of(&self, index: usize) -> Option<tuinix::Region> {
        let region = self.region_of(index)?;
        if self.full_screen && Some(index) == self.focused_index() {
            return Some(region);
        }
        Some(tuinix::Region {
            position: tuinix::Position {
                row: region.position.row + crate::sheet_content_origin(self.orientation).row,
                col: region.position.col + crate::sheet_content_origin(self.orientation).col,
            },
            size: crate::sheet_content_size(region.size, self.orientation),
        })
    }

    /// Returns whether the sheet at `index` is drawn without a border.
    ///
    /// Only the full-screen sheet is: it is shown as if it were the whole
    /// terminal, so it has no neighbour to be closed off from and no line to
    /// spend on one. The renderer asks this so the sheet's content fills the
    /// whole region rather than the rows above a border.
    pub fn sheet_is_bordered(&self, index: usize) -> bool {
        !(self.full_screen && Some(index) == self.focused_index())
    }

    /// Records the new size and lays every sheet out again.
    ///
    /// A new sheet is spawned when the stack was empty, so startup and the
    /// first resize share one path. Every sheet's session is resized to the
    /// region the new size gives it, so a rotation or a window change reaches
    /// the children. The *axis* is not touched: a resize re-lays the stack out
    /// along the orientation it already has, which is why rotating the device
    /// no longer rearranges the sheets -- `prefix r` does that, and only when
    /// pressed.
    fn handle_resize(&mut self, size: tuinix::Size) -> Vec<Action> {
        self.size = size;
        let mut actions = Vec::new();

        if self.stack.is_empty() {
            let id = self.stack.push_top();
            self.focused = Some(id);
            actions.push(Action::SpawnSession {
                index: STACK_TOP,
                program: Program::Shell,
            });
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
        // No `CloseSession` is returned: the child is already gone -- this event
        // is how the edge reports that -- so there is no session left to close.
        // The sheet is all that had to go.
        let mut actions = Vec::new();
        actions.extend(self.resize_actions());
        actions
    }
}
