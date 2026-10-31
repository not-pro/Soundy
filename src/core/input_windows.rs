//! Windows input: two low-level hooks. They observe presses only, so one
//! physical action is exactly one sound.

use std::{
    sync::{Mutex, MutexGuard, mpsc::SyncSender},
    time::Instant,
};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN,
};

use crate::core::{
    audio::Command,
    i18n::{Language, Text},
};

static SENDER: Mutex<Option<SyncSender<Command>>> = Mutex::new(None);
static PRESSED: Mutex<Pressed> = Mutex::new(Pressed::new());

pub struct InputHook {
    keyboard: HHOOK,
    mouse: HHOOK,
}

impl InputHook {
    /// Must run on the thread that pumps messages, because that is where
    /// Windows delivers low-level hook callbacks.
    pub fn start(sender: SyncSender<Command>, language: Language) -> Result<Self, String> {
        *lock(&SENDER) = Some(sender);
        // SAFETY: Both hooks are process-wide listen-only callbacks with no user
        // data; the static sender outlives the hooks and the callbacks are fast.
        let keyboard =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0) }
                .map_err(|e| format!("{}: {e}", language.text(Text::InputConnectionFailed)))?;
        let mouse = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), None, 0) } {
            Ok(mouse) => mouse,
            Err(error) => {
                // SAFETY: Removing the hook installed just above inside this call.
                unsafe {
                    let _ = UnhookWindowsHookEx(keyboard);
                };
                return Err(format!(
                    "{}: {error}",
                    language.text(Text::InputConnectionFailed)
                ));
            }
        };
        Ok(Self { keyboard, mouse })
    }
}

impl Drop for InputHook {
    fn drop(&mut self) {
        // SAFETY: Both handles were returned by SetWindowsHookExW and are removed once.
        unsafe {
            let _ = UnhookWindowsHookEx(self.keyboard);
            let _ = UnhookWindowsHookEx(self.mouse);
        }
        *lock(&SENDER) = None;
    }
}

fn lock<T>(mutex: &'static Mutex<T>) -> MutexGuard<'static, T> {
    // A panicking callback must not disable sound for the rest of the session.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: For nCode >= 0 Windows passes a KBDLLHOOKSTRUCT pointer in lParam.
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = event.vkCode;
        match wparam.0 as u32 {
            // Ignore auto-repeat and input injected by other software: both would
            // play a sound the user did not physically make.
            WM_KEYDOWN | WM_SYSKEYDOWN if event.flags.0 & LLKHF_INJECTED == 0 => {
                if lock(&PRESSED).down(vk) {
                    send(Command::Key {
                        keycode: keycode(vk),
                        at: Instant::now(),
                    });
                }
            }
            WM_KEYUP | WM_SYSKEYUP => lock(&PRESSED).up(vk),
            _ => {}
        }
    }
    // SAFETY: Passing this callback's arguments through unchanged is the contract.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: For nCode >= 0 Windows passes an MSLLHOOKSTRUCT pointer in lParam.
        let event = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let injected = event.flags & LLMHF_INJECTED != 0;
        if !injected
            && matches!(
                wparam.0 as u32,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            )
        {
            send(Command::Mouse { at: Instant::now() });
        }
    }
    // SAFETY: Passing this callback's arguments through unchanged is the contract.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn send(command: Command) {
    if let Some(sender) = lock(&SENDER).as_ref() {
        // A full queue drops a sound, never stalls the input thread.
        let _ = sender.try_send(command);
    }
}

const LLKHF_INJECTED: u32 = 0x10;
const LLMHF_INJECTED: u32 = 0x01;

/// The catalog addresses sounds by the macOS key codes Soundy was built with:
/// space 49, return 36, delete 51/117. Every other key is routed by its own code.
fn keycode(vk: u32) -> u16 {
    match vk {
        0x20 => 49,  // VK_SPACE
        0x0D => 36,  // VK_RETURN
        0x08 => 51,  // VK_BACK
        0x2E => 117, // VK_DELETE
        // These virtual keys are numerically equal to the macOS space, return
        // (both 36 and 76), backspace and forward-delete codes, so they are moved
        // out of the way and keep an ordinary sample instead of a special one.
        // 0x24 VK_HOME, 0x31 '1', 0x4C 'L', 0x33 '3', 0x75 VK_F6.
        0x24 | 0x31 | 0x33 | 0x4C | 0x75 => 0x100 + vk as u16,
        _ => u16::try_from(vk).unwrap_or(0),
    }
}

/// Virtual-key press state, used to recognise auto-repeat (a key held down
/// repeats WM_KEYDOWN without an intervening WM_KEYUP).
struct Pressed([bool; 256]);

impl Pressed {
    const fn new() -> Self {
        Self([false; 256])
    }

    /// True only for the first press of a key, false while it is held.
    fn down(&mut self, vk: u32) -> bool {
        let Some(slot) = self.0.get_mut(vk as usize) else {
            return false;
        };
        !std::mem::replace(slot, true)
    }

    fn up(&mut self, vk: u32) {
        if let Some(slot) = self.0.get_mut(vk as usize) {
            *slot = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_repeat_is_ignored_until_the_key_is_released() {
        let mut pressed = Pressed::new();
        assert!(pressed.down(0x41));
        assert!(!pressed.down(0x41), "a held key must not play again");
        pressed.up(0x41);
        assert!(pressed.down(0x41), "a fresh press must play again");
        assert!(!pressed.down(0x200), "codes beyond 255 are out of range");
    }

    #[test]
    fn catalog_roles_keep_their_macos_key_codes() {
        assert_eq!(keycode(0x20), 49); // space
        assert_eq!(keycode(0x0D), 36); // return
        assert_eq!(keycode(0x08), 51); // backspace
        assert_eq!(keycode(0x2E), 117); // delete
        assert_eq!(keycode(0x41), 0x41);
    }

    #[test]
    fn keys_that_share_a_macos_code_do_not_borrow_its_sample() {
        // VK_HOME, '1', 'L', '3' and VK_F6 are 36, 49, 76, 51 and 117 in decimal.
        for vk in [0x24, 0x31, 0x4C, 0x33, 0x75] {
            let code = keycode(vk);
            assert!(![36, 49, 51, 76, 117].contains(&code), "{vk:#x} -> {code}");
        }
    }
}
