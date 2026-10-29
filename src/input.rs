use std::{cell::Cell, ffi::c_void, ptr::NonNull, sync::mpsc::SyncSender, time::Instant};

use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType, CGPreflightListenEventAccess,
};

use crate::{
    audio::Command,
    i18n::{Language, Text},
};

struct Context {
    sender: SyncSender<Command>,
    port: Cell<Option<NonNull<CFMachPort>>>,
}

pub struct InputTap {
    port: CFRetained<CFMachPort>,
    source: CFRetained<CFRunLoopSource>,
    // Its stable address remains valid until the port is invalidated in Drop.
    _context: Box<Context>,
}

impl InputTap {
    pub fn start(sender: SyncSender<Command>, language: Language) -> Result<Self, String> {
        if !CGPreflightListenEventAccess() {
            return Err(language.text(Text::InputPermissionRequired).into());
        }
        let mut context = Box::new(Context {
            sender,
            port: Cell::new(None),
        });
        // SAFETY: Listen-only tap never changes input. Box is retained for the tap's lifetime;
        // callbacks and teardown both run on the main run loop, without concurrent access.
        let port = unsafe {
            CGEvent::tap_create(
                CGEventTapLocation::SessionEventTap,
                CGEventTapPlacement::TailAppendEventTap,
                CGEventTapOptions::ListenOnly,
                [
                    CGEventType::KeyDown,
                    CGEventType::KeyUp,
                    CGEventType::FlagsChanged,
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                    CGEventType::OtherMouseDown,
                    CGEventType::OtherMouseUp,
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
    } else if matches!(
        event_type,
        CGEventType::KeyDown | CGEventType::KeyUp | CGEventType::FlagsChanged
    ) {
        let event_ref = Some(unsafe { event.as_ref() });
        if CGEvent::integer_value_field(event_ref, CGEventField::KeyboardEventAutorepeat) == 0 {
            let keycode =
                CGEvent::integer_value_field(event_ref, CGEventField::KeyboardEventKeycode);
            if let Ok(keycode) = u16::try_from(keycode) {
                if let Some(pressed) = key_pressed(event_type, keycode, CGEvent::flags(event_ref)) {
                    // A full queue drops a sound, never stalls keyboard input.
                    let _ = context.sender.try_send(Command::Key {
                        keycode,
                        pressed,
                        at: Instant::now(),
                    });
                }
            }
        }
    } else if let Some(pressed) = mouse_pressed(event_type) {
        let _ = context.sender.try_send(Command::Mouse {
            pressed,
            at: Instant::now(),
        });
    }
    event.as_ptr()
}

fn key_pressed(kind: CGEventType, keycode: u16, flags: CGEventFlags) -> Option<bool> {
    match kind {
        CGEventType::KeyDown => Some(true),
        CGEventType::KeyUp => Some(false),
        // Caps Lock reports a toggle, not a physical release. Sound on either toggle.
        CGEventType::FlagsChanged if keycode == 57 => Some(true),
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
                _ => return None,
            };
            Some(flags.bits() & mask != 0)
        }
        _ => None,
    }
}

fn mouse_pressed(kind: CGEventType) -> Option<bool> {
    match kind {
        CGEventType::LeftMouseDown | CGEventType::RightMouseDown | CGEventType::OtherMouseDown => {
            Some(true)
        }
        CGEventType::LeftMouseUp | CGEventType::RightMouseUp | CGEventType::OtherMouseUp => {
            Some(false)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_events_distinguish_press_release_and_both_sides() {
        for (left, right, left_mask, right_mask, aggregate) in [
            (55, 54, 0x08, 0x10, CGEventFlags::MaskCommand),
            (59, 62, 0x01, 0x2000, CGEventFlags::MaskControl),
            (58, 61, 0x20, 0x40, CGEventFlags::MaskAlternate),
            (56, 60, 0x02, 0x04, CGEventFlags::MaskShift),
        ] {
            let flags = |bits| CGEventFlags::from_bits_retain(aggregate.bits() | bits);
            for (keycode, bits, pressed) in [
                (left, left_mask, true),
                (right, left_mask | right_mask, true),
                (left, right_mask, false),
                (right, 0, false),
                (right, right_mask, true),
                (left, left_mask | right_mask, true),
                (right, left_mask, false),
                (left, 0, false),
            ] {
                assert_eq!(
                    key_pressed(CGEventType::FlagsChanged, keycode, flags(bits)),
                    Some(pressed)
                );
            }
        }
        for flags in [CGEventFlags::MaskAlphaShift, CGEventFlags::empty()] {
            assert_eq!(
                key_pressed(CGEventType::FlagsChanged, 57, flags),
                Some(true)
            );
        }
        assert_eq!(
            key_pressed(CGEventType::FlagsChanged, 63, CGEventFlags::MaskSecondaryFn),
            Some(true)
        );
        assert_eq!(
            key_pressed(CGEventType::FlagsChanged, 63, CGEventFlags::empty()),
            Some(false)
        );
        assert_eq!(
            key_pressed(CGEventType::FlagsChanged, 0, CGEventFlags::empty()),
            None
        );
        assert_eq!(
            key_pressed(CGEventType::KeyDown, 0, CGEventFlags::empty()),
            Some(true)
        );
        assert_eq!(
            key_pressed(CGEventType::KeyUp, 0, CGEventFlags::empty()),
            Some(false)
        );
        assert_eq!(
            key_pressed(CGEventType::MouseMoved, 0, CGEventFlags::empty()),
            None
        );
        for profile in crate::catalog::Profile::all() {
            for keycode in 54..=63 {
                assert!(profile.sample_index(keycode, true).is_some());
            }
        }
    }

    #[test]
    fn mouse_buttons_sound_on_press_and_release_only() {
        for kind in [
            CGEventType::LeftMouseDown,
            CGEventType::RightMouseDown,
            CGEventType::OtherMouseDown,
        ] {
            assert_eq!(mouse_pressed(kind), Some(true));
        }
        for kind in [
            CGEventType::LeftMouseUp,
            CGEventType::RightMouseUp,
            CGEventType::OtherMouseUp,
        ] {
            assert_eq!(mouse_pressed(kind), Some(false));
        }
        for kind in [
            CGEventType::KeyDown,
            CGEventType::MouseMoved,
            CGEventType::ScrollWheel,
            CGEventType::LeftMouseDragged,
        ] {
            assert_eq!(mouse_pressed(kind), None);
        }
    }
}
