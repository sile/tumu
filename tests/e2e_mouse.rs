//! End-to-end tests for mouse handling: a click moves focus, and nothing else.
//!
//! These drive the real binary through mouse reports, the way a terminal would,
//! and read the resulting grid back. Focus is visible in the border -- only the
//! focused sheet carries the command label (`[C-q ...]`) -- so "the click moved
//! focus" is an assertion about the frame, not about the core's internals.
//!
//! The focus tests run their sheets *horizontally* (`--orientation
//! horizontal`), so two sheets split the columns 40/40 and each border runs
//! down its right column: the upper sheet's at column 39, the lower one's at
//! column 79. Focus is read from those columns. The forwarding tests use a
//! single vertical sheet instead, and ask for it; the axis is a setting now.
//!
//! The reports go through `termnix`, which only encodes one when the child has
//! asked for mouse reporting; that `tumu` has (see `App::new`) is part of what
//! these tests pin down.
//!
//! Every test pins `$SHELL` to a silent script, so a sheet's border shows only
//! tumu's own text. A real login shell sets a window title of its own (fish
//! does, on every prompt), which would land in the border and make these
//! assertions about *tumu's* text flaky.

mod e2e;

use std::fs;
use std::path::PathBuf;

/// The column the lower sheet of a 80-column terminal starts at.
const LOWER_SHEET_COL: u16 = 60;

/// The column inside the upper sheet of a 80-column terminal.
const UPPER_SHEET_COL: u16 = 10;

/// The last column of the upper sheet, where its border is written.
const UPPER_BORDER_COL: u16 = 39;

/// The last column of the lower sheet, where its border is written.
const LOWER_BORDER_COL: u16 = 79;

/// Waits until the two sheets are laid out and one of their borders carries the
/// focus label. `focused_column` is 39 for the upper sheet, 79 for the lower.
fn wait_for_focus(tumu: &mut e2e::TumuHarness, upper_focused: bool) {
    let (focused, other) = if upper_focused {
        (UPPER_BORDER_COL, LOWER_BORDER_COL)
    } else {
        (LOWER_BORDER_COL, UPPER_BORDER_COL)
    };
    tumu.wait_until("focus to settle", |h| {
        h.screen_column(focused).starts_with("└ [C-q ...]")
            && h.screen_column(other).starts_with("└─")
    });
}

/// Writes a silent shell script -- one that never sets a title -- and returns
/// its path. The script just sleeps, so the sheet stays alive and quiet.
fn silent_shell(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tumu-mouse-{}-{}.sh", std::process::id(), name));
    fs::write(&path, "#!/bin/sh\nexec sleep 30\n").expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// Opens a harness whose sheets run a silent shell, stacked horizontally.
fn harness(name: &str) -> e2e::TumuHarness {
    let shell = silent_shell(name);
    e2e::TumuHarness::with_shell_and_orientation(
        24,
        80,
        shell.to_str().expect("utf-8 path"),
        "horizontal",
    )
}
#[test]
fn clicking_a_sheet_moves_focus_to_it() {
    // 24x80 is wider than tall, so two sheets split the columns 40/40. The new
    // sheet goes on top, at index 0, and takes the focus.
    let mut tumu = harness("moves-focus");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    wait_for_focus(&mut tumu, true);

    // Click the sheet below: it takes the focus, and the label moves with it.
    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);
}

#[test]
fn clicking_back_returns_focus_to_the_top_sheet() {
    let mut tumu = harness("click-back");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    wait_for_focus(&mut tumu, true);

    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);

    tumu.click(5, UPPER_SHEET_COL);
    wait_for_focus(&mut tumu, true);
}

#[test]
fn a_click_does_not_disturb_the_layout() {
    let mut tumu = harness("layout-unchanged");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    wait_for_focus(&mut tumu, true);

    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);

    // Focus is all a click changes: both sheets' borders are still there, in the
    // same two columns, so neither sheet was moved or resized.
    assert!(tumu.screen_column(UPPER_BORDER_COL).starts_with("└"));
    assert!(tumu.screen_column(LOWER_BORDER_COL).starts_with("└"));
}

#[test]
fn clicking_a_sheets_border_does_not_open_the_menu() {
    // The border carries the command label, but the label is not a target: the
    // menu belongs to the prefix key, so a click on the line only lands on the
    // sheet under it. The border column of the lower sheet is 79.
    let mut tumu = harness("border-no-menu");
    tumu.wait_for_column_text("[C-q ...]");

    tumu.click(5, LOWER_BORDER_COL);

    // The menu never appears, and nothing else changed either: one sheet, still
    // focused, still filling the terminal.
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(!tumu.screen_contains("┌"), "no menu from a border click");
    tumu.wait_until("still one focused sheet", |h| {
        h.screen_column(LOWER_BORDER_COL).starts_with("└ [C-q ...]")
    });
}

