# Document rendering

markview draws Markdown as a document rather than raw text. Headings are larger than body text and underlined with a rule, tables get box borders and aligned columns, task lists get checkboxes, blockquotes get a side bar, inline code and links are colored, and fenced code is syntax-highlighted in a labeled frame.

## Sub-features

- `render-heading-sized` makes h1 to h3 larger than body text in kitty 0.40+ (text sizing) and in other kitty-graphics terminals (heading images).
- `render-heading-rule` puts a full-width rule under h1 and h2.
- `render-table` draws bordered tables that wrap cell text and handle emoji width.
- `render-tasklist` shows `☑` and `☐` for checked and unchecked items.
- `render-blockquote` draws a `│` bar and italics.
- `render-inline` colors bold, italic, inline code, and links.
- `render-code` frames fenced code with a `╭─ <lang>` header and highlights it.

## How to get to it (user POV)

- Run `markview sample.md` and read the first two screens.

## Driving it with mv-verify

Preconditions:

- `$V launch render-1 sample.md` is ready at `0%`.

- **Table and task list text.** Run `$V text render-1 top`. It contains `┌` and `┐` borders, a row with `Index builder`, `Ana`, and `✅ shipped`, and the lines `☑ Backfill 12M documents into the new index` and `☐ Delete the legacy KeywordSearch service`.
- **Sized headings.** Run `$V shot render-1 top`. Open the PNG. `Search v2 launch plan` is about twice the body text height, and `Milestones` is visibly larger than body text. Each has a thin rule under it. `sample.md` has only h1 and h2, so h3 needs a fixture.
- **Blockquote.** `top.txt` has the line starting `│ Rollback is one flag.`, and `top.png` shows it in italics with a bar on the left.
- **Rust code.** `top.txt` contains `╭─ rust`, and `top.png` shows `pub async fn` in a different color from identifiers such as `query`.
- **More code.** Press `space` once and `$V wait render-1 'sample.md  30%'`. `$V text render-1 code` contains `╭─ typescript`, `╭─ bash`, and `╭─ mermaid`. In `$V shot render-1 code`, the bash comment is italic and `flagctl` is colored. TypeScript shows only `export` and type names in color, a known upstream issue (see `../ownership.md`).
- **Proof.** `top.txt`, `top.png`, `code.txt`, and `code.png` from `render-1`. The heading-image path is covered in [Ghostty](./ghostty.md).

## Gotchas

- In kitty, headings use the text sizing protocol, so `get-text` still shows the heading text.
- Table cell wrapping depends on window width. No cell in `sample.md` wraps at 174 columns, so wrapping needs a fixture with a long cell. Assert cell contents, not where a cell breaks.
- Tables, task lists, blockquotes, inline styles, code frames and highlighting come from `ratatui-markdown`. The colors come from markview's `src/theme.rs`. Heading size and rules are markview's.
- Colors come from Catppuccin Frappé on dark backgrounds and Latte on light ones. Compare against the right palette for the theme you launched with.
