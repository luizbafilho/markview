//! Catppuccin Latte in light terminals, Frappé in dark ones, <https://catppuccin.com/palette>

use catppuccin::PALETTE;
use ratatui::style::{Color, Style};
use ratatui_markdown::theme::{CodeColors, Generation, RichTextTheme, ThemeConfig};
use terminal_colorsaurus::ThemeMode;

const fn rgb(c: catppuccin::Color) -> Color {
    Color::Rgb(c.rgb.r, c.rgb.g, c.rgb.b)
}

/// `ThemeConfig` has no background slot, and Mermaid picks its light or dark
/// palette from `get_background_color`, so the background is kept here.
pub struct Theme {
    mode: ThemeMode,
    config: ThemeConfig,
    background: Color,
    bar_background: Color,
    bar_text: Color,
    rule: Color,
}

impl Theme {
    pub fn new(mode: ThemeMode) -> Self {
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

    pub const fn mode(&self) -> ThemeMode {
        self.mode
    }

    pub fn base_style(&self) -> Style {
        Style::new().bg(self.background).fg(self.get_text_color())
    }

    pub const fn rule_style(&self) -> Style {
        Style::new().bg(self.background).fg(self.rule)
    }

    pub const fn bar_style(&self) -> Style {
        Style::new().bg(self.bar_background).fg(self.bar_text)
    }
}

impl RichTextTheme for Theme {
    fn generation(&self) -> Generation {
        self.config.generation()
    }
    fn get_text_color(&self) -> Color {
        self.config.get_text_color()
    }
    fn get_muted_text_color(&self) -> Color {
        self.config.get_muted_text_color()
    }
    fn get_primary_color(&self) -> Color {
        self.config.get_primary_color()
    }
    fn get_popup_selected_background(&self) -> Color {
        self.config.get_popup_selected_background()
    }
    fn get_border_color(&self) -> Color {
        self.config.get_border_color()
    }
    fn get_focused_border_color(&self) -> Color {
        self.config.get_focused_border_color()
    }
    fn get_secondary_color(&self) -> Color {
        self.config.get_secondary_color()
    }
    fn get_info_color(&self) -> Color {
        self.config.get_info_color()
    }
    fn get_json_key_color(&self) -> Color {
        self.config.get_json_key_color()
    }
    fn get_json_string_color(&self) -> Color {
        self.config.get_json_string_color()
    }
    fn get_json_number_color(&self) -> Color {
        self.config.get_json_number_color()
    }
    fn get_json_bool_color(&self) -> Color {
        self.config.get_json_bool_color()
    }
    fn get_json_null_color(&self) -> Color {
        self.config.get_json_null_color()
    }
    fn get_accent_yellow(&self) -> Color {
        self.config.get_accent_yellow()
    }
    fn get_code_colors(&self) -> CodeColors {
        self.config.get_code_colors()
    }
    fn get_background_color(&self) -> Color {
        self.background
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
}
