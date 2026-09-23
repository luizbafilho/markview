use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use catppuccin::PALETTE;
use image::DynamicImage;
use ratatui::{
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind},
        execute,
    },
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
    Frame,
};
use ratatui_image::{
    Image, Resize,
    picker::{Picker, ProtocolType},
    protocol::Protocol,
};

mod heading_image;
mod kitty;
mod text_sizing;
use ratatui_markdown::{
    highlight::{HighlightHooks, TreeSitterHighlighter},
    markdown::{
        MarkdownRenderer,
        image::{ImagePlacement, ImageResolver},
    },
    theme::{CodeColors, Generation, RichTextTheme, ThemeConfig},
};
use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};

fn rgb(c: catppuccin::Color) -> Color {
    Color::Rgb(c.rgb.r, c.rgb.g, c.rgb.b)
}

/// Catppuccin Latte in light terminals, Frappé in dark ones.
/// `ThemeConfig` has no background slot, and Mermaid picks its light or dark
/// palette from `get_background_color`, so the background is kept here.
struct Theme {
    mode: ThemeMode,
    config: ThemeConfig,
    background: Color,
    bar_background: Color,
    bar_text: Color,
    rule: Color,
}

impl Theme {
    fn new(mode: ThemeMode) -> Self {
        let (flavor, generation) = match mode {
            ThemeMode::Light => (&PALETTE.latte, Generation(1)),
            ThemeMode::Dark => (&PALETTE.frappe, Generation(2)),
        };
        let c = &flavor.colors;
        Self {
            mode,
            config: ThemeConfig {
                r#gen: generation,
                text_color: rgb(c.text),
                muted_text_color: rgb(c.subtext0),
                primary_color: rgb(c.mauve),
                popup_selected_background: rgb(c.surface0),
                border_color: rgb(c.overlay0),
                focused_border_color: rgb(c.lavender),
                secondary_color: rgb(c.blue),
                info_color: rgb(c.sapphire),
                json_key_color: rgb(c.blue),
                json_string_color: rgb(c.green),
                json_number_color: rgb(c.peach),
                json_bool_color: rgb(c.mauve),
                json_null_color: rgb(c.overlay1),
                accent_yellow: rgb(c.yellow),
                code_colors: CodeColors {
                    comment: rgb(c.overlay1),
                    keyword: rgb(c.mauve),
                    string: rgb(c.green),
                    string_escape: rgb(c.pink),
                    number: rgb(c.peach),
                    constant: rgb(c.peach),
                    function: rgb(c.blue),
                    r#type: rgb(c.yellow),
                    variable: rgb(c.text),
                    property: rgb(c.lavender),
                    operator: rgb(c.sky),
                    punctuation: rgb(c.overlay1),
                    attribute: rgb(c.yellow),
                    tag: rgb(c.mauve),
                    label: rgb(c.sapphire),
                    error: rgb(c.red),
                },
            },
            background: rgb(c.base),
            bar_background: rgb(c.mantle),
            bar_text: rgb(c.subtext0),
            rule: rgb(c.surface0),
        }
    }

    fn base_style(&self) -> Style {
        Style::new().bg(self.background).fg(self.get_text_color())
    }

    fn rule_style(&self) -> Style {
        Style::new().bg(self.background).fg(self.rule)
    }
}

