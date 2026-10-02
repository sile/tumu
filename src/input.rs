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

/// Returns whether `key` is the prefix.
///
/// The prefix is matched on the whole key -- Ctrl on, Alt off -- so an
/// `Alt-C-q` is not the prefix.
pub fn is_prefix(key: tuinix::KeyInput) -> bool {
    key == PREFIX
}

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

    Some(match (ctrl, code) {
        (false, tuinix::KeyCode::Char('n')) => Command::NewSheet,
        (false, tuinix::KeyCode::Char('x')) => Command::CloseSheet,
        (false, tuinix::KeyCode::Char('o')) => Command::RotateSheets,
        (false, tuinix::KeyCode::Char('q')) => Command::Quit,
        _ => return None,
    })
}
