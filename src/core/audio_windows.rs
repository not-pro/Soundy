//! Windows audio: rodio mixes one decoded WAV per key press, so sounds overlap
//! instead of cutting each other off.

use std::{
    fs::File,
    path::Path,
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
    Mouse { at: Instant },
    Preview,
    PreviewMouse,
    Stop,
}

pub struct Worker {
    pub sender: SyncSender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(root: &Path, controls: Arc<Controls>, language: Language) -> Result<Self, String> {
        catalog::validate(root, language)?;
        let root = root.to_owned();
        let (sender, receiver) = mpsc::sync_channel(32);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("soundy-audio".into())
            .spawn(move || {
                let mut audio = match Audio::new(language) {
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
                    let index = match command {
                        Some(Command::Key { keycode, at })
                            if at.elapsed() < Duration::from_millis(100) =>
                        {
                            profile.sample_index(keycode)
                        }
                        Some(Command::Mouse { at })
                            if at.elapsed() < Duration::from_millis(100) =>
                        {
                            mouse.sample_index()
                        }
                        Some(Command::Preview) => profile.sample_index(0),
                        Some(Command::PreviewMouse) => mouse.sample_index(),
                        _ => None,
                    };
                    if let Some(index) = index {
                        audio.play(&root, index, controls.volume.load(Ordering::Relaxed));
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
    // Declared first so every player is dropped before the device it plays on.
    voices: Vec<Voice>,
    device: MixerDeviceSink,
    // ponytail: per-file 15ms gate, matching the macOS build; hammering one key
    // faster than that is the only case worth debouncing.
    last_play: Vec<Option<Instant>>,
}

impl Audio {
    fn new(language: Language) -> Result<Self, String> {
        let device = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("{}: {e}", language.text(Text::AudioOutputFailed)))?;
        Ok(Self {
            voices: Vec::new(),
            device,
            last_play: vec![None; catalog::files().count()],
        })
    }

    fn play(&mut self, root: &Path, index: usize, volume: u8) {
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
        let path = root.join(name);
        // No decoded-sample cache: a keypress-sized WAV decodes in well under a
        // millisecond, so caching would only add bookkeeping.
        let Ok(file) = File::open(&path) else {
            eprintln!("Soundy: {}: cannot open", path.display());
            return;
        };
        let decoder = match Decoder::try_from(file) {
            Ok(decoder) => decoder,
            Err(error) => {
                eprintln!("Soundy: {}: {error}", path.display());
                return;
            }
        };
        let duration = decoder
            .total_duration()
            .unwrap_or(Duration::from_millis(100));
        let player = Player::connect_new(self.device.mixer());
        player.set_volume(f32::from(volume.min(100)) / 100.0);
        player.append(decoder);
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
    for name in catalog::files() {
        let path = root.join(name);
        // Decode every sample so a truncated WAV fails here, not at play time.
        let decoder =
            Decoder::try_from(File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?)
                .map_err(|e| format!("{}: {e}", path.display()))?;
        if decoder.count() == 0 {
            return Err(format!(
                "{}: {}",
                language.text(Text::IncompleteWav),
                path.display()
            ));
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
