use std::path::Path;

use crate::i18n::{Language, Text};

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
    include_str!("../assets/sounds/manifest.sha256")
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

    pub fn sample_index(self, keycode: u16, pressed: bool) -> Option<usize> {
        let prefix = KEYBOARDS[self.0 as usize].1;
        let group = files()
            .enumerate()
            .filter_map(move |(index, file)| file.strip_prefix(prefix).map(|role| (index, role)));
        if !pressed {
            return group
                .clone()
                .find(|(_, role)| *role == "_release.wav")
                .map(|(i, _)| i);
        }
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

    pub fn sample_index(self, pressed: bool) -> Option<usize> {
        if self.0 == 0 {
            return None;
        }
        let id = if self.0 == 15 {
            "c".into()
        } else {
            self.0.to_string()
        };
        // Soundy uses each pair as press/release for all mouse buttons.
        // This is an explicit product rule, not a claim about KeyBell's routing.
        let name = format!("mouse_{id}_{}.wav", if pressed { 1 } else { 2 });
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
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_covers_every_packaged_sound_and_preserves_routes() {
        let names: Vec<_> = files().collect();
        assert_eq!(names.len(), 240);
        assert_eq!(names.iter().copied().collect::<HashSet<_>>().len(), 240);
        for (category, count) in [
            (Text::Mechanical, 14),
            (Text::Electronic, 6),
            (Text::Fun, 11),
            (Text::Typewriters, 9),
        ] {
            assert_eq!(
                Profile::all()
                    .filter(|profile| profile.category() == category)
                    .count(),
                count
            );
        }
        validate(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds"),
            Language::English,
        )
        .unwrap();
        let mut used = HashSet::new();
        for profile in Profile::all() {
            for keycode in 0..128 {
                for pressed in [false, true] {
                    if let Some(index) = profile.sample_index(keycode, pressed) {
                        used.insert(index);
                    }
                }
            }
        }
        for profile in MouseProfile::all() {
            for pressed in [false, true] {
                if let Some(index) = profile.sample_index(pressed) {
                    used.insert(index);
                }
            }
        }
        assert_eq!(used.len(), 240, "every WAV must be reachable");
        for profile in [Profile(0), Profile(1), Profile(2)] {
            let prefix = KEYBOARDS[profile.0 as usize].1;
            for (keycode, role) in [
                (49, "space"),
                (36, "key5"),
                (51, "key4"),
                (0, "key1"),
                (8, "key1"),
            ] {
                assert_eq!(
                    names[profile.sample_index(keycode, true).unwrap()],
                    format!("{prefix}_{role}.wav")
                );
            }
            assert_eq!(profile.sample_index(0, false), None);
        }
        assert_eq!(MouseProfile(0).sample_index(true), None);
        assert_eq!(
            names[MouseProfile(1).sample_index(true).unwrap()],
            "mouse_1_1.wav"
        );
        assert_eq!(
            names[MouseProfile(1).sample_index(false).unwrap()],
            "mouse_1_2.wav"
        );
        assert_eq!(
            names[MouseProfile(15).sample_index(false).unwrap()],
            "mouse_c_2.wav"
        );
        assert_eq!(Profile::from_index(255), Profile(0));
        assert_eq!(MouseProfile::from_index(255), MouseProfile(0));
    }
}
