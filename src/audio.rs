use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc::{self, SyncSender},
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

use crate::catalog::{self, MouseProfile, Profile};

pub struct Controls {
    pub enabled: AtomicBool,
    pub profile: AtomicU8,
    pub mouse_profile: AtomicU8,
    pub volume: AtomicU8,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(true),
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
    Stop,
}

pub struct Worker {
    pub sender: SyncSender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(root: &Path, controls: Arc<Controls>) -> Result<Self, String> {
        catalog::validate(root)?;
        let root = root.to_owned();
        let (sender, receiver) = mpsc::sync_channel(32);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("soundy-audio".into())
            .spawn(move || {
                autoreleasepool(|_| {
                    let mut audio = match Player::load(&root) {
                        Ok(audio) => audio,
                        Err(error) => {
                            let _ = ready_sender.send(Err(error));
                            return;
                        }
                    };
                    if let Err(error) = audio.start() {
                        let _ = ready_sender.send(Err(error));
                        return;
                    }
                    if ready_sender.send(Ok(())).is_err() {
                        return;
                    }
                    while let Ok(command) = receiver.recv() {
                        autoreleasepool(|_| {
                            let profile = Profile::from_index(
                                controls.profile.load(Ordering::Relaxed).into(),
                            );
                            let mouse = MouseProfile::from_index(
                                controls.mouse_profile.load(Ordering::Relaxed).into(),
                            );
                            let active = controls.enabled.load(Ordering::Relaxed);
                            let index = match command {
                                Command::Key {
                                    keycode,
                                    pressed,
                                    at,
                                } if active && at.elapsed() < Duration::from_millis(100) => {
                                    profile.sample_index(keycode, pressed)
                                }
                                Command::Mouse { pressed, at }
                                    if active && at.elapsed() < Duration::from_millis(100) =>
                                {
                                    mouse.sample_index(pressed)
                                }
                                Command::Preview => profile.sample_index(0, true),
                                Command::PreviewMouse => mouse.sample_index(true),
                                _ => None,
                            };
                            if let Some(index) = index {
                                audio.play(index, controls.volume.load(Ordering::Relaxed));
                            }
                        });
                        if matches!(command, Command::Stop) {
                            break;
                        }
                    }
                });
            })
            .map_err(|e| e.to_string())?;
        match ready_receiver
            .recv()
            .map_err(|_| "Аудиопоток завершился при запуске")?
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

struct Voice {
    node: Retained<AVAudioPlayerNode>,
    busy_until: Instant,
}

struct Sample {
    buffer: Retained<AVAudioPCMBuffer>,
    duration: Duration,
    pool: usize,
    last_play: Option<Instant>,
}

struct VoicePool {
    format: Retained<objc2_avf_audio::AVAudioFormat>,
    voices: Vec<Voice>,
}

struct Player {
    engine: Retained<AVAudioEngine>,
    samples: Vec<Sample>,
    pools: Vec<VoicePool>,
    _limiter: Retained<AVAudioUnitEffect>,
}

impl Player {
    fn load(root: &Path) -> Result<Self, String> {
        // SAFETY: Engine, nodes, buffers and mixer stay on this one audio owner thread.
        // Formats are read from AVAudioFile, validated, and connected before engine start.
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
            let mut samples = Vec::new();
            let mut pools: Vec<VoicePool> = Vec::new();
            for name in catalog::files() {
                let buffer = read_buffer(&root.join(name))?;
                let format = buffer.format();
                let duration =
                    Duration::from_secs_f64(f64::from(buffer.frameLength()) / format.sampleRate());
                // ponytail: six shared voices per PCM format; increase if dense bursts clip tails.
                let pool = match pools.iter().position(|pool| pool.format.isEqual(&format)) {
                    Some(index) => index,
                    None => {
                        let voices = (0..6)
                            .map(|_| {
                                let node = AVAudioPlayerNode::new();
                                engine.attachNode(&node);
                                engine.connect_to_format(&node, &mixer, Some(&format));
                                Voice {
                                    node,
                                    busy_until: Instant::now(),
                                }
                            })
                            .collect();
                        pools.push(VoicePool { format, voices });
                        pools.len() - 1
                    }
                };
                samples.push(Sample {
                    buffer,
                    duration,
                    pool,
                    last_play: None,
                });
            }
            engine.prepare();
            Ok(Self {
                engine,
                samples,
                pools,
                _limiter: limiter,
            })
        }
    }

    fn start(&self) -> Result<(), String> {
        unsafe { self.engine.startAndReturnError() }
            .map_err(|e| format!("Не удалось запустить аудиовыход: {e}"))
    }

    fn play(&mut self, index: usize, volume: u8) {
        if volume == 0 {
            return;
        }
        let now = Instant::now();
        let sample = &mut self.samples[index];
        // ponytail: per-file 15ms gate; refine its scope after the full cache key is recovered.
        if sample
            .last_play
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(15))
        {
            return;
        }
        let voice = self.pools[sample.pool]
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

fn read_buffer(path: &Path) -> Result<Retained<AVAudioPCMBuffer>, String> {
    let path_text = path.to_str().ok_or("Путь к звукам должен быть UTF-8")?;
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
                "Недопустимый формат или длительность: {}",
                path.display()
            ));
        }
        let capacity = u32::try_from(frames).map_err(|_| "Слишком большой WAV")?;
        let buffer = AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
            AVAudioPCMBuffer::alloc(),
            &format,
            capacity,
        )
        .ok_or("Не удалось выделить аудиобуфер")?;
        file.readIntoBuffer_error(&buffer)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if buffer.frameLength() != capacity {
            return Err(format!("Неполный WAV: {}", path.display()));
        }
        Ok(buffer)
    }
}

pub fn check(root: &Path) -> Result<(), String> {
    catalog::validate(root)?;
    autoreleasepool(|_| {
        for name in catalog::files() {
            let _ = read_buffer(&root.join(name))?;
        }
        Ok(())
    })
}
