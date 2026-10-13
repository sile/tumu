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
  - Tool sheets: run the diff / grep browser in a sheet of its own
  - Stacking axis set by `prefix r` or `--orientation`
  - Split odds set by `--ratio`, moved at run time with `prefix +` / `prefix -`
  - No server (a standalone process)
  - Mouse: click a sheet to focus it; a sheet's program gets the mouse when it
  asks for it, and the wheel always goes to the focused sheet
  - tmux like copy-mode (PoC): `prefix l`, or a scroll down when the sheet's
  program is not using the mouse
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
  - the odds are a setting (`--ratio`, or `prefix +` / `prefix -` at run time),
    but the sheet sizes are calculated from them, so a
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

Tool sheets
-----------

A sheet does not have to run the shell. Two chords open a sheet whose session is
an external tool instead:

- `prefix d` -- the diff browser (`mamediff`)
- `prefix s` -- the grep browser (`mamegrep`)

The sheet is a sheet like any other: it is pushed on top, takes the focus, and
is painted by tumu, so the tool is a child of tumu and its exit closes the sheet
through the ordinary session-exit path. Nothing is handed over, exactly as for
the editor sheet `prefix l` opens.

The tool is a child, not a program tumu knows how to speak to: no pattern or
revision is passed in, so the tool starts on its own prompt and the search or the
diff is chosen inside it. Where it looks is where the sheet is started -- the
focused sheet's directory, like any new sheet (see
[Where a new sheet starts](#where-a-new-sheet-starts)) -- so opening it from a
shell that has `cd`-ed into a repository reads that repository.

The program is configurable, so a user (or a test) can point the chords at
another tool without a rebuild:

- `$TUMU_MAMEDIFF` (default `mamediff`)
- `$TUMU_MAMEGREP` (default `mamegrep`)

The two names sit alongside `$TUMU_EDITOR` and are read the same way. There is no
`--` flag for them and no `prefix` command: they are tools, not settings of the
multiplexer, and a run is expected to reach for the same ones every time.

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

Split odds
----------

Every sheet's size comes from a single proportion: at each step the sheet at
hand takes `N` parts and every sheet below it together takes `1`. `N` starts at
`1` (the halving the examples above show) and is set at startup:

- `--ratio N` (or `-r N`)
- `$TUMU_RATIO`

The flag wins over the environment, the environment over the default (`1`),
exactly as for `--scrollback`. The value is a single whole number -- the `N` of
the `N : 1` odds, not a `N:D` pair -- because the other side is fixed at `1`:
the sheet at hand is measured against everything below it, and a finer shape is
neither needed nor easier to reason about. A value out of range is clamped into
`1..=5` rather than rejected, so `--ratio 99` starts at the widest share instead
of refusing to run.

`prefix +` and `prefix -` move `N` by one part while tumu runs: `+` widens the
sheets at hand, `-` narrows them. The bounds are the same `1..=5`, so at either
end the step is a no-op rather than a share that grows without end or a sheet
squeezed away.

It is a *layout* change, not a view one: every region is derived from the odds,
so the children are resized to match -- the same shape `prefix r` has. And like
the axis it is only the *shape* that is recorded, not any rectangle: a resize
re-derives every region from the odds and the size, so the ratio survives a
rotation and a terminal change unchanged.

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

- `prefix c` -- new sheet on top (runs the shell)
- `prefix d` -- new sheet running the diff browser (`mamediff`)
- `prefix s` -- new sheet running the grep browser (`mamegrep`)
- `prefix x` -- open the extended menu (see below)
- `prefix p` -- move the focus up (vertical) / left (horizontal)
- `prefix n` -- move the focus down (vertical) / right (horizontal)
- `prefix t` -- move the focused sheet to the top of the stack
- `prefix +` -- widen the sheets at hand (a wider share of the split)
- `prefix -` -- narrow the sheets at hand
- `prefix r` -- flip the stacking axis (vertical / horizontal)
- `prefix z` -- show the focused sheet full screen, and back
- `prefix l` -- open the focused sheet's scrollback in an editor (see below)
- `prefix y` -- paste the clipboard into the focused sheet (see below)
- any other key -- sent to the focused sheet
- `prefix prefix` -- send the prefix itself to the focused sheet (send-prefix)

### The extended menu

Two of the commands cannot be taken back: closing a sheet ends the child running
in it, and quitting ends everything. They are one step removed from the prefix,
so a single mistyped key cannot run one. `prefix x` opens the **extended menu**,
a second box with its own chords:

- `k` -- close the focused sheet (`k` for *kill*)
- `q` -- quit

The menu is modal: while it is open the prefix is inert and a key is one of the
menu's chords or nothing. One key closes it, the way the prefix latch does, so
a key the menu does not name is a "no" that dismisses it -- never a command.
That is the whole point: the user reads a row and picks, and a stray press
costs a dismissal rather than a sheet.

A menu rather than a yes/no prompt, because a prompt asks the *same* question
every time and can be answered by muscle memory, while a menu makes the user
read a different row before choosing. It is the shape `kk`'s ext mode has. The
menu is shown only when the box fits, so on a very small screen these commands
are unreachable rather than offered unreadably.

The focused sheet is the sheet the commands act on and the one the keyboard is
sent to. A new sheet goes on top and takes the focus; closing a sheet through
the extended menu moves the focus to the sheet that takes the focused one's
place, so the stack comes up under the keyboard rather than the keyboard moving
away.

`prefix n` and `prefix p` move the focus one place along the stack, in the
direction the stack advances: `n` is *next* (down when the sheets run down the
screen, right when they run across it) and `p` is *previous* (up / left). The
step wraps -- from either end of the stack it comes around to the other, so
repeated steps cycle through every sheet -- and it moves only the keyboard: the
stack keeps its order, so nothing is resized. `c` is *create* rather than the
more usual `n` for a new sheet, because `n` is the step downwards.

`prefix t` moves the focused sheet itself to the top of the stack: it is lifted
out and put back first, the sheets that were above it closing ranks behind it.
Unlike a focus step this reorders the stack, so the sessions are resized -- but
the keyboard does not move, because the sheet that moved keeps it. That is the
point: it pins the sheet in hand where new sheets appear, so `prefix t` then
`prefix c` stacks the next sheets under a stable one. A sheet already on top has
nowhere to go, so `prefix t` on it does nothing.

`prefix z` is a view, not a rearrangement: the focused sheet fills the
terminal while the others keep their places, and clearing the flag brings the
same layout straight back.

What a sheet shows when the layout changes
------------------------------------------

A sheet's region is its own little terminal, and the session behind it is a
child with its own size. When the stack is re-laid out -- a new sheet opens, one
closes, `prefix t` moves one, the axis or the odds change, the terminal itself
resizes -- every region takes a new shape and the children are resized to match
(`SIGWINCH` to each). A child that keeps a screen full of text repaints itself at
the new size, so what it drew last is still there.

The session's grid and the region rarely agree at the instant of a change: the
child has not repainted yet, or it never will (a program that printed a log and
went quiet). A sheet shows the *newest* lines of its content -- the tail of the
scrollback first, then the bottom of the visible grid -- anchored to the bottom
of the region. So shrinking a region does not blank it: the lines that no longer
fit fall off the top into history rather than being lost, and growing one fills
the new space from history. This is the tmux behavior of redrawing from
scrollback, without re-wrapping: a line keeps the width it had when it was
pushed off, so a row that was wider or narrower than the region today is clipped
or padded rather than re-flowed.

Where a new sheet starts
------------------------

A new sheet starts in the directory of the sheet that was focused when it was
asked for, so a shell that has `cd`-ed into a project opens the next sheet --
the shell, a tool sheet, the editor -- where the user already is, not where tumu
was launched.

The directory lives in the focused child's own process, not in tumu, so tumu
reads it out of the running child: the child is a session leader, so its pid is
its process group id, and `/proc/<pid>/cwd` is where it stands. Nothing is asked
of the child, so a shell needs no cooperation for this and a program that does
not report its directory still works.

This is best-effort. The first sheet of a run has nothing to inherit from, so it
starts where tumu was launched, and every failure to read the directory -- no
`/proc`, a child already gone, a child that changed directory between the read
and the spawn -- falls back to the same place rather than refusing to open the
sheet. A sheet that starts one directory off is a smaller surprise than one that
will not open.

Copy mode (PoC)
---------------

The focused sheet's scrollback -- the history the emulator has pushed off the
top, followed by the rows still on screen -- is joined into text and opened in
an external editor. The editor runs in a **sheet of its own**, pushed on top
like any new sheet: tumu writes the text to a temporary file and spawns the
editor as that sheet's session, so the editor is painted like any other child
and the editor's exit closes its sheet through the ordinary session-exit path.

There are two doors into it, and they open onto the same room:

- `prefix l` -- the key one, always there.
- a **scroll up** (a wheel turn away from the user), when the focused sheet's
  child has not asked for the mouse -- the one a hand reaches for without
  leaving the pointer.

A wheel turn is only tumu's when the child did not ask for the mouse (see
[Mouse](#mouse)): a pager that wanted the mouse pages on the turn, and the
history is not opened on top of it. When nothing is asking there is nothing to
page, and "scroll up past the top" reads as "show me the history", which
is the editor sheet. Only *up* opens it -- a down turn has no such reading
(there is no "below the bottom" to go to), so it is dropped like any other report
with nothing behind it.

Opening it moves the focus to the editor sheet, which is what keeps a wheel
*gesture* from opening a sheet per turn: the first turn opens the history and
the keyboard follows the editor in, so the turns after it are the editor's own
(it asked for the mouse) and scroll it instead of stacking another history on
top of it.

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
days does not grow without end, and the file the editor is opened on is only as
long as the bound allows. The default is 1000 lines; the earliest lines are
dropped first, so the newest history always survives.

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
   │ d    diff sheet   │
   │ s    grep sheet   │
   │ x    ext menu     │
   | p    ...          |
   | n    ...          |
   | t    ...          |
   | +    ...          |
   | -    ...          |
   | ...               |
   | C-q  send prefix  |
   └───────────────────┘
```

The menu lists every prefix chord. Its rows come from the same table the keys
are resolved from, so a chord cannot be bound without appearing in the menu, or
listed without being bound. A command is run by pressing its key. If an unbound
key is pressed, the menu is closed and nothing else happens.

The one row that is not a chord is `send prefix`: it stands for the prefix
pressed twice, which sends the prefix itself to the focused sheet. The extended
menu is the same box built from its own, shorter table; see
*The extended menu* above.

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

The **wheel** is the one report that ignores the region: a turn goes to the
focused sheet's child wherever on the screen it happens. A wheel has no position
in the child's world -- a pager scrolls by the line and does not read which cell
the pointer was over -- so clipping the position into the grid and handing it on
is all the routing there is. Routing it by region instead would drop a turn
whenever the pointer was not on the focused sheet, which on a small screen is
most of the time, and the wheel would stop working. A turn moves no focus: the
pointer says where the turn happened, not which sheet the user wants next, and a
scroll that moved the focus would move the sheet out from under the reader.

A turn with no child asking is not dropped: a scroll **up** opens the focused
sheet's history in an editor sheet, the same command `prefix l` runs (see
[Copy mode](#copy-mode-poc)). It is the one thing tumu does behind the wheel --
there is no other command there -- and only *up* does it: a down turn has no
"below the bottom" to mean, so it is dropped like any other report with nothing
behind it. Opening it moves the focus to the editor sheet, so a wheel gesture
opens one history and then scrolls the editor, rather than opening a sheet per
turn.

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

- A sheet whose program fails to launch -- a `$TUMU_EDITOR` that is missing or
  does not accept `--tail`, or a `$TUMU_MAMEDIFF` / `$TUMU_MAMEGREP` that is not
  installed -- stays in the stack as an empty sheet and cannot be closed by
  typing to it, because it has no child to type to. It is closed like any other
  sheet, with `prefix x` then `k`.
- Only the *focused* sheet's reports are forwarded. A background sheet gets
  nothing while it is not focused -- the same as the keyboard -- even if its
  child asked for the mouse, so a tracking program in a background sheet does not
  see a drag that started there. The mode is not remembered per sheet: leaving
  and returning re-reads it from the child, which still has it set.

