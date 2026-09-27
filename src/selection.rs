use std::ops::Range;

use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use unicode_width::UnicodeWidthChar;

/// One rendered row of the document as plain text.
#[derive(Debug, PartialEq, Eq)]
pub enum Row {
    Plain(String),
    /// Drawn scaled, so screen columns do not map to its characters. It is
    /// selected, copied and skipped for highlighting as a whole.
    Heading(String),
}

/// A cell in document coordinates, so a selection stays put while scrolling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub line: usize,
    pub col: usize,
}

/// Every cell from `anchor` to `head` in reading order, both included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    anchor: Point,
    head: Point,
}

impl Selection {
    pub const fn at(point: Point) -> Self {
        Self {
            anchor: point,
            head: point,
        }
    }

    pub const fn extend(&mut self, to: Point) {
        self.head = to;
    }

    /// A press that never moved selects nothing.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The whitespace-delimited word under `at`, or the whole row for a heading.
    pub fn word(rows: &[Row], at: Point) -> Option<Self> {
        let cols = match rows.get(at.line)? {
            Row::Heading(text) => 0..width(text),
            Row::Plain(text) => words(text).find(|w| w.contains(&at.col))?,
        };
        let line = at.line;
        (!cols.is_empty()).then(|| Self {
            anchor: Point {
                line,
                col: cols.start,
            },
            head: Point {
                line,
                col: cols.end - 1,
            },
        })
    }

    /// The columns selected on `line`, end exclusive.
    pub fn cols(&self, line: usize) -> Option<Range<usize>> {
        let (start, end) = (self.anchor.min(self.head), self.anchor.max(self.head));
        if !(start.line..=end.line).contains(&line) {
            return None;
        }
        let from = if line == start.line { start.col } else { 0 };
        let to = if line == end.line {
            end.col + 1
        } else {
            usize::MAX
        };
        Some(from..to)
    }

    pub fn text(&self, rows: &[Row]) -> String {
        rows.iter()
            .enumerate()
            .filter_map(|(line, row)| {
                let cols = self.cols(line)?;
                Some(match row {
                    Row::Plain(text) => slice(text, cols).trim_end(),
                    Row::Heading(text) => text.trim_end(),
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Paints the selected cells of the plain rows in view.
    pub fn highlight(&self, buf: &mut Buffer, area: Rect, scroll: usize, rows: &[Row], bg: Color) {
        for y in 0..area.height {
            let line = scroll + usize::from(y);
            let (Some(Row::Plain(_)), Some(cols)) = (rows.get(line), self.cols(line)) else {
                continue;
            };
            for x in cols.start..cols.end.min(usize::from(area.width)) {
                if let Some(cell) = buf.cell_mut((area.x + x as u16, area.y + y)) {
                    cell.set_bg(bg);
                }
            }
        }
    }
}

/// Asks the terminal to put `text` on the system clipboard.
pub fn osc52(text: &str) -> String {
    format!(
        "\x1b]52;c;{}\x07",
        base64_simd::STANDARD.encode_to_string(text)
    )
}

fn width(text: &str) -> usize {
    text.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// Column ranges of the whitespace-delimited words in `text`.
fn words(text: &str) -> impl Iterator<Item = Range<usize>> {
    let mut spans = Vec::new();
    let mut start = None;
    let mut col = 0;
    for c in text.chars() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                spans.push(s..col);
            }
        } else if start.is_none() {
            start = Some(col);
        }
        col += c.width().unwrap_or(0);
    }
    if let Some(s) = start {
        spans.push(s..col);
    }
    spans.into_iter()
}

/// The characters of `text` that overlap the given display columns.
fn slice(text: &str, cols: Range<usize>) -> &str {
    let mut from = None;
    let mut to = text.len();
    let mut col = 0;
    for (i, c) in text.char_indices() {
        if col >= cols.end {
            to = i;
            break;
        }
        let w = c.width().unwrap_or(0);
        if from.is_none() && col + w > cols.start {
            from = Some(i);
        }
        col += w;
    }
    from.and_then(|f| text.get(f..to)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: &[&str]) -> Vec<Row> {
        lines.iter().map(|l| Row::Plain((*l).to_owned())).collect()
    }

    const fn pt(line: usize, col: usize) -> Point {
        Point { line, col }
    }

    fn select(from: Point, to: Point) -> Selection {
        let mut s = Selection::at(from);
        s.extend(to);
        s
    }

    #[test]
    fn copies_from_the_anchor_to_the_head_across_lines() {
        let rows = plain(&["  hello world", "  second line   ", "  third"]);
        let s = select(pt(0, 8), pt(2, 4));
        assert_eq!(s.text(&rows), "world\n  second line\n  thi");
    }

    #[test]
    fn dragging_backwards_copies_the_same_text() {
        let rows = plain(&["  hello world", "  second line"]);
        assert_eq!(
            select(pt(1, 7), pt(0, 2)).text(&rows),
            select(pt(0, 2), pt(1, 7)).text(&rows)
        );
    }

    #[test]
    fn wide_characters_are_copied_whole() {
        let rows = plain(&["a✅b"]);
        assert_eq!(select(pt(0, 2), pt(0, 3)).text(&rows), "✅b");
    }

    #[test]
    fn a_selection_touching_a_heading_copies_all_of_it() {
        let rows = vec![
            Row::Heading("Search v2 launch plan".to_owned()),
            Row::Plain("body text".to_owned()),
        ];
        assert_eq!(
            select(pt(0, 30), pt(1, 3)).text(&rows),
            "Search v2 launch plan\nbody"
        );
    }

    #[test]
    fn double_clicking_selects_the_word_under_the_pointer() {
        let rows = plain(&["  ☑ Backfill 12M documents"]);
        let word = |col| Selection::word(&rows, pt(0, col)).map(|s| s.text(&rows));
        assert_eq!(word(6).as_deref(), Some("Backfill"));
        assert_eq!(word(13).as_deref(), Some("12M"));
        assert_eq!(word(1), None, "whitespace");
        assert_eq!(word(40), None, "past the end");
    }

    #[test]
    fn highlight_paints_only_selected_plain_cells() {
        let rows = vec![
            Row::Plain("abcdef".to_owned()),
            Row::Heading("Title".to_owned()),
            Row::Plain("ghijkl".to_owned()),
        ];
        let area = Rect::new(0, 0, 6, 3);
        let mut buf = Buffer::empty(area);
        select(pt(0, 4), pt(2, 1)).highlight(&mut buf, area, 0, &rows, Color::Red);
        let painted: Vec<(u16, u16)> = area
            .positions()
            .filter(|p| buf[(p.x, p.y)].bg == Color::Red)
            .map(|p| (p.x, p.y))
            .collect();
        assert_eq!(painted, [(4, 0), (5, 0), (0, 2), (1, 2)]);
    }
}