impl RichTextTheme for Theme {
    fn generation(&self) -> Generation { self.config.generation() }
    fn get_text_color(&self) -> Color { self.config.get_text_color() }
    fn get_muted_text_color(&self) -> Color { self.config.get_muted_text_color() }
    fn get_primary_color(&self) -> Color { self.config.get_primary_color() }
    fn get_popup_selected_background(&self) -> Color { self.config.get_popup_selected_background() }
    fn get_border_color(&self) -> Color { self.config.get_border_color() }
    fn get_focused_border_color(&self) -> Color { self.config.get_focused_border_color() }
    fn get_secondary_color(&self) -> Color { self.config.get_secondary_color() }
    fn get_info_color(&self) -> Color { self.config.get_info_color() }
    fn get_json_key_color(&self) -> Color { self.config.get_json_key_color() }
    fn get_json_string_color(&self) -> Color { self.config.get_json_string_color() }
    fn get_json_number_color(&self) -> Color { self.config.get_json_number_color() }
    fn get_json_bool_color(&self) -> Color { self.config.get_json_bool_color() }
    fn get_json_null_color(&self) -> Color { self.config.get_json_null_color() }
    fn get_accent_yellow(&self) -> Color { self.config.get_accent_yellow() }
    fn get_code_colors(&self) -> CodeColors { self.config.get_code_colors() }
    fn get_background_color(&self) -> Color { self.background }
}

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
        let (fw, fh) = (self.cell_px.0 as f64, self.cell_px.1 as f64);
        let (pw, ph) = (img.width() as f64, img.height() as f64);
        let max_w = max_w.min(kitty::MAX_CELLS) as f64;
        let max_h = max_h.min(kitty::MAX_CELLS) as f64;
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
        Span::styled(format!("[image not loaded: {label}]"), Style::new().italic().fg(self.fallback_color))
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
    Image(ab_glyph::FontVec),
}

