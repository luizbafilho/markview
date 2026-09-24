---
name: verify
description: Drive markview (the Rust terminal Markdown viewer in this repo) in a hidden kitty or Ghostty window and capture screen text and screenshots as proof. Use to verify a change to rendering, navigation, images, Mermaid diagrams, theme following or Ghostty heading fonts before calling it done, or to reproduce a reported markview bug.
---

# Verify markview

markview is a full-screen TUI. It queries the terminal at startup (background color, graphics protocol, text sizing), then shows a pager with a status bar. Proof comes from the real binary running in a real terminal. Unit tests are not proof here.

Each run gets its own headless Hyprland output (`mv-verify-<run>`, 1920x1080 at scale 1) with the terminal fullscreen on it. The output sits left of every real monitor with a gap, so the user never sees the window and the pointer can't reach it. The terminal opens with `silent`, so the user's focus doesn't move. It still renders every frame, so screenshots are real pixels.

Everything goes through one helper, run from the repo root:

```sh
V=.claude/skills/verify/scripts/mv-verify
$V help
```

It needs a running Hyprland session (0.56 or later, for `hyprctl eval` and Lua dispatch), plus `kitty`, `grim`, `jq`, `magick`, `git`, and `cargo`. `ghostty` is needed for `--ghostty` runs.

## Pick the terminal

markview takes a different path in each terminal, so pick the one that exercises the code you changed.

- **kitty (default).** kitty 0.40+ sizes headings with the text sizing protocol. kitty has remote control, so you get screen text (`text`, `wait`), live color changes (`set-colors`), and mouse-wheel input. Use it for almost everything.
- **Ghostty (`--ghostty`).** Ghostty has no text sizing, so markview draws each heading as an image in the font from the user's Ghostty config (`src/heading_font/ghostty.rs`, `src/heading_image.rs`). Ghostty has no remote control, so a Ghostty run is screenshot-only: `key`, `settle`, and `shot` work, but `text`, `wait`, and `kitty` refuse. Use it when a change touches heading images, heading fonts, or anything Ghostty-specific. `features/ghostty.md` has the recipes.
markview does not support tmux, so there is no tmux mode. Don't verify or fix tmux behavior.

## Launch

```sh
$V build                                        # cargo build --locked, prints target/debug/markview
$V launch <run> sample.md                       # hidden output + kitty running markview
$V launch <run> sample.md --ghostty             # same, in Ghostty
$V launch <run> fixture.md --light              # the terminal starts with a Catppuccin Latte background
$V launch <run> sample.md --env=KEY=VALUE       # environment for the terminal and markview
```

Flags combine. Pick a fresh `<run>` id per run (`render-1`, `theme-2`). It must match `[a-z0-9-]+`. The id names the output (`mv-verify-<run>`), the control socket, and the evidence dir. markview runs from the file's directory, so relative image paths resolve the way they do for a user.

Hyprland starts the terminal, not your shell, so your shell's environment does not reach markview. Pass variables with `--env`, for example `--env=MARKVIEW_HEADING_FONT="Liberation Serif"` or `--env=XDG_CONFIG_HOME=<fixture dir>`.

`launch` returns once markview has drawn. In kitty, that means the status bar (` <file>  N%  ·  … q quit`) is on screen. In Ghostty, it means the screen has more than 50 colors and three screenshots in a row are identical. It fails after 15s otherwise.

When markview exits, the window stays open and prints `markview exited <code>`. The code is also written to `target/verify/<run>/exit-code`. The exact terminal command is in `target/verify/<run>/launch.sh`.

## Doctor

```sh
$V doctor <run>
```

Read-only. It reports whether the terminal is alive, the hidden output, the window's address and class and whether it sits on the output, the markview process, whether it has exited, the git rev at launch vs now, and whether `src/` is newer than the binary. For kitty it also checks the socket and prints the terminal size (174x43 cells by default). Run it first whenever something looks off. `binary: STALE` means rebuild and relaunch. `window: NOT on the hidden output` means screenshots are wrong, so clean up and relaunch.

## Drive

Any terminal:

```sh
$V key <run> j              # press a key, wait for the screen to settle, print changed or unchanged
$V key <run> g SHIFT        # G
$V key <run> Next           # PgDn; also Prior, Home, End, Down, Up, space, Return, Escape
$V settle <run>             # wait until three screenshots in a row are identical
```

`key` sends the key through Hyprland's `send_shortcut`, addressed to this run's window by address, so it never types into the user's focused window. A wrong target fails with `window not found` instead of falling back. Key names are xkb keysyms, and the key must exist in the user's keymap (F13 does not).

