//! Compact 240 × 240 settings window for Windows, mirroring the macOS layout.

use std::ffi::c_void;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_AUTOCHECKBOX, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    FindWindowW, GWLP_USERDATA, GetDlgItem, GetWindowLongPtrW, HMENU, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_HSCROLL, WS_CAPTION, WS_CHILD,
    WS_CLIPCHILDREN, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

use crate::{
    core::{
        catalog::{MouseProfile, Profile},
        i18n::{Language, Text},
    },
    view_model::ViewModel,
};

const CLASS_NAME: PCWSTR = w!("SoundySettingsWindow");
const WINDOW_WIDTH: i32 = 240;
const WINDOW_HEIGHT: i32 = 240;

const ID_ENABLED: i32 = 1001;
const ID_KEYBOARD: i32 = 1002;
const ID_MOUSE: i32 = 1003;
const ID_VOLUME: i32 = 1004;
const ID_VOLUME_LABEL: i32 = 1005;
const ID_LANG_RU: i32 = 1010;
const ID_LANG_EN: i32 = 1011;
const ID_LANG_CN: i32 = 1012;
const ID_LOGIN: i32 = 1020;
const ID_GITHUB: i32 = 1030;
const ID_TELEGRAM: i32 = 1031;
const ID_CLOSE: i32 = 1032;

const TBM_GETPOS: u32 = 0x0400 + 50;
const TBM_SETPOS: u32 = 0x0400 + 51;
const TBM_SETRANGE: u32 = 0x0400 + 55;
const TBS_NOTICKS: u32 = 0x0010;
const SW_SHOW: i32 = 5;

pub fn show(owner: HWND, view_model: &ViewModel) -> Option<HWND> {
    if let Ok(hwnd) = unsafe { FindWindowW(CLASS_NAME, PCWSTR::null()) } {
        if !hwnd.is_invalid() {
            unsafe {
                let _ = ShowWindow(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(SW_SHOW),
                );
                let _ = SetForegroundWindow(hwnd);
            }
            return Some(hwnd);
        }
    }

    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }.ok()?;
    let class_name = wide("SoundySettingsWindow");
    let window_class = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: HINSTANCE(instance.0),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassW(&window_class) };

    let title = wide(view_model.language().text(Text::OpenSettings));
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            Some(owner),
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .ok()?;

    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, owner.0 as isize);
        let _ = ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(SW_SHOW),
        );
        let _ = SetForegroundWindow(hwnd);
    }
    Some(hwnd)
}

