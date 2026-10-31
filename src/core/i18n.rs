use std::{collections::HashMap, sync::LazyLock};

use objc2_foundation::{NSLocale, NSString, NSUserDefaults, ns_string};

// Translations live in assets/i18n.json; this enum only names the message keys.
static MESSAGES: LazyLock<HashMap<String, HashMap<String, String>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../assets/i18n.json")).expect("valid i18n.json")
});

macro_rules! text_keys {
    ($($key:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Text { $($key),* }

        impl Text {
            #[cfg(test)]
            pub const ALL: &[Self] = &[$(Self::$key),*];

            pub fn key(self) -> &'static str {
                match self { $(Self::$key => stringify!($key)),* }
            }
        }
    };
}

text_keys! {
    OpenSettings, InputNeeded, Preview, CloseWindow, About, Version,
    SoundEnabled, SoundMuted, AllowInput, RetryInput, KeyboardSound,
    MouseSound, Mechanical, Electronic, Fun, Typewriters, LaunchAtLogin,
    UpdateAvailable, ApproveLogin, LoginFailed, Volume, LanguageMenu, Quit,
    Off, Click, Tooltip, InputUnavailable, StartFailed, OK,
    InputPermissionRequired, InputConnectionFailed, InputSourceFailed,
    MainLoopUnavailable, MainThreadRequired, AudioThreadStopped,
    AudioOutputFailed, PathUtf8Required, InvalidAudioFormat, WavTooLarge,
    AudioBufferFailed, IncompleteWav, InvalidSoundFile, SoundsPathRequired,
    UnknownArgument, Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Language {
    English,
    Russian,
    Chinese,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::English, Self::Russian, Self::Chinese];

    pub fn code(self) -> &'static str {
        ["en", "ru", "zh-Hans"][self as usize]
    }

    pub fn name(self) -> &'static str {
        ["English", "Русский", "简体中文"][self as usize]
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.split(['-', '_']).next()?.to_ascii_lowercase().as_str() {
            "en" => Some(Self::English),
            "ru" => Some(Self::Russian),
            "zh" => Some(Self::Chinese),
            _ => None,
        }
    }

    pub fn resolve(saved: Option<&str>, preferred: impl IntoIterator<Item = String>) -> Self {
        saved
            .and_then(Self::from_code)
            .or_else(|| {
                preferred
                    .into_iter()
                    .find_map(|code| Self::from_code(&code))
            })
            .unwrap_or(Self::English)
    }

    pub fn current() -> Self {
        Self::load(&NSUserDefaults::standardUserDefaults())
    }

    pub fn load(defaults: &NSUserDefaults) -> Self {
        let saved = defaults
            .stringForKey(ns_string!("language"))
            .map(|value| value.to_string());
        Self::resolve(
            saved.as_deref(),
            NSLocale::preferredLanguages().iter().map(|s| s.to_string()),
        )
    }

    pub fn save(self, defaults: &NSUserDefaults) {
        let code = NSString::from_str(self.code());
        // SAFETY: NSUserDefaults copies this immutable NSString property-list value.
        unsafe { defaults.setObject_forKey(Some(&code), ns_string!("language")) };
    }

    pub fn text(self, message: Text) -> &'static str {
        let key = message.key();
        MESSAGES
            .get(key)
            .and_then(|translations| translations.get(self.code()))
            .map(String::as_str)
            .unwrap_or(key)
    }

    pub fn mouse_name(self, index: u8) -> String {
        match index {
            0 => self.text(Text::Off).into(),
            15 => format!("{} C", self.text(Text::Click)),
            n => format!("{} {n}", self.text(Text::Click)),
        }
    }
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