kitty only:

```sh
$V kitty <run> send-text j            # literal keys through kitty
$V kitty <run> send-text '\x1b[<65;10;10M'   # one mouse-wheel notch down
$V kitty <run> set-colors background='#eff1f5' foreground='#4c4f69'   # flip the terminal theme
$V wait <run> 'sample.md  100%'       # poll the screen text for a regex, 10s default
```

`kitty` passes the rest of the line to `kitty @ --to <socket>`, so any kitty remote-control command works. After each action, wait for a screen change you can name. Don't sleep for a fixed time. In kitty, the status bar percentage is the scroll handle and `markview exited N` is the exit handle. In Ghostty, `key` does the waiting, and the screenshot is the handle.

For CLI-only behavior (argument errors), run the binary directly with stdin from `/dev/null`. See `features/launch.md`.

## Evidence

```sh
$V text <run> <name>     # kitty only: target/verify/<run>/<name>.txt, the screen as text, also printed to stdout
$V shot <run> <name>     # target/verify/<run>/<name>.png, the window pixels
```

Proof standards:

- Drive the path a user takes: open a file, press keys, change the terminal's colors or config. Never call internal functions or edit state to reach a screen.
- Capture the action and the resulting state. For a scroll, that is the status percentage before and after plus the text that came into view. A final screenshot alone is not proof.
- Text proves content and layout (tables, wrapping, status bar, fallbacks, diagrams). Pixels prove anything graphical: heading size, heading font, images, colors, theme. `get-text` shows image cells as blank or as placeholder glyphs, so never claim an image or heading rendered from text alone. Open the PNG and look.
- `key` printing `changed` only means some pixels moved. Open the screenshot to see what changed.
- Record the exit code when quitting is part of the claim. Also capture the screen after exit: if it shows `markview exited 0` and none of the document, the alternate screen closed and the terminal was restored.
- Write fixtures into `target/verify/<run>/` rather than into the repo, and launch them from there.

Evidence lives in `target/verify/<run>/` (git-ignored with `target/`). Cleanup never touches it. `cargo clean` deletes it, so copy anything you need to keep.

## App or library

Much of what markview shows is produced by `ratatui-markdown`, which markview does not control. Tag every failure `app` or `upstream: <crate> <version>` before reporting it. `ownership.md` maps each stage to its owner, explains how to decide, and lists the known upstream issues. Don't work around a library bug in markview's code.

## Cleanup

```sh
$V cleanup <run>
```

It kills the terminal PID recorded at launch, waits for it to exit, removes the `mv-verify-<run>` output, and deletes the socket and pidfile. It never kills by process name, which matters most for Ghostty: the user's own Ghostty is running too. Run it after every run, failed ones included, so hidden outputs don't pile up. `hyprctl monitors -j | jq -r '.[].name'` lists any that leaked.

## Feature map

`features/README.md` indexes one recipe per user-facing feature. Read the matching file before driving. A proof that covers one entry point is incomplete when the map lists others.

## Gotchas

- Only the helper may create or remove outputs. Never remove an output that isn't named `mv-verify-<run>` for your run.
- Never target keys at a Ghostty window by class. If Ghostty ever ignores `--class`, its windows share `com.mitchellh.ghostty` with the user's terminals, and a `q` would close one. The helper always targets by window address.
- Ghostty launches with `--gtk-single-instance=false`. Without it, the launch is handed to the user's running Ghostty process, and the recorded PID would be theirs.
- Run one launch at a time. Parallel runs can end up sharing a Hyprland workspace, and a window then drops off its hidden output, shrinks (85x42 cells instead of 174x43), and its screenshots show the wallpaper. Run `doctor` again before trusting a screenshot, and relaunch when it reports `NOT on the hidden output`.
- The user's waybar attaches to every new output, including the hidden one, and draws over the top 27px of the fullscreen window. That strip covers markview's blank top padding row, and also the `markview exited N` line after quitting. Ignore it in screenshots, prove the exit with the `exit-code` file, and don't touch the user's waybar config.
- A `hyprctl reload` drops the runtime rule that parks the output far away, and Hyprland may place it next to a real monitor. Clean up and relaunch after a reload.
- The user's own terminal config applies (font, background image). That is deliberate, because heading images are drawn in the configured font. Expect a watermark in kitty screenshots if the user has `background_image` set.
- Do not drive markview from a bare PTY or a detached multiplexer session. With no terminal answering its startup queries, markview blocks forever at the cursor-position probe after the graphics query, and the screen stays blank.