#[test]
fn clicking_a_menu_row_neither_runs_it_nor_dismisses_the_menu() {
    // The menu is a keyboard thing: a click on a row is a click on the sheet
    // under the box, and nothing more. The row to aim at is one whose command
    // has a visible effect -- `new sheet` adds a border -- because if a click
    // *did* select, the frame would change.
    let mut tumu = harness("menu-keys-only");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.wait_until("the menu appears", |h| h.screen_contains("new sheet"));

    let row = tumu
        .screen_rows()
        .iter()
        .position(|line| line.contains("new sheet"))
        .expect("a new sheet row") as u16;
    tumu.click(row, 30);

    // Still running, with the menu still up and no second sheet made. A key
    // still works, which is what proves the latch survived the click.
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(tumu.screen_contains("new sheet"), "the menu is still up");

    // `c` makes a sheet, which closes the latch with the menu and proves a key
    // still reaches the chord table after the click.
    tumu.send_char('c');
    tumu.wait_until("a second sheet is made", |h| {
        h.screen_column(39).starts_with("└ [C-q ...]")
    });
}

/// Writes a shell script that asks for the mouse, then reports what it gets.
///
/// The script turns on SGR mouse reporting with the DECSET escape a real program
/// uses, prints a marker so the test knows the ask has landed, and then runs a
/// one-line `cat`-like reader in the shell: the report tumu forwards arrives on
/// the script's standard input, and the shell echoes it back to the screen. What
/// the test reads back is therefore the bytes the *child* received, not the
/// bytes the host sent.
fn mouse_asking_shell(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("tumu-mouse-ask-{}-{}.sh", std::process::id(), name));
    // `?1000h` is plain tracking and `?1006h` selects SGR; together they are what
    // an application turns on when it wants the mouse. `read` takes one line and
    // `od` prints it, so the report's exact bytes are on screen.
    let body = concat!(
        "#!/bin/sh\n",
        "printf '\\033[?1000h\\033[?1006h'\n",
        "printf 'MOUSE-ASKED\\n'\n",
        "IFS= read -r line\n",
        "printf '%s\\n' \"$line\" | od -An -c\n",
        "exec sleep 30\n",
    );
    fs::write(&path, body).expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

#[test]
fn a_child_that_asks_for_the_mouse_receives_the_report() {
    // One sheet, 40x30 -- taller than wide, so the stack runs vertically and the
    // border is the last row. Clicking inside the content area with no focus
    // change to make is the cleanest case: with the child asking, the report is
    // the child's.
    let shell = mouse_asking_shell("receives");
    let mut tumu = e2e::TumuHarness::with_shell_and_orientation(
        30,
        40,
        shell.to_str().expect("utf-8 path"),
        "vertical",
    );
    tumu.wait_for_text("MOUSE-ASKED");

    tumu.click(5, 10);

    // The child read a line and echoed it: an SGR mouse report for a left press
    // at the cell the click landed on. Row 5 and column 10 are zero-based in
    // tumu's coordinates and one-based in the wire form, so the report names
    // column 11 and row 6.
    tumu.wait_until("the child sees the report", |h| {
        h.screen_text().contains(";11;6M")
    });
}

#[test]
fn a_child_that_has_not_asked_never_sees_a_report() {
    // The same script but with the DECSET escaped so the child never asks. The
    // click must move the focus (a no-op here, there is one sheet) and reach no
    // child: the marker is the last thing on the grid.
    let shell = script_without_ask("ignores");
    let mut tumu = e2e::TumuHarness::with_shell_and_orientation(
        30,
        40,
        shell.to_str().expect("utf-8 path"),
        "vertical",
    );
    tumu.wait_for_text("MOUSE-ASKED");

    tumu.click(5, 10);

    // Give the binary time to forward it if it were going to, then assert the
    // child's reader is still waiting: no `od` output, so no report arrived.
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(
        !tumu.screen_text().contains(";11;6M"),
        "no report reaches a child that did not ask:\n{}",
        tumu.screen_text()
    );
}

/// The mouse-asking script with the DECSET removed, so nothing asks.
fn script_without_ask(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tumu-mouse-noask-{}-{}.sh",
        std::process::id(),
        name
    ));
    let body = concat!(
        "#!/bin/sh\n",
        "printf 'MOUSE-ASKED\\n'\n",
        "IFS= read -r line\n",
        "printf '%s\\n' \"$line\" | od -An -c\n",
        "exec sleep 30\n",
    );
    fs::write(&path, body).expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

/// Writes a shell script that asks for the mouse and echoes back every report
/// it receives, one per line.
///
/// Unlike [`mouse_asking_shell`], which reads a single report, this keeps
/// reading: a test can send a press and a wheel turn and see both, in order.
fn mouse_echoing_shell(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tumu-mouse-echo-{}-{}.sh",
        std::process::id(),
        name
    ));
    let body = concat!(
        "#!/bin/sh\n",
        "printf '\\033[?1000h\\033[?1006h'\n",
        "printf 'MOUSE-ASKED\\n'\n",
        "while IFS= read -r line; do\n",
        "  printf '%s\\n' \"$line\" | od -An -c\n",
        "done\n",
        "exec sleep 30\n",
    );
    fs::write(&path, body).expect("failed to write the test script");
    let mut perms = fs::metadata(&path).expect("stat the script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&path, perms).expect("chmod the script");
    path
}

