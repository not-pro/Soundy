use std::path::Path;

use crate::core::i18n::{Language, Text};

// Keep the first three IDs stable for settings saved by Soundy 1.0.0.
const KEYBOARDS: [(&str, &str); 40] = [
    ("Tactile", "m_tactile"),
    ("Thocky", "m_thocky"),
    ("Topre", "m_topre"),
    ("Atari", "m_atari"),
    ("Blue Phone", "m_bluephone"),
    ("Classic Mechanical", "m_classic"),
    ("Commodore", "m_commodore"),
    ("IBM", "m_ibm"),
    ("Jade", "m_jade"),
    ("Linear", "m_linear"),
    ("Lubed Brown", "m_lubedbrown"),
    ("Osborne", "m_osborne"),
    ("Sinclair", "m_sinclair"),
    ("Tandy", "m_tandy"),
    ("Classic Electronic", "e_classic"),
    ("iPhone", "e_iphone"),
    ("Mini Computer", "e_minicomputer"),
    ("Morse Code", "e_morsecode"),
    ("Space", "e_space"),
    ("Toy Computer", "e_toycomputer"),
    ("Bubble", "fun_bubble"),
    ("Drums", "fun_drums"),
    ("Duck", "fun_duck"),
    ("Echo", "fun_echo"),
    ("Low", "fun_low"),
    ("Music Box", "fun_musicbox"),
    ("Notification", "fun_notification"),
    ("Pop", "fun_pop"),
    ("Snap", "fun_snap"),
    ("Sparks", "fun_sparks"),
    ("Sword", "fun_sword"),
    ("Bell", "t_bell"),
    ("Brother", "t_brother"),
    ("Classic Typewriter", "t_classic"),
    ("Epoch", "t_epoch"),
    ("Hermes", "t_hermes"),
    ("Mettoy", "t_mettoy"),
    ("Olivetti", "t_olivetti"),
    ("Olympia", "t_olympia"),
    ("Portable", "t_portable"),
];

pub fn files() -> impl Iterator<Item = &'static str> + Clone {
    include_str!("../../assets/sounds/manifest.sha256")
        .lines()
        .map(|line| line.split_once("  ").expect("sound manifest").1)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Profile(pub u8);

impl Profile {
    pub fn all() -> impl Iterator<Item = Self> {
        (0..KEYBOARDS.len() as u8).map(Self)
    }

    pub fn from_index(index: usize) -> Self {
        Self(if index < KEYBOARDS.len() {
            index as u8
        } else {
            0
        })
    }

    pub fn name(self) -> &'static str {
        KEYBOARDS[self.0 as usize].0
    }

    pub fn category(self) -> Text {
        match KEYBOARDS[self.0 as usize].1.split_once('_').unwrap().0 {
            "m" => Text::Mechanical,
            "e" => Text::Electronic,
            "fun" => Text::Fun,
            "t" => Text::Typewriters,
            _ => unreachable!("every keyboard has a known category prefix"),
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // macOS prefetch cache
    pub fn sample_indices(self) -> impl Iterator<Item = usize> {
        let prefix = KEYBOARDS[self.0 as usize].1;
        files()
            .enumerate()
            .filter_map(move |(index, file)| file.starts_with(prefix).then_some(index))
    }

    pub fn sample_index(self, keycode: u16) -> Option<usize> {
        let prefix = KEYBOARDS[self.0 as usize].1;
        let group = files()
            .enumerate()
            .filter_map(move |(index, file)| file.strip_prefix(prefix).map(|role| (index, role)));
        let special = match keycode {
            49 => "_space.wav",
            36 | 76 => "_enter.wav",
            51 | 117 => "_delete.wav",
            _ => "",
        };
        if let Some((index, _)) = group.clone().find(|(_, role)| *role == special) {
            return Some(index);
        }
        // Soundy's routing for the additional groups is based on filename roles.
        // Only Tactile/Thocky/Topre modulo routing is confirmed in the RE notes.
        // `_release.wav` (only t_bell ships one) is deliberately unused: key-up is
        // not observed, so one key press is exactly one sound.
        let mut keys = group.filter(|(_, role)| role.starts_with("_key") || *role == ".wav");
        let count = keys.clone().count();
        (count > 0).then(|| keys.nth(usize::from(keycode) % count).unwrap().0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseProfile(pub u8);

impl MouseProfile {
    pub fn all() -> impl Iterator<Item = Self> {
        (0..=15).map(Self)
    }

    pub fn from_index(index: usize) -> Self {
        Self(if index <= 15 { index as u8 } else { 0 })
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // macOS prefetch cache
    pub fn sample_indices(self) -> impl Iterator<Item = usize> {
        self.sample_index().into_iter()
    }

    pub fn sample_index(self) -> Option<usize> {
        if self.0 == 0 {
            return None;
        }
        let id = if self.0 == 15 {
            "c".into()
        } else {
            self.0.to_string()
        };
        // The pair packs two variants of one click; only the first is routed.
        // KeyBell plays on mouse-down alone, so a click must stay one sound.
        let name = format!("mouse_{id}_1.wav");
        files().position(|file| file == name)
    }
}

pub fn validate(root: &Path, language: Language) -> Result<(), String> {
    for name in files() {
        let file = root.join(name);
        let metadata = file
            .metadata()
            .map_err(|e| format!("{}: {e}", file.display()))?;
        if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
            return Err(format!(
                "{}: {}",
                language.text(Text::InvalidSoundFile),
                file.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
