//! End-to-end test for opening the scrollback in an external editor.
//!
//! `prefix l` runs the editor named by `$TUMU_EDITOR` in a **new sheet**, on a
//! file holding the focused sheet's scrollback. The tests point `$TUMU_EDITOR`
//! at a small script that copies the file it was named to a known path, so the
//! text tumu wrote can be read back and asserted on. The editor is a stand-in
//! for `kk`: what is under test is that tumu writes the scrollback to a file and
//! spawns the editor on it, not what the child is.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// Writes `body` to a unique temp script and returns its path.
fn script(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-e2e-{}-{}.sh", std::process::id(), name));
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// A path in the temp dir for one test's editor output.
fn out_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("tumu-e2e-{}-{}.out", std::process::id(), name))
}

/// Builds an editor script that copies the file it is opened on to `out`.
///
/// `prefix l` spawns the editor as `<program> --tail <path>`, so the file to
/// read is the *last* argument. The script takes the last argument and copies
/// it to `out`, which is the stand-in for `kk` opening the file: what the test
/// reads back is the bytes tumu wrote, not what the editor did with them. It
/// then exits, which closes its sheet through the ordinary session-exit path.
fn copy_editor(out: &std::path::Path) -> String {
    format!("eval \"last=\\${{$#}}\"; cp \"$last\" {}", out.display())
}

#[test]
fn the_editor_receives_the_sheet_scrollback_and_closes_on_exit() {
    // The sheet prints a marker and then waits, so its output stays on screen
    // while the scrollback is collected.
    let shell = script("scrollback-shell", "echo SHEET-HELLO; exec sleep 30");
    // The editor copies the file it was opened on to the output file.
    let out = out_path("handoff");
    let editor = script("scrollback-editor", &copy_editor(&out));

    let mut tumu = e2e::TumuHarness::with_shell_and_editor(
        40,
        24,
        shell.to_str().expect("utf-8 path"),
        editor.to_str().expect("utf-8 path"),
    );
    tumu.wait_for_text("SHEET-HELLO");

    tumu.send_prefix();
    tumu.send_char('l');

    // The editor runs and copies the scrollback out. The sheet's marker must be
    // part of it, which proves the session's own output reached the file the
    // editor was opened on.
    tumu.wait_until("the editor received the scrollback", |_| {
        fs::read_to_string(&out)
            .map(|text| text.contains("SHEET-HELLO"))
            .unwrap_or(false)
    });

    // The editor's own exit closes its sheet through the ordinary session-exit
    // path, so the stack is back to the one sheet it started with: the label is
    // on the only border there is, the last row.
    tumu.wait_until("the shell is the only sheet again", |h| {
        let rows = h.screen_rows();
        rows[rows.len() - 1].starts_with("└ [C-q ...]")
    });
}

#[test]
fn the_editor_opens_in_a_new_sheet_above_the_one_it_came_from() {
    // The editor stays alive, so its sheet is still up while it is asserted on.
    //
    // The shell keeps printing after the editor opens, so its region has fresh
    // output at the bottom when the stack splits and the shell's region shrinks:
    // a sheet shows the *newest* lines of its history (see `grid_of`), so a lone
    // line printed before the split would have scrolled into history and left the
    // region blank.
    let shell = script(
        "editor-sheet-shell",
        "echo SHEET-HELLO; while true; do echo SHELL-STILL-HERE; sleep 0.1; done",
    );
    let editor = script("editor-sheet-editor", "echo EDITOR-RUNNING; exec sleep 30");

    // A 40x30 terminal is taller than wide, so the stack runs vertically and the
    // editor sheet -- pushed on top -- takes the rows above the shell's.
    let mut tumu = e2e::TumuHarness::with_shell_and_editor(
        40,
        30,
        shell.to_str().expect("utf-8 path"),
        editor.to_str().expect("utf-8 path"),
    );
    tumu.wait_for_text("SHEET-HELLO");

    // One sheet before: the label is on the only border there is, the last row.
    assert!(
        tumu.screen_rows()[39].starts_with("└ [C-q ...]"),
        "the single sheet carries the label:\n{}",
        tumu.screen_text()
    );

    tumu.send_prefix();
    tumu.send_char('l');
    tumu.wait_for_text("EDITOR-RUNNING");

    // Two sheets now, the editor on top. The label stays on the *focused*
    // sheet's border, so its moving up to row 19 is what proves the editor
    // sheet exists and holds focus.
    assert!(
        tumu.screen_rows()[19].starts_with("└ [C-q ...]"),
        "the editor's sheet carries the label:\n{}",
        tumu.screen_text()
    );
    // The sheet the editor came from is still there, below, still painting.
    // The shell's output has to reach the bottom of its region, which takes a
    // few prints while its session is still taller than the region.
    tumu.wait_until("the shell's sheet stays below and keeps painting", |h| {
        h.screen_contains("SHELL-STILL-HERE")
    });
}

