use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use image::DynamicImage;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
};
use ratatui_image::protocol::Protocol;
use ratatui_markdown::{
    highlight::{HighlightHooks, TreeSitterHighlighter},
    markdown::{
        MarkdownRenderer,
        image::{ImagePlacement, ImageResolver},
    },
    theme::RichTextTheme,
};

use crate::{heading_image, kitty, selection, text_sizing, theme::Theme};

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

pub struct Doc {
    pub size: (u16, u16),
    pub lines: Vec<Line<'static>>,
    /// What selecting each of `lines` copies; image headings keep their text here.
    pub rows: Vec<selection::Row>,
    pub images: Vec<ImagePlacement>,
    pub headings: Vec<text_sizing::Heading>,
    pub rules: Vec<usize>,
    /// Non-kitty protocols, per image: (rows cut off the top, visible rows, encoded protocol).
    pub encoded: HashMap<usize, (u16, u16, Protocol)>,
}

pub enum Sizing {
    Osc66,
    Image(heading_image::HeadingFont),
}

pub fn render(
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
    let heading_rows = usize::from(text_sizing::ROWS);
    let rows = out
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let text = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if headings
                .iter()
                .any(|h| (h.row..h.row + heading_rows).contains(&i))
            {
                selection::Row::Heading(text)
            } else if rules.contains(&i) {
                selection::Row::Plain(String::new())
            } else {
                selection::Row::Plain(text)
            }
        })
        .collect();
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
    }
    if !matches!(sizing, Some(Sizing::Osc66)) {
        rules.clear();
    }
    Ok(Doc {
        size: (area.width, area.height),
        lines: out.lines,
        rows,
        images: out.images,
        headings,
        rules,
        encoded: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use terminal_colorsaurus::ThemeMode;

    use super::*;
    use crate::{heading_font, selection::Row};

    #[test]
    fn copied_rows_keep_heading_text_and_drop_heading_rules() {
        let area = Rect::new(0, 0, 40, 20);
        let theme = Theme::new(ThemeMode::Light);
        let md = "# Title\n\nBody text\n";
        let plain = |s: &str| Row::Plain(s.to_owned());

        let plain_doc = render(md, Path::new("."), area, &theme, (10, 20), None).unwrap();
        assert_eq!(
            plain_doc.rows.get(..3),
            Some(&[plain("Title"), plain(""), plain("")][..])
        );
        assert!(plain_doc.rules.is_empty(), "no OSC 66 rules to draw");

        let osc = render(
            md,
            Path::new("."),
            area,
            &theme,
            (10, 20),
            Some(&Sizing::Osc66),
        )
        .unwrap();
        assert_eq!(
            osc.rows.get(..4),
            Some(
                &[
                    Row::Heading("Title".to_owned()),
                    Row::Heading(String::new()),
                    plain(""),
                    plain("")
                ][..]
            )
        );
        assert_eq!(osc.rules, [2]);
    }

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
