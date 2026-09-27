use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Context;
use doc::{Doc, Sizing, render};
use ratatui::{
    Frame,
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
            MouseEventKind,
        },
        execute,
    },
    layout::{Constraint, Layout, Position, Rect},
    text::Line,
    widgets::{Block, Padding, Paragraph},
};
use ratatui_image::{
    Image, Resize,
    picker::{Picker, ProtocolType},
};
use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};
use theme::Theme;

mod doc;
mod heading_font;
mod heading_image;
mod kitty;
mod pointer;
mod scrollbar;
mod selection;
mod text_sizing;
mod theme;
mod watch;

/// Idle time before asking the terminal whether its background changed.
const THEME_POLL: Duration = Duration::from_secs(1);
/// How often to check whether the open file changed on disk.
const FILE_POLL: Duration = Duration::from_millis(100);

fn kitty_id(image_index: usize) -> u32 {
    0x4D_4B_00 + image_index as u32 + 1
}

/// Draws each image into the blank rows the renderer reserved for it,
/// cropping the part that is scrolled out of the viewport.
fn draw_images(
    f: &mut Frame,
    doc: &mut Doc,
    inner: Rect,
    scroll: usize,
    picker: &Picker,
) -> Result<(), ratatui_image::errors::Errors> {
    for (i, p) in doc.images.iter().enumerate() {
        let top = p.row as i64 - scroll as i64;
        let bottom = top + i64::from(p.height_cells);
        let vis_top = top.max(0);
        let vis_bottom = bottom.min(i64::from(inner.height));
        if vis_top >= vis_bottom {
            continue;
        }
        let cut = (vis_top - top) as u16;
        let rows = (vis_bottom - vis_top) as u16;

        if picker.protocol_type() == ProtocolType::Kitty {
            let fg = kitty::id_color(kitty_id(i));
            let buf = f.buffer_mut();
            for y in 0..rows {
                for x in 0..p.width_cells.min(inner.width) {
                    if let Some(c) =
                        buf.cell_mut((inner.x + p.col as u16 + x, inner.y + vis_top as u16 + y))
                        && let Some(symbol) = kitty::cell(cut + y, x)
                    {
                        c.set_symbol(&symbol).set_fg(fg);
                    }
                }
            }
            continue;
        }

        if !matches!(doc.encoded.get(&i), Some((c, r, _)) if *c == cut && *r == rows) {
            let px_per_row = f64::from(p.image.height()) / f64::from(p.height_cells);
            let y = (f64::from(cut) * px_per_row) as u32;
            let h = ((f64::from(rows) * px_per_row) as u32).clamp(1, p.image.height() - y);
            let cropped = p.image.crop_imm(0, y, p.image.width(), h);
            let proto = picker.new_protocol(
                cropped,
                Rect::new(0, 0, p.width_cells, rows),
                Resize::Fit(None),
            )?;
            doc.encoded.insert(i, (cut, rows, proto));
        }

        let rect = Rect::new(
            inner.x + p.col as u16,
            inner.y + vis_top as u16,
            p.width_cells.min(inner.width),
            rows,
        );
        if let Some((_, _, proto)) = doc.encoded.get(&i) {
            f.render_widget(Image::new(proto), rect);
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: markview <file.md>")?;
    let mut file = watch::Watched::open(&path)?;
    let base = Path::new(&path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let mut terminal = ratatui::init();
    // mosh, GNU Screen, PuTTY and the Linux console never report their background.
    let theme = Theme::new(theme_mode(QueryOptions::default()).unwrap_or(ThemeMode::Light));
    let picker = Picker::from_query_stdio().context("querying terminal graphics support")?;
    let sizing = if text_sizing::probe().context("probing text sizing support")? {
        Some(Sizing::Osc66)
    } else if picker.protocol_type() == ProtocolType::Kitty {
        Some(Sizing::Image(heading_image::HeadingFont::load(
            &heading_font::configured()?,
        )?))
    } else {
        None
    };
    terminal.clear()?;
    execute!(std::io::stdout(), EnableMouseCapture)?;
    let result = run(
        &mut terminal,
        &path,
        &mut file,
        &base,
        theme,
        &picker,
        sizing.as_ref(),
    );
    if picker.protocol_type() == ProtocolType::Kitty {
        std::io::stdout().write_all(kitty::DELETE_ALL.as_bytes())?;
    }
    execute!(std::io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    path: &str,
    file: &mut watch::Watched,
    base: &Path,
    mut theme: Theme,
    picker: &Picker,
    sizing: Option<&Sizing>,
) -> anyhow::Result<()> {
    let mut scroll: usize = 0;
    let mut doc: Option<Doc> = None;
    let mut page: usize;
    let mut drawn: Vec<text_sizing::Placed> = Vec::new();
    let mut pointer = pointer::Pointer::default();
    let mut notice: Option<String> = None;

    loop {
        let [body, status] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)])
            .areas(Rect::from((Position::default(), terminal.size()?)));
        let block = Block::new()
            .padding(Padding::new(2, 2, 1, 0))
            .style(theme.base_style());
        let inner = block.inner(body);

        let d = match doc.take() {
            Some(d) if d.size == (inner.width, inner.height) => doc.insert(d),
            stale => {
                // ratatui clears the whole screen on resize, taking every sized heading with it.
                if stale.is_some() {
                    drawn.clear();
                }
                pointer.clear();
                let d = render(
                    file.contents(),
                    base,
                    inner,
                    &theme,
                    picker.font_size(),
                    sizing,
                )?;
                if picker.protocol_type() == ProtocolType::Kitty {
                    let mut out = std::io::stdout().lock();
                    for (i, p) in d.images.iter().enumerate() {
                        let data =
                            kitty::transmit(&p.image, kitty_id(i), p.width_cells, p.height_cells)?;
                        out.write_all(data.as_bytes())?;
                    }
                    out.flush()?;
                }
                doc.insert(d)
            }
        };
        page = inner.height.max(1) as usize;
        scroll = scroll.min(d.lines.len().saturating_sub(page));
        let mut placed: Vec<text_sizing::Placed> = d
            .headings
            .iter()
            .filter(|h| {
                h.row >= scroll
                    && h.row - scroll + text_sizing::ROWS as usize <= inner.height as usize
            })
            .filter_map(|h| {
                let y = inner.y + (h.row - scroll) as u16;
                let base = theme.base_style();
                text_sizing::place(d.lines.get(h.row)?, h.level, inner.x, y, inner.width, base)
                    .transpose()
            })
            .collect::<Result<_, _>>()?;
        for r in d
            .rules
            .iter()
            .copied()
            .filter(|&r| r >= scroll && r - scroll < inner.height as usize)
        {
            placed.push(text_sizing::rule(
                inner.x,
                inner.y + (r - scroll) as u16,
                inner.width,
                theme.rule_style(),
            )?);
        }
        let gone: Vec<_> = drawn
            .iter()
            .filter(|p| !placed.contains(p))
            .cloned()
            .collect();
        let fresh: Vec<_> = placed
            .iter()
            .filter(|p| !drawn.contains(p))
            .cloned()
            .collect();
        text_sizing::erase(&mut std::io::stdout(), &gone)?;

        let total = d.lines.len();
        let bar = (body.width > 0 && total > page)
            .then(|| Rect::new(body.right().saturating_sub(1), inner.y, 1, inner.height));

        terminal.try_draw(|f| {
            f.render_widget(
                Paragraph::new(d.lines.clone())
                    .block(block)
                    .scroll((scroll as u16, 0)),
                body,
            );
            if let Some(sel) = pointer.selection() {
                let bg = theme.selection_background();
                sel.highlight(f.buffer_mut(), inner, scroll, &d.rows, bg);
            }
            draw_images(f, d, inner, scroll, picker).map_err(std::io::Error::other)?;
            for p in &placed {
                text_sizing::mark_skip(f.buffer_mut(), p);
            }

            if let Some(area) = bar
                && let Some(mut state) = scrollbar::state(total, page, scroll)
            {
                scrollbar::draw(f.buffer_mut(), area, &mut state, &theme);
            }
            let pct = if total <= page {
                100
            } else {
                scroll * 100 / (total - page)
            };
            let hint = notice
                .as_deref()
                .unwrap_or("j/k ↑/↓ scroll · space/b page · g/G top/bottom · q quit");
            let status_line =
                Line::from(format!(" {path}  {pct}%  ·  {hint}")).style(theme.bar_style());
            f.render_widget(status_line, status);
            Ok::<(), std::io::Error>(())
        })?;
        text_sizing::draw(&mut std::io::stdout(), &fresh)?;
        drawn = placed;

        let mut theme_checked = Instant::now();
        let event = loop {
            if event::poll(FILE_POLL)? {
                break Some(event::read()?);
            }
            if file.refresh() {
                doc = None;
                break None;
            }
            if theme_checked.elapsed() < THEME_POLL {
                continue;
            }
            theme_checked = Instant::now();
            if let Ok(mode) = theme_mode(QueryOptions::default())
                && mode != theme.mode()
            {
                theme = Theme::new(mode);
                doc = None;
                break None;
            }
        };
        let Some(event) = event else { continue };
        if matches!(event, Event::Key(_))
            || matches!(event, Event::Mouse(m) if matches!(m.kind, MouseEventKind::Down(_)))
        {
            notice = None;
        }
        match event {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('j') | KeyCode::Down => scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => scroll = scroll.saturating_sub(1),
                KeyCode::Char(' ' | 'd') | KeyCode::PageDown => scroll += page,
                KeyCode::Char('b' | 'u') | KeyCode::PageUp => {
                    scroll = scroll.saturating_sub(page);
                }
                KeyCode::Char('g') | KeyCode::Home => scroll = 0,
                KeyCode::Char('G') | KeyCode::End => scroll = usize::MAX / 2,
                _ => {}
            },
            Event::Mouse(m) => {
                let view = pointer::View {
                    body,
                    text: inner,
                    bar,
                    scroll,
                    max_scroll: total.saturating_sub(page),
                };
                let rows = doc.as_ref().map_or(&[][..], |d| &d.rows);
                let out = pointer.handle(m, view, rows, Instant::now());
                scroll = out.scroll;
                if let Some(text) = out.copied {
                    let mut stdout = std::io::stdout();
                    stdout.write_all(selection::osc52(&text).as_bytes())?;
                    stdout.flush()?;
                    notice = Some(format!("copied {} characters", text.chars().count()));
                }
            }
            _ => {}
        }
    }
}
