#[cfg(target_os = "macos")]
pub mod audio;
#[cfg(windows)]
#[path = "audio_windows.rs"]
pub mod audio;
pub mod catalog;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // macOS-only feature set so far
pub mod hotkeys;
pub mod i18n;
#[cfg(target_os = "macos")]
pub mod input;
#[cfg(windows)]
#[path = "input_windows.rs"]
pub mod input;
#[cfg(target_os = "macos")]
pub mod login;
#[cfg(windows)]
#[path = "login_windows.rs"]
pub mod login;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // macOS-only feature set so far
pub mod pan;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // macOS-only feature set so far
pub mod perapp;
#[cfg(target_os = "macos")]
pub mod routes;
#[cfg(windows)]
pub mod settings;
#[cfg(target_os = "macos")]
pub mod update;
pub mod variation;