#[test]
fn the_wheel_reaches_the_focused_child_from_another_sheets_region() {
    // The top sheet runs a program that tracks the mouse; the bottom one does
    // not. A wheel turn over the *bottom* sheet must still reach the tracking
    // child in the focused top sheet: a pager scrolls by the line, and routing
    // the wheel by region would drop the turn whenever the pointer was not on
    // the sheet the user is reading.
    //
    // SGR button code 65 is a wheel turn towards the user.
    let shell = mouse_echoing_shell("wheel-elsewhere");
    let mut tumu = e2e::TumuHarness::with_shell_and_orientation(
        24,
        80,
        shell.to_str().expect("utf-8 path"),
        "horizontal",
    );
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    // The new top sheet runs the tracking script: wait for the focus to settle
    // on it first, because the ask that matters is the *focused* child's and the
    // one on screen before the split belongs to the sheet it came from.
    wait_for_focus(&mut tumu, true);
    tumu.wait_for_text("MOUSE-ASKED");

    // Turn the wheel over the *lower* sheet, which is not the focused one.
    tumu.send_mouse_code(65, 5, LOWER_SHEET_COL);

    // The focused child reads it. The report names column 39, not the 60 the
    // turn happened on: the position is clipped into the focused sheet's grid
    // (columns 1..=40, one-based on the wire) before the child sees it, because
    // a position outside the child's own grid has no coordinates in the
    // protocol. Row 5 becomes 6, SGR's rows being one-based.
    tumu.wait_until("the focused child sees the wheel", |h| {
        h.screen_text().contains("<65;39;6M")
    });
}

#[test]
fn a_wheel_turn_does_not_move_the_focus() {
    // A turn over the lower sheet, while the upper sheet is focused and asking:
    // the focus must stay put, or the sheet the reader is on would slide away
    // under a scroll.
    let shell = mouse_echoing_shell("wheel-no-focus");
    let mut tumu = e2e::TumuHarness::with_shell_and_orientation(
        24,
        80,
        shell.to_str().expect("utf-8 path"),
        "horizontal",
    );
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    wait_for_focus(&mut tumu, true);
    tumu.wait_for_text("MOUSE-ASKED");

    tumu.send_mouse_code(65, 5, LOWER_SHEET_COL);

    // The label is still on the upper sheet's border: the turn reached the
    // child (clipped into its grid) and left the focus where it was.
    tumu.wait_until("the wheel reaches the child", |h| {
        h.screen_text().contains("<65;39;6M")
    });
    assert!(
        tumu.screen_column(UPPER_BORDER_COL)
            .starts_with("└ [C-q ...]"),
        "the wheel moved the focus:\n{}",
        tumu.screen_text()
    );
    assert!(
        tumu.screen_column(LOWER_BORDER_COL).starts_with("└─"),
        "the lower sheet took the focus:\n{}",
        tumu.screen_text()
    );
}

#[test]
fn a_click_outside_a_mouse_tracking_child_still_moves_focus() {
    // The upper sheet runs a program that tracks the mouse; the lower one does
    // not. A click on the lower sheet is a click on tumu's chrome, not on the
    // program, so it must move the focus rather than be swallowed by the
    // tracking child. The report is still tumu's because it lands outside the
    // focused sheet's region.
    let shell = mouse_asking_shell("neighbour-focus");
    let mut tumu = e2e::TumuHarness::with_shell_and_orientation(
        24,
        80,
        shell.to_str().expect("utf-8 path"),
        "horizontal",
    );
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    wait_for_focus(&mut tumu, true);
    // The new top sheet runs the mouse-asking script; wait for its ask to land.
    tumu.wait_for_text("MOUSE-ASKED");

    // Click the lower sheet: focus moves down, and the label moves with it. The
    // tracking child did not get the report, so no `od` line appears.
    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);
    assert!(
        !tumu.screen_text().contains(";61;6M"),
        "the report for the lower sheet is not forwarded to the tracking child:\n{}",
        tumu.screen_text()
    );
}

#[test]
fn the_keyboard_follows_a_click() {
    let mut tumu = harness("keyboard-follows");
    tumu.wait_for_column_text("[C-q ...]");
    tumu.send_prefix();
    tumu.send_char('c');
    wait_for_focus(&mut tumu, true);

    // Click the lower sheet, then close it. Closing is behind the extended
    // menu -- `x` then `k` -- and acts on the focused sheet, so the lower one
    // must go and the upper one must take the whole terminal.
    tumu.click(5, LOWER_SHEET_COL);
    wait_for_focus(&mut tumu, false);
    tumu.send_prefix();
    tumu.send_char('x');
    tumu.send_char('k');

    // One sheet is left, focused, filling the terminal. Its border is now the
    // screen's last column (79), and the column the closed sheet's border used
    // to be in (39) is content, so no border char is left there.
    tumu.wait_until("the lower sheet is gone", |h| {
        h.screen_column(79).starts_with("└ [C-q ...]")
            && h.screen_column(39).chars().all(|c| !"└─".contains(c))
    });
}