/// Prints `count` numbered markers, then waits so they stay in the history.
///
/// The markers are what tell whether the history was trimmed: the first ones
/// scroll off and are what a small limit drops, the last ones stay.
fn marker_shell(count: u32) -> String {
    format!("for i in $(seq 1 {count}); do echo LINE-$i; done; exec sleep 30")
}

#[test]
fn the_editor_file_holds_history_and_visible_rows_without_trailing_blanks() {
    // More lines than the sheet can show, so the earliest markers scroll off
    // the visible grid and into history. The file must hold both halves: the
    // history first, then the rows still on screen.
    let shell = script("text-shell", &marker_shell(60));
    let out = out_path("text");
    let editor = script("text-editor", &copy_editor(&out));

    let mut tumu = e2e::TumuHarness::with_args(24, 80, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let editor = editor.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_EDITOR", &editor);
        }
    });
    tumu.wait_for_text("LINE-60");

    let text = hand_off_scrollback(&mut tumu, &out);

    // The oldest marker is in history and the newest is on screen: the file
    // spans both, head to tail.
    assert!(
        text.contains("LINE-1\n"),
        "the file starts with the oldest retained line:\n{text}"
    );
    assert!(
        text.contains("LINE-60"),
        "the file ends with the newest line:\n{text}"
    );
    // Each row is trimmed of its trailing blanks, so a line is just its
    // marker: no line carries the padding a full-width terminal row would.
    for line in text.lines() {
        assert_eq!(
            line,
            line.trim_end(),
            "a line kept trailing blanks: {line:?}"
        );
    }
}

/// Prints two markers near the top of the screen, then leaves the cursor far
/// below them without writing anything else.
///
/// The grid keeps its height whether or not the child filled it, so after this
/// the sheet's visible rows are two lines of output followed by a screenful of
/// padding. That padding is what must not reach the history file.
///
/// The screen is *not* cleared to make the padding: a clear scrolls the output
/// into history, and the blank rows it leaves there are the child's own rows
/// (they are history, not padding) -- a different matter from this test.
fn padded_shell() -> String {
    "printf 'PAD-FIRST\\nPAD-SECOND\\n'; printf '\\033[24;1H'; exec sleep 30".to_string()
}

#[test]
fn the_editor_file_does_not_end_with_the_grid_padding() {
    // The sheet shows two lines of output and fills the rest of its grid with
    // blanks. Those blanks are the buffer's padding, not the child's output, so
    // the file must end at the second marker -- not carry a screenful of empty
    // lines after it.
    let shell = script("pad-shell", &padded_shell());
    let out = out_path("pad");
    let editor = script("pad-editor", &copy_editor(&out));

    let mut tumu = e2e::TumuHarness::with_args(24, 80, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let editor = editor.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_EDITOR", &editor);
        }
    });
    tumu.wait_for_text("PAD-SECOND");

    let text = hand_off_scrollback(&mut tumu, &out);

    assert!(
        text.contains("PAD-SECOND"),
        "the sheet's output must be in the file:\n{text:?}"
    );
    assert!(
        !text.ends_with('\n'),
        "the file must not end with the grid's padding:\n{text:?}"
    );
    // The file is the two markers and nothing else: no blank line after them.
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines,
        vec!["PAD-FIRST", "PAD-SECOND"],
        "the file must hold only the child's output:\n{text:?}"
    );
}

/// Runs `prefix l` and returns the text the editor received.
///
/// The editor copies the file it was opened on to `out`; the test waits for the
/// file to appear and non-empty, then reads it. Passing the editor the output
/// path keeps every caller from repeating the wait.
fn hand_off_scrollback(tumu: &mut e2e::TumuHarness, out: &std::path::Path) -> String {
    tumu.send_prefix();
    tumu.send_char('l');
    tumu.wait_until("the editor received the scrollback", |_| {
        std::fs::read_to_string(out)
            .map(|text| !text.is_empty())
            .unwrap_or(false)
    });
    std::fs::read_to_string(out).expect("read the editor output")
}

#[test]
fn the_scrollback_is_trimmed_to_the_cli_limit() {
    // Sixty markers against a five-line history bound: the last markers survive
    // in the history and on screen, the earliest are trimmed away, so the
    // editor sees the tail and not the head.
    let shell = script("trim-shell", &marker_shell(60));
    let out = out_path("trim-cli");
    let editor = script("trim-editor", &copy_editor(&out));

    let mut tumu = e2e::TumuHarness::with_args(24, 80, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let editor = editor.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_EDITOR", &editor);
            command.arg("--scrollback").arg("5");
        }
    });
    tumu.wait_for_text("LINE-60");

    let text = hand_off_scrollback(&mut tumu, &out);

    assert!(
        text.contains("LINE-60"),
        "the newest marker must survive the trim:\n{text}"
    );
    assert!(
        !text.contains("LINE-1\n"),
        "the oldest marker must be trimmed away:\n{text}"
    );
}

