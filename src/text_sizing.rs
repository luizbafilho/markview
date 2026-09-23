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
    text::Line,
};
use ratatui_markdown::markdown::{ImagePlacement, MarkdownBlock};
use unicode_width::UnicodeWidthChar;

/// Private-use code points prefixed to heading text so the rendered rows can be found.
const H1: char = '\u{F0001}';
const H2: char = '\u{F0002}';

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
            _ => {}
        }
    }
}

/// Strips the tags, inserts the extra row each sized heading needs, and
/// shifts image placements below it.
pub fn extract(lines: &mut Vec<Line<'static>>, images: &mut [ImagePlacement]) -> Vec<Heading> {
    let mut out = Vec::with_capacity(lines.len());
    let mut headings = Vec::new();
    let mut src_rows = Vec::new();
    for (src_row, mut line) in std::mem::take(lines).into_iter().enumerate() {
        let level = line.spans.first().and_then(|s| match s.content.chars().next() {
            Some(H1) => Some(1),
            Some(H2) => Some(2),
            _ => None,
        });
        let Some(level) = level else {
            out.push(line);
            continue;
        };
        let first = &mut line.spans[0];
        first.content = first.content.chars().skip(1).collect::<String>().into();
        headings.push(Heading { row: out.len(), level });
        src_rows.push(src_row);
        out.push(line);
        out.push(Line::default());
    }
    for img in images {
        img.row += src_rows.iter().filter(|&&r| r < img.row).count();
    }
    *lines = out;
    headings
}

/// Builds the escape for a heading whose top-left cell is `(x, y)`, or
/// `None` when it does not fit in `max_width` cells.
pub fn place(line: &Line, level: u8, x: u16, y: u16, max_width: u16, base: Style) -> Option<Placed> {
    let mut escape = String::new();
    let mut width = 0u16;
    for span in &line.spans {
        sgr(&mut escape, base.patch(line.style).patch(span.style));
        match level {
            1 => {
                write!(escape, "\x1b]66;s=2;{}\x07", span.content).unwrap();
                width += 2 * span.width() as u16;
            }
            _ => {
                // 1.5x text: every 4 columns of text fill 3 scaled cells (6 columns).
                for (chunk, cols) in chunks(&span.content, 4) {
                    let w = (3 * cols).div_ceil(4);
                    write!(escape, "\x1b]66;s=2:n=3:d=4:v=2:w={w};{chunk}\x07").unwrap();
                    width += 2 * w;
                }
            }
        }
    }
    SetAttribute(Attribute::Reset).write_ansi(&mut escape).unwrap();
    ResetColor.write_ansi(&mut escape).unwrap();
    (width <= max_width).then_some(Placed { x, y, width, escape })
}

pub fn mark_skip(buf: &mut Buffer, p: &Placed) {
    for y in p.y..p.y + ROWS {
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
        for y in p.y..p.y + ROWS {
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
