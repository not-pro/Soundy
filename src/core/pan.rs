//! Stereo Key Position: pans each key to its physical spot on an ANSI layout.
//!
//! KeyBell's exact pan formula was not recovered; this table maps macOS
//! keycodes to the horizontal center of their key in ANSI units (the main
//! block spans 0–15) and normalizes to [-1, 1]. Keys outside the table
//! (function row, modifiers, media keys) stay centered.

/// Horizontal key-center in ANSI units for keycodes with a physical position.
fn key_center(keycode: u16) -> Option<f64> {
    Some(match keycode {
        // Number row.
        50 => 0.5,
        18 => 1.5,
        19 => 2.5,
        20 => 3.5,
        21 => 4.5,
        23 => 5.5,
        22 => 6.5,
        26 => 7.5,
        28 => 8.5,
        25 => 9.5,
        29 => 10.5,
        27 => 11.5,
        24 => 12.5,
        51 => 14.0, // Delete sits in the Backspace slot.
        // Q row.
        12 => 2.0,
        13 => 3.0,
        14 => 4.0,
        15 => 5.0,
        17 => 6.0,
        16 => 7.0,
        32 => 8.0,
        34 => 9.0,
        31 => 10.0,
        35 => 11.0,
        33 => 12.0,
        30 => 13.0,
        42 => 14.25,
        // A row.
        0 => 2.25,
        1 => 3.25,
        2 => 4.25,
        3 => 5.25,
        5 => 6.25,
        4 => 7.25,
        38 => 8.25,
        40 => 9.25,
        37 => 10.25,
        39 => 11.25,
        41 => 12.25,
        // Z row.
        6 => 2.75,
        7 => 3.75,
        8 => 4.75,
        9 => 5.75,
        11 => 6.75,
        45 => 7.75,
        46 => 8.75,
        43 => 9.75,
        47 => 10.75,
        44 => 11.75,
        // Space is centered by construction; Return and the keypad Enter share
        // the right-edge slot.
        36 | 76 => 13.875,
        // The keypad block and the arrow cluster live on the far right.
        65..=92 => 14.5,
        123..=126 => 13.5,
        // Forward Delete and the navigation column hug the right edge.
        117 | 115 | 119 | 116 => 14.0,
        _ => return None,
    })
}

/// Pan for a keycode in `[-1, 1]`; unknown keys return `0.0` (centered).
pub fn key_pan(keycode: u16) -> f32 {
    const CENTER: f64 = 7.0;
    match key_center(keycode) {
        Some(x) => (((x - CENTER) / CENTER).clamp(-1.0, 1.0)) as f32,
        None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_tracks_the_physical_position_and_stays_in_range() {
        assert!(key_pan(0) < 0.0, "A sits left of center");
        assert!(key_pan(0) < key_pan(37), "L sits right of A");
        assert!(key_pan(12) < key_pan(35), "Q sits left of P");
        assert!(
            key_pan(42) > 0.0,
            "the backslash key sits at the right edge"
        );
        assert_eq!(key_pan(49), 0.0, "space is centered");
        assert_eq!(key_pan(999), 0.0, "unknown keys stay centered");
        for keycode in 0..128 {
            assert!((-1.0..=1.0).contains(&key_pan(keycode)), "{keycode}");
        }
    }
}
