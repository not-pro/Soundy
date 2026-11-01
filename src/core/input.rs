use std::{
    cell::Cell,
    ffi::c_void,
    ptr::NonNull,
    sync::{Arc, Mutex, mpsc::SyncSender},
    time::Instant,
};

use objc2::{msg_send, runtime::AnyObject};
use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType, CGPreflightListenEventAccess,
};
use objc2_foundation::NSString;

use crate::core::{
    audio::Command,
    hotkeys::{self, State as HotkeyState},
    i18n::{Language, Text},
};

// CGEventFlags bits for the four standard modifiers (NX_*MASK values).
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

struct Context {
    sender: SyncSender<Command>,
    hotkeys: Arc<Mutex<HotkeyState>>,
    // The UI delegate handles hotkey actions and recording completion. It
    // outlives this tap: teardown happens in the delegate's terminate path on
    // the same main run loop that delivers these callbacks.
    delegate: *const AnyObject,
    port: Cell<Option<NonNull<CFMachPort>>>,
    /// A KeyDown consumed by a hotkey, waiting for its KeyUp so a release
    /// sample never answers a shortcut.
    hotkey_armed: Cell<Option<u16>>,
}

pub struct InputTap {
    port: CFRetained<CFMachPort>,
    source: CFRetained<CFRunLoopSource>,
    // Its stable address remains valid until the port is invalidated in Drop.
    _context: Box<Context>,
}

impl InputTap {
    pub fn start(
        sender: SyncSender<Command>,
        language: Language,
        hotkeys: Arc<Mutex<HotkeyState>>,
        delegate: &AnyObject,
    ) -> Result<Self, String> {
        if !CGPreflightListenEventAccess() {
            return Err(language.text(Text::InputPermissionRequired).into());
        }
        let mut context = Box::new(Context {
            sender,
            hotkeys,
            delegate,
            port: Cell::new(None),
            hotkey_armed: Cell::new(None),
        });
        // SAFETY: Listen-only tap never changes input. Box is retained for the tap's lifetime;
        // callbacks and teardown both run on the main run loop, without concurrent access.
        let port = unsafe {
            CGEvent::tap_create(
                CGEventTapLocation::SessionEventTap,
                CGEventTapPlacement::TailAppendEventTap,
                CGEventTapOptions::ListenOnly,
                // Presses sound; key-up is only watched so profiles that ship a
                // release sample can answer it. Mouse-up never sounds.
                [
                    CGEventType::KeyDown,
                    CGEventType::KeyUp,
                    CGEventType::FlagsChanged,
                    CGEventType::LeftMouseDown,
                    CGEventType::RightMouseDown,
                    CGEventType::OtherMouseDown,
                ]
                .into_iter()
                .fold(0, |mask, kind| mask | (1 << kind.0)),
                Some(callback),
                (&mut *context as *mut Context).cast(),
            )
        }
        .ok_or(language.text(Text::InputConnectionFailed))?;
        context.port.set(Some(NonNull::from(&*port)));
        let source = CFMachPort::new_run_loop_source(None, Some(&port), 0)
            .ok_or(language.text(Text::InputSourceFailed))?;
        let run_loop = CFRunLoop::main().ok_or(language.text(Text::MainLoopUnavailable))?;
        // SAFETY: This is a system-owned immutable CoreFoundation mode constant.
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        CGEvent::tap_enable(&port, true);
        Ok(Self {
            port,
            source,
            _context: context,
        })
    }

    pub fn is_enabled(&self) -> bool {
        CGEvent::tap_is_enabled(&self.port)
    }
}

impl Drop for InputTap {
    fn drop(&mut self) {
        CGEvent::tap_enable(&self.port, false);
        if let Some(run_loop) = CFRunLoop::main() {
            run_loop.remove_source(Some(&self.source), unsafe { kCFRunLoopCommonModes });
        }
        self.source.invalidate();
        self.port.invalidate();
    }
}

