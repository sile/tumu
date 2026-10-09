//! End-to-end tests for the prefix menu: pressing the prefix shows it, a chord
//! runs through it, and a click on a row selects it.
//!
//! These drive the real binary, so they prove the menu is drawn from the same
//! bindings the keys are resolved from and that its rows are clickable where
//! they are drawn. A tall terminal is used so the sheets' borders are rows and
//! the menu's rows are ordinary rows of text, which `screen_contains` can read.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// Writes a silent shell script -- one that never sets a title -- and returns
/// its path. The script just sleeps, so the sheet stays alive and quiet.
fn silent_shell(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-menu-{}-{}.sh", std::process::id(), name));
    fs::write(&path, "#!/bin/sh\nexec sleep 30\n").expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// Opens a harness whose sheets run a silent shell.
fn harness(name: &str) -> e2e::TumuHarness {
    let shell = silent_shell(name);
    e2e::TumuHarness::with_shell(40, 60, shell.to_str().expect("utf-8 path"))
}

#[test]
fn the_prefix_shows_the_menu() {
    let mut tumu = harness("shows");
    tumu.wait_for_text("└ [C-q ...]");
    assert!(!tumu.screen_contains("┌"), "no menu before the prefix");

    tumu.send_prefix();

    // The box appears, with each binding's key and label on its rows.
    tumu.wait_until("the menu appears", |h| h.screen_contains("┌"));
    assert!(tumu.screen_contains("new sheet"));
    assert!(tumu.screen_contains("quit"));
}

#[test]
fn a_chord_run_from_the_menu_closes_it_and_runs() {
    let mut tumu = harness("runs");
    tumu.wait_for_text("└ [C-q ...]");

    tumu.send_prefix();
    tumu.wait_until("the menu appears", |h| h.screen_contains("┌"));

    // `n` adds a sheet; the menu goes away with it.
    tumu.send_char('n');

    tumu.wait_until("the menu closes and a sheet is added", |h| {
        // Two sheets, so a second border is drawn below the first.
        !h.screen_contains("┌")
            && h.screen_rows()
                .iter()
                .filter(|line| line.starts_with("└"))
                .count()
                >= 2
    });
}

#[test]
fn an_unbound_key_closes_the_menu_without_running_anything() {
    let mut tumu = harness("dismiss");
    tumu.wait_for_text("└ [C-q ...]");

    tumu.send_prefix();
    tumu.wait_until("the menu appears", |h| h.screen_contains("┌"));

    // `Z` is not bound: the menu closes and nothing else happens.
    tumu.send_char('Z');

    tumu.wait_until("the menu closes", |h| !h.screen_contains("┌"));
    // Still one sheet, still focused.
    assert!(tumu.screen_text().contains("└ [C-q ...]"));
}

#[test]
fn clicking_a_menu_row_runs_its_command() {
    let mut tumu = harness("click-row");
    tumu.wait_for_text("└ [C-q ...]");

    tumu.send_prefix();
    tumu.wait_until("the menu appears", |h| h.screen_contains("┌"));

    // Find the row that says "quit" and click it. The menu is centred, so the
    // test reads the row's own position from the screen rather than guessing.
    let row = tumu
        .screen_rows()
        .iter()
        .position(|line| line.contains("quit"))
        .expect("a quit row") as u16;
    tumu.click(row, 30);

    tumu.wait_for_exit();
}
