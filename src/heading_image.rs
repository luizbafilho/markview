//! Sized headings drawn as kitty images, for terminals that forward kitty
//! graphics but drop OSC 66 (herdr, whose libghostty-vt ignores it).

use std::process::Command;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use anyhow::Context;
use image::{DynamicImage, Rgba, RgbaImage};
use ratatui::{style::Color, text::Line};

use crate::text_sizing::{self, ROWS};

/// The font fontconfig resolves for `monospace:bold`, the closest match to
/// the bold heading text a terminal would draw.
pub fn load_font() -> anyhow::Result<FontVec> {
    let out = Command::new("fc-match").args(["monospace:bold", "-f", "%{file}"]).output().context("running fc-match")?;
    let path = String::from_utf8(out.stdout).context("fc-match printed a non-UTF-8 path")?;
    let data = std::fs::read(path.trim()).with_context(|| format!("reading font {path}"))?;
    FontVec::try_from_vec(data).with_context(|| format!("parsing font {path}"))
}

/// Draws `line` at `level`'s scale, vertically centred in a `ROWS`-high
/// image no wider than `max_cols` cells. Returns the image and its width in cells.
pub fn rasterize(font: &FontVec, line: &Line, level: u8, cell_px: (u16, u16), max_cols: u16, fg: Color, bg: Color) -> (DynamicImage, u16) {
    let cw = cell_px.0 as f32;
    let glyphs: Vec<(char, Color)> = line
        .spans
        .iter()
        .flat_map(|s| {
            let color = s.style.fg.or(line.style.fg).unwrap_or(fg);
            s.content.chars().map(move |c| (c, color))
        })
        .collect();

    // Size the font so one monospace advance spans `scale` cells, then
    // shrink it if the whole line would not fit.
    let unit = font.as_scaled(PxScale::from(1.0));
    let one_cell = cw * text_sizing::scale(level) / unit.h_advance(font.glyph_id('M'));
    let unit_width: f32 = glyphs.iter().map(|&(c, _)| unit.h_advance(font.glyph_id(c))).sum();
    let px = one_cell.min(max_cols as f32 * cw / unit_width);
    let sf = font.as_scaled(PxScale::from(px));

    let cols = ((unit_width * px / cw).ceil() as u16).clamp(1, max_cols);
    let (w, h) = (cols as u32 * cell_px.0 as u32, ROWS as u32 * cell_px.1 as u32);
    let bg_px = rgba(bg);
    let mut img = RgbaImage::from_pixel(w, h, bg_px);
    let baseline = (h as f32 - sf.height()) / 2.0 + sf.ascent();

    let mut x = 0.0;
    let mut prev = None;
    for (c, color) in glyphs {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            x += sf.kern(p, id);
        }
        let color = rgba(color);
        if let Some(outline) = font.outline_glyph(id.with_scale_and_position(sf.scale(), point(x, baseline))) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                let (px, py) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
                if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                    let dst = img.get_pixel_mut(px as u32, py as u32);
                    for i in 0..3 {
                        dst.0[i] = (color.0[i] as f32 * cov + dst.0[i] as f32 * (1.0 - cov)).round() as u8;
                    }
                }
            });
        }
        x += sf.h_advance(id);
        prev = Some(id);
    }
    (DynamicImage::ImageRgba8(img), cols)
}

fn rgba(c: Color) -> Rgba<u8> {
    match c {
        Color::Rgb(r, g, b) => Rgba([r, g, b, 255]),
        other => unreachable!("the Catppuccin themes only use RGB colors, got {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: (u16, u16) = (10, 20);
    const BG: Color = Color::Rgb(0xef, 0xf1, 0xf5);
    const FG: Color = Color::Rgb(0x4c, 0x4f, 0x69);

    #[test]
    fn h1_fills_two_rows_at_about_twice_the_text_width() {
        let font = load_font().unwrap();
        let (img, cols) = rasterize(&font, &Line::from("Claude Code"), 1, CELL, 80, FG, BG);

        assert_eq!((img.width(), img.height()), (cols as u32 * 10, 2 * 20));
        assert!((18..=24).contains(&cols), "11 chars at 2x took {cols} cells");
        assert!(img.to_rgba8().pixels().any(|p| p.0[..3] != [0xef, 0xf1, 0xf5]), "nothing was drawn");
    }

    #[test]
    fn heading_wider_than_max_cols_is_shrunk_to_fit() {
        let font = load_font().unwrap();
        let (img, cols) = rasterize(&font, &Line::from("A heading that is far too long"), 1, CELL, 20, FG, BG);

        assert!(cols <= 20);
        assert_eq!(img.width(), cols as u32 * 10);
    }

    #[test]
    fn smaller_levels_take_fewer_cells() {
        let font = load_font().unwrap();
        let widths: Vec<u16> = (1..=3).map(|l| rasterize(&font, &Line::from("Heading"), l, CELL, 80, FG, BG).1).collect();

        assert!(widths[0] > widths[1] && widths[1] > widths[2], "{widths:?}");
    }
}
