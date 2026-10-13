//! End-to-end tests for the tool sheets: `prefix d` and `prefix s`.
//!
//! `prefix d` opens `mamediff` in a new sheet and `prefix s` opens `mamegrep`,
//! each in place of the shell. The real tools need a git repository and a
//! terminal, so the tests point `$TUMU_MAMEDIFF` / `$TUMU_MAMEGREP` at small
//! scripts that print a marker and wait: what is under test is that tumu runs
//! the configured program in a sheet of its own, not what the tool is.

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

/// A silent shell that never sets a title and stays alive.
fn silent_shell(name: &str) -> std::path::PathBuf {
    script(name, "exec sleep 30")
}

#[test]
fn the_diff_command_runs_the_configured_program_in_a_new_sheet() {
    let shell = silent_shell("diff-shell");
    let tool = script("diff-tool", "echo DIFF-TOOL-RUNNING; exec sleep 30");

    let mut tumu = e2e::TumuHarness::with_args(40, 24, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let tool = tool.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_MAMEDIFF", &tool);
        }
    });
    // The shell sheet is up and focused: its border carries the label on the
    // bottom row of the terminal.
    tumu.wait_for_text("[C-q ...]");

    tumu.send_prefix();
    tumu.send_char('d');

    // The tool runs in a sheet of its own, so its output is painted by tumu like
    // any other child.
    tumu.wait_for_text("DIFF-TOOL-RUNNING");
}

#[test]
fn the_grep_command_runs_the_configured_program_in_a_new_sheet() {
    let shell = silent_shell("grep-shell");
    let tool = script("grep-tool", "echo GREP-TOOL-RUNNING; exec sleep 30");

    let mut tumu = e2e::TumuHarness::with_args(40, 24, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let tool = tool.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_MAMEGREP", &tool);
        }
    });
    tumu.wait_for_text("[C-q ...]");

    tumu.send_prefix();
    tumu.send_char('s');

    tumu.wait_for_text("GREP-TOOL-RUNNING");
}

#[test]
fn the_tool_sheet_stacks_on_top_and_the_shell_stays_below() {
    // The tool sheet is pushed on top like any new sheet, so the shell that was
    // focused loses the label and the tool sheet takes it. 40x30 is taller than
    // wide, so the stack runs vertically and the tool sheet takes the rows above
    // the shell's.
    //
    // The shell keeps printing after the tool opens, so its region has fresh
    // output at the bottom when the stack splits and the shell's region shrinks:
    // a sheet shows the *newest* lines of its history (see `grid_of`), so a lone
    // line printed before the split would have scrolled into history and left the
    // region blank.
    let shell = script(
        "stack-shell",
        "echo SHELL-RUNNING; while true; do echo SHELL-STILL-HERE; sleep 0.1; done",
    );
    let tool = script("stack-tool", "echo DIFF-TOOL-RUNNING; exec sleep 30");

    let mut tumu = e2e::TumuHarness::with_args(40, 30, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let tool = tool.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_MAMEDIFF", &tool);
        }
    });
    tumu.wait_for_text("SHELL-RUNNING");
    // One sheet: the label is on the only border, the last row.
    assert!(
        tumu.screen_rows()[39].starts_with("└ [C-q ...]"),
        "the single sheet carries the label:\n{}",
        tumu.screen_text()
    );

    tumu.send_prefix();
    tumu.send_char('d');
    tumu.wait_for_text("DIFF-TOOL-RUNNING");

    // Two sheets now, the tool on top: the label moved up to the tool sheet's
    // border, and the shell's output is still there below.
    assert!(
        tumu.screen_rows()[19].starts_with("└ [C-q ...]"),
        "the tool's sheet carries the label:\n{}",
        tumu.screen_text()
    );
    // The shell's output has to reach the bottom of its region, which takes a
    // few prints while its session is still taller than the region.
    tumu.wait_until("the shell's sheet stays below and keeps painting", |h| {
        h.screen_contains("SHELL-STILL-HERE")
    });
}

