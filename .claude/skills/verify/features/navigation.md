# Navigation

A user scrolls the document by line, by page, or to either end, with the keyboard or the mouse wheel. The status bar shows how far down they are as a percentage. Scrolling stops at the top and bottom.

## Sub-features

- `nav-line` moves one line with `j`/`k` or `↓`/`↑`.
- `nav-page` moves one screen with `space`/`d`/`PgDn` and `b`/`u`/`PgUp`.
- `nav-jump` goes to the top with `g`/`Home` and to the bottom with `G`/`End`.
- `nav-wheel` moves three lines per wheel notch.
- `nav-clamp` never scrolls above 0% or past 100%.
- `nav-status` keeps the percentage in the status bar matched to the scroll position.

## How to get to it (user POV)

- Open any document longer than the window, like `markview sample.md`, and press the keys from the README table.
- Scroll with the mouse wheel over the window.

## Driving it with mv-verify

Preconditions:

- `$V launch nav-1 sample.md` is ready. Run `$V kitty nav-1 send-text g` and `$V wait nav-1 'sample.md  0%'` before starting.

- **Page down.** Run `$V kitty nav-1 send-text ' '`. `$V wait nav-1 'sample.md  30%'` matches (174x43 cells, the default output size).
- **Page up.** Run `$V kitty nav-1 send-text b`, then `$V wait nav-1 'sample.md  0%'`.
- **Line down.** From 0%, save `$V text nav-1 before-j`, run `$V kitty nav-1 send-text j`, then save `$V text nav-1 after-j`. Compare the document rows (every row but the last, which is the status bar) row by row: row *i* of `after-j` equals row *i+1* of `before-j`, and no other shift matches. Don't count non-blank lines. Sized headings leave blank rows in the screen text, so non-blank counts don't track rows. `k` returns a screen identical to `before-j`. The percentage stays at `0%` because one line is under 1% of `sample.md`.
- **Named keys.** Repeat page and jump with `send-key page_down`, `page_up`, `down`, `up`, `end`, `home`. Each moves like its letter key.
- **Bottom.** Run `$V kitty nav-1 send-text G`, then `$V wait nav-1 'sample.md  100%'`. The screen ends with `Last updated by the search team on 2026-09-23.`
- **Clamp.** At 100%, press `j` and `space`. The status stays `100%`. Press `g`, then `k` and `b`. The status stays `0%`.
- **Wheel.** From 0%, run `$V kitty nav-1 send-text '\x1b[<65;10;10M'` for one wheel-down notch. `$V wait nav-1 'sample.md  2%'` matches, and the document rows shifted by exactly 3, by the same row-by-row comparison as line down. `'\x1b[<64;10;10M'` is wheel-up and returns to `0%`.
- **Proof.** `$V text nav-1 top`, `$V text nav-1 page-1`, and `$V text nav-1 bottom` at 0%, 30%, and 100%, plus the status line from each step.

## Gotchas

- The percentages above hold only at the default 1920x1080 output (174x43 cells). If `doctor` reports another size, assert relative moves instead.
- The status percentage is an integer, so small moves can leave it unchanged. Compare screen text for line-level moves.
- The wheel sequences only work because markview enables mouse capture. They are SGR mouse reports sent through kitty, which is the same path a real wheel takes.
