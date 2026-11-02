//! Windows shell: tray icon, its native menu, and the message loop the input
//! hooks and tray callbacks need.

use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    sync::{Arc, atomic::Ordering},
};

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMenuItemCount, GetMessageW,
    GetSubMenu, GetWindowLongPtrW, HICON, HMENU, IDI_APPLICATION, IMAGE_ICON, LR_LOADFROMFILE,
    LoadIconW, LoadImageW, MB_ICONERROR, MB_OK, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, MSG, MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW, SW_SHOWNORMAL,
    SetForegroundWindow, SetWindowLongPtrW, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP, WM_NULL,
    WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{PCWSTR, w};

use crate::{
    core::{
        audio::{Controls, Worker},
        catalog::{MouseProfile, Profile},
        i18n::{Language, Text},
        input::InputHook,
        login,
        settings::Config,
    },
    view_model::{self, ViewModel},
};

#[path = "ui_windows_settings.rs"]
mod settings;

const TRAY_MESSAGE: u32 = WM_APP + 1;
const TRAY_ID: u32 = 1;
const WM_POWERBROADCAST: u32 = 0x0218;
const PBT_APMRESUMEAUTOMATIC: u32 = 0x0012;
const PBT_APMRESUMESUSPEND: u32 = 0x0007;
static MENU_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const MENU_KEYBOARD: u32 = 100;
const MENU_KEYBOARD_LAST: u32 = MENU_KEYBOARD + 39;
const MENU_MOUSE: u32 = 200;
const MENU_MOUSE_LAST: u32 = MENU_MOUSE + 15;
const MENU_VOLUME: u32 = 300;
const VOLUMES: [u8; 6] = [0, 10, 25, 50, 75, 100];
const MENU_LANGUAGE: u32 = 400;
const MENU_LANGUAGE_LAST: u32 = MENU_LANGUAGE + 2;
const MENU_OPEN_SETTINGS: u32 = 600;
const MENU_LOGIN: u32 = 500;
const MENU_GITHUB: u32 = 501;
const MENU_TELEGRAM: u32 = 502;
const MENU_QUIT: u32 = 503;

/// Top-level tray menu entries: Open, five groups, separators, links and Quit.
const MENU_ITEMS: i32 = 15;

pub(crate) struct App {
    hwnd: HWND,
    pub(crate) view_model: ViewModel,
    hook: Option<InputHook>,
    tray: Tray,
}

impl App {
    fn preview(&self) {
        self.view_model.play_preview();
    }

    fn preview_mouse(&self) {
        self.view_model.play_mouse_preview();
    }

    fn activate(&mut self, id: u32) {
        match id {
            _ if (MENU_KEYBOARD..=MENU_KEYBOARD_LAST).contains(&id) => {
                let profile = Profile::from_index((id - MENU_KEYBOARD) as usize);
                self.view_model.select_profile(profile);
                self.preview();
            }
            _ if (MENU_MOUSE..=MENU_MOUSE_LAST).contains(&id) => {
                let profile = MouseProfile::from_index((id - MENU_MOUSE) as usize);
                self.view_model.select_mouse(profile);
                self.preview_mouse();
            }
            _ if (MENU_VOLUME..MENU_VOLUME + VOLUMES.len() as u32).contains(&id) => {
                let volume = VOLUMES[(id - MENU_VOLUME) as usize];
                self.view_model.set_volume(volume);
                self.preview();
            }
            _ if (MENU_LANGUAGE..=MENU_LANGUAGE_LAST).contains(&id) => {
                let language = Language::ALL[(id - MENU_LANGUAGE) as usize];
                self.view_model.select_language(language);
                self.tray.set_tip(language.text(Text::Tooltip));
            }
            MENU_LOGIN => {
                let language = self.view_model.language();
                if let Err(error) = self.view_model.toggle_login() {
                    show_error(&error, language);
                }
            }
            MENU_OPEN_SETTINGS => {
                settings::show(self.hwnd, &self.view_model);
            }
            MENU_GITHUB => open_url(self.view_model.github_url()),
            MENU_TELEGRAM => open_url(self.view_model.telegram_url()),
            MENU_QUIT => unsafe {
                let _ = DestroyWindow(self.hwnd);
            },
            _ => {}
        }
    }

