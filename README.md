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
  - Stacking axis set by `prefix r` or `--orientation`
  - No server (a standalone process)
  - Mouse: click a sheet to focus it; a sheet's program gets the mouse when it
  asks for it
  - tmux like copy-mode (PoC)
  - Bounded scrollback, set by `--scrollback` or `$TUMU_SCROLLBACK`
  - Prefix key character, set by `--prefix` or `$TUMU_PREFIX`

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
  - odds rate can be changes. but the sheet sizes are calculated automatically, so a
    resize (a rotation, most of all) re-lays the stack out rather than moving it
- a sheet can be swaped to the top, or, move to the stop
- support full screen mode (like tmux does)
- the stacking axis is a setting, not a guess: `--orientation vertical|horizontal`
  (or `$TUMU_ORIENTATION`) picks it at startup and `prefix r` flips it
- a prefix chord starts tumu commands (see `Keys` below)

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

The axis is a setting (`prefix r`, or `--orientation` at startup), so the border
follows it: flipping the axis moves the line from a sheet's bottom edge to its
right edge and back. A device rotation does **not** flip it -- a resize re-lays
the sheets out along the axis they already have, so the arrangement only changes
when it is asked to. The label `[C-q ...]` is drawn only on the focused sheet, so
which sheet the keyboard is aimed at is visible in the frame alone.

A sheet's content is the region above its border (or left of it, horizontally):
the border line is the sheet's own, not the child's, so the child's grid is one
row (or column) shorter than the sheet. The one exception is the **full-screen**
sheet (`prefix z`): it has no neighbour to be closed off from, so it draws no
border at all and its child's grid fills the whole terminal.

Shell
-----

Every sheet runs the same program: a shell, started as a login shell. Which
program it is can be set at startup, with the flag taking precedence over the
environment:

- `--shell PATH`
- `$SHELL`

The default is `/bin/sh`, used when the flag is absent and `$SHELL` is unset or
empty. The value is a **path**, not a short name, so the flag points tumu at a
specific program -- a shell, or a script that stands in for one. The `-l` is
always passed, so a program that does not understand it has to ignore it.

`$SHELL` keeps its usual name rather than a `$TUMU_SHELL` twin, because a shell
is what it names everywhere else, so there is nothing new to learn. There is no
`prefix` command for it either: the program is fixed at startup, because every
sheet runs it and a mid-run change would either miss the sheets already up or
replace a shell a user is working in.

Orientation
-----------

Which way the sheets stack is a setting, not something derived from the
terminal's shape:

- `--orientation ORIENTATION` (or `-O ORIENTATION`)
- `$TUMU_ORIENTATION`

The value is `vertical` (sheets top to bottom, the default) or `horizontal`
(sheets left to right). The flag wins over the environment, the environment over
the default, exactly as for `--scrollback` and `--prefix`. A value that is
neither word is an error rather than a silent fallback.

`prefix r` flips the axis while tumu runs. It is a *layout* change, not a view
one: every sheet's region takes a different shape, so the children are resized
to match.

The axis used to be derived from the terminal's size, so rotating the device
flipped the stack by itself. That is gone: the shape is a poor proxy for the
intent (a phone in landscape and a laptop in landscape are the same shape and
want different stacks), and rearranging the sheets because the device turned is
surprising rather than helpful. A resize now re-lays the stack out along the axis
it already has, and the axis changes only when asked.

Keys
----

The prefix is a latch, in the tmux style: press it, then one more key. The
prefix is `Ctrl` plus a character; the `Ctrl` is fixed and only the character is
a setting:

- `--prefix CHAR` (or `-p CHAR`)
- `$TUMU_PREFIX`

The default is `C-q`. `c-o` is the prefix tumu aims at, but `c-o` is also tmux's
own prefix, so the default waits until nested use is a non-issue -- set `c-o`
now with `--prefix o`. The flag wins over the environment, the environment over
the default, exactly as for `--scrollback`. A value that is not a single
character (for example `--prefix C-o`) is an error rather than a silent
fallback: the `Ctrl` is fixed, so the value is the character alone (`o`).

