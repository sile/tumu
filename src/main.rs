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

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    // The real size arrives as a resize the driver reports once the terminal
    // is taken over, so the seed here is only a placeholder that the first
    // resize replaces.
    let seed = tuinix::Size { rows: 24, cols: 80 };
    let shell = shell_command();
    match app::App::new(tumu::State::new(seed), shell) {
        Ok(mut app) => app.run()?,
        Err(err) => {
            eprintln!("tumu: {err}");
            std::process::exit(1);
        }
    }
    Ok(())
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