    fn show_menu(&mut self) {
        // A right click can arrive as both WM_RBUTTONUP and WM_CONTEXTMENU, and a
        // second menu would open inside the first one's loop.
        if MENU_OPEN.swap(true, Ordering::Relaxed) {
            return;
        }
        let menu = self.build_menu();
        let mut point = POINT::default();
        // SAFETY: `point` is a valid out pointer and the menu is owned here.
        let command = unsafe {
            let _ = GetCursorPos(&mut point);
            let _ = SetForegroundWindow(self.hwnd);
            let selected = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                None,
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);
            // Documented workaround: the tray menu needs a follow-up message to
            // dismiss itself reliably.
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            selected.0 as u32
        };
        MENU_OPEN.store(false, Ordering::Relaxed);
        if command != 0 {
            self.activate(command);
        }
    }

    fn build_menu(&self) -> HMENU {
        let language = self.view_model.language();
        // SAFETY: Every handle created here is owned by this function's result and
        // destroyed by `show_menu`; labels live until the append call returns.
        unsafe {
            let menu = CreatePopupMenu().expect("tray menu");
            add_item(
                menu,
                MENU_OPEN_SETTINGS,
                language.text(Text::OpenSettings),
                false,
            );
            add_separator(menu);
            let keyboard = CreatePopupMenu().expect("keyboard submenu");
            for (category, profiles) in self.view_model.keyboard_profiles_by_category() {
                if GetMenuItemCount(Some(keyboard)) > 0 {
                    add_separator(keyboard);
                }
                add_grayed(keyboard, language.text(category));
                for profile in profiles {
                    add_item(
                        keyboard,
                        MENU_KEYBOARD + u32::from(profile.0),
                        profile.name(),
                        profile.0 == self.view_model.profile().0,
                    );
                }
            }
            add_submenu(menu, keyboard, language.text(Text::KeyboardSound));

            let mouse = CreatePopupMenu().expect("mouse submenu");
            for profile in self.view_model.mouse_profiles() {
                add_item(
                    mouse,
                    MENU_MOUSE + u32::from(profile.0),
                    &language.mouse_name(profile.0),
                    profile.0 == self.view_model.mouse_profile().0,
                );
            }
            add_submenu(menu, mouse, language.text(Text::MouseSound));

            add_separator(menu);
            let volume = CreatePopupMenu().expect("volume submenu");
            for (index, value) in VOLUMES.into_iter().enumerate() {
                add_item(
                    volume,
                    MENU_VOLUME + index as u32,
                    &format!("{value}%"),
                    value == self.view_model.volume(),
                );
            }
            add_submenu(menu, volume, language.text(Text::Volume));

            let languages = CreatePopupMenu().expect("language submenu");
            for (index, option) in Language::ALL.into_iter().enumerate() {
                add_item(
                    languages,
                    MENU_LANGUAGE + index as u32,
                    option.name(),
                    option == language,
                );
            }
            add_submenu(menu, languages, language.text(Text::LanguageMenu));

            add_separator(menu);
            add_item(
                menu,
                MENU_LOGIN,
                language.text(Text::LaunchAtLogin),
                login::status() == login::ENABLED,
            );
            add_separator(menu);
            add_grayed(menu, &format!("Soundy {}", env!("CARGO_PKG_VERSION")));
            add_item(menu, MENU_GITHUB, "GitHub", false);
            add_item(menu, MENU_TELEGRAM, "Telegram", false);
            add_separator(menu);
            add_item(menu, MENU_QUIT, language.text(Text::Quit), false);
            menu
        }
    }

    fn check_menu(&self) {
        // SAFETY: The menu is built here and destroyed before returning.
        unsafe {
            let menu = self.build_menu();
            assert_eq!(GetMenuItemCount(Some(menu)), MENU_ITEMS, "tray menu shape");
            let submenu = |index: i32| GetSubMenu(menu, index);
            assert_eq!(GetMenuItemCount(Some(submenu(2))), 47, "keyboard groups");
            assert_eq!(GetMenuItemCount(Some(submenu(3))), 16, "mouse pairs");
            assert_eq!(
                GetMenuItemCount(Some(submenu(5))),
                VOLUMES.len() as i32,
                "volume steps"
            );
            assert_eq!(GetMenuItemCount(Some(submenu(6))), 3, "languages");
            let language = self.view_model.language();
            let _ = DestroyMenu(menu);
            println!(
                "OK: tray menu in {:?}: 5 groups, {} volume steps, 3 languages; no audio or hooks started",
                language,
                VOLUMES.len()
            );
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.tray.remove();
    }
}

