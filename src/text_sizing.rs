//! Larger headings via the kitty text sizing protocol (OSC 66).
//! https://sw.kovidgoyal.net/kitty/text-sizing-protocol/
//!
//! Escapes go straight to the terminal after each frame, never through
//! ratatui's buffer. Its diff derives how many cells to skip from a symbol's
//! display width, and an OSC payload inflates that. The heading's cells are
//! marked `skip` in the buffer instead.

use std::{
    fmt::Write as _,
    io::{self, Write},
};

use ratatui::{
    buffer::Buffer,
    crossterm::{
        cursor::{position, MoveTo},
        execute, queue, Command,
        style::{Attribute, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    },
    style::{Modifier, Style},
    text::{Line, Span},
};
use ratatui_markdown::markdown::{ImagePlacement, MarkdownBlock};
use unicode_width::UnicodeWidthChar;

/// Private-use code points prefixed to heading text so the rendered rows can be found.
const H1: char = '\u{F0001}';
const H2: char = '\u{F0002}';
const H3: char = '\u{F0003}';

/// Every sized heading is a 2-row multicell.
pub const ROWS: u16 = 2;

pub struct Heading {
    pub row: usize,
    pub level: u8,
}

#[derive(Clone, PartialEq)]
pub struct Placed {
    x: u16,
    y: u16,
    width: u16,
    rows: u16,
    escape: String,
}

/// Probes by drawing a space at scale 2 and checking the cursor moved two cells.
pub fn probe() -> io::Result<bool> {
    let mut out = io::stdout();
    execute!(out, MoveTo(0, 0))?;
    let (x0, _) = position()?;
    execute!(out, Print("\x1b]66;s=2; \x07"))?;
    let (x1, _) = position()?;
    Ok(x1 == x0 + 2)
}

pub fn tag(blocks: &mut [MarkdownBlock]) {
    for b in blocks {
        match b {
            MarkdownBlock::Heading1(t) => t.insert(0, H1),
            MarkdownBlock::Heading2(t) => t.insert(0, H2),
            MarkdownBlock::Heading3(t) => t.insert(0, H3),
            _ => {}
        }
    }
}

/// Strips the tags, underlines each h1 and h2 with a `width`-wide rule, wraps
/// sized headings to `width` and gives each wrapped row the extra row it
/// needs, and shifts image placements below. Returns the heading rows and rule
/// rows to draw sized, both empty when `sized` is off.
pub fn extract(
    lines: &mut Vec<Line<'static>>,
    images: &mut [ImagePlacement],
    sized: bool,
    width: u16,
    rule: Style,
) -> (Vec<Heading>, Vec<usize>) {
    let mut out = Vec::with_capacity(lines.len());
    let mut headings = Vec::new();
    let mut rules = Vec::new();
    // (source row, rows added below it)
    let mut shifts = Vec::new();
    for (src_row, mut line) in std::mem::take(lines).into_iter().enumerate() {
        let level = line.spans.first().and_then(|s| match s.content.chars().next() {
            Some(H1) => Some(1),
            Some(H2) => Some(2),
            Some(H3) => Some(3),
            _ => None,
        });
        let Some(level) = level else {
            out.push(line);
            continue;
        };
        let start = out.len();
        let first = &mut line.spans[0];
        first.content = first.content.chars().skip(1).collect::<String>().into();
        for span in &mut line.spans {
            span.style = span.style.remove_modifier(Modifier::UNDERLINED);
        }
        if sized {
            for row in wrap(&line, level, width) {
                headings.push(Heading { row: out.len(), level });
                out.push(row);
                out.push(Line::default());
            }
        } else {
            out.push(line);
        }
        if level < 3 {
            if sized {
                rules.push(out.len());
            }
            out.push(Line::styled("─".repeat(width as usize), rule));
        }
        shifts.push((src_row, out.len() - start - 1));
    }
    for img in images {
        img.row += shifts.iter().filter(|(r, _)| *r < img.row).map(|(_, n)| n).sum::<usize>();
    }
    *lines = out;
    (headings, rules)
}

/// Builds the escape for a heading whose top-left cell is `(x, y)`, or
/// `None` when it does not fit in `max_width` cells.
pub fn place(line: &Line, level: u8, x: u16, y: u16, max_width: u16, base: Style) -> Option<Placed> {
    let mut escape = String::new();
    for span in &line.spans {
        sgr(&mut escape, base.patch(line.style).patch(span.style));
        match fraction(level) {
            None => write!(escape, "\x1b]66;s=2;{}\x07", span.content).unwrap(),
            Some((n, d)) => {
                for (chunk, cols) in chunks(&span.content, d) {
                    let w = (n * cols).div_ceil(d);
                    write!(escape, "\x1b]66;s=2:n={n}:d={d}:v=2:w={w};{chunk}\x07").unwrap();
                }
            }
        }
    }
    SetAttribute(Attribute::Reset).write_ansi(&mut escape).unwrap();
    ResetColor.write_ansi(&mut escape).unwrap();
    let width = scaled_width(line, level);
    (width <= max_width).then_some(Placed { x, y, width, rows: ROWS, escape })
}

/// Font scale within a 2x2 block as `n/d`, `None` for the full 2x.
/// Every `d` columns of text fill `n` scaled cells.
fn fraction(level: u8) -> Option<(u16, u16)> {
    match level {
        1 => None,
        2 => Some((3, 4)),
        _ => Some((5, 8)),
    }
}

/// How many cells wide one column of text is drawn at `level`.
pub fn scale(level: u8) -> f32 {
    fraction(level).map_or(2.0, |(n, d)| 2.0 * n as f32 / d as f32)
}

/// Columns `line` takes once drawn at `level`'s scale.
fn scaled_width(line: &Line, level: u8) -> u16 {
    line.spans
        .iter()
        .map(|span| match fraction(level) {
            None => 2 * span.width() as u16,
            Some((n, d)) => chunks(&span.content, d).iter().map(|(_, cols)| 2 * (n * cols).div_ceil(d)).sum(),
        })
        .sum()
}

/// Breaks a heading into rows that fit `max` columns at `level`'s scale,
/// at spaces where possible and mid-word only when a word alone is too wide.
fn wrap(line: &Line<'static>, level: u8, max: u16) -> Vec<Line<'static>> {
    let chars: Vec<(char, Style)> =
        line.spans.iter().flat_map(|s| s.content.chars().map(move |c| (c, s.style))).collect();
    let row = |cs: &[(char, Style)]| {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for &(c, style) in cs {
            match spans.last_mut() {
                Some(last) if last.style == style => last.content.to_mut().push(c),
                _ => spans.push(Span::styled(c.to_string(), style)),
            }
        }
        Line::from(spans).style(line.style)
    };
    let mut rows = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = start;
        let mut space = None;
        while end < chars.len() && scaled_width(&row(&chars[start..=end]), level) <= max {
            if chars[end].0 == ' ' {
                space = Some(end);
            }
            end += 1;
        }
        if end < chars.len()
            && chars[end].0 != ' '
            && let Some(s) = space
        {
            end = s;
        }
        // A single glyph wider than `max` still takes a row of its own.
        end = end.max(start + 1);
        let mut cut = end;
        while cut > start && chars[cut - 1].0 == ' ' {
            cut -= 1;
        }
        rows.push(row(&chars[start..cut]));
        start = end;
        while start < chars.len() && chars[start].0 == ' ' {
            start += 1;
        }
    }
    rows
}

/// Builds the escape for a `width`-cell rule drawn at quarter scale and
/// top-aligned, so the line is thin and sits right under the row above.
pub fn rule(x: u16, y: u16, width: u16, style: Style) -> Placed {
    let mut escape = String::new();
    sgr(&mut escape, style);
    let mut left = width;
    while left > 0 {
        let w = left.min(7);
        write!(escape, "\x1b]66;n=1:d=4:w={w};{}\x07", "─".repeat(4 * w as usize)).unwrap();
        left -= w;
    }
    SetAttribute(Attribute::Reset).write_ansi(&mut escape).unwrap();
    ResetColor.write_ansi(&mut escape).unwrap();
    Placed { x, y, width, rows: 1, escape }
}

pub fn mark_skip(buf: &mut Buffer, p: &Placed) {
    for y in p.y..p.y + p.rows {
        for x in p.x..p.x + p.width {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_skip(true);
            }
        }
    }
}

