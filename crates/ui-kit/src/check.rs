//! Theme package checks: format, field references, resource limits. Shared by the `theme-check`
//! command-line tool and theme import on the Themes page.

use std::path::{Path, PathBuf};

use crate::theme::{MAX_IMAGE_SIDE, MAX_PACK_BYTES, Theme};

/// Built-in modules a theme may replace; IDs with a `.` are plugin modules and aren't checked.
pub const MODULES: [&str; 9] = ["monster", "buffs", "damage", "area", "player", "clock", "weapon", "base", "perf"];

#[derive(Debug, Default)]
pub struct Report {
    pub theme: Option<Theme>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub bytes: u64,
    pub files: usize,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn image_size(path: &Path) -> Option<(u32, u32)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => image::image_dimensions(path).ok(),
        "svg" => {
            let bytes = std::fs::read(path).ok()?;
            let t = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default()).ok()?;
            Some((t.size().width().ceil() as u32, t.size().height().ceil() as u32))
        }
        _ => None,
    }
}

pub fn check(dir: &Path) -> Report {
    let mut r = Report::default();
    let mut files = Vec::new();
    walk(dir, &mut files);
    r.files = files.len();
    for f in &files {
        r.bytes += std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "png" | "svg") {
            match image_size(f) {
                Some((w, h)) if w > MAX_IMAGE_SIDE || h > MAX_IMAGE_SIDE => {
                    r.errors.push(format!("{}: {w}x{h}, no side may exceed {MAX_IMAGE_SIDE} pixels", rel(dir, f)));
                }
                Some(_) => {}
                None => r.errors.push(format!("{}: cannot read the image", rel(dir, f))),
            }
        }
        if matches!(ext.as_str(), "exe" | "dll" | "js" | "ps1" | "bat" | "cmd") {
            r.warnings.push(format!("{}: themes hold data only, this file is never used", rel(dir, f)));
        }
    }
    if r.bytes > MAX_PACK_BYTES {
        r.errors.push(format!(
            "the pack is {:.1} MB, it may not exceed {} MB",
            r.bytes as f64 / 1048576.0,
            MAX_PACK_BYTES / 1048576
        ));
    }
    match Theme::load_dir(dir) {
        Ok(t) => {
            for e in &t.module_errors {
                r.errors.push(e.to_string());
            }
            for id in t.modules.keys() {
                if !id.contains('.') && !MODULES.contains(&id.as_str()) {
                    r.warnings.push(format!("there is no module {id}, this template is never used"));
                }
            }
            if let Some(p) = &t.manifest.preview
                && !dir.join(p).is_file()
            {
                r.errors.push(format!("preview image {} does not exist", p.display()));
            }
            r.theme = Some(t);
        }
        Err(e) => r.errors.push(e.to_string()),
    }
    r
}

fn rel(dir: &Path, f: &Path) -> String {
    f.strip_prefix(dir).unwrap_or(f).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_example_themes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/themes");
        let mut n = 0;
        for e in std::fs::read_dir(&root).unwrap().flatten() {
            let r = check(&e.path());
            assert!(r.ok(), "{}: {:?}", e.path().display(), r.errors);
            assert!(r.warnings.is_empty(), "{:?}", r.warnings);
            n += 1;
        }
        assert_eq!(n, 2);
    }

    #[test]
    fn reports_bad_field() {
        let dir = std::env::temp_dir().join(format!("rh-theme-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("theme.xml"),
            "<theme xmlns=\"urn:rustyhunt:theme:1\" id=\"t\" name=\"t\">\n<module id=\"clock\">\n<text text=\"{quest.nope}\"/>\n</module>\n</theme>",
        )
        .unwrap();
        let r = check(&dir);
        assert!(!r.ok());
        assert!(r.errors[0].contains("quest.nope"), "{:?}", r.errors);
        let _ = std::fs::remove_dir_all(dir);
    }
}
