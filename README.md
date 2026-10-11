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
  - Bounded scrollback, set by `--scrollback` or `$TUMU_SCROLLBACK`

Stacked sheets
--------------

Vertically stacked:

```
|                |
| sheet 1        |
| $              |
|└ [C-q ...] s1  |
| sheet 2        |
| $              |
|└ s2            |
```

Horizontally stacked:

```
sheet 1      |   | sheet 2 |
             |└  |   └     |
$            |[C |   s2    |
             | - |         |
             | q |         |
             | . |         |
             | . |         |
             | . |         |
             | ] |         |
             | s |         |
             | 1 |         |
             | ─ |         |
             | ─ |         |
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

Vertically stacked, the border is the sheet's **bottom edge** -- a corner, the
command label (focused sheet only), the title the child last set, then rule to
the edge:

```
focused:  └ [C-q ...] TITLE ───────
others:   └ TITLE ─────────────────
```

Horizontally stacked, it is the sheet's **right edge**, the same text written
*down* its column, one character per row, so it reads top to bottom the way the
other one reads left to right.

Which edge it is follows from where a sheet ends along the axis: the line is the
one a sheet closes on, so it sits on the edge the sheet advances towards.
Vertically the sheets stack down the screen, so a sheet's line runs along its
**bottom** edge; horizontally they stack across it, so it runs along the
**right** edge. Either way the sheet after it in the stack starts on the far
side of that line, and each sheet keeps its own line and its own title.

The axis is not a setting, so a rotation flips the border with the layout. The
label `[C-q ...]` is drawn only on the focused sheet, so which sheet the keyboard
is aimed at is visible in the frame alone.

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
- `prefix y` -- paste the clipboard into the focused sheet (see below)
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
followed by the rows still on screen -- is joined into text and opened in an
external editor. The editor runs in a **sheet of its own**, pushed on top like
any new sheet: tumu writes the text to a temporary file and spawns the editor
as that sheet's session, so the editor is painted like any other child and the
editor's exit closes its sheet through the ordinary session-exit path.

Nothing is handed over. The terminal driver stays up, so the frames of the
other sheets keep their meaning and the screen is never repainted from scratch.
The temporary file is removed when the editor exits.

The editor program is fixed to `kk` by default and can be changed with
`$TUMU_EDITOR`; it is always launched as `<program> --tail <path>`, so the
program must accept `--tail` and open the file it names. `--tail` opens at the
end, so the latest scrollback is what the editor shows first; what it does with
the file afterwards is its own business, and selecting and copying live in the
editor for now.

### Scrollback

A sheet's scrollback is bounded, because the emulator keeps none of its own.
Each session is trimmed to a line limit as it runs, so a sheet that prints for
days does not grow without end, and the file `prefix l` opens in the editor is
only as long as the bound allows. The default is 1000 lines; the earliest lines
are dropped first, so the newest history always survives.

The bound can be set two ways, with the command line taking precedence over the
environment:

- `--scrollback LINES` (or `-s LINES`)
- `$TUMU_SCROLLBACK`

A limit of `0` means "keep no history", not "keep everything": the sheet shows
its screen and nothing above it.

### Clipboard

A child that copies does it with OSC 52, which asks the *terminal* to change a
selection. tumu is that terminal for its children, so it answers the ask itself
for the one thing it can reach: it forwards the text to the host terminal's
clipboard and keeps a copy of the last one. `prefix y` then pastes that same
text back into the focused sheet, as a bracketed paste when the child asked for
one.

The copy is the *last* one, not a history, and until a child has copied there is
nothing to paste, so `prefix y` does nothing. The copy is remembered as the
bytes the child asked for, so a paste puts back exactly what was copied, even
when the host write was lossy or was ignored -- OSC 52 has no reply, so whether
the host took the text is not observable.

### Future: an embedded editor (memo)

The external-editor sheet above is a stopgap. Running `kk` as a child in its own
sheet removed the whole-terminal handoff, but the editor still gets a whole
sheet to itself; a copy mode that shared the focused sheet's own region would be
lighter still.

That needs `kk` used as a *library* instead of as a child process, which its
design already allows: `kk`'s core is Sans I/O, so `State` plus the `render_*`
functions paint a `tuinix::Frame` directly, and tumu is already building one
frame per sheet. An embedded copy mode would:

- build a `kk::State` over the focused sheet's scrollback text (`kk::TextBuffer`
  over the same rows the edge's `scrollback_text` collects today),
- route keys and mouse to it while it is open (`kk::Mode::resolve` -> the
  matching `State::handle_*`), and
- paint it into a `tuinix::Frame` the size of the sheet's *content* region and
  `put_frame` that into tumu's frame, the same way `kk`'s own edge composes its
  sub-frames.

Then the editor shares the sheet's region exactly -- full screen when the sheet
is, a pane of the stack when it is not -- with no terminal handover, no
`tuinix::TerminalDriver` drop/rebuild, and no from-scratch repaint. `C-q` would
end copy mode, as it already quits `kk`, and it is not tumu's prefix.

What is missing is not the library, which already works, but an API shaped for
embedding. `kk`'s edge (`kk::app`) currently owns the pieces an embedder needs
but cannot reach: the order the renderers are called in, the region maths, the
legend/cursor arbitration, `State::adjust_viewport`, and the key-to-`Action`
dispatch. Pulling those down into `kk::State` -- roughly a `render(&mut self,
&mut tuinix::Frame, ..)` that draws a whole view and an `handle_input(&mut self,
&tuinix::Input, &mut Mode)` that returns "continue or quit" -- would let tumu
embed `kk` cleanly, and would leave `kk`'s own edge as a thin caller of the very
same methods. That is a small change to `kk` in cooperation with it, not a
rewrite, and it is deferred for now.

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

Known issues
------------

- A sheet whose `prefix l` editor fails to launch (a `$TUMU_EDITOR` that is
  missing, not executable, or does not accept `--tail`) stays in the stack as an
  empty sheet and cannot be closed by typing to it, because it has no child to
  type to. It is closed like any other sheet, with `prefix x`.

