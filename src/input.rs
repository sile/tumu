//! The prefix key and the hard-coded resolver that turns a latched key into a
//! command.
//!
//! The bindings are written out by hand, the way `kk` writes its own: one
//! `match` over `(ctrl, code)` per role, with no table to keep in step. The
//! prefix key is a single constant so restoring it later -- from the `c-q` this
//! PoC uses to the `c-o` the README names -- is a one-line change.

use crate::Command;

/// The key that arms the command latch.
///
/// It is `C-q` for now: `C-o` is the prefix the README settles on, but `C-o`
/// is also tmux's own prefix, so pressing it inside a nested tmux would be
/// ambiguous while this is under development. Once tumu is the outer
/// multiplexer, or nested use is a non-issue, this returns to `C-o` -- and
/// because it is the only place the key is written, that is the whole change.
pub const PREFIX: tuinix::KeyInput = tuinix::KeyInput {
    ctrl: true,
    alt: false,
    code: tuinix::KeyCode::Char('q'),
};

/// Returns the prefix written the way it is pressed, for the border and the
/// menu: `C-q` today.
///
/// It is derived from [`PREFIX`] rather than written a second time, so restoring
/// the prefix to `C-o` changes this with it.
pub fn prefix_key_text() -> String {
    let base = match PREFIX.code {
        tuinix::KeyCode::Char(c) => c.to_string(),
        other => format!("{other:?}"),
    };
    match (PREFIX.ctrl, PREFIX.alt) {
        (false, false) => base,
        (true, false) => format!("C-{base}"),
        (false, true) => format!("M-{base}"),
        (true, true) => format!("C-M-{base}"),
    }
}

/// Returns whether `key` is the prefix.
///
/// The prefix is matched on the whole key -- Ctrl on, Alt off -- so an
/// `Alt-C-q` is not the prefix.
pub fn is_prefix(key: tuinix::KeyInput) -> bool {
    key == PREFIX
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
