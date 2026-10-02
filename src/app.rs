//! The I/O edge: the poll loop, raw terminal, PTYs, and frame diffing.
//!
//! This module is compiled into the binary only (`mod app;` in `main.rs`); the
//! library stays Sans I/O. It owns the [`tuinix::TerminalDriver`] and the
//! [`termnix::Session`]s, and it is the one place that spawns children, reads
//! input from, and paints frames to the terminal. The [`tumu`] core it drives
//! takes plain values and returns plain values, and never calls back into here.
//!
//! Keys reach the core as [`Event::Key`](tumu::Event::Key): a key the prefix
//! latch does not claim is forwarded to the focused sheet's session, and a
//! latched key runs a command. Mouse reports reach it as
//! [`Event::Mouse`](tumu::Event::Mouse), which is how a click moves focus. Paste
//! and undecodable input are drained but not yet acted on.
//!
//! One command leaves the loop for a moment: `prefix l` hands the focused
//! sheet's scrollback to an external editor. That is the one place the edge
//! gives the terminal up and takes it back, so the handover -- dropping the
//! driver, running the child, building a fresh driver -- lives in
//! [`App::edit_scrollback`] and nowhere else.
//!
//! The loop is edge-triggered, following `termnix`'s contract: every session's
//! pending work is drained with [`Session::needs_pump()`](termnix::Session::needs_pump)
//! *before* blocking in `poll`, and the poll set is rebuilt from
//! [`Session::interests()`](termnix::Session::interests) each round, because
//! those interests change as the write queue drains.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// How long to wait for the rest of an escape sequence before a lone `ESC` byte
/// is treated as the Escape key.
const ESCAPE_TIMEOUT_MS: libc::c_int = 50;

/// Identifies one entry in the poll set, so a readiness result can be mapped
/// back to what became ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Interest {
    /// The terminal-input fd is readable.
    Input,
    /// The resize-signal fd is readable.
    Resize,
    /// The PTY of the session with this id is readable or writable.
    Session(tumu::TermnixSessionId),
}

/// The program and arguments to run in a sheet, rebuilt per spawn.
#[derive(Debug, Clone)]
pub struct ShellCommand {
    program: PathBuf,
    args: Vec<OsString>,
}

impl ShellCommand {
    /// Builds a recipe from a program and its arguments.
    pub fn new(program: PathBuf, args: Vec<OsString>) -> Self {
        Self { program, args }
    }

    /// Builds a fresh [`Command`] from the recipe.
    ///
    /// The recipe is not a live command: `termnix` registers child setup on the
    /// command it is handed, and a command cannot be spawned twice, so every
    /// spawn gets a new one.
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command
    }
}

/// The program the scrollback is handed to.
///
/// This mirrors [`ShellCommand`]: the recipe is a program, not a live
/// [`Command`], so a fresh one is built for each launch. The editor is treated
/// as opaque -- tumu writes the scrollback to a temporary file and passes its
/// path as the only argument, and does not care how the editor reads it -- so
/// the recipe fixes just the program.
#[derive(Debug, Clone)]
pub struct EditorCommand {
    program: PathBuf,
}

impl EditorCommand {
    /// Builds an editor recipe from a program.
    pub fn new(program: PathBuf) -> Self {
        Self { program }
    }

    /// Builds a fresh [`Command`] that opens `file` as its file.
    ///
    /// The command is left with no standard streams set: the caller lets the
    /// terminal streams be inherited, which is the handover the editor needs.
    /// The path is passed as a plain argument, so the editor opens it the same
    /// way it would open any file the user named on the command line.
    pub fn command(&self, file: &Path) -> Command {
        let mut command = Command::new(&self.program);
        command.arg(file);
        command
    }
}

