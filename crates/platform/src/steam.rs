//! Steam install location, libraries, and accounts that have logged in on this machine.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A minimal parser for Valve's KeyValues text format (.vdf / .acf).
#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Obj(BTreeMap<String, Vdf>),
}

impl Vdf {
    pub fn parse(text: &str) -> Option<Vdf> {
        let tokens = tokenize(text);
        let mut pos = 0;
        let mut root = BTreeMap::new();
        while pos < tokens.len() {
            let (k, v) = parse_pair(&tokens, &mut pos)?;
            root.insert(k, v);
        }
        Some(Vdf::Obj(root))
    }

    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(m) => m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = (&String, &Vdf)> {
        match self {
            Vdf::Obj(m) => Some(m.iter()),
            Vdf::Str(_) => None,
        }
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, PartialEq)]
enum Tok {
    S(String),
    Open,
    Close,
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        _ => s.push(c),
                    }
                }
                out.push(Tok::S(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            _ => {
                let mut s = c.to_string();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                out.push(Tok::S(s));
            }
        }
    }
    out
}

fn parse_pair(tokens: &[Tok], pos: &mut usize) -> Option<(String, Vdf)> {
    let Tok::S(key) = tokens.get(*pos)? else {
        return None;
    };
    *pos += 1;
    match tokens.get(*pos)? {
        Tok::S(v) => {
            *pos += 1;
            Some((key.clone(), Vdf::Str(v.clone())))
        }
        Tok::Open => {
            *pos += 1;
            let mut map = BTreeMap::new();
            while *pos < tokens.len() && tokens[*pos] != Tok::Close {
                let (k, v) = parse_pair(tokens, pos)?;
                map.insert(k, v);
            }
            *pos += 1;
            Some((key.clone(), Vdf::Obj(map)))
        }
        Tok::Close => None,
    }
}

#[cfg(windows)]
fn registry_string(subkey: &str, value: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let subkey = crate::wide(std::ffi::OsStr::new(subkey));
    let value = crate::wide(std::ffi::OsStr::new(value));
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut _,
            &mut size,
        )
    };
    if rc != 0 {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..len]))
}

#[cfg(not(windows))]
fn registry_string(_subkey: &str, _value: &str) -> Option<String> {
    None
}

/// Steam's install directory.
pub fn steam_dir() -> Option<PathBuf> {
    if let Some(p) = registry_string("Software\\Valve\\Steam", "SteamPath") {
        let p = PathBuf::from(p.replace('/', "\\"));
        if p.is_dir() {
            return Some(p);
        }
    }
    let fallback = PathBuf::from(r"C:\Program Files (x86)\Steam");
    fallback.is_dir().then_some(fallback)
}

/// All library directories (each has a steamapps folder).
pub fn library_dirs(steam: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![steam.to_path_buf()];
    let file = steam.join("steamapps").join("libraryfolders.vdf");
    if let Some(vdf) = std::fs::read_to_string(file).ok().and_then(|t| Vdf::parse(&t))
        && let Some(root) = vdf.get("libraryfolders")
    {
        for (_, lib) in root.entries() {
            if let Some(p) = lib.str("path") {
                let p = PathBuf::from(p);
                if !dirs.iter().any(|d| d.as_os_str().eq_ignore_ascii_case(p.as_os_str())) {
                    dirs.push(p);
                }
            }
        }
    }
    dirs
}

/// Finds an installed game's directory by Steam AppID.
pub fn app_dir(steam: &Path, app_id: u32) -> Option<PathBuf> {
    for lib in library_dirs(steam) {
        let manifest = lib.join("steamapps").join(format!("appmanifest_{app_id}.acf"));
        let Some(vdf) = std::fs::read_to_string(&manifest).ok().and_then(|t| Vdf::parse(&t)) else {
            continue;
        };
        if let Some(dir) = vdf.get("AppState").and_then(|s| s.str("installdir")) {
            let p = lib.join("steamapps").join("common").join(dir);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    /// Directory name under userdata, i.e. the 32-bit account ID
    pub account_id: u32,
    pub persona: String,
    pub account_name: String,
    /// Unix time of the most recent login
    pub timestamp: u64,
    pub most_recent: bool,
}

const STEAM_ID64_BASE: u64 = 76561197960265728;

/// Steam accounts that have logged in on this machine, most recent first.
pub fn accounts(steam: &Path) -> Vec<Account> {
    let file = steam.join("config").join("loginusers.vdf");
    let mut out = Vec::new();
    if let Some(vdf) = std::fs::read_to_string(file).ok().and_then(|t| Vdf::parse(&t))
        && let Some(users) = vdf.get("users")
    {
        for (id64, u) in users.entries() {
            let Ok(id64) = id64.parse::<u64>() else { continue };
            let Some(account_id) = id64.checked_sub(STEAM_ID64_BASE) else { continue };
            out.push(Account {
                account_id: account_id as u32,
                persona: u.str("PersonaName").unwrap_or_default().to_string(),
                account_name: u.str("AccountName").unwrap_or_default().to_string(),
                timestamp: u.str("Timestamp").and_then(|t| t.parse().ok()).unwrap_or(0),
                most_recent: u.str("MostRecent") == Some("1"),
            });
        }
    }
    // Also list accounts that are in userdata but not in loginusers.vdf
    if let Ok(entries) = std::fs::read_dir(steam.join("userdata")) {
        for e in entries.flatten() {
            if let Some(id) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok())
                && id != 0
                && !out.iter().any(|a| a.account_id == id)
            {
                out.push(Account {
                    account_id: id,
                    persona: String::new(),
                    account_name: String::new(),
                    timestamp: 0,
                    most_recent: false,
                });
            }
        }
    }
    out.sort_by(|a, b| b.most_recent.cmp(&a.most_recent).then(b.timestamp.cmp(&a.timestamp)));
    out
}

/// Cloud save directory of a game under an account.
pub fn remote_dir(steam: &Path, account_id: u32, app_id: u32) -> PathBuf {
    steam.join("userdata").join(account_id.to_string()).join(app_id.to_string()).join("remote")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_vdf() {
        let text = r#"
"users"
{
    "76561198000000001"
    {
        "AccountName"   "abc"
        "PersonaName"   "Momo \"M\""
        "MostRecent"    "1"
        "Timestamp"     "1700000000"
    }
}
"#;
        let v = Vdf::parse(text).unwrap();
        let u = v.get("users").unwrap().get("76561198000000001").unwrap();
        assert_eq!(u.str("PersonaName"), Some("Momo \"M\""));
        assert_eq!(u.str("accountname"), Some("abc"));
    }

    #[test]
    fn parses_library_folders() {
        let text = r#""libraryfolders" { "0" { "path" "C:\\Program Files (x86)\\Steam" "apps" { "582010" "1" } } "1" { "path" "D:\\SteamLibrary" } }"#;
        let v = Vdf::parse(text).unwrap();
        let paths: Vec<_> = v.get("libraryfolders").unwrap().entries().filter_map(|(_, l)| l.str("path")).collect();
        assert_eq!(paths, vec![r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]);
    }
}
