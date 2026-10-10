//! End-to-end tests that a sheet's session output reaches the frame.
//!
//! These point the sheet's `$SHELL` at a small script, so what the session
//! prints is fixed and a test can assert the grid shows it. They drive the real
//! binary and read the frame back through the emulator, so they prove the whole
//! path -- spawn, pump, blit -- and not just the drawing function.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// Writes a shell script to a temp path and returns it.
///
/// The path is unique per call so tests running in parallel do not fight over
/// one file. The script is left behind (a temp dir is swept by the OS), which
/// keeps the helper free of a cleanup dance in every test.
fn script(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-e2e-{}-{}.sh", std::process::id(), name));
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("failed to write the test script");
    // Make it executable; the child execs it directly, so the bit matters.
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

#[test]
fn a_sheets_output_is_painted_into_its_content_region() {
    // The sheet runs `$SHELL -l`; a login-style script that prints a banner and
    // then waits keeps the sheet alive and its output stable. A tall terminal
    // stacks vertically, so the border is the last row and the banner lands on
    // row 0 -- the first content row.
    let shell = script("banner", "echo SHEET-BANNER; exec sleep 30");
    let mut tumu = e2e::TumuHarness::with_shell(40, 24, shell.to_str().expect("utf-8 path"));

    tumu.wait_for_text("SHEET-BANNER");
}

#[test]
fn output_goes_above_the_border_not_over_it() {
    // The border still shows on the sheet's last row even with output painted
    // above it, and the banner is on the row before.
    let shell = script("above", "echo CONTENT-MARK; exec sleep 30");
    let mut tumu = e2e::TumuHarness::with_shell(40, 24, shell.to_str().expect("utf-8 path"));

    tumu.wait_for_text("CONTENT-MARK");
    let rows = tumu.screen_rows();
    assert!(
        rows[39].starts_with("└ [C-q ...]"),
        "the border must stay on the sheet's last row, got: {:?}",
        rows[39]
    );
    assert!(
        rows[0].contains("CONTENT-MARK"),
        "the output must be on the first content row, got: {:?}",
        rows[0]
    );
}

#[test]
fn two_sheets_each_show_their_own_output() {
    // Both sheets run the same script; each paints its banner inside its own
    // half, so a second sheet neither erases the first nor draws over it.
    let shell = script("two", "echo BOTH-MARK; exec sleep 30");
    let mut tumu = e2e::TumuHarness::with_shell(40, 24, shell.to_str().expect("utf-8 path"));
    tumu.wait_for_text("BOTH-MARK");

    tumu.send_prefix();
    tumu.send_char('n');

    // Both content regions are on screen in the same frame once the second
    // sheet has also printed.
    tumu.wait_until("both sheets show a banner", |h| {
        h.screen_text().matches("BOTH-MARK").count() >= 2
    });
}
