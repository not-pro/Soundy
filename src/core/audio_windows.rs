//! Windows audio: rodio mixes one decoded WAV per key press, so sounds overlap
//! instead of cutting each other off.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::core::{
    catalog::{self, MouseProfile, Profile},
    i18n::{Language, Text},
    variation,
};

pub struct Controls {
    pub profile: AtomicU8,
    pub mouse_profile: AtomicU8,
    pub volume: AtomicU8,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            profile: AtomicU8::new(0),
            mouse_profile: AtomicU8::new(0),
            volume: AtomicU8::new(25),
        }
    }
}

pub enum Command {
    Key { keycode: u16, at: Instant },
    KeyUp { at: Instant },
    Mouse { right: bool, at: Instant },
    Preview,
    PreviewMouse,
    Stop,
}

/// Source of WAV samples: either a `Sounds/` folder on disk or a bundled,
/// compressed archive embedded in the EXE.
pub enum Sounds {
    Files(PathBuf),
    Bundled(Arc<crate::core::bundle::Cache>),
}

impl Sounds {
    /// Fetch one sample by catalog index. For the file-backed mode the WAV is
    /// read from disk; for the bundled mode it is taken from the in-memory
    /// profile cache.
    fn get(&self, index: usize) -> Option<Arc<[u8]>> {
        let name = catalog::files().nth(index)?;
        match self {
            Sounds::Files(root) => {
                let bytes = std::fs::read(root.join(name)).ok()?;
                Some(bytes.into())
            }
            Sounds::Bundled(cache) => cache.get(index),
        }
    }

    /// For the bundled mode, keep only the samples needed by the active
    /// profiles in memory. File-backed mode needs no cache management.
    fn ensure(&self, profile: Profile, mouse: MouseProfile) {
        if let Sounds::Bundled(cache) = self {
            cache.ensure(profile, mouse);
        }
    }
}

pub struct Worker {
    pub sender: SyncSender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(
        sounds: Sounds,
        controls: Arc<Controls>,
        language: Language,
    ) -> Result<Self, String> {
        if let Sounds::Files(root) = &sounds {
            catalog::validate(root, language)?;
        }
        let (sender, receiver) = mpsc::sync_channel(32);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("soundy-audio".into())
            .spawn(move || {
                let mut audio = match Audio::new(sounds, language) {
                    Ok(audio) => audio,
                    Err(error) => {
                        let _ = ready_sender.send(Err(error));
                        return;
                    }
                };
                if ready_sender.send(Ok(())).is_err() {
                    return;
                }
                loop {
                    let command = match receiver.recv_timeout(SWEEP_INTERVAL) {
                        Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                        Ok(command) => Some(command),
                        Err(RecvTimeoutError::Timeout) => None,
                    };
                    let profile =
                        Profile::from_index(controls.profile.load(Ordering::Relaxed).into());
                    let mouse = MouseProfile::from_index(
                        controls.mouse_profile.load(Ordering::Relaxed).into(),
                    );
                    audio.sounds.ensure(profile, mouse);
                    let index = match command {
                        Some(Command::Key { keycode, at })
                            if at.elapsed() < Duration::from_millis(100) =>
                        {
                            profile.sample_index(keycode)
                        }
                        Some(Command::KeyUp { at })
                            if at.elapsed() < Duration::from_millis(100) =>
                        {
                            profile.release_index()
                        }
                        Some(Command::Mouse { right, at })
                            if at.elapsed() < Duration::from_millis(100) =>
                        {
                            mouse.sample_index(right)
                        }
                        Some(Command::Preview) => profile.sample_index(0),
                        Some(Command::PreviewMouse) => mouse.sample_index(false),
                        _ => None,
                    };
                    if let Some(index) = index {
                        audio.play(index, controls.volume.load(Ordering::Relaxed));
                    }
                    audio.evict_finished();
                }
            })
            .map_err(|e| e.to_string())?;
        match ready_receiver
            .recv()
            .map_err(|_| language.text(Text::AudioThreadStopped))?
        {
            Ok(()) => Ok(Self {
                sender,
                thread: Some(thread),
            }),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

const SWEEP_INTERVAL: Duration = Duration::from_millis(250);
const RELEASE: Duration = Duration::from_millis(50);

struct Voice {
    player: Player,
    until: Instant,
}

struct Audio {
    sounds: Sounds,
    // Declared before the device so every player is dropped before the device
    // it plays on.
    voices: Vec<Voice>,
    device: MixerDeviceSink,
    // ponytail: per-file 15ms gate, matching the macOS build; hammering one key
    // faster than that is the only case worth debouncing.
    last_play: Vec<Option<Instant>>,
    last_variant: Vec<Option<u8>>,
    random: variation::Random,
}

impl Audio {
    fn new(sounds: Sounds, language: Language) -> Result<Self, String> {
        let device = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("{}: {e}", language.text(Text::AudioOutputFailed)))?;
        Ok(Self {
            sounds,
            voices: Vec::new(),
            device,
            last_play: vec![None; catalog::files().count()],
            last_variant: vec![None; catalog::files().count()],
            random: variation::Random::new(),
        })
    }

    fn play(&mut self, index: usize, volume: u8) {
        if volume == 0 || index >= self.last_play.len() {
            return;
        }
        let now = Instant::now();
        if self.last_play[index]
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(15))
        {
            return;
        }
        self.last_play[index] = Some(now);
        let name = catalog::files().nth(index).expect("catalog sample index");
        let Some(bytes) = self.sounds.get(index) else {
            eprintln!("Soundy: {}: cannot open", name);
            return;
        };
        let decoder = match Decoder::try_from(Cursor::new(bytes)) {
            Ok(decoder) => decoder,
            Err(error) => {
                eprintln!("Soundy: {}: {error}", name);
                return;
            }
        };
        // Same pitch/length and level variation as the macOS build: rodio's speed
        // filter changes the sample rate, which shifts pitch and duration together.
        let variant = self
            .random
            .pick(self.last_variant[index], variation::RATIOS.len() as u8);
        self.last_variant[index] = Some(variant);
        let source = decoder.speed(variation::RATIOS[usize::from(variant)]);
        let level = self.random.level();
        let duration = source
            .total_duration()
            .unwrap_or(Duration::from_millis(100));
        let player = Player::connect_new(self.device.mixer());
        player.set_volume((f32::from(volume.min(100)) / 100.0 * level).min(1.0));
        player.append(source);
        self.voices.push(Voice {
            player,
            until: now + duration + RELEASE,
        });
    }

    fn evict_finished(&mut self) {
        let now = Instant::now();
        self.voices
            .retain(|voice| now < voice.until || !voice.player.empty());
    }
}

pub fn check(root: &Path, language: Language) -> Result<(), String> {
    catalog::validate(root, language)?;
    let sounds = Sounds::Files(root.to_owned());
    for (index, name) in catalog::files().enumerate() {
        let bytes = sounds
            .get(index)
            .ok_or_else(|| format!("{}: cannot open", name))?;
        let decoder =
            Decoder::try_from(Cursor::new(bytes)).map_err(|e| format!("{}: {e}", name))?;
        if decoder.count() == 0 {
            return Err(format!("{}: {}", language.text(Text::IncompleteWav), name));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_wav_decodes_without_opening_an_output_device() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds");
        check(&root, Language::English).unwrap();
    }
}
