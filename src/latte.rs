//! Catppuccin Latte, <https://catppuccin.com/palette>
#![expect(
    clippy::unreadable_literal,
    reason = "hex colors match the published palette"
)]

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
