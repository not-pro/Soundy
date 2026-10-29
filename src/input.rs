use std::{cell::Cell, ffi::c_void, ptr::NonNull, sync::mpsc::SyncSender, time::Instant};

use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType, CGPreflightListenEventAccess,
};

use crate::audio::Command;

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
    pub fn start(sender: SyncSender<Command>) -> Result<Self, String> {
        if !CGPreflightListenEventAccess() {
            return Err("Требуется доступ к мониторингу ввода".into());
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
        .ok_or(
            "Не удалось подключить клавиатуру и мышь. Перезапустите Soundy после выдачи доступа.",
        )?;
        context.port.set(Some(NonNull::from(&*port)));
        let source = CFMachPort::new_run_loop_source(None, Some(&port), 0)
            .ok_or("Не удалось создать источник событий клавиатуры")?;
        let run_loop = CFRunLoop::main().ok_or("Основной run loop недоступен")?;
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
    } else if event_type == CGEventType::KeyDown || event_type == CGEventType::KeyUp {
        let event_ref = Some(unsafe { event.as_ref() });
        if CGEvent::integer_value_field(event_ref, CGEventField::KeyboardEventAutorepeat) == 0 {
            let keycode =
                CGEvent::integer_value_field(event_ref, CGEventField::KeyboardEventKeycode);
            if let Ok(keycode) = u16::try_from(keycode) {
                // A full queue drops a sound, never stalls keyboard input.
                let _ = context.sender.try_send(Command::Key {
                    keycode,
                    pressed: event_type == CGEventType::KeyDown,
                    at: Instant::now(),
                });
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
