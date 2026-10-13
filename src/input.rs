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
/// table and the chord lookup answers out of it, so a chord cannot be bound
/// without a label or labelled without being bound.
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
///
/// `c` makes a sheet (`c` for *create*, since `n` has another job now) and the
/// two steps are bound the way a pager spells its own paging: `p` is *previous*
/// and `n` is *next*, where "next" is the direction the stack advances --
/// downwards when the sheets run down the screen, rightwards when they run
/// across it.
///
/// The split's share is bound to `+` and `-`, the two keys that grow and shrink
/// a thing everywhere else: `+` widens the sheets at hand, `-` narrows them.
/// They are the run-time half of the `--ratio` setting and read left to right
/// the way the arithmetic does.
///
/// The tool sheets sit next to `c` because they are the other ways to get a
/// sheet: `d` for the *diff* browser and `s` for the grep one, each opening the
/// tool in a sheet of its own. The keys are mnemonics for the tools (`d` for
/// *diff*, `s` for *search*) rather than words, so they read as the pair the
/// tools make.
pub const BINDINGS: &[Binding] = &[
    Binding {
        key: chord('c'),
        command: Command::NewSheet,
        label: "new sheet",
    },
    Binding {
        key: chord('d'),
        command: Command::NewDiffSheet,
        label: "diff sheet",
    },
    Binding {
        key: chord('s'),
        command: Command::NewGrepSheet,
        label: "grep sheet",
    },
    Binding {
        key: chord('x'),
        command: Command::OpenExtMenu,
        label: "ext menu",
    },
    Binding {
        key: chord('p'),
        command: Command::FocusPrevious,
        label: "focus up/left",
    },
    Binding {
        key: chord('n'),
        command: Command::FocusNext,
        label: "focus down/right",
    },
    Binding {
        key: chord('t'),
        command: Command::MoveSheetToTop,
        label: "move to top",
    },
    Binding {
        key: chord('+'),
        command: Command::WidenSheet,
        label: "widen sheet",
    },
    Binding {
        key: chord('-'),
        command: Command::NarrowSheet,
        label: "narrow sheet",
    },
    Binding {
        key: chord('r'),
        command: Command::RotateOrientation,
        label: "rotate layout",
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
];

/// The chords the extended menu offers.
///
/// These are the commands kept out of [`BINDINGS`] because their effect is not
/// a view change and cannot be taken back: closing a sheet ends the child in it
/// and quitting ends everything. They are reached through `prefix x`, so a
/// single mistyped key cannot run one -- the user has to open the menu and then
/// pick, which is the extra step the effect deserves.
///
/// The table is a second one rather than a flag on a binding because the two
/// sets are read in different modes: the prefix chords are looked up while the
/// prefix is latched, the menu chords while the menu is open. Keeping them apart
/// is what keeps a command from drifting from the mode
/// it belongs to -- a `q` at the prefix is unbound (the prefix already carries
/// the command), while a `q` in the menu is *quit*.
///
/// The keys are the mnemonic, not the effect: `q` is *quit* and `k` is *kill*,
/// the word a process manager uses for ending what runs in a sheet. `k` is not
/// `x` again, because the menu is a fresh set of chords and not a repeat of the
/// one that opened it.
pub const EXT_BINDINGS: &[Binding] = &[
    Binding {
        key: chord('k'),
        command: Command::CloseSheet,
        label: "close sheet",
    },
    Binding {
        key: chord('q'),
        command: Command::Quit,
        label: "quit",
    },
];

/// Resolves a latched key to the command it names, or `None` when unbound.
///
/// It reads [`BINDINGS`], the prefix's own table. Only the key's `ctrl`/`code`
/// take part: no bound key carries Alt, so an Alt-modified key is unbound rather
/// than falling back to the bare chord, as in `kk`. The prefix itself is handled
/// by the caller (it becomes *send-prefix*), so it is not listed here.
pub fn resolve_command(key: tuinix::KeyInput) -> Option<Command> {
    resolve_in(BINDINGS, key)
}

/// Resolves a key pressed while the extended menu is open to its command, or
/// `None` when it is not one of the menu's chords.
///
/// It reads [`EXT_BINDINGS`], the menu's own table, so the keys that close a
/// sheet or quit are live only in the menu and unknown at the prefix. Like
/// [`resolve_command`] an Alt-modified key is unbound rather than falling back
/// to the bare chord, and `None` is what closes the menu without running
/// anything: a key the menu does not name is a "no", not a command.
pub fn resolve_ext_command(key: tuinix::KeyInput) -> Option<Command> {
    resolve_in(EXT_BINDINGS, key)
}

/// Looks `key` up in `bindings`, the one lookup both modes share.
///
/// The two resolvers differ only in which table they read, so the matching
/// lives here once: a key is the bound one when its `ctrl` and `code` agree and
/// it carries no Alt. Alt is rejected in both modes, so `Alt-<char>` is never a
/// chord -- see [`resolve_command`].
fn resolve_in(bindings: &[Binding], key: tuinix::KeyInput) -> Option<Command> {
    if key.alt {
        return None;
    }
    let tuinix::KeyInput { ctrl, code, .. } = key;

    bindings
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
