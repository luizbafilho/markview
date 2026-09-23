use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use image::DynamicImage;
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
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
};
use ratatui_image::{
    Image, Resize,
    picker::{Picker, ProtocolType},
    protocol::Protocol,
};

mod heading_font;
mod heading_image;
mod kitty;
mod text_sizing;
mod theme;
use ratatui_markdown::{
    highlight::{HighlightHooks, TreeSitterHighlighter},
    markdown::{
        MarkdownRenderer,
        image::{ImagePlacement, ImageResolver},
    },
    theme::RichTextTheme,
};
use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};
use theme::Theme;

/// Loads images relative to the markdown file and sizes them using the
/// terminal's real cell size in pixels.
struct FsResolver {
    base: PathBuf,
    cell_px: (u16, u16),
    fallback_color: Color,
}

impl ImageResolver for FsResolver {
    fn resolve(&mut self, path: &str) -> Option<DynamicImage> {
        image::open(self.base.join(path)).ok()
    }

    fn cell_dimensions(&mut self, img: &DynamicImage, max_w: u16, max_h: u16) -> (u16, u16) {
        let (fw, fh) = (f64::from(self.cell_px.0), f64::from(self.cell_px.1));
        let (pw, ph) = (f64::from(img.width()), f64::from(img.height()));
        let max_w = f64::from(max_w.min(kitty::MAX_CELLS));
        let max_h = f64::from(max_h.min(kitty::MAX_CELLS));
        let mut w = (pw / fw).ceil().min(max_w);
        let mut h = (ph * w * fw / pw / fh).ceil();
        if h > max_h {
            h = max_h;
            w = (pw * h * fh / ph / fw).ceil();
        }
        (w.max(1.0) as u16, h.max(1.0) as u16)
    }

    fn fallback(&self, path: &str, alt: &str) -> Span<'static> {
        let label = if alt.is_empty() { path } else { alt };
        Span::styled(
            format!("[image not loaded: {label}]"),
            Style::new().italic().fg(self.fallback_color),
        )
    }
}

struct Doc {
    size: (u16, u16),
    lines: Vec<Line<'static>>,
    images: Vec<ImagePlacement>,
    headings: Vec<text_sizing::Heading>,
    rules: Vec<usize>,
    /// Non-kitty protocols, per image: (rows cut off the top, visible rows, encoded protocol).
    encoded: HashMap<usize, (u16, u16, Protocol)>,
}

/// Idle time before asking the terminal whether its background changed.
const THEME_POLL: Duration = Duration::from_secs(1);

fn kitty_id(image_index: usize) -> u32 {
    0x4D_4B_00 + image_index as u32 + 1
}

enum Sizing {
    Osc66,
    Image(heading_image::HeadingFont),
}

