use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Online,
    Offline,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub hotkey: String,
    pub area_hotkey: String,
    pub window_hotkey: Option<String>,
    pub mode: Mode,
    pub excluded: Vec<String>,
    pub autostart: bool,
    pub history: bool,
    pub repeat_ms: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            hotkey: "Alt+Q".into(),
            area_hotkey: "Alt+Shift+Q".into(),
            window_hotkey: None,
            mode: Mode::Online,
            excluded: ["keepass.exe", "keepassxc.exe", "1password.exe", "bitwarden.exe", "mstsc.exe"].map(String::from).to_vec(),
            autostart: false,
            history: false,
            repeat_ms: 1500,
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("Tolk")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.json")
    }

    pub fn load() -> Config {
        let Ok(text) = std::fs::read_to_string(Self::path()) else { return Config::default() };
        serde_json::from_str(&text).unwrap_or_else(|_| {
            let _ = std::fs::rename(Self::path(), Self::path().with_extension("bad.json"));
            Config::default()
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(Self::dir())?;
        let tmp = Self::path().with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, Self::path())
    }

    pub fn is_excluded(&self, exe: &str) -> bool {
        self.excluded.iter().any(|e| e.eq_ignore_ascii_case(exe))
    }
}

pub const MOD_SHIFT: u32 = 1 << 16;
pub const MOD_CTRL: u32 = 1 << 17;
pub const MOD_ALT: u32 = 1 << 18;
pub const MOD_WIN: u32 = 1 << 19;

pub fn parse_hotkey(text: &str) -> Option<u32> {
    let mut mods = 0;
    let mut vk = None;
    for part in text.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "shift" => mods |= MOD_SHIFT,
            "ctrl" | "control" => mods |= MOD_CTRL,
            "alt" => mods |= MOD_ALT,
            "win" => mods |= MOD_WIN,
            "pause" | "break" => vk = Some(0x13),
            "insert" => vk = Some(0x2D),
            "space" => vk = Some(0x20),
            "`" | "~" | "ё" => vk = Some(0xC0),
            p if p.len() > 1 && p.starts_with('f') => {
                let n: u32 = p[1..].parse().ok().filter(|n| (1..=24).contains(n))?;
                vk = Some(0x70 + n - 1);
            }
            p if p.len() == 1 && p.chars().all(|c| c.is_ascii_alphanumeric()) => {
                vk = Some(p.to_ascii_uppercase().as_bytes()[0] as u32);
            }
            _ => return None,
        }
    }
    let vk = vk?;
    let bare_letter = mods & (MOD_CTRL | MOD_ALT | MOD_WIN) == 0 && vk < 0x70 && vk != 0x13 && vk != 0x2D;
    (!bare_letter).then_some(mods | vk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys() {
        assert_eq!(parse_hotkey("Alt+Q"), Some(MOD_ALT | b'Q' as u32));
        assert_eq!(parse_hotkey("alt+shift+q"), Some(MOD_ALT | MOD_SHIFT | b'Q' as u32));
        assert_eq!(parse_hotkey("Ctrl+Alt+F12"), Some(MOD_CTRL | MOD_ALT | 0x7B));
        assert_eq!(parse_hotkey("Pause"), Some(0x13));
        assert_eq!(parse_hotkey("Q"), None);
        assert_eq!(parse_hotkey("Shift+Q"), None);
        assert_eq!(parse_hotkey("Hyper+X"), None);
    }

    #[test]
    fn config_roundtrip_and_defaults() {
        let c: Config = serde_json::from_str(r#"{"hotkey":"Ctrl+Alt+T"}"#).unwrap();
        assert_eq!(c.hotkey, "Ctrl+Alt+T");
        assert_eq!(c.area_hotkey, "Alt+Shift+Q");
        assert!(c.is_excluded("KeePass.exe"));
        assert_eq!(serde_json::from_str::<Config>(&serde_json::to_string(&c).unwrap()).unwrap(), c);
    }
}