fn render(md: &str, base: &Path, area: Rect, theme: &Theme, cell_px: (u16, u16), sizing: Option<&Sizing>) -> Doc {
    let width = area.width as usize;
    let highlighter = Arc::new(TreeSitterHighlighter::new().with_code_colors(theme.get_code_colors()));
    let hooks = HighlightHooks::new(highlighter, width).with_border_color(theme.get_border_color());
    let renderer = MarkdownRenderer::new(width).with_render_hooks(Box::new(hooks));
    let mut resolver = FsResolver {
        base: base.to_path_buf(),
        cell_px,
        fallback_color: theme.get_muted_text_color(),
    };
    let (mut blocks, resolved) = renderer.parse_with_images(md, &mut resolver);
    text_sizing::tag(&mut blocks);
    let max_img_h = area.height.saturating_sub(2).max(1);
    let mut out = renderer.render_full(&blocks, theme, &resolved, &mut resolver, area.width, max_img_h);
    let (mut headings, mut rules) =
        text_sizing::extract(&mut out.lines, &mut out.images, sizing.is_some(), area.width, theme.rule_style());
    if let Some(Sizing::Image(font)) = sizing {
        for h in headings.drain(..) {
            let line = std::mem::take(&mut out.lines[h.row]);
            let (image, cols) =
                heading_image::rasterize(font, &line, h.level, cell_px, area.width, theme.get_text_color(), theme.background);
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
    Doc {
        size: (area.width, area.height),
        lines: out.lines,
        images: out.images,
        headings,
        rules,
        encoded: HashMap::new(),
    }
}

/// Draws each image into the blank rows the renderer reserved for it,
/// cropping the part that is scrolled out of the viewport.
fn draw_images(f: &mut Frame, doc: &mut Doc, inner: Rect, scroll: usize, picker: &Picker) {
    for (i, p) in doc.images.iter().enumerate() {
        let top = p.row as i64 - scroll as i64;
        let bottom = top + p.height_cells as i64;
        let vis_top = top.max(0);
        let vis_bottom = bottom.min(inner.height as i64);
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
                    if let Some(c) = buf.cell_mut((inner.x + p.col as u16 + x, inner.y + vis_top as u16 + y)) {
                        c.set_symbol(&kitty::cell(cut + y, x)).set_fg(fg);
                    }
                }
            }
            continue;
        }

        if !matches!(doc.encoded.get(&i), Some((c, r, _)) if *c == cut && *r == rows) {
            let px_per_row = p.image.height() as f64 / p.height_cells as f64;
            let y = (cut as f64 * px_per_row) as u32;
            let h = ((rows as f64 * px_per_row) as u32).clamp(1, p.image.height() - y);
            let cropped = p.image.crop_imm(0, y, p.image.width(), h);
            let proto = picker
                .new_protocol(cropped, Rect::new(0, 0, p.width_cells, rows), Resize::Fit(None))
                .expect("encoding image for terminal");
            doc.encoded.insert(i, (cut, rows, proto));
        }

        let rect = Rect::new(inner.x + p.col as u16, inner.y + vis_top as u16, p.width_cells.min(inner.width), rows);
        f.render_widget(Image::new(&doc.encoded[&i].2), rect);
    }
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).context("usage: mkviewer <file.md>")?;
    let md = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
    let base = Path::new(&path).parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut terminal = ratatui::init();
    let theme = Theme::new(theme_mode(QueryOptions::default()).context("querying terminal background")?);
    let picker = Picker::from_query_stdio().context("querying terminal graphics support")?;
    let sizing = if text_sizing::probe().context("probing text sizing support")? {
        Some(Sizing::Osc66)
    } else if picker.protocol_type() == ProtocolType::Kitty {
        Some(Sizing::Image(heading_image::load_font()?))
    } else {
        None
    };
    terminal.clear()?;
    execute!(std::io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal, &path, &md, &base, theme, &picker, sizing.as_ref());
    if picker.protocol_type() == ProtocolType::Kitty {
        std::io::stdout().write_all(kitty::delete_all(std::env::var_os("TMUX").is_some()).as_bytes())?;
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
    sizing_mode: Option<&Sizing>,
) -> anyhow::Result<()> {
    let mut scroll: usize = 0;
    let mut doc: Option<Doc> = None;
    let mut page: usize;
    let protocol = format!("{:?}", picker.protocol_type());
    let tmux = std::env::var_os("TMUX").is_some();
    let mut sizing = true;
    let mut drawn: Vec<text_sizing::Placed> = Vec::new();

    loop {
        let [body, status] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(Rect::from((Default::default(), terminal.size()?)));
        let block = Block::new()
            .padding(Padding::new(2, 2, 1, 0))
            .style(theme.base_style());
        let inner = block.inner(body);

        // ratatui clears the whole screen on resize, taking every sized heading with it.
        if doc.as_ref().is_some_and(|d| d.size != (inner.width, inner.height)) {
            drawn.clear();
        }
        if doc.as_ref().map(|d| d.size) != Some((inner.width, inner.height)) {
            let d = render(md, base, inner, &theme, picker.font_size(), sizing_mode.filter(|_| sizing));
            if picker.protocol_type() == ProtocolType::Kitty {
                let mut out = std::io::stdout().lock();
                for (i, p) in d.images.iter().enumerate() {
                    let data = kitty::transmit(&p.image, kitty_id(i), p.width_cells, p.height_cells, tmux);
                    out.write_all(data.as_bytes())?;
                }
                out.flush()?;
            }
            doc = Some(d);
        }

        let d = doc.as_ref().unwrap();
        page = inner.height.max(1) as usize;
        scroll = scroll.min(d.lines.len().saturating_sub(page));
        let mut placed: Vec<text_sizing::Placed> = d
            .headings
            .iter()
            .filter(|h| h.row >= scroll && h.row - scroll + text_sizing::ROWS as usize <= inner.height as usize)
            .filter_map(|h| {
                let y = inner.y + (h.row - scroll) as u16;
                text_sizing::place(&d.lines[h.row], h.level, inner.x, y, inner.width, theme.base_style())
            })
            .collect();
        placed.extend(
            d.rules
                .iter()
                .copied()
                .filter(|&r| r >= scroll && r - scroll < inner.height as usize)
                .map(|r| text_sizing::rule(inner.x, inner.y + (r - scroll) as u16, inner.width, theme.rule_style())),
        );
        let gone: Vec<_> = drawn.iter().filter(|p| !placed.contains(p)).cloned().collect();
        let fresh: Vec<_> = placed.iter().filter(|p| !drawn.contains(p)).cloned().collect();
        text_sizing::erase(&mut std::io::stdout(), &gone)?;

        terminal.draw(|f| {
            let doc = doc.as_mut().unwrap();
            f.render_widget(Paragraph::new(doc.lines.clone()).block(block).scroll((scroll as u16, 0)), body);
            draw_images(f, doc, inner, scroll, picker);
            for p in &placed {
                text_sizing::mark_skip(f.buffer_mut(), p);
            }

            let total = doc.lines.len();
            let pct = if total <= page { 100 } else { scroll * 100 / (total - page) };
            let headings = match (sizing_mode, sizing) {
                (None, _) => "unsupported",
                (Some(_), false) => "off",
                (Some(Sizing::Osc66), true) => "on",
                (Some(Sizing::Image(_)), true) => "image",
            };
            let status_line = Line::from(format!(
                " {path}  {pct}%  ·  images: {protocol}  ·  sized headings: {headings}  ·  j/k ↑/↓ scroll · space/b page · g/G top/bottom · t sizing · q quit"
            ))
            .style(Style::new().bg(theme.bar_background).fg(theme.bar_text));
            f.render_widget(status_line, status);
        })?;
        text_sizing::draw(&mut std::io::stdout(), &fresh)?;
        drawn = placed;

        let event = loop {
            if event::poll(THEME_POLL)? {
                break Some(event::read()?);
            }
            let mode = theme_mode(QueryOptions::default()).context("querying terminal background")?;
            if mode != theme.mode {
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
                KeyCode::Char(' ') | KeyCode::PageDown | KeyCode::Char('d') => scroll += page,
                KeyCode::Char('b') | KeyCode::PageUp | KeyCode::Char('u') => scroll = scroll.saturating_sub(page),
                KeyCode::Char('g') | KeyCode::Home => scroll = 0,
                KeyCode::Char('G') | KeyCode::End => scroll = usize::MAX / 2,
                KeyCode::Char('t') if sizing_mode.is_some() => {
                    sizing = !sizing;
                    doc = None;
                }
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
    fn light_mode_uses_latte_and_dark_mode_uses_frappe() {
        let light = Theme::new(ThemeMode::Light);
        let dark = Theme::new(ThemeMode::Dark);
        assert_eq!(light.get_background_color(), rgb(PALETTE.latte.colors.base));
        assert_eq!(light.get_text_color(), rgb(PALETTE.latte.colors.text));
        assert_eq!(dark.get_background_color(), rgb(PALETTE.frappe.colors.base));
        assert_eq!(dark.get_text_color(), rgb(PALETTE.frappe.colors.text));
        assert_ne!(light.generation(), dark.generation());
    }

    #[test]
    fn image_headings_are_drawn_in_the_current_themes_colors() {
        let sizing = Sizing::Image(heading_image::load_font().unwrap());
        let area = Rect::new(0, 0, 80, 40);
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let theme = Theme::new(mode);
            let doc = render("# Title\n\n## Sub\n\n### Third\n", Path::new("."), area, &theme, (10, 20), Some(&sizing));
            let Color::Rgb(r, g, b) = theme.background else { unreachable!() };

            assert_eq!(doc.images.len(), 3, "{mode:?}");
            // Inline formatting gives every heading span the text color.
            let Color::Rgb(tr, tg, tb) = theme.get_text_color() else { unreachable!() };
            for img in &doc.images {
                let px = img.image.to_rgba8();
                assert_eq!(px.get_pixel(0, 0).0[..3], [r, g, b], "{mode:?} background");
                assert!(px.pixels().any(|p| p.0[..3] == [tr, tg, tb]), "{mode:?} text color missing");
            }
        }
    }
}
