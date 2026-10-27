//! Reads the Hunter's Notes from the save `SAVEDATA1000`: for each large monster the slain and
//! captured counts, largest and smallest size, and research level. Read-only, never written.
//!
//! The whole file is first decrypted with the public Blowfish key; from Iceborne on each
//! character slot is encrypted again. The decryption is ported from Cirilla (MIT license,
//! Copyright (c) 2018 Wouter Visser), which in turn comes from LEGENDFF/mhw-Savecrypt.

use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

use aes::Aes128;
use aes::cipher::{BlockCipherDecrypt, KeyInit};
use blowfish::BlowfishLE;

use crate::save_tables::{FLOATS, INTEGERS};

const KEY: &[u8] = b"xieZjoe#P2134-3zmaghgpqoe0z8$3azeq";
const MAGIC: [u8; 4] = [1, 0, 0, 0];
const ICEBORNE_SIZE: usize = 11_284_704;

/// Position and length of the three character slots; each slot is followed by a 0x200-byte checksum block.
const SLOT_OFFSETS: [usize; 3] = [0x3010D8, 0x50AB98, 0x714658];
const SLOT_LENGTH: usize = 0x2098C0;

const GUILDCARD_MONSTERS: &str = include_str!("../../../data/world/guildcard_monsters.toml");

/// Offsets inside a character slot. Based on Cirilla's `misc/savedata.bt` and checked against real saves:
/// the slot starts with 4 unknown bytes, then the name (64 bytes) and 7 ints;
/// the guild card starts at 690 (beginning with the Steam ID), and the Hunter's Notes are 6923 bytes into the guild card.
const NAME_OFFSET: usize = 4;
const GUILDCARD_OFFSET: usize = 690;
const NOTES_OFFSET: usize = GUILDCARD_OFFSET + 6923;
const MONSTER_SLOTS: usize = 96;

#[derive(Debug, Clone, PartialEq)]
pub enum SaveError {
    Size(usize),
    Magic,
    NoHunters,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Size(n) => write!(f, "文件大小 {n} 字节，不是冰原的存档"),
            SaveError::Magic => write!(f, "解密后的文件头不对"),
            SaveError::NoHunters => write!(f, "存档里没有角色"),
        }
    }
}

