//! Rules recovered in keybell-research/native-audio-notes.md, 0x10005913c..19c.
//! Pitch variants and other presets are deliberately not inferred from filenames.
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Profile {
    #[default]
    Tactile,
    Thocky,
    Topre,
}

impl Profile {
    pub const ALL: [Self; 3] = [Self::Tactile, Self::Thocky, Self::Topre];

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Tactile => "Tactile",
            Self::Thocky => "Thocky",
            Self::Topre => "Topre",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Tactile => "m_tactile",
            Self::Thocky => "m_thocky",
            Self::Topre => "m_topre",
        }
    }

    pub fn sample_index(self, keycode: u16) -> usize {
        if keycode == 49 {
            8
        } else {
            usize::from(keycode % 8)
        }
    }

    pub fn files(self, root: &Path) -> Vec<PathBuf> {
        (1..=8)
            .map(|i| root.join(format!("{}_key{i}.wav", self.prefix())))
            .chain(std::iter::once(
                root.join(format!("{}_space.wav", self.prefix())),
            ))
            .collect()
    }
}

pub fn validate(root: &Path) -> Result<(), String> {
    for file in Profile::ALL
        .into_iter()
        .flat_map(|profile| profile.files(root))
    {
        let metadata = file
            .metadata()
            .map_err(|e| format!("{}: {e}", file.display()))?;
        if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
            return Err(format!("Недопустимый звуковой файл: {}", file.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovered_keyboard_routes() {
        for profile in Profile::ALL {
            assert_eq!(profile.sample_index(49), 8); // Dedicated Space.
            assert_eq!(profile.sample_index(36), 4); // Return uses key5, not enter.wav.
            assert_eq!(profile.sample_index(51), 3); // Backspace uses key4.
            assert_eq!(profile.sample_index(0), 0);
            assert_eq!(profile.sample_index(8), 0);
            assert_eq!(profile.files(Path::new("/sounds")).len(), 9);
        }
        assert_eq!(Profile::from_index(255), Profile::Tactile);
    }
}
