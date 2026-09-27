use std::{fs, path::PathBuf, time::SystemTime};

use anyhow::Context;

/// A file on disk and the last contents markview rendered from it.
#[derive(Debug)]
pub struct Watched {
    path: PathBuf,
    stamp: Option<(SystemTime, u64)>,
    contents: String,
}

fn stamp(meta: &fs::Metadata) -> Option<(SystemTime, u64)> {
    meta.modified().ok().map(|t| (t, meta.len()))
}

impl Watched {
    pub fn open(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let stamp = fs::metadata(&path).ok().as_ref().and_then(stamp);
        let contents =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(Self {
            path,
            stamp,
            contents,
        })
    }

    pub fn contents(&self) -> &str {
        &self.contents
    }

    /// Returns true when the file's text differs from what was last read.
    pub fn refresh(&mut self) -> bool {
        // Stat by name, not through an open handle, so rename-based saves are seen.
        let Ok(meta) = fs::metadata(&self.path) else {
            return false;
        };
        let new_stamp = stamp(&meta);
        if new_stamp == self.stamp {
            return false;
        }
        let Ok(text) = fs::read_to_string(&self.path) else {
            return false;
        };
        self.stamp = new_stamp;
        if text == self.contents {
            return false;
        }
        self.contents = text;
        true
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, path::Path, time::Duration};

    use super::*;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("markview-watch-{}-{name}.md", std::process::id()))
    }

    fn write_at(path: &Path, text: &str, secs: u64) {
        fs::write(path, text).unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn unchanged_file_is_not_reloaded() {
        let path = temp("unchanged");
        write_at(&path, "# a", 1_000);
        let mut w = Watched::open(&path).unwrap();
        assert!(!w.refresh());
        assert_eq!(w.contents(), "# a");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn new_content_is_reloaded() {
        let path = temp("new-content");
        write_at(&path, "# a", 1_000);
        let mut w = Watched::open(&path).unwrap();
        write_at(&path, "# b", 2_000);
        assert!(w.refresh());
        assert_eq!(w.contents(), "# b");
        assert!(!w.refresh());
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn touch_without_new_content_is_not_reloaded() {
        let path = temp("touch");
        write_at(&path, "# a", 1_000);
        let mut w = Watched::open(&path).unwrap();
        write_at(&path, "# a", 2_000);
        assert!(!w.refresh());
        assert_eq!(w.contents(), "# a");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_file_keeps_contents() {
        let path = temp("missing");
        write_at(&path, "# a", 1_000);
        let mut w = Watched::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(!w.refresh());
        assert_eq!(w.contents(), "# a");
    }

    #[test]
    fn rename_over_path_is_reloaded() {
        let path = temp("rename");
        let sibling = temp("rename-tmp");
        write_at(&path, "# a", 1_000);
        let mut w = Watched::open(&path).unwrap();
        write_at(&sibling, "# b", 2_000);
        fs::rename(&sibling, &path).unwrap();
        assert!(w.refresh());
        assert_eq!(w.contents(), "# b");
        fs::remove_file(&path).unwrap();
    }
}
