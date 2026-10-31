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
    for profile in crate::core::catalog::Profile::all() {
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
