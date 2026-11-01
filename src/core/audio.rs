use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
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
    AVAudioPlayerNodeBufferOptions, AVAudioStereoMixing, AVAudioUnitEffect,
};
use objc2_foundation::{NSString, NSURL};

use crate::core::{
    catalog::{self, MouseProfile, Profile},
    i18n::{Language, Text},
    pan, variation,
};

/// macOS uses a real `Sounds/` folder, so the bundled variant is not needed.
pub enum Sounds {
    Files(PathBuf),
}

pub struct Controls {
    pub profile: AtomicU8,
    pub mouse_profile: AtomicU8,
    pub volume: AtomicU8,
    /// Global power switch; hotkeys keep working while this is off.
    pub enabled: AtomicBool,
    /// Per-app "no keyboard sound" override.
    pub keyboard_off: AtomicBool,
    /// Stereo Key Position: pan keys to their physical spots.
    pub stereo_pan: AtomicBool,
    /// Audio-route gating (headphones/speakers mode).
    pub route_allows: AtomicBool,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            profile: AtomicU8::new(0),
            mouse_profile: AtomicU8::new(0),
            volume: AtomicU8::new(25),
            enabled: AtomicBool::new(true),
            keyboard_off: AtomicBool::new(false),
            stereo_pan: AtomicBool::new(false),
            route_allows: AtomicBool::new(true),
        }
    }
}

