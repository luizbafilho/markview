# markview verification map

This directory is the maintained source for verifying markview's user-facing behavior. Read this index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- Run everything from the repo root with `V=.claude/skills/verify/scripts/mv-verify`.
- `$V build` succeeded and `$V doctor <run>` reports `binary: up to date with src/`.
- The session runs on Hyprland with kitty installed. `doctor` reports `window: on the hidden output`.
- Every run uses a fresh run id and its own kitty window. Never drive a markview the user started.
- `sample.md` at the repo root is the baseline document. Fixtures go in `target/verify/<run>/`.

## Driving conventions

- Start each recipe from a fresh `launch` unless it says otherwise. Run ids in recipes are examples. Use a fresh id for every run.
- In kitty, send keys with `$V kitty <run> send-text <key>` or `send-key <name>`, then `$V wait` for a named screen change. Ghostty has no remote control. Use `$V key` and `$V shot` there, and prove text claims with screenshots.
- Use the status bar (` <file>  N%  ·`) as the scroll handle and `markview exited N` as the exit handle.
- Change the terminal theme only through `$V kitty <run> set-colors`, the same channel a user's theme switcher uses.
- Run `$V cleanup <run>` at the end, including after failures. Cleanup keeps evidence.

## Proof and skip reporting

- Tag every failure `app` or `upstream: <crate> <version>` per `../ownership.md`.
- Text claims get `$V text` artifacts. Graphical claims (heading size, images, colors) get `$V shot` artifacts, which you open and look at.
- Capture the state before and after each action, not only the final screen.
- CLI claims include the command, stderr, and exit code.
- Name the run id, terminal, and terminal size from `doctor` with every artifact.
- If an entry point can't be reached (no Hyprland, no kitty), report the attempted command and the missing precondition. Do not report it as verified through another path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then has exactly four H2 sections in this order: `Sub-features`, `How to get to it (user POV)`, `Driving it with mv-verify`, `Gotchas`. The driving section starts with `Preconditions:`, then pairs each user action with an exact command and an observable result.

## Features

- [Launch and exit](./launch.md) covers opening a file, argument errors, quitting, and restoring the terminal.
- [Navigation](./navigation.md) covers line, page, and jump keys, the mouse wheel, and the status bar percentage.
- [Document rendering](./render.md) covers sized headings, bordered tables, task lists, blockquotes, inline styles, and highlighted code.
- [Mermaid diagrams](./diagrams.md) covers flowcharts, sequence diagrams, Gantt charts, and pie charts drawn in text.
- [Images](./images.md) covers inline images, relative paths, the missing-image fallback, and cropping while scrolling.
- [Theme following](./theme.md) covers light or dark at startup and repainting when the terminal's background changes.
- [Ghostty](./ghostty.md) covers heading images in the Ghostty config's font, the `MARKVIEW_HEADING_FONT` override, images, and keys, all through screenshots.
