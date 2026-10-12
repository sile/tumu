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

    // The prefix is `Ctrl` plus one character, and only the character is a
    // setting -- see `tumu::prefix_key`. The flag, the environment, and the
    // built-in default come from this one declaration, in the same precedence
    // as `--scrollback`. The default is the library's own constant spelled out
    // as a literal because `noargs` takes a `&'static str`; it must stay equal
    // to `tumu::DEFAULT_PREFIX_CHAR`.
    let prefix_char: char = noargs::opt("prefix")
        .short('p')
        .ty("CHAR")
        .env("TUMU_PREFIX")
        .default("q")
        .doc("Character of the prefix key (Ctrl is fixed; e.g. `o` for C-o)")
        .take(&mut args)
        .then(|a| single_char(a.value()))?;

    // The odds the sheets split by, as the `num` of the `num : 1` share the
    // sheet at hand claims against everything below it. One number is enough
    // because the other side is fixed at one -- a finer shape is not needed and
    // would make the run-time steps harder to reason about; the share is moved
    // from here on by `prefix +` and `prefix -`, which the library bounds. The
    // flag, the environment, and the built-in default come from this one
    // declaration, in the same precedence as `--scrollback`. The default is the
    // library's own constant spelled out as a literal because `noargs` takes a
    // `&'static str`; it must stay equal to `tumu::DEFAULT_RATIO_NUM`.
    //
    // Out-of-range numbers are clamped by `Ratio::from_num`, not rejected: a
    // `--ratio 99` starts at the widest share tumu offers rather than refusing
    // to run, which matches the run-time steps never leaving the range either.
    let ratio_num: u32 = noargs::opt("ratio")
        .short('r')
        .ty("N")
        .env("TUMU_RATIO")
        .default("1")
        .doc("Split odds as N:1, the sheet's share against the sheets below (1-5)")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;

    // Which way the sheets stack. It used to be derived from the terminal's
    // shape, which made a rotation flip the layout by itself; now it is a
    // setting, chosen here once and then by `prefix r` at run time. The flag,
    // the environment, and the built-in default come from this one declaration,
    // in the same precedence as `--scrollback`. The default is the library's own
    // constant spelled out as a literal because `noargs` takes a `&'static str`;
    // it must stay equal to `tumu::Orientation::DEFAULT`.
    let orientation: tumu::Orientation = noargs::opt("orientation")
        .short('O')
        .ty("ORIENTATION")
        .env("TUMU_ORIENTATION")
        .default("vertical")
        .doc("How the sheets stack: `vertical` or `horizontal`")
        .take(&mut args)
        .then(|a| orientation_name(a.value()))?;

    // The program every sheet runs. It is a path, not a short name, because the
    // point is to point tumu at a specific shell (or a script that stands in for
    // one) rather than to pick from a list. The flag, the environment, and the
    // built-in default come from this one declaration, in the same precedence
    // as `--scrollback`: `--shell` beats `$SHELL`, which beats `/bin/sh`. The
    // default is a literal because `noargs` takes a `&'static str`. `$SHELL`
    // keeps its name -- a shell is what it names everywhere else -- so there is
    // no `$TUMU_SHELL` to learn. An empty `$SHELL` is ignored by `noargs`, which
    // matches the fallback tumu has always had.
    let shell: PathBuf = noargs::opt("shell")
        .ty("PATH")
        .env("SHELL")
        .default("/bin/sh")
        .doc("Program each sheet runs (the login shell; `-l` is always passed)")
        .take(&mut args)
        .then(|a| a.value().parse::<PathBuf>())?;

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
    let shell = shell_command(shell);
    let editor = editor_command();
    let editor_dir = editor_dir();
    let state = tumu::State::new(seed)
        .with_prefix_char(prefix_char)
        .with_orientation(orientation)
        .with_ratio(tumu::Ratio::from_num(ratio_num));
    match app::App::new(state, shell, editor, scrollback, editor_dir) {
        Ok(mut app) => app.run()?,
        Err(err) => {
            eprintln!("tumu: {err}");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Parses `text` as exactly one character.
///
/// The prefix setting is one character and nothing else: the `Ctrl` that goes
/// with it is fixed, so a value like `C-o` or `Escape` is not accepted -- it is
/// the character alone (`o`). Anything else is a parse error rather than a
/// silently ignored value, so a mistyped `--prefix` is reported instead of
/// leaving the built-in default in place.
fn single_char(text: &str) -> Result<char, String> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(format!("expected a single character, got `{text}`")),
    }
}

/// Parses `text` as an orientation name.
///
/// The two words are `vertical` and `horizontal`, the spelling
/// [`Orientation::name`](tumu::Orientation::name) produces, so the value a user
/// types and the value the menu label comes from are the same word. Anything
/// else is a parse error rather than a silent fallback, so a mistyped
/// `--orientation` is reported instead of leaving the default in place.
fn orientation_name(text: &str) -> Result<tumu::Orientation, String> {
    tumu::Orientation::parse(text)
        .ok_or_else(|| format!("expected `vertical` or `horizontal`, got `{text}`"))
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
/// The program is decided in `main` -- `--shell`, then `$SHELL`, then `/bin/sh`
/// -- so all this adds is the `-l` that makes it a login shell and the wrapper
/// that lets a fresh command be built per spawn. The recipe is a program and its
/// arguments, not a live [`Command`](std::process::Command): a fresh command is
/// built from it per spawn, because `termnix` registers child setup on the
/// command and a command cannot be spawned twice.
fn shell_command(program: PathBuf) -> app::ShellCommand {
    app::ShellCommand::new(program, vec![OsString::from("-l")])
}
