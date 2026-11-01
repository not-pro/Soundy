//! Bundled Windows sounds: all WAV files are compressed into the EXE at build
//! time and only the active keyboard/mouse profile is decompressed into memory.

use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, Mutex},
};

use flate2::read::ZlibDecoder;
use rodio::Decoder;

use crate::core::catalog::{MouseProfile, Profile};

include!(concat!(env!("OUT_DIR"), "/sounds_data.rs"));

struct Active {
    /// `None` until the first `ensure`. A plain default pair would compare
    /// equal on the very first call (default config is keyboard 0 / mouse 0)
    /// and leave the cache empty, so the app would start silent.
    loaded: Option<(Profile, MouseProfile)>,
    samples: HashMap<usize, Arc<[u8]>>,
}

pub struct Cache {
    bytes: &'static [u8],
    lookup: HashMap<String, (usize, usize)>,
    names: Vec<&'static str>,
    active: Mutex<Active>,
}

impl Cache {
    pub fn load() -> Self {
        let mut lookup = HashMap::new();
        let mut names = Vec::new();
        let mut cursor = 0usize;
        let count = u32::from_le_bytes([
            SOUND_BYTES[0],
            SOUND_BYTES[1],
            SOUND_BYTES[2],
            SOUND_BYTES[3],
        ]) as usize;
        cursor += 4;

        for _ in 0..count {
            let name_len =
                u16::from_le_bytes([SOUND_BYTES[cursor], SOUND_BYTES[cursor + 1]]) as usize;
            cursor += 2;
            let name = std::str::from_utf8(&SOUND_BYTES[cursor..cursor + name_len])
                .expect("valid UTF-8 filename in bundled sounds");
            cursor += name_len;
            let compressed_len = u32::from_le_bytes([
                SOUND_BYTES[cursor],
                SOUND_BYTES[cursor + 1],
                SOUND_BYTES[cursor + 2],
                SOUND_BYTES[cursor + 3],
            ]) as usize;
            cursor += 4;
            lookup.insert(name.to_string(), (cursor, compressed_len));
            names.push(name);
            cursor += compressed_len;
        }

        Self {
            bytes: SOUND_BYTES,
            lookup,
            names,
            active: Mutex::new(Active {
                loaded: None,
                samples: HashMap::new(),
            }),
        }
    }

    /// Decompress the bundled file by catalog index.
    pub fn get(&self, index: usize) -> Option<Arc<[u8]>> {
        self.active.lock().ok()?.samples.get(&index).cloned()
    }

    /// Ensure the in-memory cache holds exactly the samples needed for the
    /// given profiles. Old samples are dropped here; any voices still playing
    /// keep their own cloned `Arc` until they finish.
    pub fn ensure(&self, profile: Profile, mouse: MouseProfile) {
        let Ok(mut state) = self.active.lock() else {
            return;
        };
        if state.loaded == Some((profile, mouse)) {
            return;
        }
        state.loaded = Some((profile, mouse));
        state.samples.clear();

        let indices: Vec<_> = profile
            .sample_indices()
            .chain(mouse.sample_indices())
            .collect();
        for index in indices {
            let name = self.names[index];
            let Some((offset, len)) = self.lookup.get(name).copied() else {
                eprintln!("Soundy: bundled sample missing: {name}");
                continue;
            };
            match decompress(&self.bytes[offset..offset + len]) {
                Some(bytes) => {
                    state.samples.insert(index, bytes.into());
                }
                None => eprintln!("Soundy: failed to decompress {name}"),
            }
        }
    }

    /// Validate every bundled sample by decompressing and decoding it.
    pub fn check_all(&self) -> Result<(), String> {
        for name in self.names.iter().copied() {
            let (offset, len) = self
                .lookup
                .get(name)
                .copied()
                .ok_or_else(|| format!("bundled sample missing: {name}"))?;
            let bytes = decompress(&self.bytes[offset..offset + len])
                .ok_or_else(|| format!("failed to decompress {name}"))?;
            let decoder = Decoder::new(std::io::Cursor::new(Arc::<[u8]>::from(bytes)))
                .map_err(|e| format!("{name}: {e}"))?;
            if decoder.count() == 0 {
                return Err(format!("{name}: empty decoded audio"));
            }
        }
        Ok(())
    }
}

fn decompress(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_order_matches_the_catalog_manifest() {
        // build.rs sorts by filename while the catalog follows manifest.sha256;
        // a regenerated, unsorted manifest would silently play the wrong sounds.
        let cache = Cache::load();
        let catalog: Vec<_> = crate::core::catalog::files().collect();
        assert_eq!(cache.names.len(), catalog.len());
        for (index, name) in catalog.iter().enumerate() {
            assert_eq!(cache.names[index], *name, "catalog index {index}");
        }
    }

    #[test]
    fn default_profiles_load_their_samples() {
        let cache = Cache::load();
        let (profile, mouse) = (Profile::default(), MouseProfile::default());
        cache.ensure(profile, mouse);
        let mut indices = profile.sample_indices().peekable();
        assert!(indices.peek().is_some());
        for index in indices {
            assert!(cache.get(index).is_some(), "missing sample {index}");
        }
    }
}