unsafe extern "C-unwind" fn callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: user_info is the stable Context owned by InputTap; the event is borrowed
    // for this callback only. No references or text escape the callback.
    let context = unsafe { &*user_info.cast::<Context>() };
    if event_type == CGEventType::TapDisabledByTimeout {
        if CGPreflightListenEventAccess() {
            if let Some(port) = context.port.get() {
                CGEvent::tap_enable(unsafe { port.as_ref() }, true);
            }
        }
    } else if matches!(event_type, CGEventType::KeyDown | CGEventType::FlagsChanged) {
        let event_ref = Some(unsafe { event.as_ref() });
        if CGEvent::integer_value_field(event_ref, CGEventField::KeyboardEventAutorepeat) == 0 {
            let keycode =
                CGEvent::integer_value_field(event_ref, CGEventField::KeyboardEventKeycode);
            if let Ok(keycode) = u16::try_from(keycode) {
                if event_type == CGEventType::KeyDown && handle_hotkey(context, keycode, event_ref)
                {
                    return event.as_ptr();
                }
                if key_pressed(event_type, keycode, CGEvent::flags(event_ref)) {
                    // A full queue drops a sound, never stalls keyboard input.
                    let _ = context.sender.try_send(Command::Key {
                        keycode,
                        at: Instant::now(),
                    });
                }
            }
        }
    } else if event_type == CGEventType::KeyUp {
        let event_ref = Some(unsafe { event.as_ref() });
        let keycode = CGEvent::integer_value_field(event_ref, CGEventField::KeyboardEventKeycode);
        // Swallow the release of a key that triggered a hotkey, so a release
        // sample never answers a shortcut.
        let consumed = u16::try_from(keycode)
            .ok()
            .filter(|code| context.hotkey_armed.get() == Some(*code))
            .map(|_| context.hotkey_armed.take());
        if consumed.is_none() {
            let _ = context
                .sender
                .try_send(Command::KeyUp { at: Instant::now() });
        }
    } else if let Some(right) = mouse_clicked(event_type) {
        let _ = context.sender.try_send(Command::Mouse {
            right,
            at: Instant::now(),
        });
    }
    event.as_ptr()
}

/// Normalized four-standard-modifier mask for the hotkey matcher.
fn normalized_modifiers(flags: CGEventFlags) -> u16 {
    let bits = flags.bits();
    u16::from(bits & FLAG_CONTROL != 0)
        | (u16::from(bits & FLAG_OPTION != 0) << 1)
        | (u16::from(bits & FLAG_SHIFT != 0) << 2)
        | (u16::from(bits & FLAG_COMMAND != 0) << 3)
}

/// Handles a KeyDown against the hotkey table and recording state. Returns
/// true when the event was consumed by a shortcut or by recording, so no key
/// sound (and no further hotkey processing) happens for it.
fn handle_hotkey(context: &Context, keycode: u16, event: Option<&CGEvent>) -> bool {
    let modifiers = normalized_modifiers(CGEvent::flags(event));
    let mut state = context.hotkeys.lock().expect("hotkey state not poisoned");
    if let Some(action) = state.recording {
        state.recording = None;
        // Escape with no modifiers cancels instead of recording.
        let combo = (keycode != hotkeys::Shortcut::ESCAPE || modifiers != 0)
            .then(|| NSString::from_str(&hotkeys::Shortcut { keycode, modifiers }.encode()));
        drop(state);
        // SAFETY: the delegate outlives the tap (see Context); the callback runs
        // on the main run loop, the delegate's own thread.
        unsafe {
            let _: () = msg_send![
                context.delegate,
                recordShortcut: combo.as_deref(),
                forAction: action.slot() as isize
            ];
        }
        return true;
    }
    let matched = state.matched_by(keycode, modifiers);
    drop(state);
    if let Some(action) = matched {
        context.hotkey_armed.set(Some(keycode));
        // SAFETY: same lifetime and threading guarantees as recording above.
        unsafe {
            let _: () = msg_send![context.delegate, handleHotkey: action.slot() as isize];
        }
        return true;
    }
    false
}

fn key_pressed(kind: CGEventType, keycode: u16, flags: CGEventFlags) -> bool {
    match kind {
        CGEventType::KeyDown => true,
        // Caps Lock reports a toggle, not a physical release. Sound on either toggle.
        CGEventType::FlagsChanged if keycode == 57 => true,
        CGEventType::FlagsChanged => {
            // Device-specific masks from IOKit/hidsystem/IOLLEvent.h distinguish
            // left/right modifiers even while the opposite key is still held.
            let mask = match keycode {
                54 => 0x10,       // Right Command
                55 => 0x08,       // Left Command
                56 => 0x02,       // Left Shift
                58 => 0x20,       // Left Option
                59 => 0x01,       // Left Control
                60 => 0x04,       // Right Shift
                61 => 0x40,       // Right Option
                62 => 0x2000,     // Right Control
                63 => 0x00800000, // Fn
                _ => return false,
            };
            flags.bits() & mask != 0
        }
        _ => false,
    }
}

/// The profile ships one click per button branch, so the right button is told
/// apart from every other button; the primary and the extra buttons share `_1`.
fn mouse_clicked(kind: CGEventType) -> Option<bool> {
    match kind {
        CGEventType::RightMouseDown => Some(true),
        CGEventType::LeftMouseDown | CGEventType::OtherMouseDown => Some(false),
        _ => None,
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