impl std::error::Error for SaveError {}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonsterRecord {
    /// Index on the guild card
    pub index: usize,
    /// Key in the term table; empty for unknown indices
    pub key: String,
    pub slain: u16,
    pub captured: u16,
    /// Size in percent, e.g. 123 means 1.23x; 0 means no record
    pub largest: u16,
    pub smallest: u16,
    pub research: u8,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hunter {
    pub slot: usize,
    pub name: String,
    pub hr: i32,
    pub mr: i32,
    pub playtime: u32,
    pub monsters: Vec<MonsterRecord>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SaveFile {
    pub steam_id: u64,
    pub hunters: Vec<Hunter>,
}

/// Layer one: the whole file in Blowfish ECB (each 32-bit word read and written little-endian).
pub fn decrypt(mut data: Vec<u8>) -> Vec<u8> {
    let cipher = BlowfishLE::new_from_slice(KEY).expect("密钥长度合法");
    for chunk in data.as_chunks_mut::<8>().0 {
        cipher.decrypt_block(chunk.into());
    }
    data
}

fn crc32(mut iv: u32, data: &[u8]) -> u32 {
    for &b in data {
        let mut t = (iv ^ b as u32) & 0xFF;
        for _ in 0..8 {
            t = if t & 1 == 1 { (t >> 1) ^ 0xEDB8_8320 } else { t >> 1 };
        }
        iv = (iv >> 8) ^ t;
    }
    iv
}

fn salt(key_salt: u32) -> [u8; 0x200] {
    let c = key_salt ^ 0x4BF0_CF23;
    let step = (key_salt >> 24)
        .wrapping_add((key_salt >> 16) & 0xFF)
        .wrapping_add((key_salt >> 8) & 0xFF)
        .wrapping_add(key_salt & 0xFF)
        .wrapping_add(1);
    let mut offset: u32 = 0x5D7;
    let mut out = [0u8; 0x200];
    for chunk in out.as_chunks_mut::<4>().0 {
        let mut v = INTEGERS[(offset & 0xFFF) as usize] ^ c;
        if v & 7 == 1 {
            v ^= 0x0BD7_5F29;
        }
        chunk.copy_from_slice(&v.to_le_bytes());
        offset = offset.wrapping_add(step);
    }
    out
}

fn keys(key_salt: u32, salt: &[u8; 0x200]) -> Vec<[u8; 16]> {
    let c = [0x5A8B_79A9 ^ key_salt, 0x3461_6F90 ^ key_salt, 0xC4C6_38DF ^ key_salt, 0x94FB_64E8 ^ key_salt];
    (0..32)
        .map(|i| {
            let index = u32::from_le_bytes(salt[i * 4..i * 4 + 4].try_into().unwrap());
            let mut k = [0u8; 16];
            for (j, cj) in c.iter().enumerate() {
                k[j * 4..j * 4 + 4].copy_from_slice(&(index ^ cj).to_le_bytes());
            }
            k
        })
        .collect()
}

fn key_lengths(key_salt: u32, length: usize) -> [usize; 32] {
    let average = (length >> 5) as i64;
    let mut expected = average;
    let mut out = [0usize; 32];
    for (i, slot) in out.iter_mut().enumerate().take(31) {
        let a = INTEGERS[((key_salt.wrapping_add(i as u32) & 0xFFF) ^ 0x5D7) as usize];
        let b = FLOATS[((a & 0xFFF) ^ 0x885) as usize] - 0.5;
        let v = ((b * average as f32) as i64 + expected + 0xF) & 0xFFFF_FFF0;
        *slot = v as usize;
        expected += average;
    }
    out[31] = length;
    out
}

/// Layer two: Iceborne's slot encryption. `data[offset+length..+0x200]` is the checksum block.
pub fn decrypt_region(data: &mut [u8], offset: usize, length: usize) {
    let key_salt = crc32(0xA37A_55D7, &data[offset + length..offset + length + 0x200]);
    let salt = salt(key_salt);
    let keys = keys(key_salt, &salt);
    let lengths = key_lengths(key_salt, length);

    let mut pos = offset;
    for (key, &end) in keys.iter().zip(lengths.iter()) {
        let aes = Aes128::new_from_slice(key).expect("16 字节密钥");
        let mut s = 0usize;
        while pos < end + offset && pos + 16 <= offset + length {
            let branch: isize = if salt[s & 0x1FF] & 1 == 0 { 4 } else { 0 };
            let at = |k: isize| (pos as isize + k) as usize;
            for (k, so) in [(4, 8), (5, 9), (6, 10), (7, 11), (12, 12), (13, 13), (14, 14), (15, 15)] {
                data[at(k - branch)] ^= salt[(s + so) & 0x1FF];
            }
            aes.decrypt_block((&mut data[pos..pos + 16]).try_into().unwrap());
            for (k, so) in [(0, 0), (1, 1), (2, 2), (3, 3), (8, 4), (9, 5), (10, 6), (11, 7)] {
                data[at(k + branch)] ^= salt[(s + so) & 0x1FF];
            }
            s += 4;
            pos += 16;
        }
    }
}

fn guildcard_keys() -> &'static HashMap<usize, String> {
    static KEYS: OnceLock<HashMap<usize, String>> = OnceLock::new();
    KEYS.get_or_init(|| {
        let t: toml::Table = toml::from_str(GUILDCARD_MONSTERS).expect("内置对照表格式正确");
        t.iter().filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_string()))).collect()
    })
}

