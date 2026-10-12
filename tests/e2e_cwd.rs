//! End-to-end test that a new sheet inherits the focused sheet's directory.
//!
//! A shell that has `cd`-ed keeps that place in its own process, not in tumu, so
//! tumu reads it out of the child through `/proc/<pid>/cwd` when it spawns the
//! next sheet. What is under test is that the new child starts where the
//! focused one stands, not how the directory is discovered: the test `cd`s in a
//! scripted shell and opens a tool sheet whose script prints its own `pwd`.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// Writes `body` to a unique temp script and returns its path.
fn script(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-cwd-{}-{}.sh", std::process::id(), name));
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// A directory unique to this test, left behind for the OS temp sweeper.
///
/// The directory's *name* is what the scripts report, because the full path is
/// longer than a sheet is wide and would wrap across rows, where a single-row
/// `contains` could not see it. The name is kept short so it fits one row, and
/// made unique so the assertion cannot pass by matching another directory's
/// name.
fn unique_dir_name(tag: &str) -> String {
    let name = format!("tcd{}-{tag}", std::process::id());
    let dir = std::env::temp_dir().join(&name);
    fs::create_dir_all(&dir).expect("create the test directory");
    name
}

#[test]
fn a_tool_sheet_starts_in_the_focused_sheets_directory() {
    let dir_name = unique_dir_name("diff");
    let dir = std::env::temp_dir().join(&dir_name);
    // The shell `cd`s to `dir` and prints a sentinel *after* the `cd`, so the
    // test can wait for the directory to change before opening the next sheet.
    let shell = script(
        "cwd-shell",
        &format!("cd {:?}\necho SHELL-READY\nexec sleep 30", dir),
    );
    // The tool prints the name of the directory it was started in and waits;
    // that one line is the whole assertion.
    let tool = script(
        "cwd-tool",
        "echo TOOL-BASE=$(basename \"$PWD\"); exec sleep 30",
    );

    let mut tumu = e2e::TumuHarness::with_args(40, 24, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        let tool = tool.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
            command.env("TUMU_MAMEDIFF", &tool);
        }
    });
    // The shell has `cd`-ed once it prints the sentinel.
    tumu.wait_for_text("SHELL-READY");

    tumu.send_prefix();
    tumu.send_char('d');

    // The tool sheet starts in the shell's directory, so the name it prints is
    // the one the shell `cd`-ed into, not tumu's own launch directory's name.
    let expected = format!("TOOL-BASE={dir_name}");
    tumu.wait_until("the tool sheet reports the shell's directory", |h| {
        h.screen_contains(&expected)
    });
}

#[test]
fn the_first_sheet_starts_in_tumu_itselfs_directory() {
    // A run whose first sheet has nothing to inherit from starts where tumu was
    // launched. The harness starts tumu from the test process's directory, so
    // the name the sheet's child reports is that directory's name.
    let shell = script(
        "first-shell",
        "echo SHELL-BASE=$(basename \"$PWD\"); exec sleep 30",
    );
    let here = fs::canonicalize(".").expect("canonicalize the working directory");
    let here_name = here
        .file_name()
        .expect("the working directory has a name")
        .to_string_lossy()
        .into_owned();

    let mut tumu = e2e::TumuHarness::with_args(40, 24, {
        let shell = shell.to_str().expect("utf-8 path").to_string();
        move |command| {
            command.env("SHELL", &shell);
        }
    });

    let expected = format!("SHELL-BASE={here_name}");
    tumu.wait_until("the first sheet reports the launch directory", |h| {
        h.screen_contains(&expected)
    });
}
