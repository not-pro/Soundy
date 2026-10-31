#![deny(unsafe_op_in_unsafe_fn)]

mod consts;
mod core;
#[cfg(target_os = "macos")]
mod ui;
#[cfg(target_os = "macos")]
mod utils;

#[cfg(not(target_os = "macos"))]
compile_error!("Soundy currently supports macOS 13+ only");

use std::{path::PathBuf, sync::Arc, time::Duration};

use core::i18n::{Language, Text};

fn main() {
    let language = Language::current();
    let mut root = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("../Resources/Sounds")))
        .filter(|dir| dir.is_dir())
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
            println!("OK: 240 WAV decoded, 40 keyboard profiles, 15 mouse pairs; no input monitoring started")
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