Because the prefix is `Ctrl+<char>` and every chord is an unmodified `<char>`,
the two never collide: any character may be the prefix, even one a chord uses,
and the setting needs no check against the binding table.

- `prefix c` -- new sheet on top
- `prefix x` -- close the focused sheet
- `prefix p` -- move the focus up (vertical) / left (horizontal)
- `prefix n` -- move the focus down (vertical) / right (horizontal)
- `prefix o` -- move the focused sheet to the bottom
- `prefix r` -- flip the stacking axis (vertical / horizontal)
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

`prefix n` and `prefix p` move the focus one place along the stack, in the
direction the stack advances: `n` is *next* (down when the sheets run down the
screen, right when they run across it) and `p` is *previous* (up / left). The
step does not wrap -- at either end of the stack it does nothing -- and it moves
only the keyboard: the stack keeps its order, so nothing is resized. `c` is
*create* rather than the more usual `n` for a new sheet, because `n` is the step
downwards.

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

When the prefix (default to C-q) is pressed, an overlay menu window is shown on
the center of the terminal:

```
   ┌───────────────────┐
   │ c    new sheet    │
   │ x    ...          │
   | p    ...          |
   | n    ...          |
   | ...               |
   | C-q  send prefix  |
   └───────────────────┘
```

The menu lists every prefix chord. Its rows come from the same table the keys
are resolved from, so a chord cannot be bound without appearing in the menu, or
listed without being bound. A command is run by pressing its key. If an unbound
key is pressed, the menu is closed and nothing else happens.

The menu is chosen with the keyboard: a click is not a selection, here or
anywhere else in tumu, so a click on a row is a click on the sheet under the box
and leaves the menu up.

The menu is a view, like full screen: it changes no sheet's region and leaves
the stack alone. When the terminal is too small for the box, no menu is shown
at all, so nothing can be typed at a menu that was not drawn.

Mouse
-----

The mouse has one owner at a time, and which one is the focused sheet's child:

- A sheet whose program has **not** asked for the mouse gets no reports at all.
  There, a click is tumu's, and the one thing it does is move the focus to the
  sheet it landed on.
- A sheet whose program **has** asked (a pager, an editor, `tmux` inside a sheet)
  gets every report that lands in its own region, and such a report moves no
  focus: a program tracking the mouse is tracking it inside its own grid, so a
  click on the sheet it runs in is a click on *it*, not a request to go
  elsewhere. The report is clipped into the child's grid, and its coordinates are
  the child's own, not the screen's.
- A report that lands **outside** the focused sheet's region is tumu's even while
  that child is asking: the child's grid is the whole of what it can have a
  report about, so a click on a neighbour is a click on tumu's chrome, and it
  moves the focus there. That is how the user leaves a tracking program -- point
  at the sheet they want and click.

The switch is the program's own mode, the one it sets with a DECSET escape
sequence; tumu only reads it back and follows it. Nothing is configured, and the
host terminal's mouse reporting never changes -- what changes is what tumu does
with a report. The switch is read from the *focused* sheet, because the mouse
follows the focus the way the keyboard does: there is one pointer and one focused
sheet.

Dragging works: tumu remembers which button it forwarded a press for, so a drag
names the button the child is holding. A drag that began on a tracking child keeps
going to it even if it wanders outside the region -- the gesture started on the
program and has to stay with it. A click that only moved the focus leaves nothing
behind for a later drag to continue.

Only the left button moves the focus, and only a press at that. A click runs no
command: the [menu](#menu) is opened by the prefix and chosen with a key, so a
click on a menu row is a click on the sheet under the box.

Known issues
------------

- A sheet whose `prefix l` editor fails to launch (a `$TUMU_EDITOR` that is
  missing, not executable, or does not accept `--tail`) stays in the stack as an
  empty sheet and cannot be closed by typing to it, because it has no child to
  type to. It is closed like any other sheet, with `prefix x`.
- Only the *focused* sheet's reports are forwarded. A background sheet gets
  nothing while it is not focused -- the same as the keyboard -- even if its
  child asked for the mouse, so a tracking program in a background sheet does not
  see a drag that started there. The mode is not remembered per sheet: leaving
  and returning re-reads it from the child, which still has it set.

