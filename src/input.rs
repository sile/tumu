//! The prefix key and the hard-coded resolver that turns a latched key into a
//! command.
//!
//! The bindings are written out by hand, the way `kk` writes its own: one
//! `match` over `(ctrl, code)` per role, with no table to keep in step.
//!
//! The prefix is `Ctrl` plus one character. Only the character is a setting
//! ([`DEFAULT_PREFIX_CHAR`], `--prefix`, `$TUMU_PREFIX`); the `Ctrl` is fixed --
//! see [`prefix_key`]. A prefix is therefore always `Ctrl+<char>` and a chord is
//! always an unmodified `<char>`, so the two are different keys and no chord can
//! be shadowed by the prefix, whatever character it is set to.

use crate::Command;

/// The stem of the file an editor sheet is opened on.
///
/// The core names the file the editor reads, but *where* it goes is the edge's
/// business -- only the edge may touch the file system. So the core writes a
/// bare name and the edge puts it in its own directory; this constant is what
/// keeps the two halves of that convention in one place.
pub const SCROLLBACK_FILE_PREFIX: &str = "tumu-scrollback";

/// The character the prefix uses when nothing overrides it: `C-q`.
///
/// `C-o` is the prefix the README settles on, but `C-o` is also tmux's own
/// prefix, so pressing it inside a nested tmux would be ambiguous while this is
/// under development. The default therefore stays `q` for now; `C-o` is one
/// `--prefix o` away at run time, so reaching it does not wait on a rebuild, and
/// this is the only place the built-in default is written.
pub const DEFAULT_PREFIX_CHAR: char = 'q';

/// Builds the prefix key for character `c`: `Ctrl` plus `c`.
///
/// The `Ctrl` is fixed and only `c` varies, so the prefix is always a modified
/// key and every chord (an unmodified `<char>`) is a distinct one. `Alt` is
/// never part of the prefix, so `Alt-C-<c>` is not it.
pub fn prefix_key(c: char) -> tuinix::KeyInput {
    tuinix::KeyInput {
        ctrl: true,
        alt: false,
        code: tuinix::KeyCode::Char(c),
    }
}

/// Returns the prefix written the way it is pressed, for the border and the
/// menu: `C-q` when `c` is `q`.
///
/// It is derived from the character rather than written a second time, so the
/// label follows the setting and a configured `--prefix o` reads `C-o`
/// everywhere the key is named.
pub fn prefix_key_text(c: char) -> String {
    format!("C-{c}")
}

/// Returns whether `key` is the prefix for character `c`.
///
/// The prefix is matched on the whole key -- Ctrl on, Alt off -- so an
/// `Alt-C-<c>` is not the prefix.
pub fn is_prefix(c: char, key: tuinix::KeyInput) -> bool {
    key == prefix_key(c)
}

/// One prefix chord: the key, the command it names, and the label the menu
/// shows for it.
///
/// The three live together so they cannot drift: the menu is built from this
/// table and
/// [`resolve_command`] answers out of it, so a chord cannot be bound without a
/// label or labelled without being bound.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    /// The key, after the prefix, that runs the command.
    pub key: tuinix::KeyInput,

    /// What the key does.
    pub command: Command,

    /// The word the menu shows next to the key.
    pub label: &'static str,
}

/// A chord on a plain (unmodified) character, the shape every binding has.
const fn chord(c: char) -> tuinix::KeyInput {
    tuinix::KeyInput {
        ctrl: false,
        alt: false,
        code: tuinix::KeyCode::Char(c),
    }
}

/// Every prefix chord, in the order the menu lists them.
///
/// This is the order the commands are worth reading in, not the order of the
/// keys: making a sheet, then acting on it, then leaving.
pub const BINDINGS: &[Binding] = &[
    Binding {
        key: chord('n'),
        command: Command::NewSheet,
        label: "new sheet",
    },
    Binding {
        key: chord('x'),
        command: Command::CloseSheet,
        label: "close sheet",
    },
    Binding {
        key: chord('o'),
        command: Command::RotateSheets,
        label: "rotate to bottom",
    },
    Binding {
        key: chord('z'),
        command: Command::ToggleFullScreen,
        label: "full screen",
    },
    Binding {
        key: chord('l'),
        command: Command::EditScrollback,
        label: "edit scrollback",
    },
    Binding {
        key: chord('y'),
        command: Command::PasteClipboard,
        label: "paste clipboard",
    },
    Binding {
        key: chord('q'),
        command: Command::Quit,
        label: "quit",
    },
];

/// Resolves a latched key to the command it names, or `None` when unbound.
///
/// Only the key's `ctrl`/`code` take part: no bound key carries Alt, so an
/// Alt-modified key is unbound rather than falling back to the bare chord, as
/// in `kk`. The prefix itself is handled by the caller (it becomes
/// *send-prefix*), so it is not listed here.
pub fn resolve_command(key: tuinix::KeyInput) -> Option<Command> {
    if key.alt {
        return None;
    }
    let tuinix::KeyInput { ctrl, code, .. } = key;

    BINDINGS
        .iter()
        .find(|binding| {
            let tuinix::KeyInput {
                ctrl: bound_ctrl,
                code: bound_code,
                ..
            } = binding.key;
            ctrl == bound_ctrl && code == bound_code
        })
        .map(|binding| binding.command)
}
