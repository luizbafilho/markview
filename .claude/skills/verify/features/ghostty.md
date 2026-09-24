# Ghostty

In Ghostty, markview can't size headings with the text sizing protocol, so it draws each heading as an image in the font from the user's Ghostty config. It reads `font-family` and `font-family-bold` from `$XDG_CONFIG_HOME/ghostty/config` (or `config.ghostty`), follows `config-file` includes, and lets `MARKVIEW_HEADING_FONT` override the heading font. Images use the kitty graphics protocol, as in kitty.

## Sub-features

- `ghostty-heading-image` draws h1 to h3 larger than body text, with a rule under h1 and h2.
- `ghostty-heading-font` draws headings in the config's `font-family-bold`, or in `font-family` when no bold family is set.
- `ghostty-heading-env` draws headings in `MARKVIEW_HEADING_FONT` when it is set.
- `ghostty-image` draws inline images at full resolution.
- `ghostty-keys` scrolls and quits with the same keys as in kitty.

## How to get to it (user POV)

- Run `markview sample.md` in Ghostty.
- Change the Ghostty font in the config, open a new Ghostty window, and run it again.
- Run `MARKVIEW_HEADING_FONT="Some Font" markview sample.md`.

## Driving it with mv-verify

Preconditions:

- Ghostty is installed. Ghostty runs are screenshot-only, so every check below ends in `$V shot` and you look at the PNG.

- **Headings in the user's font.** Run `$V launch gh-1 sample.md --ghostty` and `$V shot gh-1 top`. `Search v2 launch plan` is about twice the body text height with a thin rule under it, in the same typeface as the body when the user's config sets only `font-family`.
- **Headings follow the config.** Write `target/verify/gh-2/xdg/ghostty/config` with the two lines `font-family = "JetBrainsMono Nerd Font"` and `font-family-bold = "Liberation Serif"`. Run `$V launch gh-2 sample.md --ghostty --env=XDG_CONFIG_HOME=$PWD/target/verify/gh-2/xdg` and `$V shot gh-2 serif`. `Search v2 launch plan`, `Milestones`, and `How a query runs` are in a serif face. Body text is still monospace.
- **Env override.** Run `$V launch gh-3 sample.md --ghostty --env=MARKVIEW_HEADING_FONT="Liberation Serif"` and `$V shot gh-3 env`. Headings are serif and body text keeps the user's font.
- **Scroll and image.** In `gh-1`, run `$V key gh-1 g SHIFT`. It prints `changed`. `$V shot gh-1 bottom` shows `Offsite`, the fjord photo in full color, and ` sample.md  100%` in the status bar.
- **Clamp.** At the bottom, `$V key gh-1 j` prints `unchanged`.
- **Quit.** Run `$V key gh-1 q`. `target/verify/gh-1/exit-code` holds `0`, and `$V doctor gh-1` reports `markview: exited 0`. `$V shot gh-1 quit` shows none of the document. The `markview exited 0` line sits on the top row, under the waybar strip, so the screenshot can't show it.
- **Proof.** `top.png`, `bottom.png`, `quit.png`, and `exit-code` from `gh-1`, `serif.png` from `gh-2`, and `env.png` from `gh-3`.

## Gotchas

- `XDG_CONFIG_HOME` changes Ghostty's own config too, so the fixture must set `font-family` or the body falls back to Ghostty's default font. That is also what a user would see.
- Ghostty uses `font-family-bold` for bold body text as well, so bold words, table headers, and bold code keywords turn serif in the fixture run. That is Ghostty, not markview.
- There is no live theme switch in Ghostty runs. Use `--light` at launch instead.
- `key` sees any pixel change as `changed`. Before relying on `changed` or `unchanged`, open the screenshot.