/// Must run before ratatui redraws cells inside a heading's old block.
/// Kitty 0.48.2 keeps the lower row occupied when only the top row is
/// erased, so a multicell drawn onto that row later gets pushed right.
pub fn erase(out: &mut impl Write, gone: &[Placed]) -> io::Result<()> {
    queue!(out, SetAttribute(Attribute::Reset), ResetColor)?;
    for p in gone {
        for y in p.y..p.y + p.rows {
            queue!(out, MoveTo(p.x, y), Print(format!("\x1b[{}X", p.width)))?;
        }
    }
    out.flush()
}

pub fn draw(out: &mut impl Write, new: &[Placed]) -> io::Result<()> {
    for p in new {
        queue!(out, MoveTo(p.x, p.y), Print(&p.escape))?;
    }
    out.flush()
}

fn chunks(s: &str, cols: u16) -> Vec<(String, u16)> {
    let mut out: Vec<(String, u16)> = vec![(String::new(), 0)];
    for ch in s.chars() {
        let w = ch.width().unwrap_or(0) as u16;
        if out.last().unwrap().1 + w > cols {
            out.push((String::new(), 0));
        }
        let last = out.last_mut().unwrap();
        last.0.push(ch);
        last.1 += w;
    }
    out.retain(|(_, c)| *c > 0);
    out
}

