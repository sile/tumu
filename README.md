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
