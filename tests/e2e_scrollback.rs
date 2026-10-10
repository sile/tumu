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
