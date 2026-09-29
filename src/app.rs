//! The I/O edge: the poll loop, raw terminal, PTYs, and frame diffing.
//!
//! This module is compiled into the binary only (`mod app;` in `main.rs`); the
//! library stays Sans I/O. It owns the [`tuinix::TerminalDriver`] and the
//! [`termnix::Session`]s, and it is the one place that spawns children, reads
//! input from, and paints frames to the terminal. The [`tumu`] core it drives
//! takes plain values and returns plain values, and never calls back into here.
//!
//! The PoC has no key handling yet: it takes over the terminal, spawns one
//! window, re-lays the stack out on resize, and repaints. That is enough to
//! prove the model against a real terminal.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

/// How long to wait for the rest of an escape sequence before a lone `ESC` byte
/// is treated as the Escape key.
const ESCAPE_TIMEOUT_MS: libc::c_int = 50;

/// The program and arguments to run in a window, rebuilt per spawn.
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
}

impl App {
    /// Takes over the terminal and prepares to run `shell` in every window.
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
        };
        // Mouse reporting is a convenience, not a requirement: a terminal that
        // refuses it still works.
        app.driver.enable_mouse_reporting()?;
        // The startup size arrives as a resize, so the first window is spawned
        // by the same path a later rotation takes.
        let size = app.driver.size();
        app.report_resize(size)?;
        Ok(app)
    }

    /// Runs the poll loop until no session is left.
    pub fn run(&mut self) -> std::io::Result<()> {
        while !self.sessions.is_empty() {
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
            // The PoC does not act on keys yet; draining keeps the decoder from
            // holding an uncommitted escape forever.
            while self.decoder.next().is_some() {}
        }
        Ok(())
    }

    /// Reports a display size to the core and carries out what it returns.
    fn report_resize(&mut self, size: tuinix::Size) -> std::io::Result<()> {
        let actions = self.state.update(tumu::Event::Resize(size));
        for action in actions {
            match action {
                tumu::Action::SpawnSession { index } => self.spawn_session(index)?,
                tumu::Action::ResizeSession { index, size } => {
                    self.resize_session(index, size);
                }
                tumu::Action::CloseSession { index } => self.close_session(index),
                tumu::Action::Exit => {}
            }
        }
        self.dirty = true;
        Ok(())
    }

    /// Spawns a session for the window at `index` and attaches it.
    ///
    /// The PTY is sized to the window's *content* region, the rows below the
    /// title, so the title row never costs the child a row it cannot use.
    fn spawn_session(&mut self, index: usize) -> std::io::Result<()> {
        let Some(region) = self.state.stack.region_of(index, self.state.size) else {
            return Ok(());
        };
        let content = tumu::window_content_region(region);
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

    /// Resizes the session hosted by the window at `index`.
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

    /// Closes the session hosted by the window at `index`.
    fn close_session(&mut self, index: usize) {
        if let Some(id) = self.session_at(index) {
            self.sessions.remove(&id);
        }
        self.dirty = true;
    }

    /// Returns the session id attached to the window at `index`.
    fn session_at(&self, index: usize) -> Option<tumu::TermnixSessionId> {
        self.state
            .stack
            .windows()
            .get(index)
            .and_then(|(_, window)| window.session)
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