struct Tray {
    data: NOTIFYICONDATAW,
    added: bool,
}

impl Tray {
    fn new(hwnd: HWND) -> Self {
        let icon = load_icon();
        let data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: TRAY_MESSAGE,
            hIcon: icon,
            ..Default::default()
        };
        // SAFETY: `data` is fully initialized and owned by this struct.
        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
        if !added {
            eprintln!("Soundy: the notification area rejected the tray icon");
        }
        Self { data, added }
    }

    fn set_tip(&mut self, text: &str) {
        let units = wide(text);
        let len = units.len().min(self.data.szTip.len() - 1);
        self.data.szTip.fill(0);
        self.data.szTip[..len].copy_from_slice(&units[..len]);
        if self.added {
            // SAFETY: Same initialized NOTIFYICONDATAW as NIM_ADD, now with a new tip.
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &self.data);
            };
        }
    }

    fn remove(&mut self) {
        if self.added {
            // SAFETY: The icon was added above and is removed exactly once.
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
            };
            self.added = false;
            if !self.data.hIcon.is_invalid() {
                // SAFETY: `hIcon` was returned by LoadImageW/LoadIconW for this process.
                unsafe {
                    let _ = DestroyIcon(self.data.hIcon);
                }
            }
        }
    }
}

fn load_icon() -> HICON {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("Soundy.ico");
            if path.is_file() {
                let icon_path = wide(&path);
                // SAFETY: NUL-terminated path to an icon file shipped with the app.
                if let Ok(image) = unsafe {
                    LoadImageW(
                        None,
                        PCWSTR(icon_path.as_ptr()),
                        IMAGE_ICON,
                        0,
                        0,
                        LR_LOADFROMFILE,
                    )
                } {
                    return HICON(image.0);
                }
            }
        }
    }
    // SAFETY: IDI_APPLICATION is a predefined system icon.
    unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default()
}

/// NUL-terminated UTF-16 for any Win32 wide-string parameter.
fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain([0]).collect()
}

fn add_item(menu: HMENU, id: u32, text: &str, checked: bool) {
    let mut flags = MF_STRING;
    if checked {
        flags |= MF_CHECKED;
    }
    let label = wide(text);
    // SAFETY: `menu` is a live popup menu and `label` outlives this call.
    let _ = unsafe { AppendMenuW(menu, flags, id as usize, PCWSTR(label.as_ptr())) };
}

fn add_grayed(menu: HMENU, text: &str) {
    let label = wide(text);
    // SAFETY: `menu` is a live popup menu and `label` outlives this call.
    let _ = unsafe { AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(label.as_ptr())) };
}

fn add_separator(menu: HMENU) {
    // SAFETY: `menu` is a live popup menu; separators take no text.
    let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };
}

fn add_submenu(menu: HMENU, submenu: HMENU, text: &str) {
    let label = wide(text);
    // SAFETY: `menu` is live and owns `submenu` from here on; `label` outlives the call.
    let _ = unsafe {
        AppendMenuW(
            menu,
            MF_POPUP | MF_STRING,
            submenu.0 as usize,
            PCWSTR(label.as_ptr()),
        )
    };
}