/// Converts a decoded `tuinix` key into the `termnix` event it stands for.
///
/// The two crates name the same keys in their own types, so this is the one
/// place the mapping lives. A key that `termnix` cannot name -- the keypad
/// codes `tuinix` does not produce, or a function key past `F(12)` -- is sent
/// as an escape-free fallback: `BackTab` becomes `Tab` with Shift, and anything
/// unmatched becomes the bare character when there is one, or nothing at all.
/// Alt and Ctrl ride along as modifiers, which `termnix` re-encodes.
fn termnix_key(key: tuinix::KeyInput) -> termnix::KeyEvent {
    use tuinix::KeyCode;

    let mut modifiers = termnix::Modifiers::new();
    if key.ctrl {
        modifiers = modifiers.ctrl();
    }
    if key.alt {
        modifiers = modifiers.alt();
    }

    let code = match key.code {
        KeyCode::Char(ch) => termnix::KeyCode::Char(ch),
        KeyCode::Enter => termnix::KeyCode::Enter,
        KeyCode::Tab => termnix::KeyCode::Tab,
        KeyCode::BackTab => {
            modifiers = modifiers.shift();
            termnix::KeyCode::Tab
        }
        KeyCode::Backspace => termnix::KeyCode::Backspace,
        KeyCode::Escape => termnix::KeyCode::Escape,
        KeyCode::Up => termnix::KeyCode::Up,
        KeyCode::Down => termnix::KeyCode::Down,
        KeyCode::Left => termnix::KeyCode::Left,
        KeyCode::Right => termnix::KeyCode::Right,
        KeyCode::Home => termnix::KeyCode::Home,
        KeyCode::End => termnix::KeyCode::End,
        KeyCode::PageUp => termnix::KeyCode::PageUp,
        KeyCode::PageDown => termnix::KeyCode::PageDown,
        KeyCode::Insert => termnix::KeyCode::Insert,
        KeyCode::Delete => termnix::KeyCode::Delete,
        KeyCode::F(n) => termnix::KeyCode::Function(n),
    };

    termnix::KeyEvent { code, modifiers }
}

/// Owns the terminal edge and drives the core until every session is gone.
pub struct App {
    /// The terminal driver, or `None` while an external editor owns the
    /// terminal.
    ///
    /// It is an `Option` because the driver *is* the terminal takeover: to hand
    /// the terminal to an editor, tumu has to drop it (which restores raw mode,
    /// the alternate screen, and the cursor) and build a fresh one when the
    /// editor returns. While it is `None` the poll loop still watches the
    /// sessions, but nothing paints.
    driver: Option<tuinix::TerminalDriver>,
    decoder: tuinix::InputDecoder,
    previous: Option<tuinix::Frame>,
    shell: ShellCommand,
    editor: EditorCommand,
    state: tumu::State,
    sessions: HashMap<tumu::TermnixSessionId, termnix::Session>,
    next_session_id: std::num::NonZeroU64,
    dirty: bool,
    exit: bool,
}

impl App {
    /// Takes over the terminal and prepares to run `shell` in every sheet.
    pub fn new(
        state: tumu::State,
        shell: ShellCommand,
        editor: EditorCommand,
    ) -> std::io::Result<Self> {
        let driver = take_terminal()?;
        let mut app = Self {
            driver: Some(driver),
            decoder: tuinix::InputDecoder::new(),
            previous: None,
            shell,
            editor,
            state,
            sessions: HashMap::new(),
            next_session_id: std::num::NonZeroU64::MIN,
            dirty: true,
            exit: false,
        };
        app.driver_mut().enable_mouse_reporting()?;
        // The startup size arrives as a resize, so the first sheet is spawned
        // by the same path a later rotation takes.
        let size = app.driver().size();
        app.report_resize(size)?;
        Ok(app)
    }

    /// The terminal driver, which is present except while an editor owns the
    /// terminal.
    fn driver(&self) -> &tuinix::TerminalDriver {
        self.driver
            .as_ref()
            .expect("the terminal is held except while an editor owns it")
    }

