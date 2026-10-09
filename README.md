tumu
====

"tumu (積む)" is "stack" in Japanese.

- Terminal multiplexer
- Designed for
  - Termux
  - 6 inch display
  - Black and white e-ink display
- Feature
  - Stacked sheets (see below)
  - No sessions, panes, pop-up windows
  - Detect device orientation changes (via terminal size change event)
  - No server (a standalone process)
  - Mouse: click a sheet to focus it
  - tmux like copy-mode (PoC)

Stacked sheets
--------------

Vertically stacked:

```
|            |            |
|            |            |
| sheet 1    | sheet 2    |
|            |            |
| $          | $          |
|            |            |
|------------|------------|
```

Horizontally stacked:

```
----------------|---|---|
sheet 1         |   |   |
                |s  |.  |
                |2  |.  |
                |   |.  |
$               |$  |   |
----------------|---|---|
```

- When new sheet is created, it stacked to the top of the stack.
- each sheet size is decided automatically
  - e.g., s1:remainings = 1:1, remaining1:remaining-others = 1:1, ....
  - odds rate can be changes. but the sheet sizes are calculated automatically to make it easy to support display rotation
- a sheet can be swaped to the top, or, move to the stop
- support full screen mode (like tmux does)
- c-o is the prefix for tumu commands

Sheet border
------------

The sheets of a stack are divided by a border line: one line per sheet, along
the stack's axis, reading from the corner outwards.

Vertically stacked, the border is the sheet's **top edge** -- a corner, the
command label (focused sheet only), the title the child last set, then rule to
the edge:

```
focused:  └ [C-q ...] TITLE ───────
others:   └ TITLE ─────────────────
```

Horizontally stacked, it is the sheet's **left edge**, the same text written
*down* its column, one character per row, so it reads top to bottom the way the
other one reads left to right.

Which edge it is follows the terminal's shape, not a setting, so a rotation
flips the border along with the layout. The label `[C-q ...]` is drawn only on
the focused sheet, so which sheet the keyboard is aimed at is visible in the
frame alone.

Keys
----

The prefix is a latch, in the tmux style: press it, then one more key.

> The prefix is `c-q` while tumu is under development, because `c-o` is also
tmux's own prefix. It returns to `c-o` later; the key is written in one place
(`src/input.rs`).

- `prefix n` -- new sheet on top
- `prefix x` -- close the focused sheet
- `prefix o` -- move the focused sheet to the bottom
- `prefix z` -- show the focused sheet full screen, and back
- `prefix l` -- open the focused sheet's scrollback in an editor (see below)
- `prefix q` -- quit
- any other key -- sent to the focused sheet
- `prefix prefix` -- send the prefix itself to the focused sheet (send-prefix)

The focused sheet is the sheet the commands act on and the one the keyboard is
sent to. A new sheet goes on top and takes the focus; `prefix o` and
`prefix x` move it to the sheet that takes the focused one's place, so the
stack comes up under the keyboard rather than the keyboard moving away.

`prefix z` is a view, not a rearrangement: the focused sheet fills the
terminal while the others keep their places, and clearing the flag brings the
same layout straight back.

Copy mode (PoC)
---------------

`prefix l` is the first step of a tmux-like copy mode, kept simple for now: the
focused sheet's scrollback -- the history the emulator has pushed off the top,
followed by the rows still on screen -- is joined into text, written to a
temporary file, and handed to an external editor as `kk --tail <path>` (see
`../kk`). The `--tail` flag opens the file at its end, so the latest scrollback
is what the editor shows first; the editor opens the file the way it would open
any file named on its command line, and what it does with the terminal
afterwards is its own business. Selecting and copying live in the editor for
now; tumu only collects the text and opens it.

The temporary file lives only while the editor runs: it is created under
`$TMPDIR` (or `/tmp`) and removed as soon as the editor exits. A crash of tumu
itself could leave one behind, which is left to the OS to collect.

To hand the terminal over, tumu drops its `tuinix::TerminalDriver` (restoring
raw mode, the alternate screen, and the cursor), runs the editor, and builds a
fresh driver when the editor exits. The screen is repainted from scratch
afterwards, because the editor painted over it.

The editor program is fixed to `kk` by default and can be changed with
`$TUMU_EDITOR`; it is always launched as `<program> --tail <path>`, so the
program must accept `--tail` and a file path.

Menu
----

When the prefix (default to C-q) is pressed, or the `[C-q ...]` label is
clicked, an overlay menu window is shown on the center of the terminal:

```
   ┌────────────────┐
   │ n    new sheet │
   │ x    ...       │
   | o    ...       |
   | ...            |
   | C-q  send prefix
   └────────────────┘
```

The menu lists every prefix chord. Its rows come from the same table the keys
are resolved from, so a chord cannot be bound without appearing in the menu, or
listed without being bound. A command is run by pressing its key or by clicking
anywhere on its row. If an unbound key is pressed, or a region other than a
command row is clicked, the menu is closed and nothing else happens.

The menu is a view, like full screen: it changes no sheet's region and leaves
the stack alone. When the terminal is too small for the box, no menu is shown
at all, so a click never lands on one that was not drawn.

Mouse
-----

- click a sheet -- focus it
- click the focused sheet's border line -- open the menu
- click a menu row -- run that command

A click moves the focus to the sheet it landed on; it changes nothing else. The
keyboard follows, so `prefix x` and `prefix o` act on the sheet just clicked.
A click on a border line opens the menu without moving the focus, and a sheet
hidden behind a full-screen sheet cannot be clicked at all: what is not on
screen is not there to point at.

Only the left button does something so far. Nothing is forwarded to the
programs running in the sheets -- a click belongs to tumu -- so a fuller mouse
(giving a sheet its own report when it wants one) is still open.
