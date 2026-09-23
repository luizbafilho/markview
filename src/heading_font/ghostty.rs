//! Ghostty's config, where `config-file` is read after the file naming it,
//! `font-family` builds a list whose first entry is the primary font, and an
//! empty value clears that list.

use std::path::{Path, PathBuf};

use super::{Families, config_home, expand_home};

pub fn families() -> Families {
    let Some(dir) = config_home().map(|home| home.join("ghostty")) else {
        return Families::default();
    };
    #[cfg(target_os = "macos")]
    let app_support = super::home()
        .map(|home| home.join("Library/Application Support/com.mitchellh.ghostty/config"));
    #[cfg(not(target_os = "macos"))]
    let app_support = None;
    let mut fonts = Fonts::default();
    let mut seen = Vec::new();
    for path in [dir.join("config.ghostty"), dir.join("config")]
        .into_iter()
        .chain(app_support)
    {
        read(&path, &mut fonts, &mut seen);
    }
    Families {
        terminal: fonts.regular.into_iter().next(),
        heading: fonts.bold.into_iter().next(),
    }
}

#[derive(Debug, Default)]
struct Fonts {
    regular: Vec<String>,
    bold: Vec<String>,
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
    let mut includes = Vec::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "config-file" => {
                let file = unquote(value.strip_prefix('?').unwrap_or(value));
                if let Some(include) = expand_home(file) {
                    includes.push(dir.join(include));
                }
            }
            "font-family" => push(&mut fonts.regular, unquote(value)),
            "font-family-bold" => push(&mut fonts.bold, unquote(value)),
            _ => {}
        }
    }
    for include in &includes {
        read(include, fonts, seen);
    }
}

fn push(list: &mut Vec<String>, value: &str) {
    if value.is_empty() {
        list.clear();
    } else {
        list.push(value.to_owned());
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
}
