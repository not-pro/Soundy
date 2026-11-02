#![deny(unsafe_op_in_unsafe_fn)]
// Release builds are a tray app, so Windows must not attach a console window.
// Debug builds keep it so `cargo run` and the `--check` output stay visible.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod consts;
mod core;
#[cfg(target_os = "macos")]
mod ui;
#[cfg(windows)]
#[path = "ui_windows.rs"]
mod ui;
mod view_model;

#[cfg(not(any(target_os = "macos", windows)))]
compile_error!("Soundy supports macOS 13+ and Windows 10+ only");

use std::{path::PathBuf, sync::Arc, time::Duration};

use core::audio::Sounds;
use core::i18n::{Language, Text};

fn main() {
    let language = Language::current();
    let mut explicit_root: Option<PathBuf> = None;
    let mut check = false;
    let mut preview = false;
    let mut preview_mouse = false;
    let mut smoke_test = false;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--sounds-dir") => {
                explicit_root = Some(arguments.next().map(PathBuf::from).unwrap_or_else(|| {
                    eprintln!("{}", language.text(Text::SoundsPathRequired));
                    std::process::exit(2);
                }));
            }
            Some("--check") => check = true,
            Some("--preview") => preview = true,
            Some("--preview-mouse") => preview_mouse = true,
            Some("--smoke-test") => smoke_test = true,
            Some("--version") => {
                println!("Soundy {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            Some("--help" | "-h") => {
                println!("{}", language.text(Text::Help));
                return;
            }
            _ => {
                eprintln!(
                    "{}: {}",
                    language.text(Text::UnknownArgument),
                    argument.to_string_lossy()
                );
                std::process::exit(2);
            }
        }
    }

    let result = run_with_sounds(
        language,
        explicit_root,
        check,
        preview,
        preview_mouse,
        smoke_test,
    );
    if let Err(error) = result {
        eprintln!("Soundy: {error}");
        if !check && !preview && !preview_mouse {
            ui::show_error(&error, language);
        }
        std::process::exit(1);
    }
}

fn run_with_sounds(
    language: Language,
    explicit_root: Option<PathBuf>,
    check: bool,
    preview: bool,
    preview_mouse: bool,
    smoke_test: bool,
) -> Result<(), String> {
    let sounds = sounds_source(explicit_root, language)?;

    if check {
        return check_sounds(&sounds, language).map(|()| {
            println!("OK: 240 WAV decoded, 40 keyboard profiles, 15 mouse pairs; no audio or input started")
        });
    }

    if preview || preview_mouse {
        let controls = Arc::new(core::audio::Controls::default());
        controls
            .mouse_profile
            .store(1, std::sync::atomic::Ordering::Relaxed);
        #[cfg(windows)]
        if let Sounds::Bundled(cache) = &sounds {
            cache.ensure(
                core::catalog::Profile::from_index(0),
                core::catalog::MouseProfile::from_index(1),
            );
        }
        #[cfg(windows)]
        let worker = core::audio::Worker::start(sounds, controls, language)?;
        #[cfg(target_os = "macos")]
        let worker = {
            let Sounds::Files(root) = sounds;
            core::audio::Worker::start(&root, controls, language)?
        };
        let _ = worker.sender.send(if preview_mouse {
            core::audio::Command::PreviewMouse
        } else {
            core::audio::Command::Preview
        });
        std::thread::sleep(Duration::from_secs(1));
        return Ok(());
    }

    #[cfg(windows)]
    return ui::run(sounds, smoke_test, language);
    #[cfg(target_os = "macos")]
    {
        let Sounds::Files(root) = sounds;
        ui::run(&root, smoke_test, language)
    }
}

/// Resolve where the sounds come from. On Windows an explicit `--sounds-dir`
/// or a `Sounds` folder next to the EXE wins; otherwise all WAVs are taken
/// from the compressed bundle embedded in the EXE. macOS always uses a folder.
fn sounds_source(explicit_root: Option<PathBuf>, _language: Language) -> Result<Sounds, String> {
    #[cfg(target_os = "macos")]
    {
        let root = explicit_root
            .or_else(default_sounds_dir)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/sounds"));
        Ok(Sounds::Files(root))
    }

    #[cfg(windows)]
    {
        if let Some(root) = explicit_root {
            return Ok(Sounds::Files(root));
        }
        if let Some(root) = default_sounds_dir() {
            return Ok(Sounds::Files(root));
        }
        let config = core::settings::Config::load();
        let cache = Arc::new(core::bundle::Cache::load());
        cache.ensure(
            core::catalog::Profile::from_index(config.profile.into()),
            core::catalog::MouseProfile::from_index(config.mouse_profile.into()),
        );
        Ok(Sounds::Bundled(cache))
    }
}

fn check_sounds(sounds: &Sounds, language: Language) -> Result<(), String> {
    match sounds {
        Sounds::Files(root) => core::audio::check(root, language),
        #[cfg(windows)]
        Sounds::Bundled(cache) => cache.check_all(),
    }
}

/// macOS keeps sounds in `Soundy.app/Contents/Resources/Sounds`; Windows ships a
/// `Sounds` folder next to the executable. The manifest path only helps `cargo run`.
fn default_sounds_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join("Sounds"), dir.join("../Resources/Sounds")]
        .into_iter()
        .find(|candidate| candidate.is_dir())
}
