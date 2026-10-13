//! An end-to-end harness that drives the real `tumu` binary behind a PTY.
//!
//! The binary is launched as a child of a [`termnix::Session`], which owns the
//! PTY and an I/O-free terminal emulator. Resizes go through
//! [`termnix::Session::resize`], and the rendered grid is read back from the
//! emulator. Nothing here touches `tumu`'s own code: the tests exercise the same
//! bytes a real terminal would send and assert on the same grid a real terminal
//! would show.

#![allow(dead_code)]

use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

/// How long a `wait_until` predicate may stay unsatisfied before the test
/// fails. Generous because a fresh binary may need to be paged in.
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Drives one `tumu` child process behind a PTY.
///
/// The harness owns the [`termnix::Session`]; dropping it closes the session
/// and reaps the child.
pub struct TumuHarness {
    session: termnix::Session,
    rows: u16,
    cols: u16,
}

impl TumuHarness {
    /// Launches the `tumu` binary at an 80x24 terminal.
    pub fn open() -> Self {
        Self::with_size(24, 80)
    }

    /// Launches the `tumu` binary at a terminal of `rows`x`cols`.
    pub fn with_size(rows: u16, cols: u16) -> Self {
        Self::spawn(rows, cols, |_| {})
    }

    /// Launches the `tumu` binary with `$SHELL` pinned to `shell`.
    ///
    /// A sheet runs `$SHELL -l` by default, so pointing `$SHELL` at a script
    /// makes the sheet's output something a test can assert on, instead of
    /// whatever the user's login shell prints. A test that wants the flag
    /// instead uses [`with_args`](TumuHarness::with_args) and `--shell`.
    pub fn with_shell(rows: u16, cols: u16, shell: &str) -> Self {
        let shell = shell.to_string();
        Self::spawn(rows, cols, move |command| {
            command.env("SHELL", shell);
        })
    }

    /// Launches the `tumu` binary with `$SHELL` and `$TUMU_EDITOR` pinned.
    ///
    /// The sheet runs `$SHELL -l` and `prefix l` runs `$TUMU_EDITOR --tail`
    /// with the scrollback on its standard input, so pointing the two at
    /// scripts makes both a sheet's output and the scrollback handoff something
    /// a test can assert on.
    pub fn with_shell_and_editor(rows: u16, cols: u16, shell: &str, editor: &str) -> Self {
        let shell = shell.to_string();
        let editor = editor.to_string();
        Self::spawn(rows, cols, move |command| {
            command.env("SHELL", shell);
            command.env("TUMU_EDITOR", editor);
        })
    }

    /// Launches the `tumu` binary with `--orientation` set.
    ///
    /// The axis is a setting now, so a test that cares which way the sheets
    /// stack has to ask for it; leaving it out is the vertical default.
    pub fn with_orientation(rows: u16, cols: u16, orientation: &str) -> Self {
        let orientation = orientation.to_string();
        Self::spawn(rows, cols, move |command| {
            command.arg("--orientation").arg(orientation);
        })
    }

    /// Launches the `tumu` binary with `$SHELL` pinned and `--orientation` set.
    pub fn with_shell_and_orientation(
        rows: u16,
        cols: u16,
        shell: &str,
        orientation: &str,
    ) -> Self {
        let shell = shell.to_string();
        let orientation = orientation.to_string();
        Self::spawn(rows, cols, move |command| {
            command.env("SHELL", shell);
            command.arg("--orientation").arg(orientation);
        })
    }

    /// Launches the `tumu` binary, letting `setup` adjust the command first.
    ///
    /// This is the escape hatch for the tests that need an argument or an
    /// environment variable `tumu` does not otherwise take from a caller: the
    /// setup closure sees the raw [`Command`], so it can add either before the
    /// child starts.
    pub fn with_args<F>(rows: u16, cols: u16, setup: F) -> Self
    where
        F: FnOnce(&mut Command),
    {
        Self::spawn(rows, cols, setup)
    }

    /// Launches the binary, letting `setup` adjust the command first.
    fn spawn<F>(rows: u16, cols: u16, setup: F) -> Self
    where
        F: FnOnce(&mut Command),
    {
        let mut command = Command::new(tumu_binary());
        setup(&mut command);
        let session = termnix::Session::new(&mut command, size(rows, cols))
            .expect("failed to start the tumu binary behind a PTY");
        Self {
            session,
            rows,
            cols,
        }
    }

