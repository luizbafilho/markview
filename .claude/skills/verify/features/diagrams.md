# Mermaid diagrams

Fenced ` ```mermaid ` blocks render as diagrams made of text, inside a frame labeled `mermaid`. markview supports flowcharts, sequence diagrams, Gantt charts, and pie charts. `ratatui-markdown` parses and lays out every diagram, so wrong diagram content is upstream (see `../ownership.md`). markview only supplies the colors. A dotted arrow makes the whole block disappear, and `subgraph` keywords are drawn as nodes.

## Sub-features

- `diagram-flowchart` draws boxed nodes, a decision node, and labeled `yes`/`no` edges.
- `diagram-sequence` draws participant columns, lifelines, and labeled messages.
- `diagram-gantt` draws a title, sections, and bars with durations.
- `diagram-pie` draws a title and one bar per slice with its percentage.

## How to get to it (user POV)

- Open `sample.md` and scroll to `Rollout` and `Schedule`.
- Open any file with a `mermaid` fence.

## Driving it with mv-verify

Preconditions:

- `$V launch diagram-1 sample.md` is ready at `0%`.

- **Flowchart and sequence.** Press `space` once and `$V wait diagram-1 'sample.md  30%'`. Send wheel-down `'\x1b[<65;10;10M'` about 5 times, until `$V wait diagram-1 '100% of traffic'` matches. `$V text diagram-1 flow` contains `│ Deploy behind flag │`, `yes` and `no` on one row, `│ Raise rollout │`, `│ Turn flag off │`, and `│ 100% of traffic │`. The decision node is a rounded box, not a diamond. Send about 10 more wheel-down notches and `$V text diagram-1 seq`. It lists `Client`, `Gateway`, `Ranker`, and `Index` on one row, has a `GET /search` message, and draws replies dashed (`◀╌╌╌`).
- **Gantt and pie.** Press `G` and `$V wait diagram-1 'sample.md  100%'`. At 100% only the last pie row is on screen, under the `Offsite` photo. Send wheel-up `'\x1b[<64;10;10M'` about 6 times, until `$V wait diagram-1 'Where query time goes'` and `$V wait diagram-1 'Build'` both match (about 86%). `$V text diagram-1 charts` contains the `Build`, `Launch`, and `Cleanup` sections, `Index builder` with a `█` bar and `14d`, the title `Where query time goes`, and `Embedding` with `38%`.
- **Pixels.** `$V shot diagram-1 charts` shows the bars in the theme's accent color.
- **Unsupported syntax.** Write a flowchart with a `subgraph` into `target/verify/diagram-2/sub.md`, and one with `A -.-> B` into `target/verify/diagram-3/dotted.md`. Launch each and record exactly what renders. Today the subgraph keywords become boxes and the dotted-arrow block disappears. Report any change as `upstream: ratatui-markdown <version>`, not as a markview pass or failure.
- **Proof.** `flow.txt`, `seq.txt`, `charts.txt`, and `charts.png`.

## Gotchas

- Diagrams are laid out for the current width. Check the terminal size in `doctor` before calling a layout wrong.
- Wheel code 65 scrolls down and 64 scrolls up. The notch counts depend on window height. Wait for the text instead of counting.
- Gantt bars ignore `after`, so dependent tasks start at the same point. That is upstream. Check sections, names, and durations, not bar offsets.
