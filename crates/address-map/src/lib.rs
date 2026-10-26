//! 游戏地址表（`.map` 文件）的解析与查找。
//!
//! 语法：`Address|Offset|Instruction 名字 十六进制[,十六进制...] [# 注释]`，
//! `0x` 前缀可省略，文件可带 UTF-8 BOM，换行可以是 CRLF。
//! 与上游的一处不同：未知关键字会报错，而不是静默跳过。

use std::collections::HashMap;
use std::fmt;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};

fn strip_hex_prefix(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix("0x").or(s.strip_prefix("0X")).unwrap_or(s)
}

/// 解析一个十六进制数，`0x` 前缀可有可无。
pub fn parse_hex(s: &str) -> Result<u64, ParseIntError> {
    u64::from_str_radix(strip_hex_prefix(s), 16)
}

/// 解析逗号分隔的偏移列表。每项按 32 位读入后视为有符号数，
/// 与上游 `Convert.ToInt32(s, 16)` 一致：`FFFFFFF0` 得到 -16。
pub fn parse_offsets(s: &str) -> Result<Vec<i32>, ParseIntError> {
    s.split(',')
        .map(|part| u32::from_str_radix(strip_hex_prefix(part), 16).map(|v| v as i32))
        .collect()
}

/// 地址表中有效的一行。
#[derive(Debug, PartialEq)]
pub enum Entry {
    Address { name: String, value: u64 },
    Offsets { name: String, values: Vec<i32> },
}

#[derive(Debug, PartialEq)]
pub enum LineError {
    MissingName,
    MissingValue,
    UnknownKeyword(String),
    InvalidNumber(ParseIntError),
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineError::MissingName => write!(f, "缺少名字"),
            LineError::MissingValue => write!(f, "缺少值"),
            LineError::UnknownKeyword(k) => write!(f, "未知关键字 {k}"),
            LineError::InvalidNumber(e) => write!(f, "数值无效：{e}"),
        }
    }
}

fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim();
    match s.split_once(char::is_whitespace) {
        Some((first, rest)) => (first, rest.trim()),
        None => (s, ""),
    }
}

/// 解析一行。空行和纯注释返回 `Ok(None)`。
pub fn parse_line(line: &str) -> Result<Option<Entry>, LineError> {
    let content = line.split_once('#').map_or(line, |(before, _)| before);

    let (keyword, rest) = split_first_word(content);
    if keyword.is_empty() {
        return Ok(None);
    }
    let (name, value) = split_first_word(rest);
    if name.is_empty() {
        return Err(LineError::MissingName);
    }
    if value.is_empty() {
        return Err(LineError::MissingValue);
    }

    let name = name.to_string();
    match keyword {
        "Address" => {
            let value = parse_hex(value).map_err(LineError::InvalidNumber)?;
            Ok(Some(Entry::Address { name, value }))
        }
        "Offset" | "Instruction" => {
            let values = parse_offsets(value).map_err(LineError::InvalidNumber)?;
            Ok(Some(Entry::Offsets { name, values }))
        }
        _ => Err(LineError::UnknownKeyword(keyword.to_string())),
    }
}

/// 解析整个文件时的错误，行号从 1 开始。
#[derive(Debug, PartialEq)]
pub enum MapError {
    InvalidLine { line_number: usize, error: LineError },
    DuplicateName { line_number: usize, name: String },
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::InvalidLine { line_number, error } => write!(f, "第 {line_number} 行：{error}"),
            MapError::DuplicateName { line_number, name } => {
                write!(f, "第 {line_number} 行：名字 {name} 重复")
            }
        }
    }
}

impl std::error::Error for MapError {}