#[allow(dead_code)]
pub fn close(hwnd: HWND) {
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CREATE => {
            create_controls(hwnd);
            sync_ui(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as i32) & 0xffff;
            let notify = ((wparam.0 >> 16) & 0xffff) as u16;
            if let Some(owner) = owner(hwnd) {
                if let Some(app) = app_from_owner(owner) {
                    handle_command(hwnd, app, id, notify);
                }
            }
            LRESULT(0)
        }
        WM_HSCROLL => {
            if let Some(owner) = owner(hwnd) {
                if let Some(app) = app_from_owner(owner) {
                    handle_volume(hwnd, app);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn create_controls(hwnd: HWND) {
    let instance = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map(|h| HINSTANCE(h.0))
            .unwrap_or_default()
    };

    create_button(
        hwnd,
        instance,
        "Enabled",
        ID_ENABLED,
        10,
        10,
        220,
        20,
        BS_AUTOCHECKBOX as u32,
    );

    create_label(hwnd, instance, "Keyboard:", 10, 38, 70, 16);
    create_combo(hwnd, instance, ID_KEYBOARD, 80, 35, 150, 120);

    create_label(hwnd, instance, "Mouse:", 10, 68, 70, 16);
    create_combo(hwnd, instance, ID_MOUSE, 80, 65, 150, 120);

    create_label(hwnd, instance, "Volume:", 10, 98, 50, 16);
    create_trackbar(hwnd, instance, ID_VOLUME, 65, 95, 130, 24);
    create_label_with_id(hwnd, instance, "25%", ID_VOLUME_LABEL, 200, 98, 35, 16);

    create_label(hwnd, instance, "Language:", 10, 128, 60, 16);
    create_button(
        hwnd,
        instance,
        "RU",
        ID_LANG_RU,
        75,
        125,
        45,
        22,
        BS_PUSHBUTTON as u32,
    );
    create_button(
        hwnd,
        instance,
        "EN",
        ID_LANG_EN,
        125,
        125,
        45,
        22,
        BS_PUSHBUTTON as u32,
    );
    create_button(
        hwnd,
        instance,
        "中文",
        ID_LANG_CN,
        175,
        125,
        45,
        22,
        BS_PUSHBUTTON as u32,
    );

    create_button(
        hwnd,
        instance,
        "Launch at login",
        ID_LOGIN,
        10,
        155,
        220,
        20,
        BS_AUTOCHECKBOX as u32,
    );

    create_button(
        hwnd,
        instance,
        "GitHub",
        ID_GITHUB,
        10,
        185,
        65,
        22,
        BS_PUSHBUTTON as u32,
    );
    create_button(
        hwnd,
        instance,
        "Telegram",
        ID_TELEGRAM,
        85,
        185,
        65,
        22,
        BS_PUSHBUTTON as u32,
    );
    create_button(
        hwnd,
        instance,
        "Close",
        ID_CLOSE,
        170,
        185,
        60,
        22,
        BS_PUSHBUTTON as u32,
    );
}

fn sync_ui(hwnd: HWND) {
    let Some(owner) = owner(hwnd) else { return };
    let Some(app) = app_from_owner(owner) else {
        return;
    };
    let view_model = &app.view_model;
    let language = view_model.language();

    set_checkbox(hwnd, ID_ENABLED, view_model.enabled());
    set_checkbox(hwnd, ID_LOGIN, view_model.launch_at_login());

    populate_combo(
        hwnd,
        ID_KEYBOARD,
        view_model
            .keyboard_profiles_by_category()
            .iter()
            .flat_map(|(_, p)| p)
            .copied(),
        |p| p.name().to_string(),
    );
    set_combo_selection(hwnd, ID_KEYBOARD, view_model.profile().0 as usize);

    populate_combo(
        hwnd,
        ID_MOUSE,
        view_model.mouse_profiles().into_iter(),
        |p| language.mouse_name(p.0),
    );
    set_combo_selection(hwnd, ID_MOUSE, view_model.mouse_profile().0 as usize);

    let volume = view_model.volume();
    unsafe {
        SendMessageW(
            dlg_item(hwnd, ID_VOLUME),
            TBM_SETRANGE,
            Some(WPARAM(0)),
            Some(LPARAM((100 << 16) as isize)),
        );
        SendMessageW(
            dlg_item(hwnd, ID_VOLUME),
            TBM_SETPOS,
            Some(WPARAM(1)),
            Some(LPARAM(volume as isize)),
        );
    }
    set_text(hwnd, ID_VOLUME_LABEL, &format!("{volume}%"));

    set_language_buttons(hwnd, language);
}

fn handle_command(hwnd: HWND, app: &mut super::App, id: i32, notify: u16) {
    match id {
        ID_ENABLED => {
            app.view_model.toggle_enabled();
        }
        ID_LOGIN => {
            let language = app.view_model.language();
            if let Err(error) = app.view_model.toggle_login() {
                super::show_error(&error, language);
            }
        }
        ID_KEYBOARD if notify == CBN_SELCHANGE as u16 => {
            let index = combo_selection(hwnd, ID_KEYBOARD);
            if let Some(profile) = Profile::all().nth(index) {
                app.view_model.select_profile(profile);
                app.view_model.play_preview();
            }
        }
        ID_MOUSE if notify == CBN_SELCHANGE as u16 => {
            let index = combo_selection(hwnd, ID_MOUSE);
            if let Some(profile) = MouseProfile::all().nth(index) {
                app.view_model.select_mouse(profile);
                app.view_model.play_mouse_preview();
            }
        }
        ID_LANG_RU => app.view_model.select_language(Language::Russian),
        ID_LANG_EN => app.view_model.select_language(Language::English),
        ID_LANG_CN => app.view_model.select_language(Language::Chinese),
        ID_GITHUB => super::open_url(app.view_model.github_url()),
        ID_TELEGRAM => super::open_url(app.view_model.telegram_url()),
        ID_CLOSE => {
            let _ = unsafe { DestroyWindow(hwnd) };
        }
        _ => {}
    }
    sync_ui(hwnd);
}

fn handle_volume(hwnd: HWND, app: &mut super::App) {
    let pos = unsafe {
        SendMessageW(
            dlg_item(hwnd, ID_VOLUME),
            TBM_GETPOS,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        )
    };
    app.view_model.set_volume(pos.0 as u8);
    app.view_model.play_preview();
    sync_ui(hwnd);
}

fn set_language_buttons(hwnd: HWND, language: Language) {
    for (id, option) in [
        (ID_LANG_RU, Language::Russian),
        (ID_LANG_EN, Language::English),
        (ID_LANG_CN, Language::Chinese),
    ] {
        let checked = language == option;
        unsafe {
            SendMessageW(
                dlg_item(hwnd, id),
                windows::Win32::UI::WindowsAndMessaging::BM_SETCHECK,
                Some(WPARAM(if checked { 1 } else { 0 })),
                Some(LPARAM(0)),
            );
        }
    }
}

fn owner(hwnd: HWND) -> Option<HWND> {
    let raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    if raw == 0 {
        None
    } else {
        Some(HWND(raw as *mut c_void))
    }
}

fn app_from_owner(owner: HWND) -> Option<&'static mut super::App> {
    let raw = unsafe { GetWindowLongPtrW(owner, GWLP_USERDATA) };
    if raw == 0 {
        None
    } else {
        Some(unsafe { &mut *(raw as *mut super::App) })
    }
}

fn create_label(hwnd: HWND, instance: HINSTANCE, text: &str, x: i32, y: i32, w: i32, h: i32) {
    let label = wide(text);
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            Some(hwnd),
            None,
            Some(instance),
            None,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn create_label_with_id(
    hwnd: HWND,
    instance: HINSTANCE,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let label = wide(text);
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            Some(hwnd),
            Some(HMENU(id as *mut c_void)),
            Some(instance),
            None,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn create_button(
    hwnd: HWND,
    instance: HINSTANCE,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: u32,
) {
    let label = wide(text);
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
            x,
            y,
            w,
            h,
            Some(hwnd),
            Some(HMENU(id as *mut c_void)),
            Some(instance),
            None,
        );
    }
}

fn create_combo(hwnd: HWND, instance: HINSTANCE, id: i32, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("COMBOBOX"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(CBS_DROPDOWNLIST as u32) | WS_VSCROLL,
            x,
            y,
            w,
            h,
            Some(hwnd),
            Some(HMENU(id as *mut c_void)),
            Some(instance),
            None,
        );
    }
}

fn create_trackbar(hwnd: HWND, instance: HINSTANCE, id: i32, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("msctls_trackbar32"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(TBS_NOTICKS),
            x,
            y,
            w,
            h,
            Some(hwnd),
            Some(HMENU(id as *mut c_void)),
            Some(instance),
            None,
        );
    }
}