    /// The terminal driver, mutably.
    fn driver_mut(&mut self) -> &mut tuinix::TerminalDriver {
        self.driver
            .as_mut()
            .expect("the terminal is held except while an editor owns it")
    }

    /// Runs the poll loop until the core asks to leave or no session is left.
    ///
    /// A `quit` command sets [`App::exit`]; the loop also stops on its own when
    /// the last session goes, so a stack the user emptied still returns.
    pub fn run(&mut self) -> std::io::Result<()> {
        while !self.exit && !self.sessions.is_empty() {
            self.drain_sessions()?;
            if self.dirty {
                self.render()?;
            }
            self.poll_once()?;
        }
        Ok(())
    }

    /// Drains every session's pending work without blocking.
    ///
    /// This must run before `poll` blocks: a session with internal work that
    /// produces no readiness edge would otherwise leave the loop waiting while
    /// there is something to do. A session that became ready is pumped here
    /// too, and it has to be pumped *unconditionally*: `needs_pump` is false
    /// right after a read hit `WouldBlock`, so gating the first pump on it
    /// would skip the read entirely and leave the fd readable, spinning `poll`
    /// at full speed. The unconditional pump clears that flag and reads what
    /// arrived; the `while needs_pump` drain then finishes any internal work
    /// left over.
    fn drain_sessions(&mut self) -> std::io::Result<()> {
        let ids: Vec<tumu::TermnixSessionId> = self.sessions.keys().copied().collect();
        for id in ids {
            let Some(session) = self.sessions.get_mut(&id) else {
                continue;
            };
            // The terminal's revision bumps at most once per `feed`, so it is a
            // cheap way to tell whether a pump changed what a sheet shows.
            // Sampling it around the drain dirties the frame exactly when the
            // visible grid moved, and not on an idle pump.
            let before = session.terminal_state().revision();
            loop {
                match session.pump_io(termnix::PumpBudget::default()) {
                    Ok(()) => {}
                    // The child is gone; stop draining this session and let
                    // `reap_sessions` observe the exit and close the sheet.
                    Err(err) if is_child_gone(&err) => break,
                    Err(err) => return Err(err),
                }
                if !session.needs_pump() {
                    break;
                }
            }
            if session.terminal_state().revision() != before {
                self.dirty = true;
            }
        }
        self.reap_sessions()
    }

    /// Drops sessions whose child has exited and closes their sheets.
    ///
    /// The child's exit is observed with [`Session::try_wait()`], which reaps
    /// it without blocking. A session is dropped as soon as the child is reaped,
    /// so closing the shell closes the sheet and, when it was the last one, ends
    /// the loop. The session's last output is not chased before the drop: the
    /// sheet is going away, so whatever was still in the PTY would be painted
    /// and immediately thrown away by the next frame.
    fn reap_sessions(&mut self) -> std::io::Result<()> {
        let ids: Vec<tumu::TermnixSessionId> = self.sessions.keys().copied().collect();
        for id in ids {
            let exited = match self.sessions.get_mut(&id) {
                Some(session) => session.try_wait()?.is_some(),
                None => continue,
            };
            if !exited {
                continue;
            }
            self.sessions.remove(&id);
            if let Some(sheet_id) = self.sheet_for_session(id) {
                let actions = self.state.update(tumu::Event::SessionExited(sheet_id));
                self.dispatch(actions)?;
            }
            self.dirty = true;
        }
        Ok(())
    }

    /// Returns the id of the sheet hosting `session`, if any.
    fn sheet_for_session(&self, session: tumu::TermnixSessionId) -> Option<tumu::SheetId> {
        self.state
            .stack
            .sheets()
            .iter()
            .find(|(_, sheet)| sheet.session == Some(session))
            .map(|(id, _)| *id)
    }

