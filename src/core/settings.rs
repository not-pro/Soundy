//! Windows preference store: one JSON file in `%APPDATA%\Soundy\config.json`.

use std::path::PathBuf;

use windows::Win32::Globalization::GetUserDefaultUILanguage;

use crate::core::{
    catalog::{MouseProfile, Profile},
    i18n::Language,
};

const LANG_RUSSIAN: u16 = 0x19;
const LANG_CHINESE: u16 = 0x04;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub profile: u8,
    pub mouse_profile: u8,
    pub volume: u8,
    pub language: Language,
    pub launch_at_login: bool,
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            profile: 0,
            mouse_profile: 0,
            volume: 25,
            language: system_language(),
            launch_at_login: false,
            enabled: true,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .map(|text| Self::from_json(&text))
            .unwrap_or_default()
    }

    pub fn save(self) {
        let path = path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, self.to_json());
    }

    /// Out-of-range or unknown values fall back to defaults instead of failing.
    fn from_json(text: &str) -> Self {
        let mut config = Self::default();
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return config;
        };
        // Indexing a Value yields Null for missing keys, so one line per field.
        let number = |key: &str, fallback: u64| value[key].as_u64().unwrap_or(fallback);
        config.profile = Profile::from_index(number("profile", 0) as usize).0;
        config.mouse_profile = MouseProfile::from_index(number("mouseProfile", 0) as usize).0;
        config.volume = number("volume", 25).min(100) as u8;
        if let Some(code) = value["language"].as_str() {
            config.language = Language::from_code(code).unwrap_or(config.language);
        }
        config.launch_at_login = value["launchAtLogin"].as_bool().unwrap_or(false);
        config.enabled = value["enabled"].as_bool().unwrap_or(true);
        config
    }

    fn to_json(self) -> String {
        serde_json::json!({
            "profile": self.profile,
            "mouseProfile": self.mouse_profile,
            "volume": self.volume,
            "language": self.language.code(),
            "launchAtLogin": self.launch_at_login,
            "enabled": self.enabled,
        })
        .to_string()
    }
}

fn path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Soundy")
        .join("config.json")
}

fn system_language() -> Language {
    // SAFETY: GetUserDefaultUILanguage takes no arguments and returns a LANGID.
    let langid = unsafe { GetUserDefaultUILanguage() };
    let primary = langid & 0x3ff;
    Language::from_code(match primary {
        LANG_RUSSIAN => "ru",
        LANG_CHINESE => "zh",
        _ => "en",
    })
    .unwrap_or(Language::English)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_json_round_trips_and_ignores_broken_values() {
        let config = Config {
            profile: 39,
            mouse_profile: 15,
            volume: 75,
            language: Language::Chinese,
            launch_at_login: true,
            enabled: false,
        };
        assert_eq!(Config::from_json(&config.to_json()), config);
        let fallback = Config::from_json(
            r#"{"profile": 900, "mouseProfile": 900, "volume": 900, "language": "xx", "launchAtLogin": "yes", "enabled": "yes"}"#,
        );
        assert_eq!(fallback.profile, 0);
        assert_eq!(fallback.mouse_profile, 0);
        assert_eq!(fallback.volume, 100);
        assert_eq!(fallback.language, Config::default().language);
        assert!(!fallback.launch_at_login);
        assert!(fallback.enabled);
        assert_eq!(Config::from_json("not json"), Config::default());
    }
}
