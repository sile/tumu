//! End-to-end tests for the stacking axis as a setting: `--orientation` and
//! `$TUMU_ORIENTATION` choose which way the sheets stack, and the running tumu
//! draws its border on the matching edge.
//!
//! These prove three things a core test cannot: the flag and the environment
//! reach the running process, the flag wins over the environment, and the border
//! the process draws follows the setting rather than the terminal's shape. The
//! terminal here is 24x80 -- wider than tall, which used to mean horizontal --
//! so a vertical border on it is the one assertion the shape alone cannot make.
//!
//! Every test pins `$SHELL` to a silent script for the same reason
//! `tests/e2e_keys.rs` does: a real login shell sets a window title of its own,
//! which would land in the border and make assertions about tumu's own text
//! flaky.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// Writes a silent shell script and returns its path, as `tests/e2e_keys.rs`
/// does: the sheet stays alive and quiet, so the border shows tumu's text only.
fn silent_shell(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-orient-{}-{}.sh", std::process::id(), name));
    fs::write(&path, "#!/bin/sh\nexec sleep 30\n").expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// Opens a 24x80 harness whose sheet runs a silent shell, with `setup` free to
/// add an `--orientation` argument or a `$TUMU_ORIENTATION` to the command.
fn harness<F>(name: &str, setup: F) -> e2e::TumuHarness
where
    F: FnOnce(&mut std::process::Command),
{
    let shell = silent_shell(name);
    let shell = shell.to_str().expect("utf-8 path").to_string();
    e2e::TumuHarness::with_args(24, 80, move |command| {
        command.env("SHELL", &shell);
        setup(command);
    })
}

#[test]
fn the_orientation_can_be_set_by_cli() {
    // `--orientation horizontal` on a 24x80 terminal: the border is the last
    // *column*, written downwards, so the label is read column by column.
    let mut tumu = harness("cli", |command| {
        command.args(["--orientation", "horizontal"]);
    });

    tumu.wait_for_column_text("[C-q ...]");
    assert!(
        !tumu.screen_contains("└ [C-q ...]"),
        "the border is a column, not a row: {}",
        tumu.screen_text()
    );
}

#[test]
fn setting_vertical_overrides_the_shape() {
    // The same 24x80 terminal, but asked to stack vertically: the border is the
    // last row. This is the case the old derivation could never produce, so it
    // is the one that proves the shape has no say any more.
    let mut tumu = harness("cli-vertical", |command| {
        command.args(["--orientation", "vertical"]);
    });

    tumu.wait_for_text("└ [C-q ...]");
    assert!(
        !tumu.screen_column_contains("[C-q ...]"),
        "the border is a row, not a column: {}",
        tumu.screen_text()
    );
}

#[test]
fn the_orientation_can_be_set_by_environment() {
    // The same as the CLI case, through `$TUMU_ORIENTATION` instead.
    let mut tumu = harness("env", |command| {
        command.env("TUMU_ORIENTATION", "horizontal");
    });

    tumu.wait_for_column_text("[C-q ...]");
    assert!(!tumu.screen_contains("└ [C-q ...]"));
}

#[test]
fn the_cli_orientation_wins_over_the_environment() {
    // Both are set and they disagree; the flag is the one that takes effect.
    let mut tumu = harness("precedence", |command| {
        command.env("TUMU_ORIENTATION", "horizontal");
        command.args(["--orientation", "vertical"]);
    });

    tumu.wait_for_text("└ [C-q ...]");
    assert!(
        !tumu.screen_column_contains("[C-q ...]"),
        "the flag won: the border is a row"
    );
}

#[test]
fn an_unknown_orientation_is_rejected() {
    // The setting is one of two words; anything else is a parse error rather
    // than a silently ignored value.
    let mut tumu = harness("reject", |command| {
        command.args(["--orientation", "sideways"]);
    });

    // The binary reports the error and exits without taking the terminal over.
    let status = tumu.wait_for_exit();
    assert!(!status.success(), "a bad --orientation is an error");
}
