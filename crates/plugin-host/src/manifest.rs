//! Plugin manifest `plugin.toml`: ID, name, summary, version, author, required app version,
//! permissions, entry file, and the in-game modules, hotkey actions, settings, exporters and notification channels it registers.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Plugin API version, following semantic versioning (8.2).
pub const API_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Internet access (http_get)
    Net,
    /// Read and write the plugin's own folder
    Files,
    /// Read the Hunter's Notes (from the save)
    Save,
    /// Open a read-only local HTTP port
    Port,
}

impl Permission {
    pub const ALL: [Permission; 4] = [Permission::Net, Permission::Files, Permission::Save, Permission::Port];

    pub fn key(self) -> &'static str {
        match self {
            Permission::Net => "net",
            Permission::Files => "files",
            Permission::Save => "save",
            Permission::Port => "port",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Element {
    pub key: String,
    pub name: String,
    #[serde(default = "yes")]
    pub default_on: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleDecl {
    pub id: String,
    pub name: String,
    /// UI description (template) file, same format as a theme's module templates
    pub template: String,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub default_on: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionDecl {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SettingKind {
    Bool { default: bool },
    Number { default: f64, min: f64, max: f64 },
    Text { default: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingDecl {
    pub key: String,
    pub name: String,
    #[serde(flatten)]
    pub kind: SettingKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExporterDecl {
    pub id: String,
    pub name: String,
    /// File extension of the export
    pub ext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelDecl {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    /// Required app version
    #[serde(default = "default_app")]
    pub app: String,
    /// Required plugin API version
    #[serde(default = "default_api")]
    pub api: String,
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Subscribed events; empty means none
    #[serde(default)]
    pub events: Vec<String>,
    /// Data snapshots per second; 0 means none
    #[serde(default)]
    pub frame_hz: u32,
    #[serde(default)]
    pub modules: Vec<ModuleDecl>,
    #[serde(default)]
    pub actions: Vec<ActionDecl>,
    #[serde(default)]
    pub settings: Vec<SettingDecl>,
    #[serde(default)]
    pub exporters: Vec<ExporterDecl>,
    #[serde(default)]
    pub channels: Vec<ChannelDecl>,
}

fn default_app() -> String {
    "1.0".into()
}

fn default_api() -> String {
    API_VERSION.into()
}

fn default_entry() -> String {
    "plugin.wasm".into()
}

impl Manifest {
    pub fn load(dir: &Path) -> Result<Manifest, String> {
        let text = std::fs::read_to_string(dir.join("plugin.toml")).map_err(|e| format!("plugin.toml：{e}"))?;
        let m: Manifest = toml::from_str(&text).map_err(|e| format!("plugin.toml：{e}"))?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), String> {
        let ok_id = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !ok_id(&self.id) {
            return Err(format!("id \"{}\" may only use letters, digits, - and _", self.id));
        }
        for m in &self.modules {
            if !ok_id(&m.id) {
                return Err(format!("module id \"{}\" may only use letters, digits, - and _", m.id));
            }
        }
        for a in &self.actions {
            if !ok_id(&a.id) {
                return Err(format!("action id \"{}\" may only use letters, digits, - and _", a.id));
            }
        }
        if self.entry.contains("..") || self.modules.iter().any(|m| m.template.contains("..")) {
            return Err("file paths may not contain ..".into());
        }
        for e in &self.events {
            if !rh_core::Event::NAMES.contains(&e.as_str()) {
                return Err(format!("no event {e}"));
            }
        }
        Ok(())
    }

    /// ID used for plugin modules in settings and layouts: `plugin ID.module ID`
    pub fn module_key(&self, module: &str) -> String {
        format!("{}.{module}", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
id = "split-timer"
name = "速通计时"
version = "1.4.0"
author = "Kiri"
permissions = ["files"]
events = ["quest_start", "quest_end", "part_break"]
frame_hz = 2

[[modules]]
id = "split"
name = "分段计时"
template = "split.xml"
elements = [{ key = "best", name = "和最好成绩的差" }]

[[actions]]
id = "reset"
name = "分段计时 重置"
key = "F10"

[[settings]]
key = "show_diff"
name = "显示和最好成绩的差"
type = "bool"
default = true

[[settings]]
key = "splits"
name = "分段数"
type = "number"
default = 3
min = 1
max = 10
"#;

    #[test]
    fn parses_manifest() {
        let m: Manifest = toml::from_str(SAMPLE).unwrap();
        m.validate().unwrap();
        assert_eq!(m.entry, "plugin.wasm");
        assert_eq!(m.permissions, vec![Permission::Files]);
        assert!(m.modules[0].elements[0].default_on);
        assert_eq!(m.module_key("split"), "split-timer.split");
        assert!(matches!(m.settings[1].kind, SettingKind::Number { max, .. } if max == 10.0));
    }

    #[test]
    fn rejects_bad_ids_and_events() {
        let mut m: Manifest = toml::from_str(SAMPLE).unwrap();
        m.id = "bad id".into();
        assert!(m.validate().is_err());
        let mut m: Manifest = toml::from_str(SAMPLE).unwrap();
        m.events.push("nope".into());
        assert!(m.validate().is_err());
        let mut m: Manifest = toml::from_str(SAMPLE).unwrap();
        m.entry = "../x.wasm".into();
        assert!(m.validate().is_err());
    }
}
