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
//! latched key runs a command. Mouse, paste, and undecodable input are drained
//! but not yet acted on.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

/// How long to wait for the rest of an escape sequence before a lone `ESC` byte
/// is treated as the Escape key.
const ESCAPE_TIMEOUT_MS: libc::c_int = 50;

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
    driver: tuinix::TerminalDriver,
    decoder: tuinix::InputDecoder,
    previous: Option<tuinix::Frame>,
    shell: ShellCommand,
    state: tumu::State,
    sessions: HashMap<tumu::TermnixSessionId, termnix::Session>,
    next_session_id: std::num::NonZeroU64,
    dirty: bool,
    exit: bool,
}

impl App {
    /// Takes over the terminal and prepares to run `shell` in every sheet.
    pub fn new(state: tumu::State, shell: ShellCommand) -> std::io::Result<Self> {
        let driver = tuinix::TerminalDriver::new()?;
        let mut app = Self {
            driver,
            decoder: tuinix::InputDecoder::new(),
            previous: None,
            shell,
            state,
            sessions: HashMap::new(),
            next_session_id: std::num::NonZeroU64::MIN,
            dirty: true,
            exit: false,
        };
        // Mouse reporting is a convenience, not a requirement: a terminal that
        // refuses it still works.
        app.driver.enable_mouse_reporting()?;
        // The startup size arrives as a resize, so the first sheet is spawned
        // by the same path a later rotation takes.
        let size = app.driver.size();
        app.report_resize(size)?;
        Ok(app)
    }

    /// Runs the poll loop until the core asks to leave or no session is left.
    ///
    /// A `quit` command sets [`App::exit`]; the loop also stops on its own when
    /// the last session goes, so a stack the user emptied still returns.
    pub fn run(&mut self) -> std::io::Result<()> {
        while !self.exit && !self.sessions.is_empty() {
            if self.dirty {
                self.render()?;
            }
            self.poll_once()?;
        }
        Ok(())
    }

    /// Blocks once for readiness, then handles what became ready.
    fn poll_once(&mut self) -> std::io::Result<()> {
        let mut fds = vec![
            libc::pollfd {
                fd: self.driver.resize_signal_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.driver.input_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let timeout = if self.decoder.has_uncommitted_escape() {
            ESCAPE_TIMEOUT_MS
        } else {
            -1
        };

        // SAFETY: `poll` receives a fully initialized `pollfd` array and its
        // length, and only writes within that array.
        #[expect(
            unsafe_code,
            reason = "libc::poll is the one call the poll loop cannot avoid"
        )]
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(err);
        }
        if ready == 0 {
            self.decoder.commit_escape();
            return Ok(());
        }

        if fds[0].revents & libc::POLLIN != 0 {
            self.driver.handle_resize_signal()?;
            let size = self.driver.size();
            self.report_resize(size)?;
        }
        if fds[1].revents & libc::POLLIN != 0 {
            let mut raw = [0u8; 1024];
            if let Ok(n) = self.driver.read(&mut raw) {
                self.decoder.feed(&raw[..n]);
            }
            while let Some(input) = self.decoder.next() {
                self.handle_input(input)?;
            }
        }
        Ok(())
    }

    /// Turns one decoded input into core actions and carries them out.
    ///
    /// Only keys reach the core for now: a mouse report, a paste, and an
    /// undecodable sequence are all dropped where they arrive. Forwarding a
    /// paste and a mouse event is a later change; until then the decoder must
    /// still be drained, or a lone escape would never commit.
    fn handle_input(&mut self, input: tuinix::Input) -> std::io::Result<()> {
        if let tuinix::Input::Key(key) = input {
            let actions = self.state.update(tumu::Event::Key(key));
            self.dispatch(actions)?;
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
                tumu::Action::SendToFocused { key } => self.send_to_focused(key),
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
        let Some(region) = self.state.stack.region_of(index, self.state.size) else {
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

    /// Sends `key` to the focused sheet's session.
    ///
    /// The focused sheet is the top of the stack; a separate focus field is a
    /// later addition. `termnix` turns the logical key into PTY bytes using the
    /// child's own terminal modes, which is why the key is converted here rather
    /// than as raw bytes.
    fn send_to_focused(&mut self, key: tuinix::KeyInput) {
        let Some(id) = self.session_at(0) else {
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

    /// Paints the chrome and writes only what changed to the terminal.
    fn render(&mut self) -> std::io::Result<()> {
        let size = self.driver.size();
        let frame = tumu::frame(&self.state);
        let out = frame.render(self.previous.as_ref(), None);
        self.driver.write_all(&out)?;
        self.driver.flush()?;
        self.previous = Some(frame);
        self.dirty = false;
        let _ = size;
        Ok(())
    }
}
