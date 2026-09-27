# Selection

A user drags over text to select it, and markview copies it to the clipboard on release. The selected cells get a surface2 background, and the status bar shows `copied N characters`.

## Sub-features

- `sel-drag` selects from the press to the release, in reading order, and copies it with OSC 52 on release.
- `sel-word` copies the whitespace-delimited word under a double-click (two presses within 400 ms on the same cell).
- `sel-heading` copies a scaled heading whole when any part of it is selected. Its rows and its rule are not highlighted, and the rule copies as an empty line.
- `sel-autoscroll` scrolls one line per drag event while the pointer is above or below the text.
- `sel-clear` removes the highlight on a click that doesn't move, and on a resize or theme change.
- `sel-shift` passes `Shift` + drag through to the terminal's own selection. markview never sees it.

## How to get to it (user POV)

Open `markview sample.md` and drag over the first paragraph, or double-click a word.

## Driving it with mv-verify

Real drags can't reach the hidden output, so send SGR reports. A press is `\x1b[<0;COL;ROWM`, a drag is `\x1b[<32;COL;ROWM`, and a release is `\x1b[<0;COL;ROWm`. Columns and rows are 1-based. At 174x43 cells, the text starts at column 3 and row 2.

The copy goes to the real system clipboard. Save it first with `wl-paste -n > /tmp/opencode/clip`, and restore it afterwards with `wl-copy < /tmp/opencode/clip`.

- **Drag.** From the top of `sample.md`, send press `3;6`, drag `20;7`, and release `20;7`. The status shows `copied 185 characters`. `wl-paste -n` starts with `We are replacing` and ends with `rolls out to 5%, 2`. The screenshot shows the highlight on exactly those cells.
- **Word.** Send press and release at `6;7` twice in one `send-text`. The status shows `copied 5 characters`, and the clipboard holds `rolls`.
- **Heading.** Send press `10;2`, drag `15;7`, and release `15;7`. The clipboard starts with `Search v2 launch plan` followed by blank lines, and contains no `─`. The screenshot shows no highlight on the heading rows.
- **Autoscroll.** Send press `3;20`, then five drags at `10;43` (the status row), then release. The status leaves `0%`.
- **Images.** Press `G`, then drag from `3;2` to `60;42`. The screenshot shows the photo intact inside the highlight.

## Gotchas

- `get-text` shows no highlight. Only screenshots prove it.
- Scaled headings are drawn by the terminal (OSC 66) or as images, so markview cannot paint behind them.