    /// Blocks once for readiness, then handles what became ready.
    ///
    /// The poll set is rebuilt from live interests each round: a session that
    /// has queued output asks for `POLLOUT`, and one that is spent asks for
    /// nothing at all. A lone `ESC` shortens the wait to [`ESCAPE_TIMEOUT_MS`]
    /// so Escape is not held until the next key.
    fn poll_once(&mut self) -> std::io::Result<()> {
        let mut fds = vec![self.input_poll_fd(), self.resize_poll_fd()];
        let mut interests = vec![Interest::Input, Interest::Resize];
        let ids: Vec<tumu::TermnixSessionId> = self.sessions.keys().copied().collect();
        for id in ids {
            let Some(session) = self.sessions.get(&id) else {
                continue;
            };
            let Some(fd) = session.fd() else {
                continue;
            };
            let wanted = session.interests();
            let mut events = 0;
            if wanted.readable {
                events |= libc::POLLIN;
            }
            if wanted.writable {
                events |= libc::POLLOUT;
            }
            if events == 0 {
                continue;
            }
            fds.push(libc::pollfd {
                fd,
                events,
                revents: 0,
            });
            interests.push(Interest::Session(id));
        }

        let timeout = if self.decoder.has_uncommitted_escape() {
            ESCAPE_TIMEOUT_MS
        } else {
            -1
        };
        let ready = poll_fds(&mut fds, timeout);
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            // `poll` is never restarted by `SA_RESTART`, so a SIGWINCH makes it
            // return `EINTR`. The handler already wrote to the signal pipe, so
            // retrying reports the resize as readable.
            if err.kind() == std::io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(err);
        }

        // `ready == 0` means the wait elapsed with no data: the lone `ESC` is
        // the Escape key, not the start of a sequence. Committing it here and
        // re-reading is the only place a clock affects the core, and it reaches
        // the core only as an ordinary key event.
        if ready == 0 && self.decoder.has_uncommitted_escape() {
            self.decoder.commit_escape();
            self.drain_decoder()?;
            return Ok(());
        }