pub(crate) fn open_url(url: &str) {
    let url = wide(url);
    // SAFETY: Both strings are NUL-terminated and outlive this call.
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(url.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        TRAY_MESSAGE => {
            let event = lparam.0 as u32 & 0xffff;
            if matches!(event, WM_LBUTTONUP | WM_RBUTTONUP | WM_CONTEXTMENU) {
                // SAFETY: `run` stores the address of a live App and clears it on destroy.
                let app = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
                if !app.is_null() {
                    // SAFETY: Single-threaded message loop; `app` points at the live App.
                    unsafe { (*app).show_menu() };
                }
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            let event = wparam.0 as u32;
            if matches!(event, PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) {
                // SAFETY: `run` stores the address of a live App and clears it on destroy.
                let app = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
                if !app.is_null() {
                    // SAFETY: Single-threaded message loop; `app` points at the live App.
                    if let Some(sender) = unsafe { &*app }.view_model.audio_sender() {
                        let _ = sender.try_send(crate::core::audio::Command::ResetAudio);
                    }
                }
            }
            // SAFETY: Default handling for power messages this window does not own.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_DESTROY => {
            // SAFETY: Same window user data as above.
            let app = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            if !app.is_null() {
                // SAFETY: The App is still alive while the window is destroyed.
                unsafe { (*app).tray.remove() };
            }
            // SAFETY: Ends the message loop started by `run`.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => {
            // SAFETY: Default handling for every message this window does not own.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
    }
}

pub fn run(
    sounds: crate::core::audio::Sounds,
    smoke_test: bool,
    language: Language,
) -> Result<(), String> {
    // Two copies (one autostart entry plus a manual launch) would each install
    // hooks and play every sound twice.
    if !smoke_test && !claim_single_instance() {
        return Ok(());
    }
    let config = Config::load();
    let controls = Arc::new(Controls::default());
    // Re-apply the saved choice if the Run key lost the value (reinstall, cleanup tool).
    if config.launch_at_login && login::status() != login::ENABLED {
        let _ = login::set_enabled(true);
    }
    // The smoke test checks the tray menu only, so it must run without an audio
    // device (CI runners have none).
    let worker = if smoke_test {
        None
    } else {
        Some(Worker::start(sounds, controls.clone(), language)?)
    };
    let persistence = Box::new(view_model::windows::Persistence::new(config));
    let view_model = ViewModel::new(persistence, controls, worker, language, smoke_test);

    let instance = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
    let class_name = wide("SoundyTrayWindow");
    let window_class = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: HINSTANCE(instance.0),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    // SAFETY: The class name and window procedure stay valid for the process.
    unsafe { RegisterClassW(&window_class) };
    // SAFETY: The window is never shown; it only receives tray and quit messages.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            w!("Soundy"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|e| e.to_string())?;

    let mut app = App {
        hwnd,
        view_model,
        hook: None,
        tray: Tray::new(hwnd),
    };
    // SAFETY: `app` outlives every message dispatched below.
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut app as *mut App as isize) };
    app.tray
        .set_tip(app.view_model.language().text(Text::Tooltip));
    if smoke_test {
        app.check_menu();
        // SAFETY: Destroys the window created above and ends the loop after `run` returns.
        unsafe {
            let _ = DestroyWindow(hwnd);
        };
        return Ok(());
    }
    let sender = app.view_model.audio_sender();
    match sender.map(|sender| InputHook::start(sender, app.view_model.language())) {
        Some(Ok(hook)) => app.hook = Some(hook),
        Some(Err(error)) => {
            eprintln!("Soundy: {error}");
            app.tray
                .set_tip(app.view_model.language().text(Text::InputUnavailable));
        }
        None => {}
    }

    let mut message = MSG::default();
    loop {
        // SAFETY: Standard message pump for this thread.
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }
        // SAFETY: `message` was filled in by GetMessageW.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

static INSTANCE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

fn claim_single_instance() -> bool {
    // SAFETY: A named per-session mutex with no security attributes.
    let Ok(handle) = (unsafe { CreateMutexW(None, true, w!("Local\\dev.soundy.app")) }) else {
        return true;
    };
    // SAFETY: GetLastError is read immediately after CreateMutexW.
    let taken = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if taken {
        // SAFETY: The handle belongs to this process and is closed on exit.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(handle);
        };
        return false;
    }
    // Keep the mutex owned for the life of the process so a second copy sees it.
    INSTANCE.store(handle.0 as isize, Ordering::Relaxed);
    true
}

pub fn show_error(error: &str, language: Language) {
    let text = wide(error);
    let caption = wide(language.text(Text::StartFailed));
    // SAFETY: Both strings are NUL-terminated and outlive the call.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}