fn set_checkbox(hwnd: HWND, id: i32, checked: bool) {
    unsafe {
        SendMessageW(
            dlg_item(hwnd, id),
            windows::Win32::UI::WindowsAndMessaging::BM_SETCHECK,
            Some(WPARAM(if checked { 1 } else { 0 })),
            Some(LPARAM(0)),
        );
    }
}

fn set_text(hwnd: HWND, id: i32, text: &str) {
    let wide = wide(text);
    unsafe {
        let _ = SetWindowTextW(dlg_item(hwnd, id), PCWSTR(wide.as_ptr()));
    }
}

fn populate_combo<I, T>(hwnd: HWND, id: i32, items: I, label: impl Fn(T) -> String)
where
    I: Iterator<Item = T>,
{
    let combo = dlg_item(hwnd, id);
    unsafe {
        SendMessageW(combo, CB_RESETCONTENT, Some(WPARAM(0)), Some(LPARAM(0)));
    }
    for item in items {
        let text = wide(label(item));
        unsafe {
            SendMessageW(
                combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }
    }
}

fn set_combo_selection(hwnd: HWND, id: i32, index: usize) {
    unsafe {
        SendMessageW(
            dlg_item(hwnd, id),
            CB_SETCURSEL,
            Some(WPARAM(index)),
            Some(LPARAM(0)),
        );
    }
}

fn combo_selection(hwnd: HWND, id: i32) -> usize {
    unsafe {
        SendMessageW(
            dlg_item(hwnd, id),
            CB_GETCURSEL,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        )
        .0 as usize
    }
}

fn dlg_item(hwnd: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(Some(hwnd), id).unwrap_or_default() }
}

fn wide(text: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.as_ref().encode_wide().chain([0]).collect()
}
