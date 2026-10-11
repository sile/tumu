//! The `tumu` binary. Argument parsing and the poll loop live here; the Sans
//! I/O core lives in the `tumu` library.
//!
//! The binary owns the edge: raw mode, the PTYs, and the poll loop are all in
//! [`app`]. The library never performs I/O.

// The edge waits for readiness with `libc::poll`, which needs `unsafe`. That is
// the one call that cannot avoid it, so the binary relaxes the library's
// `forbid` to `deny` and marks that call with `#[expect]`.
#![deny(unsafe_code)]

mod app;

use std::ffi::OsString;
use std::path::PathBuf;

fn main() -> noargs::Result<()> {
    let mut args = noargs::raw_args();
    args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
    args.metadata_mut().app_description = env!("CARGO_PKG_DESCRIPTION");

    if noargs::VERSION_FLAG.take(&mut args).is_present() {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    noargs::HELP_FLAG.take_help(&mut args);

    // How many scrollback lines each session may keep. The option carries its
    // own precedence: `--scrollback` beats `$TUMU_SCROLLBACK`, which beats the
    // built-in default, so the three layers come from this one declaration.
    // `termnix` bounds nothing on its own, so the number reaches the edge and is
    // what keeps a long-running session from growing without limit. Zero is a
    // real choice -- "keep no history" -- and is passed through as such.
    //
    // The default is a literal because `noargs` takes a `&'static str`; it must
    // stay equal to `tumu::DEFAULT_LINES`, which spells the same number.
    let scrollback_lines: usize = noargs::opt("scrollback")
        .short('s')
        .ty("LINES")
        .env("TUMU_SCROLLBACK")
        .default("1000")
        .doc("Scrollback lines each sheet keeps (0 disables history)")
        .take(&mut args)
        .then(|a| a.value().parse::<usize>())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    // The cell bound is not a user knob: lines are what a person reasons about,
    // and the cell bound is only the guard that keeps a few pathologically wide
    // lines from dwarfing the line bound. It keeps its default.
    let scrollback = tumu::ScrollbackLimit::new(scrollback_lines, tumu::DEFAULT_CELLS);
    // The real size arrives as a resize the driver reports once the terminal
    // is taken over, so the seed here is only a placeholder that the first
    // resize replaces.
    let seed = tuinix::Size { rows: 24, cols: 80 };
    let shell = shell_command();
    let editor = editor_command();
    let editor_dir = editor_dir();
    match app::App::new(
        tumu::State::new(seed),
        shell,
        editor,
        scrollback,
        editor_dir,
    ) {
        Ok(mut app) => app.run()?,
        Err(err) => {
            eprintln!("tumu: {err}");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Builds the recipe for the external editor `prefix l` opens in a new sheet.
///
/// The editor is launched as `<program> --tail <path>`, where the path is the
/// file the edge wrote the scrollback to, so only the program is configurable.
/// `$TUMU_EDITOR` names it and falls back to `kk`, the editor this PoC is built
/// around; the variable exists so a user (or a test) can point the feature at
/// another editor without a rebuild.
fn editor_command() -> app::EditorCommand {
    let program = std::env::var_os("TUMU_EDITOR").unwrap_or_else(|| "kk".into());
    app::EditorCommand::new(PathBuf::from(program))
}

/// Returns the directory an editor sheet's file is written to.
///
/// The core names a bare file name for the text `prefix l` opens; the directory
/// is the binary's to choose, because only the edge may touch the file system.
/// `$TMPDIR` is the normal place for scratch files and is what Termux points at
/// its app-private temp, so it is preferred; `/tmp` is the fallback when the
/// variable is unset. The directory carries the process id so two tumu runs
/// cannot step on each other's files, and `tumu` names it so a leftover file is
/// recognisable.
fn editor_dir() -> PathBuf {
    let base = std::env::var_os("TMPDIR").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(base).join(format!("{}-{}", env!("CARGO_PKG_NAME"), std::process::id()))
}

/// Builds the recipe for the shell tumu runs in every sheet: the user's login
/// shell.
///
/// `$SHELL` is preferred and falls back to `/bin/sh`, so tumu starts the same
/// shell the user is used to. The recipe is a program and its arguments, not a
/// live [`Command`](std::process::Command): a fresh command is built from it per
/// spawn, because `termnix` registers child setup on the command and a command
/// cannot be spawned twice.
fn shell_command() -> app::ShellCommand {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
    app::ShellCommand::new(PathBuf::from(shell), vec![OsString::from("-l")])
}