/// 读取地址表文件时的错误。
#[derive(Debug)]
pub enum LoadError {
    Io { path: PathBuf, error: std::io::Error },
    Parse { path: PathBuf, error: MapError },
    NotFound { dir: PathBuf, version: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io { path, error } => write!(f, "{}：{error}", path.display()),
            LoadError::Parse { path, error } => write!(f, "{}：{error}", path.display()),
            LoadError::NotFound { dir, version } => {
                write!(f, "{} 里没有版本 {version} 的地址表", dir.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// 解析后的地址表，按名字查询地址和偏移。
#[derive(Debug, Default, Clone)]
pub struct AddressMap {
    addresses: HashMap<String, u64>,
    offsets: HashMap<String, Vec<i32>>,
}

impl AddressMap {
    pub fn parse(text: &str) -> Result<AddressMap, MapError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut map = AddressMap::default();

        for (index, line) in text.lines().enumerate() {
            let line_number = index + 1;
            let entry = parse_line(line).map_err(|error| MapError::InvalidLine { line_number, error })?;

            match entry {
                Some(Entry::Address { name, value }) => {
                    if map.addresses.contains_key(&name) {
                        return Err(MapError::DuplicateName { line_number, name });
                    }
                    map.addresses.insert(name, value);
                }
                Some(Entry::Offsets { name, values }) => {
                    if map.offsets.contains_key(&name) {
                        return Err(MapError::DuplicateName { line_number, name });
                    }
                    map.offsets.insert(name, values);
                }
                None => {}
            }
        }

        Ok(map)
    }

    pub fn load(path: &Path) -> Result<AddressMap, LoadError> {
        let text = std::fs::read_to_string(path).map_err(|error| LoadError::Io {
            path: path.to_path_buf(),
            error,
        })?;
        AddressMap::parse(&text).map_err(|error| LoadError::Parse {
            path: path.to_path_buf(),
            error,
        })
    }

    /// 在 `dir` 里找 `<prefix>.<version>.map` 并读取。
    pub fn load_version(dir: &Path, prefix: &str, version: &str) -> Result<AddressMap, LoadError> {
        let path = map_path(dir, prefix, version);
        if !path.is_file() {
            return Err(LoadError::NotFound {
                dir: dir.to_path_buf(),
                version: version.to_string(),
            });
        }
        AddressMap::load(&path)
    }

    pub fn address(&self, name: &str) -> Option<u64> {
        self.addresses.get(name).copied()
    }

    pub fn offsets(&self, name: &str) -> Option<&[i32]> {
        self.offsets.get(name).map(Vec::as_slice)
    }

    /// `Instruction` 行按字节取出。
    pub fn bytes(&self, name: &str) -> Option<Vec<u8>> {
        self.offsets(name).map(|v| v.iter().map(|&b| b as u8).collect())
    }

    pub fn len(&self) -> usize {
        self.addresses.len() + self.offsets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub fn map_path(dir: &Path, prefix: &str, version: &str) -> PathBuf {
    dir.join(format!("{prefix}.{version}.map"))
}

/// 列出 `dir` 里 `<prefix>.<版本>.map` 的所有版本号，从小到大。
pub fn available_versions(dir: &Path, prefix: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let head = format!("{prefix}.");
    let mut versions: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|name| name.strip_prefix(&head)?.strip_suffix(".map").map(str::to_string))
        .collect();
    versions.sort_by_key(|v| v.parse::<u64>().unwrap_or(0));
    versions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_accepts_optional_prefix_and_spaces() {
        assert_eq!(parse_hex("0x14157BC8"), Ok(0x14157BC8));
        assert_eq!(parse_hex("0X10"), Ok(0x10));
        assert_eq!(parse_hex("48"), Ok(0x48));
        assert_eq!(parse_hex("  0x10 "), Ok(0x10));
    }

    #[test]
    fn parse_hex_rejects_invalid_input() {
        assert!(parse_hex("").is_err());
        assert!(parse_hex("0x").is_err());
        assert!(parse_hex("0xZZ").is_err());
        assert!(parse_hex("-0x10").is_err());
    }

    #[test]
    fn parse_offsets_parses_a_list() {
        assert_eq!(parse_offsets("0x30,0x40,0x108"), Ok(vec![0x30, 0x40, 0x108]));
        assert_eq!(parse_offsets("0x2A0"), Ok(vec![0x2A0]));
    }

    #[test]
    fn parse_offsets_accepts_missing_prefix_and_spaces() {
        // 上游 Rise 地址表里 GUI::VisibilityFlag 有一项没写 0x
        assert_eq!(parse_offsets("0x58,48,0x68,0x10,0x30"), Ok(vec![0x58, 0x48, 0x68, 0x10, 0x30]));
        assert_eq!(parse_offsets("0x10, 0x20 "), Ok(vec![0x10, 0x20]));
    }

    #[test]
    fn parse_offsets_treats_values_as_signed_32_bit() {
        assert_eq!(parse_offsets("FFFFFFF0"), Ok(vec![-16]));
        assert_eq!(parse_offsets("0x7FFFFFFF"), Ok(vec![i32::MAX]));
    }

    #[test]
    fn parse_offsets_rejects_bad_items() {
        assert!(parse_offsets("0x1FFFFFFFF").is_err());
        assert!(parse_offsets("0x10,,0x20").is_err());
        assert!(parse_offsets("0x10,0xZZ").is_err());
    }

    #[test]
    fn parse_line_reads_address() {
        assert_eq!(
            parse_line("Address Game::PlayerManager 0x141902C8"),
            Ok(Some(Entry::Address {
                name: "Game::PlayerManager".to_string(),
                value: 0x141902C8,
            }))
        );
        assert_eq!(
            parse_line("Address Encryption::Round        0x1491F170\r"),
            Ok(Some(Entry::Address {
                name: "Encryption::Round".to_string(),
                value: 0x1491F170,
            }))
        );
    }

    #[test]
    fn parse_line_skips_blank_lines_and_comments() {
        assert_eq!(parse_line(""), Ok(None));
        assert_eq!(parse_line("   \r"), Ok(None));
        assert_eq!(parse_line("# Addresses"), Ok(None));
    }

    #[test]
    fn parse_line_reports_errors() {
        assert_eq!(parse_line("Address"), Err(LineError::MissingName));
        assert_eq!(parse_line("Address Foo"), Err(LineError::MissingValue));
        assert_eq!(parse_line("Address Foo # 0x10"), Err(LineError::MissingValue));
        assert_eq!(parse_line("Pointer Foo 0x10"), Err(LineError::UnknownKeyword("Pointer".to_string())));
        assert!(matches!(parse_line("Address Foo 0xZZ"), Err(LineError::InvalidNumber(_))));
        assert!(matches!(parse_line("Offset Foo 0x10,,0x20"), Err(LineError::InvalidNumber(_))));
    }

    #[test]
    fn parse_line_reads_offsets_and_instructions() {
        assert_eq!(
            parse_line("Offset Monster::Health 0x30,0x38,0x10"),
            Ok(Some(Entry::Offsets {
                name: "Monster::Health".to_string(),
                values: vec![0x30, 0x38, 0x10],
            }))
        );
        assert_eq!(
            parse_line("Instruction CRC_ORIGINAL_FUNC_1 0x48,0x89,0x5C"),
            Ok(Some(Entry::Offsets {
                name: "CRC_ORIGINAL_FUNC_1".to_string(),
                values: vec![0x48, 0x89, 0x5C],
            }))
        );
    }

    #[test]
    fn parse_line_ignores_trailing_comment() {
        assert_eq!(
            parse_line("Offset SESSION_PLAYER_OFFSETS 0x58 # next is 0x20 + i * 8"),
            Ok(Some(Entry::Offsets {
                name: "SESSION_PLAYER_OFFSETS".to_string(),
                values: vec![0x58],
            }))
        );
    }

    #[test]
    fn address_map_parses_a_whole_file() {
        let text = [
            "# Addresses",
            "Address Game::PlayerManager 0x141902C8",
            "",
            "# Offsets",
            "Offset Monster::Health 0x30,0x38,0x10 # 怪物血量",
            "Instruction CRC_ORIGINAL_FUNC_1 0x48,0x89,0x5C",
        ]
        .join("\r\n");

        let map = AddressMap::parse(&text).unwrap();

        assert_eq!(map.address("Game::PlayerManager"), Some(0x141902C8));
        assert_eq!(map.offsets("Monster::Health"), Some(&[0x30, 0x38, 0x10][..]));
        assert_eq!(map.bytes("CRC_ORIGINAL_FUNC_1"), Some(vec![0x48, 0x89, 0x5C]));
        assert_eq!(map.address("NoSuchName"), None);
        assert_eq!(map.address("Monster::Health"), None);
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn address_map_reports_line_number_of_bad_line() {
        let text = "Address A 0x10\nAddress B 0xZZ";
        assert!(matches!(
            AddressMap::parse(text),
            Err(MapError::InvalidLine {
                line_number: 2,
                error: LineError::InvalidNumber(_),
            })
        ));
    }

    #[test]
    fn address_map_rejects_duplicate_names() {
        let text = "Address A 0x10\nOffset B 0x20\nAddress A 0x30";
        assert_eq!(
            AddressMap::parse(text).unwrap_err(),
            MapError::DuplicateName {
                line_number: 3,
                name: "A".to_string(),
            }
        );
    }

    #[test]
    fn address_map_strips_bom() {
        let map = AddressMap::parse("\u{feff}# Addresses\r\nAddress A 0x10\r\n").unwrap();
        assert_eq!(map.address("A"), Some(0x10));
    }

    #[test]
    fn bundled_world_maps_parse() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps");
        let versions = available_versions(&dir, "MonsterHunterWorld");
        assert!(!versions.is_empty());
        for v in versions {
            let map = AddressMap::load_version(&dir, "MonsterHunterWorld", &v).unwrap();
            assert!(map.address("MONSTER_LIST_ADDRESS").is_some(), "{v}");
        }
    }

    #[test]
    fn missing_version_is_reported() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps");
        assert!(matches!(
            AddressMap::load_version(&dir, "MonsterHunterWorld", "1"),
            Err(LoadError::NotFound { .. })
        ));
    }
}
