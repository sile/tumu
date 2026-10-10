//! End-to-end test for the OSC 52 clipboard and `prefix y`.
//!
//! A sheet that copies writes an OSC 52 sequence to its PTY; tumu forwards the
//! text to the host terminal's clipboard and remembers it, and `prefix y` pastes
//! it back into the focused sheet. The shell script stands in for a program that
//! copies: it prints the OSC 52 with a known payload and then runs `cat`, so the
//! paste has a reader to land in and echo back. What is under test is that tumu
//! takes the copied bytes back out of the sequence and pastes them, not what the
//! host terminal did with the clipboard -- OSC 52 has no reply.

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

#[test]
fn the_prefix_y_hand_off_pastes_the_copied_text() {
    // `SHEET-COPIED` base64-encoded, as OSC 52 carries it. The script copies
    // it, then runs `cat` so the paste has somewhere to go and a way back to
    // the screen.
    let payload = "U0hFRVQtQ09QSUVE";
    let shell = script(
        "clipboard-shell",
        &format!("printf '\\033]52;c;{payload}\\007'; printf 'COPY-DONE\\n'; exec cat"),
    );

    let mut tumu = e2e::TumuHarness::with_shell(40, 24, shell.to_str().expect("utf-8 path"));
    // The OSC 52 has been sent and its own marker is on screen, so the copy has
    // reached tumu and `cat` is running to receive the paste.
    tumu.wait_for_text("COPY-DONE");

    tumu.send_prefix();
    tumu.send_char('y');

    // The pasted text comes back through `cat`: the bytes tumu remembered are
    // the ones the child had copied, handed back to the child as input.
    tumu.wait_for_text("SHEET-COPIED");
}
