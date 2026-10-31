use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use objc2::{
    AnyThread,
    rc::{Retained, autoreleasepool},
};
use objc2_audio_toolbox::AudioComponentDescription;
use objc2_avf_audio::{
    AVAudioEngine, AVAudioFile, AVAudioMixing, AVAudioPCMBuffer, AVAudioPlayerNode,
    AVAudioPlayerNodeBufferOptions, AVAudioUnitEffect,
};
use objc2_foundation::{NSString, NSURL};

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
    Key {
        keycode: u16,
        pressed: bool,
        at: Instant,
    },
    Mouse {
        pressed: bool,
        at: Instant,
    },
    Preview,
    PreviewMouse,
    ProfilesChanged,
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
                autoreleasepool(|_| {
                    let mut audio = Player::new();
                    if let Err(error) = audio.start(language) {
                        let _ = ready_sender.send(Err(error));
                        return;
                    }
                    if ready_sender.send(Ok(())).is_err() {
                        return;
                    }
                    let mut next_cleanup = Instant::now();
                    loop {
                        let command = match receiver.recv_timeout(CACHE_SWEEP_INTERVAL) {
                            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                            Ok(command) => Some(command),
                            Err(RecvTimeoutError::Timeout) => None,
                        };
                        autoreleasepool(|_| {
                            let profile = Profile::from_index(
                                controls.profile.load(Ordering::Relaxed).into(),
                            );
                            let mouse = MouseProfile::from_index(
                                controls.mouse_profile.load(Ordering::Relaxed).into(),
                            );
                            let now = Instant::now();
                            if let Err(error) =
                                audio.select_profiles(&root, profile, mouse, language, now)
                            {
                                eprintln!("Soundy: {error}");
                            }
                            if now >= next_cleanup {
                                audio.evict_expired(now);
                                next_cleanup = now + CACHE_SWEEP_INTERVAL;
                            }
                            let index = match command {
                                Some(Command::Key {
                                    keycode,
                                    pressed,
                                    at,
                                }) if at.elapsed() < Duration::from_millis(100) => {
                                    profile.sample_index(keycode, pressed)
                                }
                                Some(Command::Mouse { pressed, at })
                                    if at.elapsed() < Duration::from_millis(100) =>
                                {
                                    mouse.sample_index(pressed)
                                }
                                Some(Command::Preview) => profile.sample_index(0, true),
                                Some(Command::PreviewMouse) => mouse.sample_index(true),
                                _ => None,
                            };
                            if let Some(index) = index {
                                audio.play(index, controls.volume.load(Ordering::Relaxed));
                            }
                        });
                    }
                });
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

const CACHE_TTL: Duration = Duration::from_secs(60);
const CACHE_SWEEP_INTERVAL: Duration = Duration::from_secs(1);

struct Voice {
    node: Retained<AVAudioPlayerNode>,
    busy_until: Instant,
}

struct Sample {
    buffer: Retained<AVAudioPCMBuffer>,
    duration: Duration,
    pool: usize,
    last_play: Option<Instant>,
    expires_at: Option<Instant>,
}

struct VoicePool {
    format: Retained<objc2_avf_audio::AVAudioFormat>,
    voices: Vec<Voice>,
}

struct Player {
    engine: Retained<AVAudioEngine>,
    samples: Vec<Option<Sample>>,
    pools: Vec<Option<VoicePool>>,
    profiles: Option<(Profile, MouseProfile)>,
    _limiter: Retained<AVAudioUnitEffect>,
}

impl Player {
    fn new() -> Self {
        // SAFETY: Engine, nodes, buffers and mixer stay on this one audio owner thread.
        unsafe {
            let engine = AVAudioEngine::new();
            let mixer = engine.mainMixerNode();
            let limiter = AVAudioUnitEffect::initWithAudioComponentDescription(
                AVAudioUnitEffect::alloc(),
                AudioComponentDescription {
                    componentType: u32::from_be_bytes(*b"aufx"),
                    componentSubType: u32::from_be_bytes(*b"lmtr"),
                    componentManufacturer: u32::from_be_bytes(*b"appl"),
                    componentFlags: 0,
                    componentFlagsMask: 0,
                },
            );
            engine.attachNode(&limiter);
            engine.connect_to_format(&mixer, &limiter, None);
            engine.connect_to_format(&limiter, &engine.outputNode(), None);
            engine.prepare();
            Self {
                engine,
                samples: std::iter::repeat_with(|| None)
                    .take(catalog::files().count())
                    .collect(),
                pools: Vec::new(),
                profiles: None,
                _limiter: limiter,
            }
        }
    }