fn render(
    md: &str,
    base: &Path,
    area: Rect,
    theme: &Theme,
    cell_px: (u16, u16),
    sizing: Option<&Sizing>,
) -> anyhow::Result<Doc> {
    let width = area.width as usize;
    let highlighter =
        Arc::new(TreeSitterHighlighter::new().with_code_colors(theme.get_code_colors()));
    let hooks = HighlightHooks::new(highlighter, width).with_border_color(theme.get_border_color());
    let renderer = MarkdownRenderer::new(width).with_render_hooks(Box::new(hooks));
    let mut resolver = FsResolver {
        base: base.to_path_buf(),
        cell_px,
        fallback_color: theme.get_muted_text_color(),
    };
    let (mut blocks, loaded) = renderer.parse_with_images(md, &mut resolver);
    text_sizing::tag(&mut blocks);
    let max_img_h = area.height.saturating_sub(2).max(1);
    let mut out = renderer.render_full(
        &blocks,
        theme,
        &loaded,
        &mut resolver,
        area.width,
        max_img_h,
    );
    let (mut headings, mut rules) = text_sizing::extract(
        &mut out.lines,
        &mut out.images,
        sizing.is_some(),
        area.width,
        theme.rule_style(),
    );
    if let Some(Sizing::Image(font)) = sizing {
        for h in headings.drain(..) {
            let Some(line) = out.lines.get_mut(h.row).map(std::mem::take) else {
                continue;
            };
            let (image, cols) = heading_image::rasterize(
                font,
                &line,
                h.level,
                cell_px,
                area.width,
                theme.get_text_color(),
                theme.get_background_color(),
            )?;
            out.images.push(ImagePlacement {
                row: h.row,
                col: 0,
                width_cells: cols,
                height_cells: text_sizing::ROWS,
                image,
                crop: None,
            });
        }
        rules.clear();
    }
    Ok(Doc {
        size: (area.width, area.height),
        lines: out.lines,
        images: out.images,
        headings,
        rules,
        encoded: HashMap::new(),
    })
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
    let md = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
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
        &md,
        &base,
        theme,
        &picker,
        sizing.as_ref(),
    );
    if picker.protocol_type() == ProtocolType::Kitty {
        std::io::stdout()
            .write_all(kitty::delete_all(std::env::var_os("TMUX").is_some()).as_bytes())?;
    }
    execute!(std::io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    path: &str,
    md: &str,
    base: &Path,
    mut theme: Theme,
    picker: &Picker,
    sizing: Option<&Sizing>,
) -> anyhow::Result<()> {
    let mut scroll: usize = 0;
    let mut doc: Option<Doc> = None;
    let mut page: usize;
    let tmux = std::env::var_os("TMUX").is_some();
    let mut drawn: Vec<text_sizing::Placed> = Vec::new();

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
                let d = render(md, base, inner, &theme, picker.font_size(), sizing)?;
                if picker.protocol_type() == ProtocolType::Kitty {
                    let mut out = std::io::stdout().lock();
                    for (i, p) in d.images.iter().enumerate() {
                        let data = kitty::transmit(
                            &p.image,
                            kitty_id(i),
                            p.width_cells,
                            p.height_cells,
                            tmux,
                        )?;
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

        terminal.try_draw(|f| {
            f.render_widget(
                Paragraph::new(d.lines.clone())
                    .block(block)
                    .scroll((scroll as u16, 0)),
                body,
            );
            draw_images(f, d, inner, scroll, picker).map_err(std::io::Error::other)?;
            for p in &placed {
                text_sizing::mark_skip(f.buffer_mut(), p);
            }

            let total = d.lines.len();
            let pct = if total <= page {
                100
            } else {
                scroll * 100 / (total - page)
            };
            let status_line = Line::from(format!(
                " {path}  {pct}%  ·  j/k ↑/↓ scroll · space/b page · g/G top/bottom · q quit"
            ))
            .style(theme.bar_style());
            f.render_widget(status_line, status);
            Ok::<(), std::io::Error>(())
        })?;
        text_sizing::draw(&mut std::io::stdout(), &fresh)?;
        drawn = placed;

        let event = loop {
            if event::poll(THEME_POLL)? {
                break Some(event::read()?);
            }
            if let Ok(mode) = theme_mode(QueryOptions::default())
                && mode != theme.mode()
            {
                theme = Theme::new(mode);
                doc = None;
                break None;
            }
        };
        let Some(event) = event else { continue };
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
            Event::Mouse(m) => match m.kind {
                MouseEventKind::ScrollDown => scroll += 3,
                MouseEventKind::ScrollUp => scroll = scroll.saturating_sub(3),
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_headings_are_drawn_in_the_current_themes_colors() {
        let sizing = Sizing::Image(
            heading_image::HeadingFont::load(&heading_font::Families::default()).unwrap(),
        );
        let area = Rect::new(0, 0, 80, 40);
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let theme = Theme::new(mode);
            let md = "# Title\n\n## Sub\n\n### Third\n";
            let doc = render(md, Path::new("."), area, &theme, (10, 20), Some(&sizing)).unwrap();
            let Color::Rgb(r, g, b) = theme.get_background_color() else {
                unreachable!()
            };

            assert_eq!(doc.images.len(), 3, "{mode:?}");
            // Inline formatting gives every heading span the text color.
            let Color::Rgb(tr, tg, tb) = theme.get_text_color() else {
                unreachable!()
            };
            for img in &doc.images {
                let px = img.image.to_rgba8();
                assert_eq!(px.get_pixel(0, 0).0[..3], [r, g, b], "{mode:?} background");
                assert!(
                    px.pixels().any(|p| p.0[..3] == [tr, tg, tb]),
                    "{mode:?} text color missing"
                );
            }
        }
    }
}
