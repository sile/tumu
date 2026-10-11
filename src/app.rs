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
//! The edge also owns the clipboard. A child's OSC 52 copy arrives as
//! [`termnix::Event::RequestReceived`] with a `SetClipboard`; the edge forwards
//! the text to the host terminal and remembers it, and `prefix y` sends the
//! remembered bytes back to a sheet as [`termnix::Input::Paste`].
//!
//! The edge also owns the scrollback bound. `termnix` bounds no session's
//! history on its own, so the edge trims each session to a
//! [`tumu::ScrollbackLimit`] (from the binary's arguments) whenever a history
//! line is appended. `prefix l` is what makes the bound matter: the scrollback
//! it hands to an editor is only as large as the limit allows.
//!
//! `prefix l` opens the focused sheet's scrollback in an editor, and it does so
//! by *adding a sheet*: the core assembles the text (it owns the model) and
//! returns an [`OpenEditor`](tumu::Action::OpenEditor) naming the sheet, and the
//! edge writes the text to a file and spawns the editor as that sheet's session.
//! Nothing is handed over, so the driver is never dropped and the editor is
//! painted like any other child. The one file the edge owns is unlinked when the
//! editor exits, which [`reap_sessions`](App::reap_sessions) does.
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

/// The program an editor sheet runs.
///
/// This mirrors [`ShellCommand`]: the recipe is a program and its fixed flags,
/// not a live [`Command`], so a fresh one is built for each launch. The file to
/// open is not part of the recipe -- it is made fresh per sheet -- so it is a
/// parameter rather than a field.
#[derive(Debug, Clone)]
pub struct EditorCommand {
    program: PathBuf,
}

impl EditorCommand {
    /// Builds an editor recipe from a program.
    pub fn new(program: PathBuf) -> Self {
        Self { program }
    }