/// Monster key for a guild card index.
pub fn monster_key(index: usize) -> Option<&'static str> {
    guildcard_keys().get(&index).map(String::as_str)
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl SaveFile {
    /// Parses the raw (encrypted) save file contents.
    pub fn parse(raw: Vec<u8>) -> Result<SaveFile, SaveError> {
        if raw.len() != ICEBORNE_SIZE {
            return Err(SaveError::Size(raw.len()));
        }
        let mut data = if raw[..4] == MAGIC { raw } else { decrypt(raw) };
        if data[..4] != MAGIC {
            return Err(SaveError::Magic);
        }
        let steam_id = u64::from_le_bytes(data[0x28..0x30].try_into().unwrap());

        let mut hunters = Vec::new();
        for (slot, &offset) in SLOT_OFFSETS.iter().enumerate() {
            decrypt_region(&mut data, offset, SLOT_LENGTH);
            let s = &data[offset..offset + SLOT_LENGTH];
            let ints = NAME_OFFSET + 64;
            let playtime = i32_at(s, ints + 24) as u32;
            let name_raw = &s[NAME_OFFSET..ints];
            let name_end = name_raw.iter().position(|&b| b == 0).unwrap_or(64);
            let name = String::from_utf8_lossy(&name_raw[..name_end]).into_owned();
            // An empty slot decrypts to garbage; in a used slot the guild card starts with this save's Steam ID
            let card_id = u64::from_le_bytes(s[GUILDCARD_OFFSET..GUILDCARD_OFFSET + 8].try_into().unwrap());
            if card_id != steam_id || playtime == 0 {
                continue;
            }
            let notes = &s[NOTES_OFFSET..];
            let monsters = (0..MONSTER_SLOTS)
                .map(|i| MonsterRecord {
                    index: i,
                    key: monster_key(i).unwrap_or_default().to_string(),
                    captured: u16_at(notes, i * 2),
                    slain: u16_at(notes, MONSTER_SLOTS * 2 + i * 2),
                    largest: u16_at(notes, MONSTER_SLOTS * 4 + i * 2),
                    smallest: u16_at(notes, MONSTER_SLOTS * 6 + i * 2),
                    research: notes[MONSTER_SLOTS * 8 + i],
                })
                .collect();
            hunters.push(Hunter {
                slot,
                name,
                hr: i32_at(s, ints),
                mr: i32_at(s, ints + 4),
                playtime,
                monsters,
            });
        }
        if hunters.is_empty() {
            return Err(SaveError::NoHunters);
        }
        Ok(SaveFile { steam_id, hunters })
    }

    /// Reads the file. The save may be locked while the game runs, so copy it to `tmp_dir` first.
    pub fn read(path: &std::path::Path, tmp_dir: &std::path::Path) -> std::io::Result<Result<SaveFile, SaveError>> {
        let raw = match std::fs::read(path) {
            Ok(r) => r,
            Err(_) => {
                std::fs::create_dir_all(tmp_dir)?;
                let copy = tmp_dir.join("SAVEDATA1000.copy");
                std::fs::copy(path, &copy)?;
                let r = std::fs::read(&copy);
                let _ = std::fs::remove_file(&copy);
                r?
            }
        };
        Ok(SaveFile::parse(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_known_value() {
        // Given in the comments: crc32(0x1079000F, "ONSTER HUNTER WORLD: ICEBORNE") = 0xBD75F29
        assert_eq!(crc32(0x1079_000F, b"ONSTER HUNTER WORLD: ICEBORNE"), 0x0BD7_5F29);
    }

    #[test]
    fn key_lengths_are_increasing_and_end_at_length() {
        let l = key_lengths(0x1234_5678, SLOT_LENGTH);
        assert_eq!(l[31], SLOT_LENGTH);
        assert!(l.windows(2).all(|w| w[0] <= w[1]));
        assert!(l[..31].iter().all(|v| v % 16 == 0));
    }

    #[test]
    fn mapping_covers_base_monsters() {
        assert_eq!(monster_key(0), Some("great_jagras"));
        assert_eq!(monster_key(14), Some("rathalos"));
        assert_eq!(monster_key(68), Some("frostfang_barioth"));
        assert_eq!(monster_key(69), Some("alatreon"));
    }

    #[test]
    fn rejects_wrong_size() {
        assert_eq!(SaveFile::parse(vec![0; 100]), Err(SaveError::Size(100)));
    }

    /// Checks against a real local save: set the RH_TEST_SAVE environment variable to SAVEDATA1000.
    #[test]
    fn reads_real_save_when_available() {
        let Ok(path) = std::env::var("RH_TEST_SAVE") else { return };
        let save = SaveFile::parse(std::fs::read(path).unwrap()).unwrap();
        assert!(save.steam_id > 76561197960265728);
        let h = &save.hunters[0];
        assert!(!h.name.is_empty() && !h.name.contains('\u{fffd}'), "{}", h.name);
        assert!(save.hunters.iter().all(|h| h.playtime > 0));
        assert!(h.hr > 0);
        assert!(h.monsters.iter().any(|m| m.slain > 0));
        for m in &h.monsters {
            assert!(m.research <= 10, "{m:?}");
        }
    }
}
