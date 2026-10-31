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

#[cfg(not(any(target_os = "macos", windows)))]
compile_error!("Soundy supports macOS 13+ and Windows 10+ only");

use std::{path::PathBuf, sync::Arc, time::Duration};

use core::i18n::{Language, Text};

fn main() {
    let language = Language::current();
    let mut root = default_sounds_dir()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/sounds"));
    let mut check = false;
    let mut preview = false;
    let mut preview_mouse = false;
    let mut smoke_test = false;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--sounds-dir") => {
                root = arguments.next().map(PathBuf::from).unwrap_or_else(|| {
                    eprintln!("{}", language.text(Text::SoundsPathRequired));
                    std::process::exit(2);
                });
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
    let result = if check {
        core::audio::check(&root, language).map(|()| {
            println!("OK: 240 WAV decoded, 40 keyboard profiles, 15 mouse pairs; no audio or input started")
        })
    } else if preview || preview_mouse {
        let controls = Arc::new(core::audio::Controls::default());
        controls
            .mouse_profile
            .store(1, std::sync::atomic::Ordering::Relaxed);
        core::audio::Worker::start(&root, controls, language).map(|worker| {
            let _ = worker.sender.send(if preview_mouse {
                core::audio::Command::PreviewMouse
            } else {
                core::audio::Command::Preview
            });
            std::thread::sleep(Duration::from_secs(1));
        })
    } else {
        ui::run(&root, smoke_test, language)
    };
    if let Err(error) = result {
        eprintln!("Soundy: {error}");
        if !check && !preview && !preview_mouse {
            ui::show_error(&error, language);
        }
        std::process::exit(1);
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
