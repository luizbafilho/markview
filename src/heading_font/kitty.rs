//! kitty.conf, where `include` is read in place and later settings win.

use std::path::{Path, PathBuf};

use super::{Families, config_home, expand_home, system::is_monospace};

/// kitty draws only mono or dual width fonts and swaps anything else for
/// `monospace`, so non-monospace families are dropped here too.
pub fn families() -> anyhow::Result<Families> {
    let dir = match std::env::var_os("KITTY_CONFIG_DIRECTORY") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => config_home().map(|home| home.join("kitty")),
    };
    let mut fonts = Fonts::default();
    if let Some(dir) = dir {
        read(&dir.join("kitty.conf"), &mut fonts, &mut Vec::new());
    }
    Ok(Families {
        terminal: monospace(fonts.regular)?,
        heading: monospace(fonts.bold)?,
    })
}

fn monospace(family: Option<String>) -> anyhow::Result<Option<String>> {
    match family {
        Some(family) if is_monospace(&family)? => Ok(Some(family)),
        _ => Ok(None),
    }
}

#[derive(Debug, Default)]
struct Fonts {
    regular: Option<String>,
    bold: Option<String>,
}

fn read(path: &Path, fonts: &mut Fonts, seen: &mut Vec<PathBuf>) {
    if seen.iter().any(|p| p == path) {
        return;
    }
    seen.push(path.to_path_buf());
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once(char::is_whitespace) else {
            continue;
        };
        let value = value.trim();
        match key {
            "include" => {
                if let Some(include) = expand_home(value) {
                    read(&dir.join(include), fonts, seen);
                }
            }
            "font_family" => fonts.regular = family_name(value),
            "bold_font" => fonts.bold = family_name(value),
            _ => {}
        }
    }
}

/// Accepts both `font_family Name With Spaces` and `font_family family="Name" style=Bold`.
fn family_name(value: &str) -> Option<String> {
    let name = match value.strip_prefix("family=") {
        Some(rest) => {
            if let Some(quoted) = rest.strip_prefix('"') {
                quoted.split('"').next()?
            } else if let Some(quoted) = rest.strip_prefix('\'') {
                quoted.split('\'').next()?
            } else {
                rest.split_whitespace().next()?
            }
        }
        None => value,
    };
    (!name.is_empty() && name != "auto").then(|| name.to_owned())
}
