use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
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

// Catppuccin Latte, https://catppuccin.com/palette
mod latte {
    use ratatui::style::Color;
    const fn rgb(hex: u32) -> Color {
        Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
    pub const PINK: Color = rgb(0xea76cb);
    pub const MAUVE: Color = rgb(0x8839ef);
    pub const RED: Color = rgb(0xd20f39);
    pub const PEACH: Color = rgb(0xfe640b);
    pub const YELLOW: Color = rgb(0xdf8e1d);
    pub const GREEN: Color = rgb(0x40a02b);
    pub const SKY: Color = rgb(0x04a5e5);
    pub const SAPPHIRE: Color = rgb(0x209fb5);
    pub const BLUE: Color = rgb(0x1e66f5);
    pub const LAVENDER: Color = rgb(0x7287fd);
    pub const TEXT: Color = rgb(0x4c4f69);
    pub const SUBTEXT0: Color = rgb(0x6c6f85);
    pub const OVERLAY1: Color = rgb(0x8c8fa1);
    pub const OVERLAY0: Color = rgb(0x9ca0b0);
    pub const SURFACE0: Color = rgb(0xccd0da);
    pub const BASE: Color = rgb(0xeff1f5);
    pub const MANTLE: Color = rgb(0xe6e9ef);
}

/// `ThemeConfig` has no background slot, and Mermaid picks its light or dark
/// palette from `get_background_color`, so the background is set here.
struct Latte(ThemeConfig);

impl Latte {
    fn new() -> Self {
        use latte::*;
        Self(ThemeConfig {
            r#gen: Generation(1),
            text_color: TEXT,
            muted_text_color: SUBTEXT0,
            primary_color: MAUVE,
            popup_selected_background: SURFACE0,
            border_color: OVERLAY0,
            focused_border_color: LAVENDER,
            secondary_color: BLUE,
            info_color: SAPPHIRE,
            json_key_color: BLUE,
            json_string_color: GREEN,
            json_number_color: PEACH,
            json_bool_color: MAUVE,
            json_null_color: OVERLAY1,
            accent_yellow: YELLOW,
            code_colors: CodeColors {
                comment: OVERLAY1,
                keyword: MAUVE,
                string: GREEN,
                string_escape: PINK,
                number: PEACH,
                constant: PEACH,
                function: BLUE,
                r#type: YELLOW,
                variable: TEXT,
                property: LAVENDER,
                operator: SKY,
                punctuation: OVERLAY1,
                attribute: YELLOW,
                tag: MAUVE,
                label: SAPPHIRE,
                error: RED,
            },
        })
    }
}

impl RichTextTheme for Latte {
    fn generation(&self) -> Generation { self.0.generation() }
    fn get_text_color(&self) -> Color { self.0.get_text_color() }
    fn get_muted_text_color(&self) -> Color { self.0.get_muted_text_color() }
    fn get_primary_color(&self) -> Color { self.0.get_primary_color() }
    fn get_popup_selected_background(&self) -> Color { self.0.get_popup_selected_background() }
    fn get_border_color(&self) -> Color { self.0.get_border_color() }
    fn get_focused_border_color(&self) -> Color { self.0.get_focused_border_color() }
    fn get_secondary_color(&self) -> Color { self.0.get_secondary_color() }
    fn get_info_color(&self) -> Color { self.0.get_info_color() }
    fn get_json_key_color(&self) -> Color { self.0.get_json_key_color() }
    fn get_json_string_color(&self) -> Color { self.0.get_json_string_color() }
    fn get_json_number_color(&self) -> Color { self.0.get_json_number_color() }
    fn get_json_bool_color(&self) -> Color { self.0.get_json_bool_color() }
    fn get_json_null_color(&self) -> Color { self.0.get_json_null_color() }
    fn get_accent_yellow(&self) -> Color { self.0.get_accent_yellow() }
    fn get_code_colors(&self) -> CodeColors { self.0.get_code_colors() }
    fn get_background_color(&self) -> Color { latte::BASE }
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

fn rule_style() -> Style {
    Style::new().bg(latte::BASE).fg(latte::SURFACE0)
}

fn kitty_id(image_index: usize) -> u32 {
    0x4D_4B_00 + image_index as u32 + 1
}

fn render(md: &str, base: &Path, area: Rect, theme: &Latte, cell_px: (u16, u16), sized_headings: bool) -> Doc {
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
    let (headings, rules) = text_sizing::extract(&mut out.lines, &mut out.images, sized_headings, area.width, rule_style());
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
    let theme = Latte::new();

    let mut terminal = ratatui::init();
    let picker = Picker::from_query_stdio().context("querying terminal graphics support")?;
    let sizing_supported = text_sizing::probe().context("probing text sizing support")?;
    terminal.clear()?;
    execute!(std::io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal, &path, &md, &base, &theme, &picker, sizing_supported);
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
    theme: &Latte,
    picker: &Picker,
    sizing_supported: bool,
) -> anyhow::Result<()> {
    let mut scroll: usize = 0;
    let mut doc: Option<Doc> = None;
    let mut page: usize;
    let protocol = format!("{:?}", picker.protocol_type());
    let tmux = std::env::var_os("TMUX").is_some();
    let mut sizing = sizing_supported;
    let mut drawn: Vec<text_sizing::Placed> = Vec::new();

    loop {
        let [body, status] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(Rect::from((Default::default(), terminal.size()?)));
        let block = Block::new()
            .padding(Padding::new(2, 2, 1, 0))
            .style(Style::new().bg(latte::BASE).fg(latte::TEXT));
        let inner = block.inner(body);

        // ratatui clears the whole screen on resize, taking every sized heading with it.
        if doc.as_ref().is_some_and(|d| d.size != (inner.width, inner.height)) {
            drawn.clear();
        }
        if doc.as_ref().map(|d| d.size) != Some((inner.width, inner.height)) {
            let d = render(md, base, inner, theme, picker.font_size(), sizing);
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
                let base = Style::new().bg(latte::BASE).fg(latte::TEXT);
                let y = inner.y + (h.row - scroll) as u16;
                text_sizing::place(&d.lines[h.row], h.level, inner.x, y, inner.width, base)
            })
            .collect();
        placed.extend(
            d.rules
                .iter()
                .copied()
                .filter(|&r| r >= scroll && r - scroll < inner.height as usize)
                .map(|r| text_sizing::rule(inner.x, inner.y + (r - scroll) as u16, inner.width, rule_style())),
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
            let headings = match (sizing_supported, sizing) {
                (false, _) => "unsupported",
                (true, true) => "on",
                (true, false) => "off",
            };
            let status_line = Line::from(format!(
                " {path}  {pct}%  ·  images: {protocol}  ·  sized headings: {headings}  ·  j/k ↑/↓ scroll · space/b page · g/G top/bottom · t sizing · q quit"
            ))
            .style(Style::new().bg(latte::MANTLE).fg(latte::SUBTEXT0));
            f.render_widget(status_line, status);
        })?;
        text_sizing::draw(&mut std::io::stdout(), &fresh)?;
        drawn = placed;

        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('j') | KeyCode::Down => scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => scroll = scroll.saturating_sub(1),
                KeyCode::Char(' ') | KeyCode::PageDown | KeyCode::Char('d') => scroll += page,
                KeyCode::Char('b') | KeyCode::PageUp | KeyCode::Char('u') => scroll = scroll.saturating_sub(page),
                KeyCode::Char('g') | KeyCode::Home => scroll = 0,
                KeyCode::Char('G') | KeyCode::End => scroll = usize::MAX / 2,
                KeyCode::Char('t') if sizing_supported => {
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