#[test]
fn a_tool_that_is_not_installed_shows_an_error_and_leaves_tumu_running() {
    // The failure this guards against: a `$TUMU_MAMEDIFF` that is not installed
    // used to make tumu itself die on the spawn, so pressing `prefix d` lost
    // every sheet. The failure must instead close only the sheet that could not
    // start, show why in a window, and leave the run alive.
    let shell = script("missing-shell", "echo SHELL-RUNNING; exec sleep 30");
    let missing = std::env::temp_dir().join(format!(
        "tumu-e2e-missing-tool-{}-does-not-exist",
        std::process::id()
    ));
    let _ = fs::remove_file(&missing);

    let mut tumu = e2e::TumuHarness::with_args(40, 30, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let missing = missing.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_MAMEDIFF", &missing);
        }
    });
    tumu.wait_for_text("SHELL-RUNNING");

    tumu.send_prefix();
    tumu.send_char('d');

    // The window names the program that would not start, and tumu is still
    // painting: the run did not end. The message is matched against the screen
    // with rows joined, because the box wraps it and the tool's name straddles
    // the wrap for some lengths of temp path.
    tumu.wait_until("the error window names the missing tool", |h| {
        h.screen_joined_contains("failed to start") && h.screen_joined_contains("does-not-exist")
    });

    // Any key dismisses it, and the shell sheet underneath is untouched -- the
    // failed sheet was closed rather than left empty.
    tumu.send_char(' ');
    tumu.wait_until("the window closes and the shell remains", |h| {
        !h.screen_joined_contains("failed to start")
            && h.screen_rows()[39].starts_with("└ [C-q ...]")
    });
    assert!(
        tumu.screen_contains("SHELL-RUNNING"),
        "the shell is still up:\n{}",
        tumu.screen_text()
    );
}

#[test]
fn a_shell_that_is_not_installed_shows_an_error_and_then_leaves() {
    // The worst case: the *first* sheet cannot start, so there is no session to
    // fall back to. tumu must still not die on the spawn -- the window is shown
    // and the run stays up for the one key that acknowledges it, and only then
    // does the (now empty) run end.
    let missing = std::env::temp_dir().join(format!(
        "tumu-e2e-missing-shell-{}-does-not-exist",
        std::process::id()
    ));
    let _ = fs::remove_file(&missing);

    let mut tumu = e2e::TumuHarness::with_args(40, 30, {
        let missing = missing.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &missing);
        }
    });

    tumu.wait_for_text("failed to start");

    // The acknowledgement is the one thing left to do; after it the run has no
    // session and no window, so it ends on its own.
    tumu.send_char(' ');
    tumu.wait_for_exit();
}

#[test]
fn a_tool_sheet_that_exits_closes_and_leaves_the_shell() {
    // The tool exits on its own, so its sheet closes through the ordinary
    // session-exit path and the stack is back to the shell it started with.
    let shell = script("exit-shell", "echo SHELL-RUNNING; exec sleep 30");
    // The marker is printed, then the tool waits a moment before exiting: a
    // tool that exits at once can close its sheet before the frame that showed
    // its output is ever painted, which would make the assertion below race.
    let tool = script("exit-tool", "echo DIFF-TOOL-RUNNING; sleep 1");

    let mut tumu = e2e::TumuHarness::with_args(40, 30, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let tool = tool.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_MAMEDIFF", &tool);
        }
    });
    tumu.wait_for_text("SHELL-RUNNING");

    tumu.send_prefix();
    tumu.send_char('d');
    tumu.wait_for_text("DIFF-TOOL-RUNNING");

    // The tool's exit closes its sheet, so the label returns to the shell's
    // border on the last row.
    tumu.wait_until("the shell is the only sheet again", |h| {
        h.screen_rows()[39].starts_with("└ [C-q ...]")
    });
}
