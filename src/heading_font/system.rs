//! Lookups against the system's installed fonts.

use ab_glyph::FontVec;
use anyhow::Context;

/// The bold face of `family`, or of Menlo when `family` is `None` or not
/// installed. macOS has no fontconfig-style `monospace` alias, and Menlo is
/// the monospace font every Mac ships with.
#[cfg(target_os = "macos")]
pub fn load_font(family: Option<&str>) -> anyhow::Result<FontVec> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let bold = |name: &str| {
        db.query(&fontdb::Query {
            families: &[fontdb::Family::Name(name)],
            weight: fontdb::Weight::BOLD,
            ..fontdb::Query::default()
        })
    };
    let id = family
        .and_then(bold)
        .or_else(|| bold("Menlo"))
        .context("Menlo Bold is not among the system fonts")?;
    db.with_face_data(id, |data, index| {
        FontVec::try_from_vec_and_index(data.to_vec(), index)
    })
    .context("reading the heading font")?
    .context("parsing the heading font")
}

#[cfg(target_os = "macos")]
pub fn is_monospace(family: &str) -> anyhow::Result<bool> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    Ok(db.faces().any(|face| {
        face.monospaced
            && face
                .families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
    }))
}

/// The bold face fontconfig resolves for `family`, or for `monospace` when
/// `family` is `None` or not installed.
#[cfg(not(target_os = "macos"))]
pub fn load_font(family: Option<&str>) -> anyhow::Result<FontVec> {
    let installed = match family {
        Some(family) => {
            let (families, file) = fc_match(family)?;
            families
                .split(',')
                .any(|f| f.eq_ignore_ascii_case(family))
                .then_some(file)
        }
        None => None,
    };
    let path = match installed {
        Some(file) => file,
        None => fc_match("monospace")?.1,
    };
    let data = std::fs::read(&path).with_context(|| format!("reading font {path}"))?;
    FontVec::try_from_vec(data).with_context(|| format!("parsing font {path}"))
}

/// Whether fontconfig spaces `family` as mono or dual width, the fonts kitty
/// accepts for its text.
#[cfg(not(target_os = "macos"))]
pub fn is_monospace(family: &str) -> anyhow::Result<bool> {
    let out = std::process::Command::new("fc-list")
        .args([&fc_escape(family), "-f", "%{spacing}\n"])
        .output()
        .context("running fc-list")?;
    let text = String::from_utf8(out.stdout).context("fc-list printed non-UTF-8 output")?;
    Ok(text
        .lines()
        .any(|spacing| spacing == "100" || spacing == "90"))
}

/// Returns the comma-separated family names and file of the best bold match.
/// fontconfig always answers with some font, so callers compare the family.
#[cfg(not(target_os = "macos"))]
fn fc_match(family: &str) -> anyhow::Result<(String, String)> {
    let out = std::process::Command::new("fc-match")
        .args([
            &format!("{}:bold", fc_escape(family)),
            "-f",
            "%{family}\n%{file}",
        ])
        .output()
        .context("running fc-match")?;
    let text = String::from_utf8(out.stdout).context("fc-match printed non-UTF-8 output")?;
    let (families, file) = text
        .split_once('\n')
        .with_context(|| format!("fc-match found no font for {family}"))?;
    Ok((families.to_owned(), file.trim().to_owned()))
}

#[cfg(not(target_os = "macos"))]
fn fc_escape(family: &str) -> String {
    family
        .chars()
        .flat_map(|c| {
            let escape = matches!(c, '\\' | '-' | ':' | ',').then_some('\\');
            escape.into_iter().chain([c])
        })
        .collect()
}
