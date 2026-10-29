#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "macos")]
mod audio;
mod catalog;
#[cfg(target_os = "macos")]
mod input;
#[cfg(target_os = "macos")]
mod ui;

#[cfg(not(target_os = "macos"))]
compile_error!("Soundy currently supports macOS 13+ only");

use std::{path::PathBuf, sync::Arc, time::Duration};

fn main() {
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
                    eprintln!("--sounds-dir требует путь к WAV");
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
                println!(
                    "Soundy — звуки клавиатуры и мыши для macOS\n\n  --sounds-dir PATH  Свой каталог WAV (по умолчанию встроенные звуки)\n  --check            Проверить 240 WAV без звука и мониторинга ввода\n  --preview          Воспроизвести один звук и завершить работу\n  --preview-mouse    Воспроизвести клик мыши и завершить работу\n  --version          Версия\n"
                );
                return;
            }
            _ => {
                eprintln!("Неизвестный аргумент: {}", argument.to_string_lossy());
                std::process::exit(2);
            }
        }
    }
    let result = if check {
        audio::check(&root).map(|()| {
            println!("OK: 240 WAV decoded, 40 keyboard profiles, 15 mouse pairs; no input monitoring started")
        })
    } else if preview || preview_mouse {
        let controls = Arc::new(audio::Controls::default());
        controls
            .mouse_profile
            .store(1, std::sync::atomic::Ordering::Relaxed);
        audio::Worker::start(&root, controls).map(|worker| {
            let _ = worker.sender.send(if preview_mouse {
                audio::Command::PreviewMouse
            } else {
                audio::Command::Preview
            });
            std::thread::sleep(Duration::from_secs(1));
        })
    } else {
        ui::run(&root, smoke_test)
    };
    if let Err(error) = result {
        eprintln!("Soundy: {error}");
        if !check && !preview && !preview_mouse {
            ui::show_error(&error);
        }
        std::process::exit(1);
    }
}
