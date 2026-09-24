# App or library

markview is a thin layer over crates it does not control. A wrong screen can come from markview's code or from a library. Decide which before you report it, because the fix for each goes to a different place.

## Who owns each stage

Check the version you are reading in `Cargo.lock`. Library source is under `~/.cargo/registry/src/*/<crate>-<version>/src/`.

| Stage | Owner | Where |
|---|---|---|
| Markdown parsing, block layout, table borders and wrapping, task-list checkboxes, blockquote bar, inline styles, code frames | `ratatui-markdown` | `markdown/render.rs`, `markdown/inline.rs` |
| Which tokens get highlighted in code (grammars and highlight queries) | `ratatui-markdown` and the `tree-sitter-*` crates | `highlight/treesitter.rs` |
| Everything inside a `mermaid` block: parsing, layout, which syntax is supported | `ratatui-markdown` | `mermaid/` |
| Which palette color each token kind, border or diagram gets | markview | `src/theme.rs` |
| Image path resolution, image cell size, the `[image not loaded: …]` text | markview | `FsResolver` in `src/main.rs` |
| Where images sit and how they crop while scrolling | markview (kitty), `ratatui-image` (Sixel, iTerm2, half blocks) | `draw_images` in `src/main.rs`, `src/kitty.rs` |
| Which graphics protocol is picked | `ratatui-image` | `Picker::from_query_stdio` |
| Sized headings and the rule under them | markview | `src/text_sizing.rs`, `src/heading_image.rs`, `src/heading_font/` |
| Keys, mouse wheel, scroll clamp, status bar | markview | `run` in `src/main.rs` |
| Detecting light or dark background | `terminal-colorsaurus` asks the terminal, markview polls and repaints | `theme_mode`, `THEME_POLL` in `src/main.rs` |

## Deciding

1. Find the stage in the table.
2. If a library owns it, check that markview passes the library's output through unchanged. markview only rewrites what `render_full` returns in `text_sizing::extract` (heading rows and rules) and in `draw_images`. When the wrong content is outside those, the library produced it.
3. Confirm by reading the library code at the locked version, and cite the file and line.
4. When markview supplies an input to the library (colors in `theme.rs`, image sizes from `FsResolver`), check that input before blaming the library. For example, a token drawn in the text color is markview's fault if `CodeColors` sets that kind to the text color, and the library's fault if the kind never gets captured.

## Reporting

- Tag every failure `app` or `upstream: <crate> <version>`, with the file and line that shows it.
- For an upstream failure, give the smallest Markdown fixture that reproduces it. Don't patch around a library bug inside markview. The fix belongs upstream, or in a version bump.
- If you can't tell, say `owner unknown` and name what you checked. Don't guess.

## Known upstream issues

As of `ratatui-markdown` 0.3.6. Recheck after a version bump.

- **TypeScript and TSX highlighting is sparse.** `highlight/treesitter.rs` loads only `tree_sitter_typescript::HIGHLIGHTS_QUERY`. That query has no rules for `async`, `function`, `await` or strings. Those rules are in the JavaScript query, which TypeScript needs added on top.
- **A flowchart with a dotted arrow (`-.->`) disappears.** The whole block renders as nothing, with no error.
- **`subgraph` is drawn as nodes.** The flowchart renders, but `subgraph`, the subgraph's title and `end` each become a box.
- **Gantt bars ignore `after`.** A task declared `after a1` starts at the same point as `a1`.
