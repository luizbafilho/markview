# Theme following

markview asks the terminal for its background color. A light background gets Catppuccin Latte and a dark one gets Frappé. While a document is open, it checks again after each second of idle time and repaints when the terminal switches between light and dark.

## Sub-features

- `theme-start-dark` renders Frappé when the terminal is dark at launch.
- `theme-start-light` renders Latte when the terminal is light at launch.
- `theme-live-switch` repaints within about a second of idle after the terminal's background flips, keeping the scroll position.

## How to get to it (user POV)

- Start markview in a dark or a light terminal.
- Switch the terminal's theme (a system appearance change, a kitty theme kitten) while a document is open.

## Driving it with mv-verify

Preconditions:

- The user's kitty config sets a dark background. If it is light, swap the expectations in the first two steps.

- **Dark at launch.** Run `$V launch theme-1 sample.md` and `$V shot theme-1 dark`. The PNG has a dark slate background, light text, and a yellow inline `search_v2`.
- **Light at launch.** Run `$V launch theme-2 sample.md --light` and `$V shot theme-2 light`. The PNG has a near-white background, dark text, and an inline `search_v2` in Latte yellow (`#df8e1d`, which reads as orange).
- **Live switch.** In `theme-1`, press `space` and record the status percentage. Run `$V kitty theme-1 set-colors background='#eff1f5' foreground='#4c4f69'`, keep the window idle for 3 seconds, then `$V shot theme-1 switched`. The PNG uses the Latte palette everywhere, including code highlighting, table borders, and the status bar. The status percentage is unchanged.
- **Switch back.** Run `$V kitty theme-1 set-colors --reset`, wait 3 seconds, and `$V shot theme-1 back`. The Frappé palette returns.
- **Proof.** `dark.png`, `light.png`, `switched.png`, and `back.png`, and the status line before and after the switch.

## Gotchas

- The check only runs while no input arrives for a full second. Sending keys during the wait delays the repaint.
- `set-colors` without `--all` changes only the run's window. Never touch the user's other kitty windows.
- A screenshot taken less than a second after `set-colors` can show kitty's new background under markview's old colors. Wait before capturing.