pub enum Command {
    Key {
        keycode: u16,
        at: Instant,
    },
    KeyUp {
        at: Instant,
    },
    Mouse {
        right: bool,
        at: Instant,
    },
    Preview,
    PreviewMouse,
    ProfilesChanged,
    /// Restart the engine after sleep or an output-device change.
    ResetAudio,
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
                            let sounding = controls.enabled.load(Ordering::Relaxed)
                                && controls.route_allows.load(Ordering::Relaxed);
                            let keys_allowed =
                                sounding && !controls.keyboard_off.load(Ordering::Relaxed);
                            let index = match command {
                                Some(Command::Key { keycode, at })
                                    if at.elapsed() < Duration::from_millis(100)
                                        && keys_allowed =>
                                {
                                    profile.sample_index(keycode).map(|index| {
                                        let pan = controls
                                            .stereo_pan
                                            .load(Ordering::Relaxed)
                                            .then(|| pan::key_pan(keycode));
                                        (index, pan)
                                    })
                                }
                                Some(Command::KeyUp { at })
                                    if at.elapsed() < Duration::from_millis(100)
                                        && keys_allowed =>
                                {
                                    profile.release_index().map(|index| (index, None))
                                }
                                Some(Command::Mouse { right, at })
                                    if at.elapsed() < Duration::from_millis(100) && sounding =>
                                {
                                    mouse.sample_index(right).map(|index| (index, None))
                                }
                                Some(Command::Preview) => {
                                    profile.sample_index(0).map(|index| (index, None))
                                }
                                Some(Command::PreviewMouse) => {
                                    mouse.sample_index(false).map(|index| (index, None))
                                }
                                Some(Command::ResetAudio) => {
                                    if let Err(error) = audio.restart(language) {
                                        eprintln!("Soundy: {error}");
                                    }
                                    None
                                }
                                _ => None,
                            };
                            if let Some((index, pan)) = index {
                                audio.play(index, controls.volume.load(Ordering::Relaxed), pan);
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

const CACHE_TTL: Duration = Duration::from_secs(10);
const CACHE_SWEEP_INTERVAL: Duration = Duration::from_secs(1);

struct Voice {
    node: Retained<AVAudioPlayerNode>,
    busy_until: Instant,
    // Keeps the pitched variant alive for as long as this voice may still play it.
    buffer: Option<Retained<AVAudioPCMBuffer>>,
}

struct Sample {
    buffer: Retained<AVAudioPCMBuffer>,
    duration: Duration,
    pool: usize,
    last_play: Option<Instant>,
    last_variant: Option<u8>,
    expires_at: Option<Instant>,
}

struct VoicePool {
    format: Retained<objc2_avf_audio::AVAudioFormat>,
    voices: Vec<Voice>,
}

struct Player {
    samples: Vec<Option<Sample>>,
    pools: Vec<Option<VoicePool>>,
    profiles: Option<(Profile, MouseProfile)>,
    random: variation::Random,
    _limiter: Retained<AVAudioUnitEffect>,
    // Declared last so the engine outlives the nodes, buffers and effect it owns.
    // Dropping it first tears the audio graph down underneath them and crashes.
    engine: Retained<AVAudioEngine>,
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
                random: variation::Random::new(),
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
                                buffer: None,
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
            last_variant: None,
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
        for pool in self.pools.iter_mut().flatten() {
            // A pitched variant only needs to live until its voice falls silent;
            // keeping it resident triples the cache footprint for no audible gain.
            for voice in &mut pool.voices {
                if voice.busy_until <= now {
                    voice.buffer = None;
                }
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

    /// Stops and restarts the engine; used after sleep and output changes,
    /// when the render chain can come back in a broken state.
    fn restart(&mut self, language: Language) -> Result<(), String> {
        unsafe { self.engine.stop() };
        self.start(language)
    }

    fn play(&mut self, index: usize, volume: u8, pan: Option<f32>) {
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
        let variant = self
            .random
            .pick(sample.last_variant, variation::RATIOS.len() as u8);
        sample.last_variant = Some(variant);
        let ratio = variation::RATIOS[usize::from(variant)];
        let level = self.random.level();
        // SAFETY: the variant keeps the cached buffer's PCM format and is written on the
        // audio thread before it is scheduled; ratio 1.0 reuses the cached buffer.
        let pitched = (ratio != 1.0)
            .then(|| unsafe { resample(&sample.buffer, ratio) })
            .flatten();
        let voice = self.pools[sample.pool]
            .as_mut()
            .expect("cached sample voice pool")
            .voices
            .iter_mut()
            .min_by_key(|voice| voice.busy_until)
            .expect("six preallocated voices");
        voice.buffer = pitched;
        let buffer = voice.buffer.as_deref().unwrap_or(&sample.buffer);
        unsafe {
            if let Some(pan) = pan {
                // AVAudioMixing: Stereo Key Position, center when disabled.
                voice.node.setPan(pan);
            }
            // Interrupt a stolen voice instead of appending to its queue.
            let options = if voice.busy_until > now {
                AVAudioPlayerNodeBufferOptions::Interrupts
            } else {
                AVAudioPlayerNodeBufferOptions::empty()
            };
            voice
                .node
                .setVolume((f32::from(volume.min(100)) / 100.0 * level).min(1.0));
            voice.node.scheduleBuffer_atTime_options_completionHandler(
                buffer,
                None,
                options,
                std::ptr::null_mut(),
            );
            if !voice.node.isPlaying() {
                voice.node.play();
            }
        }
        voice.busy_until = now + sample.duration.div_f32(ratio);
        sample.last_play = Some(now);
    }
}

/// Rebuilds a cached sample at another pitch and length by resampling it, the
/// way the reference loader does (0x10005ecd4); a press therefore varies by up
/// to ±3% instead of replaying one identical waveform.
///
/// # Safety
/// `buffer` must be a validated 32-bit float PCM buffer owned by this thread, and
/// the result must be scheduled only on the same audio graph.
unsafe fn resample(buffer: &AVAudioPCMBuffer, ratio: f32) -> Option<Retained<AVAudioPCMBuffer>> {
    let frames = unsafe { buffer.frameLength() };
    if frames == 0 {
        return None;
    }
    // SAFETY: read_buffer only produces float PCM buffers from bundled WAV files.
    let format = unsafe { buffer.format() };
    let capacity = (f64::from(frames) / f64::from(ratio)).ceil() as u32;
    // SAFETY: The format is PCM, so the initializer has a valid byte capacity.
    let pitched = unsafe {
        AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
            AVAudioPCMBuffer::alloc(),
            &format,
            capacity,
        )
    }?;
    // SAFETY: Both buffers are alive for this call and only this thread touches them.
    let (source, target) = unsafe { (buffer.floatChannelData(), pitched.floatChannelData()) };
    if source.is_null() || target.is_null() {
        return None;
    }
    unsafe { pitched.setFrameLength(capacity) };
    let (stride, target_stride) = unsafe { (buffer.stride(), pitched.stride()) };
    let last = frames as usize - 1;
    for channel in 0..unsafe { format.channelCount() } as usize {
        // SAFETY: floatChannelData yields channelCount pointers, each to frameLength samples.
        let source = unsafe { (*source.add(channel)).as_ptr() };
        let target = unsafe { (*target.add(channel)).as_ptr() };
        for frame in 0..capacity as usize {
            let position = frame as f32 * ratio;
            let first = (position as usize).min(last);
            let fraction = position - first as f32;
            // SAFETY: first <= last, and first + 1 is read only when it is in range.
            let current = unsafe { *source.add(first * stride) };
            let next = if first < last {
                unsafe { *source.add((first + 1) * stride) }
            } else {
                current
            };
            unsafe { *target.add(frame * target_stride) = current + (next - current) * fraction };
        }
    }
    Some(pitched)
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