fn sgr(out: &mut String, style: Style) {
    SetAttribute(Attribute::Reset).write_ansi(out).unwrap();
    if let Some(fg) = style.fg {
        SetForegroundColor(fg.into()).write_ansi(out).unwrap();
    }
    if let Some(bg) = style.bg {
        SetBackgroundColor(bg.into()).write_ansi(out).unwrap();
    }
    for (m, a) in [
        (Modifier::BOLD, Attribute::Bold),
        (Modifier::ITALIC, Attribute::Italic),
        (Modifier::UNDERLINED, Attribute::Underlined),
    ] {
        if style.add_modifier.contains(m) {
            SetAttribute(a).write_ansi(out).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TITLE: &str = "Design: LLM-generated tmux window titles for agent sessions";

    fn heading(tag: char, text: &str) -> Line<'static> {
        Line::from(Span::styled(format!("{tag}{text}"), Style::new().add_modifier(Modifier::BOLD)))
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn h1_wider_than_the_viewport_wraps_into_rows_that_all_fit() {
        let mut lines = vec![heading(H1, TITLE)];
        let (headings, _) = extract(&mut lines, &mut [], true, 108, Style::new());

        assert_eq!(headings.len(), 2);
        for h in &headings {
            assert!(place(&lines[h.row], 1, 0, 0, 108, Style::new()).is_some(), "row {} does not fit", h.row);
        }
        let rows: Vec<String> = headings.iter().map(|h| text(&lines[h.row])).collect();
        assert_eq!(rows.join(" "), TITLE);
    }

    #[test]
    fn h1_that_fits_stays_on_one_row() {
        let mut lines = vec![heading(H1, TITLE)];
        let (headings, _) = extract(&mut lines, &mut [], true, 133, Style::new());

        assert_eq!(headings.len(), 1);
        assert_eq!(text(&lines[headings[0].row]), TITLE);
    }

    #[test]
    fn h3_is_tagged_and_drawn_at_one_and_a_quarter_scale() {
        let mut blocks = vec![MarkdownBlock::Heading3("Claude Code".into())];
        tag(&mut blocks);
        let MarkdownBlock::Heading3(t) = &blocks[0] else { unreachable!() };
        let mut lines = vec![Line::from(t.clone())];
        let (headings, _) = extract(&mut lines, &mut [], true, 80, Style::new());

        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].level, 3);
        // 11 columns in chunks of 8 and 3, each 5/8 of a 2-column cell, rounded up.
        let p = place(&lines[headings[0].row], 3, 0, 0, 80, Style::new()).unwrap();
        assert_eq!(p.width, 14);
    }

    #[test]
    fn only_h1_and_h2_get_a_rule() {
        let mut lines = vec![heading(H1, "One"), heading(H2, "Two"), heading(H3, "Three")];
        let (headings, rules) = extract(&mut lines, &mut [], true, 80, Style::new());

        assert_eq!(headings.iter().map(|h| h.row).collect::<Vec<_>>(), [0, 3, 6]);
        assert_eq!(rules, [2, 5]);
    }
}