        for (index, pfd) in fds.iter().enumerate() {
            if pfd.revents == 0 {
                continue;
            }
            // A hangup with nothing to read is the child going away. Mark the
            // frame dirty and let `reap_sessions` on the next round close the
            // sheet; there is no byte left to consume.
            if pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
                && pfd.revents & libc::POLLIN == 0
            {
                self.dirty = true;
                continue;
            }
            match interests.get(index) {
                Some(Interest::Input) => self.read_input()?,
                Some(Interest::Resize) => self.handle_resize()?,
                // A ready session is pumped at the top of the next round by
                // `drain_sessions`, which dirties the frame when the pump
                // changes what a sheet shows. Nothing is read here, so the
                // readiness edge is left for that pump to consume.
                Some(Interest::Session(_)) => {}
                None => {}
            }
        }
        Ok(())
    }

    /// Reads whatever terminal input is available and feeds the decoder.
    fn read_input(&mut self) -> std::io::Result<()> {
        let mut raw = [0u8; 1024];
        loop {
            match self.driver_mut().read(&mut raw) {
                Ok(0) => break,
                Ok(n) => {
                    self.decoder.feed(&raw[..n]);
                    self.drain_decoder()?;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    /// Pulls every complete input out of the decoder and feeds the core.
    fn drain_decoder(&mut self) -> std::io::Result<()> {
        while let Some(input) = self.decoder.next() {
            self.handle_input(input)?;
        }
        Ok(())
    }

    /// Applies a terminal resize: refreshes the cached size and tells the core.
    fn handle_resize(&mut self) -> std::io::Result<()> {
        self.driver_mut().handle_resize_signal()?;
        let size = self.driver().size();
        self.report_resize(size)
    }

    /// The poll fd for terminal input, tagged as [`Interest::Input`].
    fn input_poll_fd(&self) -> libc::pollfd {
        libc::pollfd {
            fd: self.driver().input_fd(),
            events: libc::POLLIN,
            revents: 0,
        }
    }

    /// The poll fd for resize signals, tagged as [`Interest::Resize`].
    fn resize_poll_fd(&self) -> libc::pollfd {
        libc::pollfd {
            fd: self.driver().resize_signal_fd(),
            events: libc::POLLIN,
            revents: 0,
        }
    }

    /// Turns one decoded input into core actions and carries them out.
    ///
    /// Keys and mouse reports reach the core; a paste and an undecodable
    /// sequence are dropped where they arrive. Forwarding a paste is a later
    /// change; until then the decoder must still be drained, or a lone escape
    /// would never commit.
    fn handle_input(&mut self, input: tuinix::Input) -> std::io::Result<()> {
        match input {
            tuinix::Input::Key(key) => {
                let actions = self.state.update(tumu::Event::Key(key));
                self.dispatch(actions)?;
            }
            tuinix::Input::Mouse(mouse) => {
                let actions = self.state.update(tumu::Event::Mouse(mouse));
                self.dispatch(actions)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Reports a display size to the core and carries out what it returns.
    fn report_resize(&mut self, size: tuinix::Size) -> std::io::Result<()> {
        let actions = self.state.update(tumu::Event::Resize(size));
        self.dispatch(actions)
    }

    /// Carries out a batch of core actions.
    ///
    /// Every action that changes what a sheet shows marks the frame dirty, so
    /// one pass over a batch repaints once rather than per action.
    fn dispatch(&mut self, actions: Vec<tumu::Action>) -> std::io::Result<()> {
        for action in actions {
            match action {
                tumu::Action::SpawnSession { index } => self.spawn_session(index)?,
                tumu::Action::ResizeSession { index, size } => {
                    self.resize_session(index, size);
                }
                tumu::Action::CloseSession { index } => self.close_session(index),
                tumu::Action::SendToFocused { index, key } => self.send_to_focused(index, key),
                tumu::Action::EditScrollback { index } => self.edit_scrollback(index)?,
                tumu::Action::PrefixPending => {}
                tumu::Action::Exit => self.exit = true,
            }
        }
        self.dirty = true;
        Ok(())
    }

    /// Spawns a session for the sheet at `index` and attaches it.
    ///
    /// The PTY is sized to the sheet's *content* region, the rows below the
    /// title, so the title row never costs the child a row it cannot use.
    fn spawn_session(&mut self, index: usize) -> std::io::Result<()> {
        let Some(region) = self.state.region_of(index) else {
            return Ok(());
        };
        let content = tumu::sheet_content_region(region);
        let Some(size) = tumu::terminal_size_to_cell_size(content.size) else {
            return Ok(());
        };
        let mut command = self.shell.command();
        let session = termnix::Session::new(&mut command, size)?;
        let id = tumu::TermnixSessionId::new(self.next_session_id);
        self.next_session_id = std::num::NonZeroU64::new(self.next_session_id.get() + 1)
            .expect("an id past the last used is non-zero");
        self.sessions.insert(id, session);
        self.state.stack.attach_session(index, id);
        self.dirty = true;
        Ok(())
    }

    /// Sends `key` to the session of the sheet at `index`.
    ///
    /// The core resolved focus to this index before the action was returned, so
    /// the edge just looks the sheet up. `termnix` turns the logical key into
    /// PTY bytes using the child's own terminal modes, which is why the key is
    /// converted here rather than as raw bytes.
    fn send_to_focused(&mut self, index: usize, key: tuinix::KeyInput) {
        let Some(id) = self.session_at(index) else {
            return;
        };
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        if session
            .enqueue_input(termnix::Input::Key(termnix_key(key)))
            .is_ok()
        {
            self.dirty = true;
        }
        // Push the byte out now rather than leaving it queued. The keys in this
        // batch were already pulled out of the input buffer, so there is no
        // later read whose readiness would flush them; without this the child
        // would sit on the byte until something else woke the loop.
        let _ = session.pump_io(termnix::PumpBudget::default());
    }

    /// Resizes the session hosted by the sheet at `index`.
    fn resize_session(&mut self, index: usize, size: termnix::Size) {
        let Some(id) = self.session_at(index) else {
            return;
        };
        if let Some(session) = self.sessions.get_mut(&id)
            && session.resize(size).is_ok()
        {
            self.dirty = true;
        }
    }

    /// Closes the session hosted by the sheet at `index`.
    fn close_session(&mut self, index: usize) {
        if let Some(id) = self.session_at(index) {
            self.sessions.remove(&id);
        }
        self.dirty = true;
    }

    /// Returns the session id attached to the sheet at `index`.
    fn session_at(&self, index: usize) -> Option<tumu::TermnixSessionId> {
        self.state
            .stack
            .sheets()
            .get(index)
            .and_then(|(_, sheet)| sheet.session)
    }

    /// Renders the state and writes only what changed to the terminal.
    ///
    /// The core lays down the chrome -- the title row of every visible sheet --
    /// and then each sheet's session grid is painted into its *content* region,
    /// the rows below the title. The two do not overlap, so the order between
    /// them does not matter; painting the contents after the chrome keeps a grid
    /// out of the chrome's way. The grid paint is edge work because it needs the
    /// sessions, which the core does not hold.
    fn render(&mut self) -> std::io::Result<()> {
        let mut frame = tumu::frame(&self.state);
        self.paint_sheet_contents(&mut frame);
        let cursor = self.focused_cursor();
        let out = frame.render(self.previous.as_ref(), cursor);
        self.driver_mut().write_all(&out)?;
        self.driver_mut().flush()?;
        self.previous = Some(frame);
        self.dirty = false;
        Ok(())
    }

    /// Paints each visible sheet's session grid into the frame.
    ///
    /// A sheet with no session, or one the full screen hides, is left blank: a
    /// hidden sheet's region is empty, so the grid loop writes nothing. The grid
    /// goes into the sheet's content region, below its title row, so it never
    /// covers the title. The grid rows are borrowed from the session for the
    /// duration of the call and copied into the frame, so the session is free to
    /// move on afterwards.
    fn paint_sheet_contents(&self, frame: &mut tuinix::Frame) {
        for (index, region) in self.state.regions().into_iter().enumerate() {
            if region.size.rows == 0 || region.size.cols == 0 {
                continue;
            }
            let Some(session_id) = self.session_at(index) else {
                continue;
            };
            let Some(session) = self.sessions.get(&session_id) else {
                continue;
            };
            let terminal = session.terminal_state();
            let rows: Vec<&[termnix::Cell]> = terminal.rows().collect();
            let grid = tumu::SheetGrid {
                rows: &rows,
                cursor: terminal.cursor(),
                cursor_visible: terminal.modes().cursor_visible,
            };
            tumu::draw_sheet_grid(frame, tumu::sheet_content_region(region), &grid);
        }
    }

    /// The terminal cursor for the focused sheet, if its session reports one.
    ///
    /// The cursor is the session's own position translated into the sheet's
    /// *content* region, one row below the sheet's origin for the title. A
    /// hidden cursor (`?25l`), or one outside the content region, is not shown:
    /// the content region is what the session owns, and drawing a cursor
    /// outside it would land on the title or another sheet.
    fn focused_cursor(&self) -> Option<tuinix::Position> {
        let index = self.state.focused_index()?;
        let session_id = self.session_at(index)?;
        let session = self.sessions.get(&session_id)?;
        let terminal = session.terminal_state();
        if !terminal.modes().cursor_visible {
            return None;
        }
        let region = self.state.region_of(index)?;
        let content = tumu::sheet_content_region(region);
        let cursor = terminal.cursor();
        if usize::from(cursor.row) >= content.size.rows
            || usize::from(cursor.col) >= content.size.cols
        {
            return None;
        }
        Some(tuinix::Position {
            row: content.position.row + usize::from(cursor.row),
            col: content.position.col + usize::from(cursor.col),
        })
    }

    /// Hands the sheet's scrollback to the external editor.
    ///
    /// This is the copy-mode entry point: the scrollback of the sheet at
    /// `index` -- the history the emulator has pushed off the top, followed by
    /// the rows still on screen -- is turned into text, written to a temporary
    /// file, and handed to the editor as `kk <path>`. The file, not a pipe, is
    /// what the editor opens: the PoC first fed the editor on standard input as
    /// `kk /dev/stdin`, but `kk` treats its file argument as a path it opens and
    /// refuses a non-terminal standard input, so a real file the editor names on
    /// its command line is the shape that works.
    ///
    /// tumu owns the terminal, and the editor needs it, so the terminal is
    /// handed over for the child's lifetime: the [`tuinix::TerminalDriver`] is
    /// dropped (restoring raw mode, the alternate screen, and the cursor),
    /// the child inherits the terminal on its own standard streams, and a fresh
    /// driver is built once the child has exited. The fresh driver starts with
    /// mouse reporting off, so it is re-requested the way startup requests it,
    /// and the previous frame is thrown away, because the child painted over the
    /// screen and a differential repaint against the old frame would skip rows
    /// it thinks are still right. Rebuilding the driver and clearing the frame
    /// is what makes the next paint a full one.
    ///
    /// The temporary file lives only for the child's lifetime: it is removed as
    /// soon as the child is reaped, on every exit path. A crash of tumu itself
    /// could leave one behind, but that is left to the OS -- the file is created
    /// where the platform already collects temporary files -- rather than
    /// tracked and swept, which is more than the PoC needs.
    ///
    /// A sheet with no session is left alone: there is nothing to read, so
    /// there is nothing to open. The index is one of the core's own and always
    /// names a sheet, so it is only the missing session that can stop this.
    fn edit_scrollback(&mut self, index: usize) -> std::io::Result<()> {
        let Some(text) = self.scrollback_text(index) else {
            return Ok(());
        };
        let file = match write_scrollback_file(&text) {
            Ok(file) => file,
            Err(err) => {
                eprintln!("tumu: editor: {err}");
                return Ok(());
            }
        };

        // Give the terminal back before the child starts, and take it again
        // after: the two brackets are the whole of the handover.
        self.driver = None;
        let status = self.run_editor(&file);
        self.driver = Some(take_terminal()?);
        // A fresh driver starts with mouse reporting off, so it is re-requested
        // here exactly as at startup. It is a convenience, not a requirement,
        // so a refusal is swallowed rather than fatal.
        let _ = self.driver_mut().enable_mouse_reporting();
        // The child painted over the screen, so the old frame is not a valid
        // baseline for a differential repaint: dropping it forces a full one.
        self.previous = None;
        self.dirty = true;

        // The child is gone, so the file has served its purpose. Removing it
        // here -- after the wait, on the one path that launched a child -- is
        // the whole of the cleanup; the scrollback is not written back, so
        // nothing else reads it.
        let _ = std::fs::remove_file(&file);

        // The editor's exit code is not tumu's to act on -- the scrollback is
        // not written back -- so a failure to launch or wait is reported and
        // swallowed. The file's own state is unchanged either way, and the user
        // is back in tumu with the stack intact.
        if let Err(err) = status {
            eprintln!("tumu: editor: {err}");
        }
        Ok(())
    }

    /// Runs the external editor on the file at `file`.
    ///
    /// The child inherits tumu's standard streams -- the terminal, since the
    /// driver was dropped first -- so it can paint its own UI, and the path is
    /// passed as an ordinary argument, so it opens the file the way it would
    /// open any file named on its command line.
    fn run_editor(&self, file: &Path) -> std::io::Result<std::process::ExitStatus> {
        self.editor.command(file).status()
    }

    /// Builds the text of the sheet's scrollback: history first, then the rows
    /// still on screen.
    ///
    /// The two halves come from the same emulator but are laid out
    /// differently: a retained [`termnix::ScrollbackLine`] keeps the width it
    /// had when it was scrolled off, while the visible rows are reflowed to the
    /// current size. Both are rows of cells, so both join the same way -- cells
    /// left to right, trailing blanks trimmed, rows joined by newlines -- and a
    /// width-0 cell (the trailing half of a wide glyph) contributes nothing of
    /// its own.
    ///
    /// Returns `None` when the sheet has no session to read from, so there is
    /// nothing to open.
    fn scrollback_text(&self, index: usize) -> Option<String> {
        let session_id = self.session_at(index)?;
        let session = self.sessions.get(&session_id)?;
        let terminal = session.terminal_state();

        let mut rows: Vec<String> = Vec::new();
        for line in terminal.scrollback_lines() {
            rows.push(row_text(line.cells()));
        }
        for row in terminal.rows() {
            rows.push(row_text(row));
        }
        Some(rows.join("\n"))
    }
}

/// Writes `text` to a fresh temporary file and returns its path.
///
/// The file is created where the platform keeps temporary files -- `$TMPDIR`
/// when set, `/tmp` otherwise -- under a name that carries the process id and a
/// clock reading, so two launches do not collide. It is opened exclusively, so a
/// name already taken is retried rather than clobbered, and it is returned by
/// path only: the editor reopens it by name, so the handle tumu wrote through is
/// closed here.
fn write_scrollback_file(text: &str) -> std::io::Result<PathBuf> {
    let dir = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.subsec_nanos())
        .unwrap_or(0);
    let path = dir.join(format!("tumu-scrollback-{}-{nanos}", std::process::id()));
    // Create first, then write through it: `create_new` fails if the name is
    // taken, so a collision is surfaced rather than silently sharing a file.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(text.as_bytes())?;
    Ok(path)
}

/// Renders one row of cells as text, trailing blanks trimmed.
///
/// A width-0 cell is the trailing half of a wide glyph and stands for no
/// character of its own, so it is skipped; every other cell contributes its
/// glyph. The result is trimmed of trailing spaces, so a row padded to the
/// terminal's width reads as the line it holds.
fn row_text(cells: &[termnix::Cell]) -> String {
    let mut line = String::new();
    for cell in cells {
        if cell.width == 0 {
            continue;
        }
        line.push(cell.ch);
    }
    line.trim_end().to_string()
}

/// Takes over the terminal with a fresh [`tuinix::TerminalDriver`].
///
/// Only one driver may exist at a time, so this is also the assertion that the
/// previous one was dropped: a driver still alive here is a bug in the handover
/// order, not a condition to recover from.
fn take_terminal() -> std::io::Result<tuinix::TerminalDriver> {
    tuinix::TerminalDriver::new()
}

/// Waits for readiness on any of `fds`, up to `timeout` milliseconds.
///
/// A negative timeout blocks until an fd is ready. The return value is
/// `poll(2)`'s own: the number of ready fds, `0` on timeout, or `-1` on error.
/// Each fd's `revents` is filled in for the caller to inspect.
#[expect(
    unsafe_code,
    reason = "libc::poll is the one call the poll loop cannot avoid"
)]
fn poll_fds(fds: &mut [libc::pollfd], timeout: libc::c_int) -> libc::c_int {
    unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) }
}

/// Returns whether an I/O error is the PTY telling us the child is gone.
///
/// `EIO` is what the kernel reports on the master fd once the last slave fd
/// closes; it is the normal way to learn a child has exited, not a failure.
fn is_child_gone(err: &std::io::Error) -> bool {
    err.raw_os_error() == Some(libc::EIO)
}
