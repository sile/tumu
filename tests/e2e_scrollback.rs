//! End-to-end test for handing the scrollback to an external editor.
//!
//! `prefix l` runs the editor named by `$TUMU_EDITOR` with the focused sheet's
//! scrollback on its standard input. The tests point `$TUMU_EDITOR` at a small
//! script that copies its standard input to a known path, so the text tumu
//! produced can be read back and asserted on. The editor is a stand-in for
//! `kk`: what is under test is that tumu collects the scrollback and writes it
//! to the child's standard input, not what the child is.

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

#[test]
fn the_prefix_l_hand_off_sends_the_sheet_output_to_the_editor() {
    // The sheet prints a marker and then waits, so its output stays on screen
    // while the scrollback is collected.
    let shell = script("scrollback-shell", "echo SHEET-HELLO; exec sleep 30");
    // The editor copies everything on its standard input to the output file.
    // It ignores its own argv (`--tail`): what matters is the pipe tumu fills,
    // which is what `kk --tail` reads.
    let out = out_path("handoff");
    let editor = script("scrollback-editor", &format!("cat > {}", out.display()));

    let mut tumu = e2e::TumuHarness::with_shell_and_editor(
        40,
        24,
        shell.to_str().expect("utf-8 path"),
        editor.to_str().expect("utf-8 path"),
    );
    tumu.wait_for_text("SHEET-HELLO");

    tumu.send_prefix();
    tumu.send_char('l');

    // The editor runs and writes the scrollback out. The sheet's marker must be
    // part of it, which proves the session's own output reached the editor.
    tumu.wait_until("the editor received the scrollback", |_| {
        fs::read_to_string(&out)
            .map(|text| text.contains("SHEET-HELLO"))
            .unwrap_or(false)
    });

    // tumu is back in control afterwards: the sheet's border is on screen again.
    tumu.wait_for_text("└ [C-q ...]");
}

/// Prints `count` numbered markers, then waits so they stay in the history.
///
/// The markers are what tell whether the history was trimmed: the first ones
/// scroll off and are what a small limit drops, the last ones stay.
fn marker_shell(count: u32) -> String {
    format!("for i in $(seq 1 {count}); do echo LINE-$i; done; exec sleep 30")
}

/// Runs `prefix l` and returns the text the editor received.
///
/// The editor copies its standard input to `out`; the test waits for the file
/// to appear and non-empty, then reads it. Passing the editor the output path
/// keeps every caller from repeating the wait.
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
    let editor = script("trim-editor", &format!("cat > {}", out.display()));

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
    let editor = script("trim-env-editor", &format!("cat > {}", out.display()));

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
    let editor = script(
        "trim-precedence-editor",
        &format!("cat > {}", out.display()),
    );

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