#[test]
fn the_scrollback_limit_can_come_from_the_environment() {
    // The same shell and marker count, but the bound comes from
    // `$TUMU_SCROLLBACK` instead of the command line. The trimmed head proves
    // the environment was read at all.
    let shell = script("trim-env-shell", &marker_shell(60));
    let out = out_path("trim-env");
    let editor = script("trim-env-editor", &copy_editor(&out));

    let mut tumu = e2e::TumuHarness::with_args(24, 80, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let editor = editor.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_EDITOR", &editor);
            command.env("TUMU_SCROLLBACK", "5");
        }
    });
    tumu.wait_for_text("LINE-60");

    let text = hand_off_scrollback(&mut tumu, &out);

    assert!(
        !text.contains("LINE-1\n"),
        "the environment must bound the history too:\n{text}"
    );
}

#[test]
fn the_cli_limit_overrides_the_environment() {
    // The environment asks for no history at all, and the command line asks for
    // a hundred lines. If the command line wins, the head survives; if the
    // environment wins, it is gone. That the head is present is the assertion.
    let shell = script("trim-precedence-shell", &marker_shell(60));
    let out = out_path("trim-precedence");
    let editor = script("trim-precedence-editor", &copy_editor(&out));

    let mut tumu = e2e::TumuHarness::with_args(24, 80, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let editor = editor.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_EDITOR", &editor);
            command.env("TUMU_SCROLLBACK", "0");
            command.arg("--scrollback").arg("100");
        }
    });
    tumu.wait_for_text("LINE-60");

    let text = hand_off_scrollback(&mut tumu, &out);

    assert!(
        text.contains("LINE-1\n"),
        "--scrollback must override $TUMU_SCROLLBACK:\n{text}"
    );
}

/// The SGR code a terminal sends for a wheel turn *down* (towards the user).
///
/// The xterm SGR button code for the wheel is 64 up and 65 down; the harness
/// writes the raw bytes so the test drives `tumu`'s own decode, not a library
/// encode.
const WHEEL_DOWN: u16 = 65;

#[test]
fn a_scroll_down_opens_the_scrollback_when_no_child_asks_for_the_mouse() {
    // The shell prints markers and then sleeps without asking for the mouse, so
    // a scroll down has nothing to page and belongs to tumu: it is the
    // copy-mode door, the same one `prefix l` opens.
    let shell = script("wheel-shell", &marker_shell(60));
    let out = out_path("wheel-copy-mode");
    let editor = script("wheel-editor", &copy_editor(&out));

    // Vertical so the border is the last row and the content is above it; the
    // axis only decides where the border runs, not whether the wheel works.
    let mut tumu = e2e::TumuHarness::with_args(24, 80, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let editor = editor.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_EDITOR", &editor);
            command.arg("--orientation").arg("vertical");
        }
    });
    tumu.wait_for_text("LINE-60");

    // A scroll down over the sheet, with no child asking: the turn opens the
    // focused sheet's history in an editor sheet.
    tumu.send_mouse_code(WHEEL_DOWN, 5, 10);
    // The wait is a `wait_until` and not a sleep on the file: a mouse report is
    // only written to the child's PTY by a pump, so the harness has to keep
    // pumping for the turn to reach `tumu` at all.
    tumu.wait_until("the wheel opens the history", |_| out.exists());

    let text = std::fs::read_to_string(&out).expect("read the editor output");

    assert!(
        text.contains("LINE-60"),
        "the wheel opened the sheet's scrollback:\n{text}"
    );
}

#[test]
fn a_scroll_up_does_not_open_the_scrollback() {
    // The mirror of the test above: only *down* opens copy mode, because there
    // is no "above the top" for an up turn to mean. The editor must not run, so
    // the output file is never written.
    let shell = script("wheel-up-shell", &marker_shell(60));
    let out = out_path("wheel-up-noop");
    let editor = script("wheel-up-editor", &copy_editor(&out));

    let mut tumu = e2e::TumuHarness::with_args(24, 80, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let editor = editor.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_EDITOR", &editor);
            command.arg("--orientation").arg("vertical");
        }
    });
    tumu.wait_for_text("LINE-60");

    // SGR 64 is a wheel turn *up*.
    tumu.send_mouse_code(64, 5, 10);

    // The report is only written to the PTY by a pump, so the wait is what makes
    // the turn reach `tumu` at all: a bare sleep would assert nothing, because
    // the bytes would never leave the harness. Two pumps give `tumu` the turn and
    // a round to act on it; if it opened the history, `out` would exist by now.
    tumu.wait_until("the turn to be delivered", |_| true);
    tumu.wait_until("a round to act", |_| true);

    assert!(
        !out.exists(),
        "an up turn must not open the history: {}",
        out.display()
    );
    // The one sheet is still the only one -- its label is on the last row -- so
    // no editor sheet was pushed on top.
    let rows = tumu.screen_rows();
    assert!(
        rows[rows.len() - 1].starts_with("└ [C-q ...]"),
        "the single sheet still fills the terminal:\n{}",
        tumu.screen_text()
    );
}
