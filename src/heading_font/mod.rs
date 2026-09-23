//! Font families for sized headings, read from `TTYMARK_HEADING_FONT` and the
//! config of the terminal ttymark runs in.

mod ghostty;
mod kitty;
mod system;

use std::path::PathBuf;

pub use system::load_font;

/// `None` means the family was not configured.
#[derive(Debug, Default)]
pub struct Families {
    /// The terminal's regular text font, whose metrics set the em size.
    pub terminal: Option<String>,
    /// The font headings are drawn in, when it differs from `terminal`.
    pub heading: Option<String>,
}

pub fn configured() -> anyhow::Result<Families> {
    // Both variables are inherited by tmux panes, unlike TERM and TERM_PROGRAM.
    let terminal = if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        kitty::families()?
    } else if std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some() {
        ghostty::families()
    } else {
        Families::default()
    };
    Ok(match std::env::var("TTYMARK_HEADING_FONT") {
        Ok(heading) if !heading.trim().is_empty() => Families {
            heading: Some(heading),
            ..terminal
        },
        _ => terminal,
    })
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home()?.join(".config")))
}

fn expand_home(path: &str) -> Option<PathBuf> {
    match path.strip_prefix("~/") {
        Some(rest) => Some(home()?.join(rest)),
        None => Some(PathBuf::from(path)),
    }
}