    /// Resizes the terminal, delivering SIGWINCH to `tumu`.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.session
            .resize(size(rows, cols))
            .expect("failed to resize the PTY");
        self.rows = rows;
        self.cols = cols;
    }

    /// Sends one key chord to `tumu`, as a real terminal would.
    ///
    /// The key is enqueued through `termnix`, so it is encoded with the modes
    /// `tumu` has set -- the same path a real keypress takes.
    pub fn send_key(&mut self, key: termnix::KeyEvent) {
        self.session
            .enqueue_input(termnix::Input::Key(key))
            .expect("failed to enqueue a key");
    }

    /// Sends a mouse report to `tumu`, as a real terminal would.
    ///
    /// The report is written as the raw SGR bytes a terminal sends, rather than
    /// through `termnix::Input::Mouse`: what is under test is `tumu`'s decode of
    /// a real report, so the harness must produce the bytes, not go through the
    /// same library `tumu` uses to encode them. SGR is what an xterm-compatible
    /// terminal sends once reporting mode `1006` is on, which `tumu` asks for.
    ///
    /// `kind` is the SGR button/motion code -- `0` for a left press, the value
    /// a terminal puts before the coordinates.
    pub fn send_mouse_code(&mut self, code: u16, row: u16, col: u16) {
        // SGR coordinates are one-based.
        let bytes = format!("\x1b[<{code};{};{}M", col + 1, row + 1);
        self.write_bytes(bytes.as_bytes());
    }

    /// Sends a left-button press at (`row`, `col`), zero-based.
    pub fn click(&mut self, row: u16, col: u16) {
        self.send_mouse_code(0, row, col);
    }

    /// Writes raw bytes to the command's terminal, as the host keyboard would.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.session
            .enqueue_input(termnix::Input::Raw(bytes))
            .expect("failed to enqueue raw input");
    }

    /// Sends the prefix chord (`C-q`).
    pub fn send_prefix(&mut self) {
        self.send_prefix_char('q');
    }

    /// Sends the prefix chord for character `c` (`Ctrl` plus `c`).
    ///
    /// The prefix is a setting, so a test that launches tumu with `--prefix`
    /// sends that character; the default `send_prefix` is this with `q`.
    pub fn send_prefix_char(&mut self, c: char) {
        self.send_key(termnix::KeyEvent {
            code: termnix::KeyCode::Char(c),
            modifiers: termnix::Modifiers::new().ctrl(),
        });
    }

    /// Sends a bare character key.
    pub fn send_char(&mut self, ch: char) {
        self.send_key(termnix::KeyEvent::new(termnix::KeyCode::Char(ch)));
    }

    /// Reads the visible grid back as one trimmed string per row.
    pub fn screen_rows(&self) -> Vec<String> {
        let state = self.session.terminal_state();
        let mut rows = Vec::new();
        for line in state.scrollback_lines() {
            rows.push(line_text(line.cells()));
        }
        for row in 0..self.rows {
            let mut cells = Vec::with_capacity(self.cols as usize);
            for col in 0..self.cols {
                let cell = state
                    .cell(termnix::Position { row, col })
                    .expect("cell within the emulator grid");
                cells.push(cell);
            }
            rows.push(line_text(&cells));
        }
        rows
    }

    /// Whether any visible row contains `needle`.
    pub fn screen_contains(&self, needle: &str) -> bool {
        self.screen_rows().iter().any(|r| r.contains(needle))
    }

    /// Reads one column of the screen back as a string, top to bottom.
    ///
    /// A horizontal stack writes each sheet's border *down* its right column, so
    /// the border text is not on any one row: this is how a test reads it.
    pub fn screen_column(&self, col: u16) -> String {
        let state = self.session.terminal_state();
        let mut column = String::new();
        for row in 0..self.rows {
            let cell = state
                .cell(termnix::Position { row, col })
                .expect("cell within the emulator grid");
            if cell.width == 0 {
                continue;
            }
            column.push(cell.ch);
        }
        column.trim_end().to_string()
    }

    /// Whether any column of the screen contains `needle`.
    pub fn screen_column_contains(&self, needle: &str) -> bool {
        (0..self.cols).any(|col| self.screen_column(col).contains(needle))
    }

    /// Waits until a column of the screen shows `needle`.
    pub fn wait_for_column_text(&mut self, needle: &str) {
        self.wait_until(needle, |h| h.screen_column_contains(needle));
    }

    /// The whole screen with rows joined by newlines.
    pub fn screen_text(&self) -> String {
        self.screen_rows().join("\n")
    }

    /// The whole screen with rows joined by nothing, so text wrapped across a
    /// row boundary reads as one run.
    ///
    /// A box narrows its text to its interior width, so a long message is split
    /// at the wrap point rather than at a word: a needle that straddles the wrap
    /// is on no single row. Joining the rows with no separator puts it back, so a
    /// test can match a message without having to know where the box happened to
    /// fold it.
    pub fn screen_joined(&self) -> String {
        self.screen_rows().join("")
    }

    /// Whether the screen, rows joined with no separator, contains `needle`.
    pub fn screen_joined_contains(&self, needle: &str) -> bool {
        self.screen_joined().contains(needle)
    }

    /// Pumps the session until `pred` holds, or panics after `WAIT_TIMEOUT`.
    ///
    /// `pred` receives the harness so it can read the grid; it must not mutate
    /// the harness, which keeps every assertion a pure read.
    pub fn wait_until<F>(&mut self, what: &str, pred: F)
    where
        F: Fn(&TumuHarness) -> bool,
    {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        loop {
            self.pump();
            if pred(self) {
                return;
            }
            if Instant::now() >= deadline {
                panic!(
                    "timed out waiting for: {what}\nscreen:\n{}",
                    self.screen_text()
                );
            }
            self.poll_once(deadline);
        }
    }

    /// Waits until the screen shows `needle`, printing the grid on failure.
    pub fn wait_for_text(&mut self, needle: &str) {
        self.wait_until(needle, |h| h.screen_contains(needle));
    }

    /// Pumps the session until the child exits, or panics after `WAIT_TIMEOUT`.
    pub fn wait_for_exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        loop {
            self.pump();
            if let Some(status) = self.session.exit_status()
                && (self.session.status() == termnix::SessionStatus::Eof
                    || (self.session.fd().is_none() && !self.session.needs_pump()))
            {
                return status;
            }
            if let Err(err) = self.session.try_wait() {
                panic!("try_wait failed: {err}");
            }
            if Instant::now() >= deadline {
                panic!(
                    "timed out waiting for tumu to exit\nscreen:\n{}",
                    self.screen_text()
                );
            }
            self.poll_once(deadline);
        }
    }

    /// Advances the session by one bounded pump.
    fn pump(&mut self) {
        self.session
            .pump_io(termnix::PumpBudget::default())
            .expect("pump_io failed");
        let mut budget = 64;
        while self.session.needs_pump() && budget > 0 {
            self.session
                .pump_io(termnix::PumpBudget::default())
                .expect("pump_io failed");
            budget -= 1;
        }
    }

    /// Blocks until the child has something to say, or the deadline passes.
    fn poll_once(&mut self, deadline: Instant) {
        if self.session.needs_pump() {
            return;
        }
        let Some(fd) = self.session.fd() else {
            return;
        };
        let interests = self.session.interests();
        let mut events = 0;
        if interests.readable {
            events |= libc::POLLIN;
        }
        if interests.writable {
            events |= libc::POLLOUT;
        }
        if events == 0 {
            return;
        }
        let timeout = poll_timeout_ms(deadline);
        let mut pollfd = libc::pollfd {
            fd: fd as libc::c_int,
            events,
            revents: 0,
        };
        // SAFETY: `poll` receives one fully initialized `pollfd` and a count of
        // one, and only writes to `revents` within that array.
        #[expect(
            unsafe_code,
            reason = "the e2e harness waits for the child with libc::poll"
        )]
        let rc = unsafe { libc::poll(&mut pollfd, 1, timeout) };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                panic!("poll failed: {err}");
            }
        }
    }
}

impl Drop for TumuHarness {
    fn drop(&mut self) {
        self.session.close();
    }
}

/// Renders one row of cells as a string, dropping wide-character padding.
fn line_text(cells: &[termnix::Cell]) -> String {
    let mut line = String::new();
    for cell in cells {
        if cell.width == 0 {
            continue;
        }
        line.push(cell.ch);
    }
    line.trim_end().to_string()
}

/// A non-zero [`termnix::Size`] from plain `u16` dimensions.
fn size(rows: u16, cols: u16) -> termnix::Size {
    termnix::Size {
        rows: std::num::NonZeroU16::new(rows).expect("rows must be non-zero"),
        cols: std::num::NonZeroU16::new(cols).expect("cols must be non-zero"),
    }
}

/// Milliseconds until `deadline`, clamped to a finite non-negative value.
fn poll_timeout_ms(deadline: Instant) -> libc::c_int {
    let now = Instant::now();
    if now >= deadline {
        return 0;
    }
    let ms = deadline.duration_since(now).as_millis();
    libc::c_int::try_from(ms).unwrap_or(libc::c_int::MAX).max(0)
}

/// The path to the `tumu` binary cargo built for this test run.
fn tumu_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tumu"))
}