    /// Builds a fresh [`Command`] that opens `path`.
    ///
    /// `--tail` asks the editor to open at the end of the file, so the newest
    /// scrollback is what it shows first: the text is a history, and its last
    /// line is where the reader left off. The path is passed positionally, after
    /// the flag, because a trailing argument would otherwise be read as the file
    /// the flag is about.
    pub fn command(&self, path: &Path) -> Command {
        let mut command = Command::new(&self.program);
        command.arg("--tail");
        command.arg(path);
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
    /// The terminal driver.
    ///
    /// Nothing hands the terminal over any more: an editor is a sheet's child
    /// like any other, so the driver lives as long as the app does and raw mode
    /// is entered once, at startup.
    driver: tuinix::TerminalDriver,
    decoder: tuinix::InputDecoder,
    previous: Option<tuinix::Frame>,
    shell: ShellCommand,
    editor: EditorCommand,
    /// Where the files an editor sheet opens are written.
    ///
    /// The core names a bare file name (see
    /// [`SCROLLBACK_FILE_PREFIX`](tumu::SCROLLBACK_FILE_PREFIX)); this is the
    /// directory the edge puts it in. Only the edge touches the file system, so
    /// only the edge knows where the directory is.
    editor_dir: PathBuf,
    /// The file each editor session was opened on, so it can be removed when the
    /// editor exits.
    ///
    /// The key is the *session*, not the sheet: by the time the child is reaped
    /// the sheet that hosted it is being closed, so the sheet is no longer a
    /// handle on the file. The map is the edge's half of the editor's lifetime.
    editor_files: HashMap<tumu::TermnixSessionId, PathBuf>,
    state: tumu::State,
    sessions: HashMap<tumu::TermnixSessionId, termnix::Session>,
    next_session_id: std::num::NonZeroU64,
    /// The last text a sheet copied with OSC 52, kept so `prefix y` has
    /// something to paste back.
    ///
    /// The host terminal owns the real clipboard, and OSC 52 has no read that
    /// reliably answers, so the edge mirrors what it forwarded to the host here.
    /// It is the last copy, not a merged history: a paste puts back what was
    /// most recently copied. It is empty until a child asks to copy, which is
    /// when a paste has nothing to do.
    clipboard: Option<Vec<u8>>,
    /// How much scrollback each session may retain.
    ///
    /// `termnix` bounds nothing on its own, so this is where the bound is
    /// applied: a session that appended a history line is trimmed to it. It
    /// comes from the binary's arguments and never changes while tumu runs.
    scrollback: tumu::ScrollbackLimit,
    dirty: bool,
    exit: bool,
}

impl App {
    /// Takes over the terminal and prepares to run `shell` in every sheet.
    ///
    /// `scrollback` is how much history each session is allowed to keep; it is
    /// the bound the edge applies as sessions run, since `termnix` applies
    /// none itself.
    ///
    /// `editor_dir` is where an editor sheet's file is written. It is made up
    /// front, before any sheet can ask for an editor, so the write cannot fail
    /// on a missing directory.
    pub fn new(
        state: tumu::State,
        shell: ShellCommand,
        editor: EditorCommand,
        scrollback: tumu::ScrollbackLimit,
        editor_dir: PathBuf,
    ) -> std::io::Result<Self> {
        std::fs::create_dir_all(&editor_dir)?;
        let driver = take_terminal()?;
        let mut app = Self {
            driver,
            decoder: tuinix::InputDecoder::new(),
            previous: None,
            shell,
            editor,
            editor_dir,
            editor_files: HashMap::new(),
            state,
            sessions: HashMap::new(),
            next_session_id: std::num::NonZeroU64::MIN,
            clipboard: None,
            scrollback,
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

    /// The terminal driver.
    fn driver(&self) -> &tuinix::TerminalDriver {
        &self.driver
    }

    /// The terminal driver, mutably.
    fn driver_mut(&mut self) -> &mut tuinix::TerminalDriver {
        &mut self.driver
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
        // Copied out before any session is borrowed: the bound is applied while
        // a session is borrowed mutably out of `self.sessions`, and reading it
        // from `self` there would be a second borrow of the same value.
        let limit = self.scrollback;
        for id in ids {
            let Some(session) = self.sessions.get_mut(&id) else {
                continue;
            };
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
            // The pump queues what the child did; draining the queue is how
            // the frame learns whether it has to repaint. This is the event
            // side of the edge: `ScreenUpdated` and `TerminalReset` both mean
            // the visible grid may have moved, which is what dirties a sheet.
            // `TitleUpdated` is the one other event the frame reacts to: the
            // title is shown in the sheet's title row, so it is read off the
            // session and handed to the core. A clipboard write
            // (`RequestReceived` with `SetClipboard`) is acted on here too:
            // the text is forwarded to the host clipboard and remembered for
            // `prefix y`. The remaining events (a retained line, a hyperlink,
            // the other requests) do not change what this frame shows, so they
            // are dropped for now.
            //
            // The title is copied out while the session is borrowed and the
            // borrow is released before `state.update` runs: the core has no
            // access to `self.sessions`, and the borrow of `session` would
            // otherwise cover the call.
            //
            // A clipboard write is handled the same way for a different reason:
            // carrying it out touches `self.driver` and `self.clipboard`, both
            // of which would clash with the borrow of `session`. The last write
            // in this drain wins, since each one replaces the clipboard rather
            // than appending to it.
            let mut title: Option<String> = None;
            let mut clipboard: Option<Vec<u8>> = None;
            // A history line was appended, so the session now holds more
            // scrollback than the bound allows and has to be trimmed. The
            // trim is a no-op while under the bound, so it is only worth a
            // call when an append actually happened; every other event leaves
            // the retained history untouched.
            let mut trim = false;
            while let Some(event) = session.dequeue_event() {
                match event {
                    termnix::Event::ScreenUpdated | termnix::Event::TerminalReset => {
                        self.dirty = true;
                    }
                    // `termnix` bounds no session's scrollback on its own: the
                    // history grows until the caller trims it. An append is the
                    // one event that moves the session over the bound, so it is
                    // the one that asks for the trim. Whether the line is still
                    // over the bound decides nothing here -- the trim checks
                    // both limits and removes nothing when they hold.
                    termnix::Event::ScrollbackLineAppended => {
                        trim = true;
                    }
                    // Several title changes since the last drain merge into
                    // one event, so the current title is read rather than a
                    // payload carried on the event.
                    termnix::Event::TitleUpdated => {
                        title = Some(session.terminal_state().title().to_owned());
                    }
                    // OSC 52: the child asked the terminal to change a
                    // selection. termnix owns no clipboard, so it is the edge
                    // that forwards the text to the host terminal and keeps a
                    // copy for `prefix y`. `append` and `selection` are not
                    // acted on -- this PoC mirrors the last write into the one
                    // host clipboard -- so only the payload is taken.
                    termnix::Event::RequestReceived(termnix::ChildRequest::SetClipboard {
                        text,
                        ..
                    }) => {
                        clipboard = Some(text);
                    }
                    _ => {}
                }
            }
            // The bound is applied once the queue is drained, so a single trim
            // covers every line that arrived in the drain, however many events
            // merged into the run. It runs here, before the borrows of `self`
            // below, because it is the last use of `session`: the title and
            // clipboard handling that follows read from `self`, which the
            // mutable borrow of `self.sessions` would otherwise reject.
            if trim {
                session.trim_scrollback(limit.lines(), limit.cells());
            }
            if let Some(text) = clipboard {
                self.set_clipboard(&text);
            }
            if let Some(title) = title
                && let Some(index) = self.index_for_session(id)
            {
                let actions = self.update_state(tumu::Event::TitleChanged { index, title });
                self.dispatch(actions)?;
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
            // The editor's temp file, if this session was one, goes with the
            // session: nothing else knows the path, and the file is only ever
            // read by the child that just exited. A failed removal is reported
            // and swallowed, since the sheet still has to close.
            if let Some(path) = self.editor_files.remove(&id)
                && let Err(err) = std::fs::remove_file(&path)
            {
                eprintln!("tumu: editor file {}: {err}", path.display());
            }
            if let Some(sheet_id) = self.sheet_for_session(id) {
                let actions = self.update_state(tumu::Event::SessionExited(sheet_id));
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

    /// Returns the stack index of the sheet hosting `session`, if any.
    ///
    /// A title is delivered by index, like every other core event, and the
    /// index is derived from the live sessions here: the core holds the sheet
    /// that names the session, not the session itself.
    fn index_for_session(&self, session: tumu::TermnixSessionId) -> Option<usize> {
        self.state
            .stack
            .sheets()
            .iter()
            .position(|(_, sheet)| sheet.session == Some(session))
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
                let actions = self.update_state(tumu::Event::Key(key));
                self.dispatch(actions)?;
            }
            tuinix::Input::Mouse(mouse) => {
                let actions = self.update_state(tumu::Event::Mouse(mouse));
                self.dispatch(actions)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Reports a display size to the core and carries out what it returns.
    fn report_resize(&mut self, size: tuinix::Size) -> std::io::Result<()> {
        let actions = self.update_state(tumu::Event::Resize(size));
        self.dispatch(actions)
    }

    /// Feeds one event to the core and returns the actions it answered with.
    ///
    /// The core takes nothing but the event: everything an action needs beyond
    /// the model -- a session's scrollback, its grid -- is read by the edge,
    /// which is the side that holds the sessions. So there is no second
    /// argument to thread through, and no borrow of the app to split.
    fn update_state(&mut self, event: tumu::Event) -> Vec<tumu::Action> {
        self.state.update(event)
    }

    /// Carries out a batch of core actions.
    ///
    /// Every action that changes what a sheet shows marks the frame dirty, so
    /// one pass over a batch repaints once rather than per action.
    fn dispatch(&mut self, actions: Vec<tumu::Action>) -> std::io::Result<()> {
        for action in actions {
            match action {
                tumu::Action::SpawnSession { index, program } => {
                    self.spawn_session(index, program)?;
                }
                tumu::Action::ResizeSession { index, size } => {
                    self.resize_session(index, size);
                }
                tumu::Action::CloseSession { session } => self.close_session(session),
                tumu::Action::SendToFocused { index, key } => self.send_to_focused(index, key),
                tumu::Action::OpenEditor { index, session } => self.open_editor(index, session),
                tumu::Action::PasteClipboard { index } => self.paste_clipboard(index),
                tumu::Action::PrefixPending => {}
                tumu::Action::Exit => self.exit = true,
            }
        }
        self.dirty = true;
        Ok(())
    }

    /// Spawns a session for the sheet at `index` and attaches it.
    ///
    /// The PTY is sized to the sheet's *content* region, the rows before the
    /// border, so the border line never costs the child a row it cannot use.
    ///
    /// What runs is the action's [`Program`](tumu::Program): the shell, or the
    /// editor on a file the core named (and which
    /// [`open_editor`](App::open_editor) has just written). The two share the
    /// whole of this: a session is a session, and which child it is makes no
    /// difference to how it is sized, attached, or painted.
    fn spawn_session(&mut self, index: usize, program: tumu::Program) -> std::io::Result<()> {
        let Some(region) = self.state.region_of(index) else {
            return Ok(());
        };
        let orientation = self.state.orientation();
        let Some(size) =
            tumu::terminal_size_to_cell_size(tumu::sheet_content_size(region.size, orientation))
        else {
            return Ok(());
        };
        // The editor's file is named by the core and placed by this edge: the
        // one join that turns the name into the path the child is handed, and
        // the path the file is removed by later.
        let mut editor_path = None;
        let mut command = match &program {
            tumu::Program::Shell => self.shell.command(),
            tumu::Program::Editor { name } => {
                let path = self.editor_dir.join(name);
                editor_path = Some(path.clone());
                self.editor.command(&path)
            }
        };
        let session = termnix::Session::new(&mut command, size)?;
        let id = tumu::TermnixSessionId::new(self.next_session_id);
        self.next_session_id = std::num::NonZeroU64::new(self.next_session_id.get() + 1)
            .expect("an id past the last used is non-zero");
        if let Some(path) = editor_path {
            // Remembered by session, because the sheet that names it is gone by
            // the time the editor exits and the file has to be removed.
            self.editor_files.insert(id, path);
        }
        self.sessions.insert(id, session);
        self.state.stack.attach_session(index, id);
        self.dirty = true;
        Ok(())
    }

    /// Writes the scrollback of `session` to the file an editor sheet opens.
    ///
    /// The core named a bare file name (see [`tumu::SCROLLBACK_FILE_PREFIX`])
    /// and the edge puts it in [`editor_dir`](App::editor_dir); the two halves
    /// are joined here, which is why the path is not carried on the action. The
    /// write is what makes the file exist before the spawn that reads it, and
    /// the file is removed when the editor exits (see
    /// [`reap_sessions`](App::reap_sessions)).
    ///
    /// Reading the text out of the session is the edge's because only the edge
    /// holds it (see [`scrollback_text`](App::scrollback_text)); the core named
    /// the session and nothing more. A session that is gone writes an empty
    /// file, which reads as "no history" rather than as a refusal.
    ///
    /// A failure to write is reported and swallowed: the editor is still
    /// spawned, on a file that does not exist, which an editor reports in its
    /// own way. That is better than dropping the sheet, which would leave the
    /// layout one sheet out of step with what the user asked for.
    fn open_editor(&mut self, index: usize, session: tumu::TermnixSessionId) {
        let Some(name) = self.editor_file_name(index) else {
            return;
        };
        let text = self.scrollback_text(session).unwrap_or_default();
        if let Err(err) = std::fs::write(self.editor_dir.join(&name), text) {
            eprintln!("tumu: editor file {name}: {err}");
        }
    }

    /// Returns the file name the editor of the sheet at `index` is opened on.
    ///
    /// The name is derived from the sheet's id, which is what makes it unique
    /// for the life of the run: two editor sheets cannot collide, and the name
    /// survives the sheet being reordered. The bare name is the core's half; the
    /// directory is this edge's, added by the caller that touches the disk.
    fn editor_file_name(&self, index: usize) -> Option<String> {
        let (id, _) = self.state.stack.sheets().get(index)?;
        Some(format!("{}-{id}.txt", tumu::SCROLLBACK_FILE_PREFIX))
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

    /// Forwards a copied text to the host terminal's clipboard and remembers it.
    ///
    /// The bytes came from the child's OSC 52 and are not required to be valid
    /// UTF-8, but the host write takes a `&str`; a copy is expected to be text,
    /// so a non-UTF-8 payload is lossily decoded rather than dropped. The host
    /// write is best-effort -- OSC 52 has no reply -- so a terminal that ignores
    /// it, or has none behind it, is indistinguishable from one that accepts it;
    /// the failure is swallowed. The remembered copy is the raw bytes, so
    /// `prefix y` pastes back exactly what the child asked to copy, whatever the
    /// host made of it.
    fn set_clipboard(&mut self, text: &[u8]) {
        let lossy = String::from_utf8_lossy(text);
        // The driver is present except while an editor owns the terminal, and an
        // editor is only launched from a key the loop is not in the middle of
        // draining, so the driver is here to take the write.
        let _ = self.driver_mut().set_clipboard(&lossy);
        self.clipboard = Some(text.to_vec());
    }

    /// Pastes the remembered clipboard into the session of the sheet at `index`.
    ///
    /// The bytes are enqueued as [`termnix::Input::Paste`], so `termnix` wraps
    /// them in the bracketed-paste markers exactly when the child asked for
    /// them. Nothing is pasted when no child has copied yet: there is no
    /// clipboard to put back. A sheet with no session has nowhere to receive the
    /// paste, so it is left alone.
    fn paste_clipboard(&mut self, index: usize) {
        let Some(text) = self.clipboard.clone() else {
            return;
        };
        let Some(id) = self.session_at(index) else {
            return;
        };
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        if session.enqueue_input(termnix::Input::Paste(&text)).is_ok() {
            self.dirty = true;
        }
        // Flush the paste now, for the same reason a forwarded key is: the
        // bytes were pulled out of the input buffer by the key that named this
        // action, and no later read's readiness would push them out.
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

    /// Closes the session with this id.
    ///
    /// The session is named by id, not by the index of a sheet: the core has
    /// already removed the sheet from the stack, so an index would name whatever
    /// sheet shifted into the gap.
    fn close_session(&mut self, session: tumu::TermnixSessionId) {
        self.sessions.remove(&session);
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
    /// The core draws the whole screen -- each visible sheet is a frame of its
    /// own, composed into the screen -- and the grids it draws under each border
    /// come from the sessions, which the core does not hold. So the edge walks
    /// the stack once and collects one grid per sheet (see
    /// [`sheet_grids`](App::sheet_grids)), in the order the core will ask for
    /// them, and hands the collection over.
    fn render(&mut self) -> std::io::Result<()> {
        let grids = self.sheet_grids();
        let frame = tumu::frame(&self.state, &grids);
        let cursor = self.focused_cursor();
        let out = frame.render(self.previous.as_ref(), cursor);
        self.driver_mut().write_all(&out)?;
        self.driver_mut().flush()?;
        self.previous = Some(frame);
        self.dirty = false;
        Ok(())
    }

    /// Returns the terminal cursor for the focused sheet, if its session
    /// reports one and it is inside the sheet's content region.
    ///
    /// The cursor is the session's own position moved into the sheet's content
    /// frame, which sits one row below the sheet's origin for the title, and
    /// then into the screen at the sheet's region. A hidden cursor (`?25l`), or
    /// one outside the content region, is not shown: the content region is what
    /// the session owns, and drawing a cursor outside it would land on the title
    /// or another sheet. The `+ 1` is that title row.
    fn focused_cursor(&self) -> Option<tuinix::Position> {
        let index = self.state.focused_index()?;
        let region = self.state.region_of(index)?;
        let orientation = self.state.orientation();
        let content_size = tumu::sheet_content_size(region.size, orientation);
        let content_origin = tumu::sheet_content_origin(orientation);
        let session_id = self.session_at(index)?;
        let session = self.sessions.get(&session_id)?;
        let terminal = session.terminal_state();
        if !terminal.modes().cursor_visible {
            return None;
        }
        let cursor = terminal.cursor();
        if usize::from(cursor.row) >= content_size.rows
            || usize::from(cursor.col) >= content_size.cols
        {
            return None;
        }
        Some(tuinix::Position {
            row: region.position.row + content_origin.row + usize::from(cursor.row),
            col: region.position.col + content_origin.col + usize::from(cursor.col),
        })
    }

    /// Returns the grid the session shows, ready to be painted.
    ///
    /// A session that is not in the table has no grid: it was just closed, and
    /// the sheet that hosted it is being reaped.
    ///
    /// The rows are collected into a `Vec`, because
    /// [`Session`](termnix::Session) exposes its rows as an *iterator* of
    /// borrowed slices rather than a slice. The cells are still borrowed, so the
    /// grid is cheap and the session's memory is never copied; the `Vec` is
    /// rebuilt per frame, which is fine at this size.
    fn grid_of(&self, id: tumu::TermnixSessionId) -> Option<tumu::SheetGrid<'_>> {
        let session = self.sessions.get(&id)?;
        let terminal = session.terminal_state();
        Some(tumu::SheetGrid {
            rows: terminal.rows().collect(),
            cursor: terminal.cursor(),
            cursor_visible: terminal.modes().cursor_visible,
        })
    }

    /// Collects what every sheet shows, in stack order.
    ///
    /// One entry per sheet, so the position in the collection is the sheet and
    /// the core never has to look a sheet's session up to know which grid is
    /// its. A sheet with no session -- one whose child has not been spawned yet,
    /// or has exited -- contributes `None`, which draws its border alone.
    fn sheet_grids(&self) -> tumu::SheetGrids<'_> {
        let mut grids = tumu::SheetGrids::new();
        for (_, sheet) in self.state.stack.sheets() {
            grids.push(sheet.session.and_then(|id| self.grid_of(id)));
        }
        grids
    }

    /// Returns the text of the sheet's scrollback: history first, then the rows
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
    /// This is what `prefix l` writes out. It lives on the *edge* rather than in
    /// the core because reading it needs the session, which only the edge holds;
    /// the core names the session and never sees a `termnix` type for it.
    ///
    /// Returns `None` when the session is not one this app holds, so there is
    /// nothing to open.
    fn scrollback_text(&self, id: tumu::TermnixSessionId) -> Option<String> {
        let session = self.sessions.get(&id)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a cell with a glyph, its display width, and default style.
    fn cell(ch: char, width: u8) -> termnix::Cell {
        termnix::Cell {
            ch,
            width,
            ..termnix::Cell::EMPTY
        }
    }

    #[test]
    fn row_text_skips_the_trailing_half_of_a_wide_glyph() {
        // A wide glyph occupies two cells: the glyph cell and a width-0 cell
        // that stands for its right half. Only the glyph cell contributes, so
        // the row reads as one character, not two.
        let cells = [cell('世', 2), cell('\0', 0)];
        assert_eq!(row_text(&cells), "世");
    }

    #[test]
    fn row_text_trims_trailing_blanks() {
        // A row padded to the terminal's width reads as the line it holds: the
        // trailing spaces a full-width row carries are not part of the text.
        let cells = [cell('h', 1), cell('i', 1), cell(' ', 1), cell(' ', 1)];
        assert_eq!(row_text(&cells), "hi");
    }

    #[test]
    fn row_text_keeps_interior_blanks() {
        // Only *trailing* blanks are trimmed: a gap between words is content,
        // and turning it into no gap at all would join the words.
        let cells = [cell('a', 1), cell(' ', 1), cell('b', 1)];
        assert_eq!(row_text(&cells), "a b");
    }

    #[test]
    fn row_text_of_all_blanks_is_empty() {
        let cells = [cell(' ', 1), cell(' ', 1)];
        assert_eq!(row_text(&cells), "");
    }
}
