//! Reads the version from the window title and picks the address map for it.

use std::path::{Path, PathBuf};

use address_map::{AddressMap, LoadError};

pub const MAP_PREFIX: &str = "MonsterHunterWorld";

/// The title looks like `MONSTER HUNTER: WORLD(421810)`.
pub fn version_from_title(title: &str) -> Option<String> {
    if !title.to_uppercase().starts_with("MONSTER HUNTER: WORLD") {
        return None;
    }
    let v = title.split('(').nth(1)?.trim_end_matches(')').trim();
    v.parse::<u64>().ok().map(|_| v.to_string())
}

/// Version range of the pre-Iceborne test builds; upstream uses the latest map for them.
pub fn is_legacy_ice(version: &str) -> bool {
    version.parse::<u64>().is_ok_and(|v| (300_000..=400_000).contains(&v))
}

/// Looks for this version's map in several directories in order; earlier directories win (separately updated maps come first).
pub fn load_map(dirs: &[PathBuf], version: &str) -> Result<(AddressMap, PathBuf), LoadError> {
    for dir in dirs {
        let path = address_map::map_path(dir, MAP_PREFIX, version);
        if path.is_file() {
            return AddressMap::load(&path).map(|m| (m, path));
        }
    }
    if is_legacy_ice(version)
        && let Some((dir, latest)) = dirs
            .iter()
            .filter_map(|d| address_map::available_versions(d, MAP_PREFIX).pop().map(|v| (d, v)))
            .max_by_key(|(_, v)| v.parse::<u64>().unwrap_or(0))
    {
        let path = address_map::map_path(dir, MAP_PREFIX, &latest);
        return AddressMap::load(&path).map(|m| (m, path));
    }
    Err(LoadError::NotFound {
        dir: dirs.first().cloned().unwrap_or_default(),
        version: version.to_string(),
    })
}

/// Versions found in all directories, deduplicated and ascending.
pub fn known_versions(dirs: &[&Path]) -> Vec<String> {
    let mut v: Vec<String> = dirs.iter().flat_map(|d| address_map::available_versions(d, MAP_PREFIX)).collect();
    v.sort_by_key(|x| x.parse::<u64>().unwrap_or(0));
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title() {
        assert_eq!(version_from_title("MONSTER HUNTER: WORLD(421810)"), Some("421810".into()));
        assert_eq!(version_from_title("Monster Hunter: World(421652)"), Some("421652".into()));
        assert_eq!(version_from_title("MONSTER HUNTER RISE"), None);
        assert_eq!(version_from_title("MONSTER HUNTER: WORLD"), None);
    }

    #[test]
    fn loads_bundled_map_and_reports_missing() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps");
        let (map, _) = load_map(std::slice::from_ref(&dir), "421810").unwrap();
        assert_eq!(map.address("FUN_DEAL_DAMAGE"), Some(0x01CC5F80));
        assert!(load_map(std::slice::from_ref(&dir), "421999").is_err());
        assert_eq!(known_versions(&[&dir]).last().map(String::as_str), Some("421810"));
    }
}
