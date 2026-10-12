//! End-to-end tests that `--shell` picks the program a sheet runs.
//!
//! The precedence of `--shell`, `$SHELL`, and `/bin/sh` is `noargs`'s to
//! guarantee, not tumu's, so these do not re-test it. What tumu has to get right
//! is passing the chosen program through to the spawn, and that is what these
//! check: the program comes from the flag and runs as a login shell.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// Writes a shell script to a temp path and returns it.
///
/// The path is unique per call so tests running in parallel do not fight over
/// one file. The script is left behind (a temp dir is swept by the OS), which
/// keeps the helper free of a cleanup dance in every test.
fn script(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-shell-{}-{}.sh", std::process::id(), name));
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("failed to write the test script");
    // Make it executable; the child execs it directly, so the bit matters.
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

#[test]
fn the_shell_flag_program_runs_in_a_sheet() {
    // `--shell` names the program, and the script it points at is what a sheet
    // prints. It is run as a login shell like any other program (`-l` is always
    // passed), which a script tolerates -- `sh` ignores the flag.
    let shell = script("flag", "echo FROM-FLAG-SHELL; exec sleep 30");
    let mut tumu = e2e::TumuHarness::with_args(40, 24, move |command| {
        command.arg("--shell").arg(shell);
    });

    tumu.wait_for_text("FROM-FLAG-SHELL");
}
