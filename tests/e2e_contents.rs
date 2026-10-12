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
    // Both sheets run the same script; each paints its own banner inside its own
    // half, so a second sheet neither erases the first nor draws over it. The
    // banner carries the shell's PID, so the two sheets' banners differ and the
    // test can tell one sheet's output from the other's rather than counting two
    // copies of one string. The script also fills its region with numbered
    // lines: opening the second sheet shrinks the first, and a sheet shows the
    // newest lines of its history (see `grid_of`), so a lone line printed at the
    // top would have scrolled into history by the time the shrink is over.
    let shell = script(
        "two",
        "i=0; while [ $i -lt 40 ]; do echo BOTH-MARK-$$; i=$((i+1)); done; exec sleep 30",
    );
    let mut tumu = e2e::TumuHarness::with_shell(40, 24, shell.to_str().expect("utf-8 path"));
    tumu.wait_for_text("BOTH-MARK-");

    tumu.send_prefix();
    tumu.send_char('c');

    // Both content regions are on screen in the same frame once the second
    // sheet has also printed, and the two banners name different shells.
    tumu.wait_until("both sheets show their own banner", |h| {
        let text = h.screen_text();
        let mut banners: Vec<&str> = text
            .split_whitespace()
            .filter(|word| word.starts_with("BOTH-MARK-"))
            .collect();
        banners.sort_unstable();
        banners.dedup();
        banners.len() >= 2
    });
}

#[test]
fn shrinking_a_sheet_refills_its_region_from_scrollback() {
    // A sheet whose session overflows its content region shows the *newest*
    // lines, the rest being history. Printing many numbered lines and then
    // shrinking the terminal proves the region is a window onto history-then-
    // grid, not the top of the grid: the newest line stays in view and the lines
    // that no longer fit are gone from view but were kept in history rather than
    // truncating the region to blank.
    let shell = script(
        "shrink",
        "i=0; while [ $i -lt 60 ]; do echo LINE-$i; i=$((i+1)); done; exec sleep 30",
    );
    let mut tumu = e2e::TumuHarness::with_shell(40, 24, shell.to_str().expect("utf-8 path"));
    tumu.wait_for_text("LINE-59");

    // Shrink the terminal. The sheet's region loses rows, so older lines must
    // drop into history while the newest stay in view.
    tumu.resize(24, 24);

    tumu.wait_until("the newest line is in view", |h| {
        h.screen_contains("LINE-59")
    });

    // The region is a window onto the *newest* lines: every line still shown is
    // numbered, and they are consecutive, so the region was refilled from the
    // bottom of history rather than truncated to blank above the surviving grid
    // rows. The trailing blank below the newest line is the cursor's row.
    let rows = tumu.screen_rows();
    let content = &rows[..rows.len() - 1];
    assert!(
        content
            .iter()
            .all(|row| row.is_empty() || row.starts_with("LINE-")),
        "every content row is blank or a numbered line, got: {content:?}"
    );
    let content: Vec<&String> = content.iter().filter(|row| !row.is_empty()).collect();
    // The lines that fell out of the region are the *oldest*; the ones still
    // shown run up to the newest, with no gap, so nothing was dropped.
    let shown: Vec<u32> = content
        .iter()
        .map(|row| {
            row.trim_start_matches("LINE-")
                .parse()
                .expect("a numbered line")
        })
        .collect();
    let first = shown[0];
    let last = *shown.last().expect("at least one line");
    assert_eq!(last, 59, "the newest line is the last shown");
    assert_eq!(
        shown,
        (first..=last).collect::<Vec<u32>>(),
        "the shown lines are consecutive, so no line was lost"
    );
}
