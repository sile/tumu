tumu
====

"tumu (詰む)" is "stack" in Japanese.

- Terminal multiplexer
- Designed for
  - Termux
  - 6 inch display
  - Black and white e-ink display
- Feature
  - Stacked windows (see below)
  - No sessions, panes, pop-up windows
  - Detect device orientation changes (via terminal size change event)
  - No server (a standalone process)
  - tmux like copy-mode

Stacked windows
---------------

Image (vertical stacked).

```
|           |
|           |
| windows 1 |
|           |
|           |
| $         |
-------------
| window 2 (show last line)
|- pwd / proc name
| window 3  |
-------------
| ...       |
-------------
```

Horizontally stacked:
```
----------------|---|---|
window 1        |   |   |
                |w  |.  |
                |2  |.  |
                |   |.  |
$               |$  |   |
----------------|---|---|
```

- When new window is created, it stacked to the top of the stack.
- each window size is decided automatically
  - e.g., w1:remainings = 1:1, remaining1:remaining-others = 1:1, ....
  - odds rate can be changes. but the window sizes are calculated automatically to make it easy to support display rotation
- a window can be swaped to the top, or, move to the stop
- support full screen mode (like tmux does)
- c-o is the prefix for tumu commands
