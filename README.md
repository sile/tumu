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
  - tmux like copy-mode

Stacked sheets
--------------

Image (vertical stacked).

```
|           |
|           |
| sheet 1   |
|           |
|           |
| $         |
-------------
| sheet 2 (show last line)
|- pwd / proc name
| sheet 3   |
-------------
| ...       |
-------------
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
- `prefix q` -- quit
- any other key -- sent to the focused sheet
- `prefix prefix` -- send the prefix itself to the focused sheet (send-prefix)

The focused sheet is the sheet the commands act on and the one the keyboard is
sent to. A new sheet goes on top and takes the focus; `prefix o` and
`prefix x` move it to the sheet that takes the focused one's place, so the
stack comes up under the keyboard rather than the keyboard moving away.

`prefix z` is a view, not a rearrangement: the focused sheet fills the
terminal while the others keep their places, and clearing the flag brings the
same layout straight back. The title row marks the focused sheet with a `>`
after the index, so both of these are visible in the frame alone.

Mouse
-----

- click a sheet -- focus it

A click moves the focus to the sheet it landed on; it changes nothing else. The
keyboard follows, so `prefix x` and `prefix o` act on the sheet just clicked.
A sheet hidden behind a full-screen sheet cannot be clicked: what is not on
screen is not there to point at.

Only the left button does something so far. Nothing is forwarded to the
programs running in the sheets -- a click belongs to tumu -- so a fuller mouse
(giving a sheet its own report when it wants one) is still open.
