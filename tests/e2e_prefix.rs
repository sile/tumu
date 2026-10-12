//! End-to-end tests for the prefix key as a setting: `--prefix` and
//! `$TUMU_PREFIX` choose the character the prefix carries, and the running
//! tumu shows the choice on its border.
//!
//! The prefix is `Ctrl` plus one character, and only the character is a
//! setting, so these set the character and then send that chord. They prove
//! three things a core test cannot: the flag and the environment reach the
//! running process, the flag wins over the environment, and the border the
//! process draws names the character that was set.
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
    let path = std::env::temp_dir().join(format!("tumu-prefix-{}-{}.sh", std::process::id(), name));
    fs::write(&path, "#!/bin/sh\nexec sleep 30\n").expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// Opens a 24x80 harness whose sheet runs a silent shell, stacked
/// horizontally, with `setup` free to add a `--prefix` argument or a
/// `$TUMU_PREFIX` to the command.
///
/// Horizontal so the border is a column and the assertions below can name one;
/// the axis is a setting, so it has to be asked for like any other.
fn harness<F>(name: &str, setup: F) -> e2e::TumuHarness
where
    F: FnOnce(&mut std::process::Command),
{
    let shell = silent_shell(name);
    let shell = shell.to_str().expect("utf-8 path").to_string();
    e2e::TumuHarness::with_args(24, 80, move |command| {
        command.env("SHELL", &shell);
        command.args(["--orientation", "horizontal"]);
        setup(command);
    })
}

#[test]
fn the_prefix_character_can_be_set_by_cli() {
    // `--prefix o` makes the prefix `C-o`, and the focused sheet's border says
    // so. The default `C-q` must not appear anywhere.
    let mut tumu = harness("cli", |command| {
        command.args(["--prefix", "o"]);
    });

    tumu.wait_for_column_text("[C-o ...]");
    assert!(
        !tumu.screen_column_contains("C-q"),
        "the default prefix is not drawn when another is set"
    );
}

#[test]
fn the_prefix_character_can_be_set_by_environment() {
    // The same, through `$TUMU_PREFIX` rather than the flag.
    let mut tumu = harness("env", |command| {
        command.env("TUMU_PREFIX", "o");
    });

    tumu.wait_for_column_text("[C-o ...]");
    assert!(!tumu.screen_column_contains("C-q"));
}

#[test]
fn the_configured_prefix_arms_the_latch() {
    // The setting is not just a label: with `--prefix o`, `C-o` opens a sheet
    // and `C-q` does not.
    let mut tumu = harness("arms", |command| {
        command.args(["--prefix", "o"]);
    });
    tumu.wait_for_column_text("[C-o ...]");

    // `C-q` is ordinary typing now, so it must not open a sheet: one sheet
    // keeps its border at the screen's last column and the terminal stays full.
    tumu.send_prefix_char('q');
    tumu.send_char('n');
    // Give the binary a chance to react before asserting it did not.
    std::thread::sleep(std::time::Duration::from_millis(200));
    tumu.wait_until("still one sheet", |h| {
        h.screen_column(79).starts_with("└ [C-o ...]")
    });

    // The configured prefix does the work: `C-o` then `n` stacks a new sheet,
    // so two borders appear (the new top sheet at column 39).
    tumu.send_prefix_char('o');
    tumu.send_char('n');
    tumu.wait_until("two sheet borders", |h| {
        h.screen_column(39).starts_with("└ [C-o ...]") && h.screen_column(79).starts_with("└")
    });
}

#[test]
fn the_cli_prefix_wins_over_the_environment() {
    // Both are set and they disagree; the flag is the one that takes effect.
    let mut tumu = harness("precedence", |command| {
        command.env("TUMU_PREFIX", "o");
        command.args(["--prefix", "a"]);
    });

    tumu.wait_for_column_text("[C-a ...]");
    assert!(!tumu.screen_column_contains("C-o"));
}

#[test]
fn a_multi_character_prefix_is_rejected() {
    // The setting is one character; anything else is a parse error rather than
    // a silently ignored value.
    let mut tumu = harness("reject", |command| {
        command.args(["--prefix", "C-o"]);
    });

    // The binary reports the error and exits without taking the terminal over.
    let status = tumu.wait_for_exit();
    assert!(!status.success(), "a bad --prefix is an error");
}