    fn select_profiles(
        &mut self,
        root: &Path,
        profile: Profile,
        mouse: MouseProfile,
        language: Language,
        now: Instant,
    ) -> Result<(), String> {
        if self.profiles == Some((profile, mouse)) {
            return Ok(());
        }
        self.profiles = Some((profile, mouse));
        let needed: Vec<_> = profile
            .sample_indices()
            .chain(mouse.sample_indices())
            .collect();
        for (index, sample) in self.samples.iter_mut().enumerate() {
            if let Some(sample) = sample {
                if needed.contains(&index) {
                    sample.expires_at = None;
                } else if sample.expires_at.is_none() {
                    sample.expires_at = Some(now + CACHE_TTL);
                }
            }
        }
        let mut first_error = None;
        for index in needed {
            if self.samples[index].is_none() {
                match self.load_sample(root, index, language, now) {
                    Ok(sample) => self.samples[index] = Some(sample),
                    Err(error) => {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn load_sample(
        &mut self,
        root: &Path,
        index: usize,
        language: Language,
        now: Instant,
    ) -> Result<Sample, String> {
        let name = catalog::files().nth(index).expect("catalog sample index");
        let buffer = read_buffer(&root.join(name), language)?;
        // SAFETY: read_buffer validated this immutable PCM buffer's length and format.
        let (format, duration) = unsafe {
            let format = buffer.format();
            let duration =
                Duration::from_secs_f64(f64::from(buffer.frameLength()) / format.sampleRate());
            (format, duration)
        };
        let pool = match self.pools.iter().position(|pool| {
            pool.as_ref()
                // SAFETY: Both formats belong to validated PCM buffers on this thread.
                .is_some_and(|pool| unsafe { pool.format.isEqual(&format) })
        }) {
            Some(index) => index,
            None => {
                // SAFETY: Graph changes and playback are serialized on the audio thread.
                let voices = unsafe {
                    let mixer = self.engine.mainMixerNode();
                    // ponytail: six shared voices per PCM format; increase if dense bursts clip tails.
                    (0..6)
                        .map(|_| {
                            let node = AVAudioPlayerNode::new();
                            self.engine.attachNode(&node);
                            self.engine.connect_to_format(&node, &mixer, Some(&format));
                            Voice {
                                node,
                                busy_until: now,
                            }
                        })
                        .collect()
                };
                let pool = Some(VoicePool { format, voices });
                if let Some(index) = self.pools.iter().position(Option::is_none) {
                    self.pools[index] = pool;
                    index
                } else {
                    self.pools.push(pool);
                    self.pools.len() - 1
                }
            }
        };
        Ok(Sample {
            buffer,
            duration,
            pool,
            last_play: None,
            expires_at: None,
        })
    }

    fn evict_expired(&mut self, now: Instant) {
        for slot in &mut self.samples {
            if slot.as_ref().is_some_and(|sample| {
                sample.expires_at.is_some_and(|expires| now >= expires)
                    && sample
                        .last_play
                        .is_none_or(|last| now.saturating_duration_since(last) >= sample.duration)
            }) {
                *slot = None;
            }
        }
        for (index, slot) in self.pools.iter_mut().enumerate() {
            if slot.as_ref().is_some_and(|pool| {
                !self
                    .samples
                    .iter()
                    .flatten()
                    .any(|sample| sample.pool == index)
                    && pool.voices.iter().all(|voice| voice.busy_until <= now)
            }) {
                let pool = slot.take().unwrap();
                for voice in pool.voices {
                    // SAFETY: No cached sample references this pool and all scheduled
                    // playback has finished. Detaching releases the engine's node ownership.
                    unsafe {
                        voice.node.stop();
                        self.engine.detachNode(&voice.node);
                    }
                }
            }
        }
    }

    fn start(&self, language: Language) -> Result<(), String> {
        unsafe { self.engine.startAndReturnError() }
            .map_err(|e| format!("{}: {e}", language.text(Text::AudioOutputFailed)))
    }

    fn play(&mut self, index: usize, volume: u8) {
        if volume == 0 {
            return;
        }
        let now = Instant::now();
        let Some(sample) = self.samples[index].as_mut() else {
            return;
        };
        // ponytail: per-file 15ms gate; refine its scope after the full cache key is recovered.
        if sample
            .last_play
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(15))
        {
            return;
        }
        let voice = self.pools[sample.pool]
            .as_mut()
            .expect("cached sample voice pool")
            .voices
            .iter_mut()
            .min_by_key(|voice| voice.busy_until)
            .expect("six preallocated voices");
        unsafe {
            // Interrupt a stolen voice instead of appending to its queue.
            let options = if voice.busy_until > now {
                AVAudioPlayerNodeBufferOptions::Interrupts
            } else {
                AVAudioPlayerNodeBufferOptions::empty()
            };
            voice.node.setVolume(f32::from(volume.min(100)) / 100.0);
            voice.node.scheduleBuffer_atTime_options_completionHandler(
                &sample.buffer,
                None,
                options,
                std::ptr::null_mut(),
            );
            if !voice.node.isPlaying() {
                voice.node.play();
            }
        }
        voice.busy_until = now + sample.duration;
        sample.last_play = Some(now);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            self.engine.stop();
        }
    }
}

fn read_buffer(path: &Path, language: Language) -> Result<Retained<AVAudioPCMBuffer>, String> {
    let path_text = path.to_str().ok_or(language.text(Text::PathUtf8Required))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path_text));
    unsafe {
        let file = AVAudioFile::initForReading_error(AVAudioFile::alloc(), &url)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let format = file.processingFormat();
        let frames = file.length();
        let rate = format.sampleRate();
        if frames <= 0
            || !rate.is_finite()
            || !(8_000.0..=192_000.0).contains(&rate)
            || frames as f64 / rate > 10.0
            || !(1..=2).contains(&format.channelCount())
        {
            return Err(format!(
                "{}: {}",
                language.text(Text::InvalidAudioFormat),
                path.display()
            ));
        }
        let capacity = u32::try_from(frames).map_err(|_| language.text(Text::WavTooLarge))?;
        let buffer = AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
            AVAudioPCMBuffer::alloc(),
            &format,
            capacity,
        )
        .ok_or(language.text(Text::AudioBufferFailed))?;
        file.readIntoBuffer_error(&buffer)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if buffer.frameLength() != capacity {
            return Err(format!(
                "{}: {}",
                language.text(Text::IncompleteWav),
                path.display()
            ));
        }
        Ok(buffer)
    }
}

pub fn check(root: &Path, language: Language) -> Result<(), String> {
    catalog::validate(root, language)?;
    autoreleasepool(|_| {
        for name in catalog::files() {
            let _ = read_buffer(&root.join(name), language)?;
        }
        Ok(())
    })
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
