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
    let mut root = PathBuf::from("/Applications/KeyBell.app/Contents/Resources");
    let mut check = false;
    let mut preview = false;
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
            Some("--smoke-test") => smoke_test = true,
            Some("--version") => {
                println!("Soundy {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            Some("--help" | "-h") => {
                println!(
                    "Soundy — звуки клавиатуры для macOS\n\n  --sounds-dir PATH  Каталог WAV (по умолчанию KeyBell.app)\n  --check            Проверить 27 WAV без звука и мониторинга ввода\n  --preview          Воспроизвести один звук и завершить работу\n  --version          Версия\n"
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
            println!("OK: 27 WAV decoded, 3 keyboard profiles; no input monitoring started")
        })
    } else if preview {
        audio::Worker::start(&root, Arc::new(audio::Controls::default())).map(|worker| {
            let _ = worker.sender.send(audio::Command::Preview);
            std::thread::sleep(Duration::from_secs(1));
        })
    } else {
        ui::run(&root, smoke_test)
    };
    if let Err(error) = result {
        eprintln!("Soundy: {error}");
        if !check && !preview {
            ui::show_error(&error);
        }
        std::process::exit(1);
    }
}
