//! Hotkeys: parses strings like "Ctrl Shift H" and queries key state.
//!
//! `RegisterHotKey` isn't used because it grabs the keys system-wide (Ctrl Tab, for example).
//! Key state is only polled while the game or the overlay is in the foreground.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Hotkey {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Windows virtual-key codes; standalone modifiers (such as Left Alt) are here too
    pub key: u16,
}

const NAMES: &[(u16, &str)] = &[
    (0x08, "Backspace"),
    (0x09, "Tab"),
    (0x0D, "Enter"),
    (0x13, "Pause"),
    (0x14, "CapsLock"),
    (0x1B, "Esc"),
    (0x20, "Space"),
    (0x21, "PageUp"),
    (0x22, "PageDown"),
    (0x23, "End"),
    (0x24, "Home"),
    (0x25, "Left"),
    (0x26, "Up"),
    (0x27, "Right"),
    (0x28, "Down"),
    (0x2D, "Insert"),
    (0x2E, "Delete"),
    (0xA0, "Left Shift"),
    (0xA1, "Right Shift"),
    (0xA2, "Left Ctrl"),
    (0xA3, "Right Ctrl"),
    (0xA4, "Left Alt"),
    (0xA5, "Right Alt"),
    (0xC0, "`"),
    (0xBD, "-"),
    (0xBB, "="),
    (0xDB, "["),
    (0xDD, "]"),
    (0xBA, ";"),
    (0xDE, "'"),
    (0xBC, ","),
    (0xBE, "."),
    (0xBF, "/"),
    (0xDC, "\\"),
    (0x60, "Num0"),
    (0x61, "Num1"),
    (0x62, "Num2"),
    (0x63, "Num3"),
    (0x64, "Num4"),
    (0x65, "Num5"),
    (0x66, "Num6"),
    (0x67, "Num7"),
    (0x68, "Num8"),
    (0x69, "Num9"),
];

pub const VK_CONTROL: u16 = 0x11;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_MENU: u16 = 0x12;
pub const VK_ESCAPE: u16 = 0x1B;

pub fn key_name(vk: u16) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        _ => NAMES
            .iter()
            .find(|(k, _)| *k == vk)
            .map(|(_, n)| n.to_string())
            .unwrap_or_else(|| format!("0x{vk:02X}")),
    }
}

fn parse_key(s: &str) -> Option<u16> {
    let upper = s.to_ascii_uppercase();
    if upper.len() == 1 {
        let c = upper.as_bytes()[0];
        if c.is_ascii_alphanumeric() {
            return Some(c as u16);
        }
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u16>().ok())
        && (1..=24).contains(&n)
    {
        return Some(0x6F + n);
    }
    NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(s)).map(|(k, _)| *k)
}

impl Hotkey {
    pub const fn new(ctrl: bool, shift: bool, alt: bool, key: u16) -> Hotkey {
        Hotkey { ctrl, shift, alt, key }
    }

    /// Parses "Ctrl Shift H", "Left Alt", "Ctrl Tab", "F9". Separated by spaces or plus signs.
    pub fn parse(s: &str) -> Option<Hotkey> {
        let words: Vec<&str> = s.split([' ', '+']).filter(|w| !w.is_empty()).collect();
        let mut hk = Hotkey::default();
        let mut i = 0;
        while i < words.len() {
            let w = words[i];
            // Two-word key names such as "Left Alt"
            if i + 1 < words.len()
                && let Some(k) = parse_key(&format!("{} {}", w, words[i + 1]))
            {
                hk.key = k;
                i += 2;
                continue;
            }
            match w.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => hk.ctrl = true,
                "shift" => hk.shift = true,
                "alt" => hk.alt = true,
                _ => hk.key = parse_key(w)?,
            }
            i += 1;
        }
        (hk.key != 0).then_some(hk)
    }

    /// Just a modifier on its own (e.g. Left Alt), for hold-type actions.
    pub fn is_modifier_only(&self) -> bool {
        matches!(self.key, 0xA0..=0xA5 | VK_CONTROL | VK_SHIFT | VK_MENU)
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        parts.push(key_name(self.key));
        f.write_str(&parts.join(" "))
    }
}

#[cfg(windows)]
pub fn is_down(vk: u16) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    (unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000) != 0
}

#[cfg(not(windows))]
pub fn is_down(_vk: u16) -> bool {
    false
}

/// Whether the whole combination is held right now. Modifiers must match exactly, so Ctrl Shift H doesn't also trigger Shift H.
pub fn chord_down(hk: &Hotkey) -> bool {
    if hk.is_modifier_only() {
        return is_down(hk.key);
    }
    is_down(hk.key) && is_down(VK_CONTROL) == hk.ctrl && is_down(VK_SHIFT) == hk.shift && is_down(VK_MENU) == hk.alt
}

/// Remembers each hotkey's previous state to detect the "just pressed" edge.
#[derive(Default)]
pub struct EdgeDetector {
    last: std::collections::HashMap<Hotkey, bool>,
}

impl EdgeDetector {
    /// Returns (held now, just pressed).
    pub fn poll(&mut self, hk: &Hotkey) -> (bool, bool) {
        let down = chord_down(hk);
        let was = self.last.insert(*hk, down).unwrap_or(false);
        (down, down && !was)
    }

    pub fn reset(&mut self) {
        self.last.clear();
    }
}

/// The first non-modifier key held now, plus modifier state. Used to "record" a hotkey in settings.
pub fn capture() -> Option<Hotkey> {
    let ctrl = is_down(VK_CONTROL);
    let shift = is_down(VK_SHIFT);
    let alt = is_down(VK_MENU);
    let scan = (0x08u16..=0xDE).filter(|vk| !matches!(vk, 0x10..=0x12 | 0xA0..=0xA5 | 0x01..=0x06));
    for vk in scan {
        if is_down(vk) && (key_name(vk) != format!("0x{vk:02X}")) {
            return Some(Hotkey { ctrl, shift, alt, key: vk });
        }
    }
    // Only a modifier was pressed: record it as a single key (e.g. Left Alt)
    for vk in 0xA0u16..=0xA5 {
        if is_down(vk) {
            return Some(Hotkey { key: vk, ..Default::default() });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_hotkeys() {
        assert_eq!(Hotkey::parse("Ctrl Shift H"), Some(Hotkey::new(true, true, false, b'H' as u16)));
        assert_eq!(Hotkey::parse("Left Alt"), Some(Hotkey::new(false, false, false, 0xA4)));
        assert_eq!(Hotkey::parse("Ctrl Tab"), Some(Hotkey::new(true, false, false, 0x09)));
        assert_eq!(Hotkey::parse("F9"), Some(Hotkey::new(false, false, false, 0x78)));
        assert_eq!(Hotkey::parse("ctrl+shift+d"), Some(Hotkey::new(true, true, false, b'D' as u16)));
        assert_eq!(Hotkey::parse(""), None);
        assert_eq!(Hotkey::parse("Ctrl"), None);
    }

    #[test]
    fn formats_round_trip() {
        for s in ["Ctrl Shift H", "Left Alt", "Ctrl Tab", "F10", "Alt 1", "Shift Num5"] {
            assert_eq!(Hotkey::parse(s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn modifier_only() {
        assert!(Hotkey::parse("Left Alt").unwrap().is_modifier_only());
        assert!(!Hotkey::parse("Ctrl Tab").unwrap().is_modifier_only());
    }
}
