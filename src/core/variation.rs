//! Per-press sound variation, recovered from KeyBell 3.1.4.
//!
//! The keyboard router stays deterministic (a physical key always picks the same
//! `keyN` sample), but the player turns one WAV into a few pitch/length variants
//! and picks one per press, never the variant it used last, plus a small random
//! level change. Reference points: loader 0x10005be20 (ratio table), buffer
//! picker 0x10005d030 (no immediate repeat), playback 0x10005c910 (level).

use std::time::{SystemTime, UNIX_EPOCH};

/// Pitch and length multipliers, in the order stored in the reference binary.
pub const RATIOS: [f32; 5] = [0.97, 0.99, 1.0, 1.01, 1.03];

/// Small linear congruential generator: variation must avoid a fixed pattern,
/// not resist an adversary.
pub struct Random(u64);

impl Random {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        Self(u64::from(nanos) | 1)
    }

    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    fn below(&mut self, count: u32) -> u32 {
        self.next() % count
    }

    /// Variant index in `0..count`, never `last` when another choice exists.
    /// The reference retries as `(index + 1 + random(count - 1)) % count`, which
    /// keeps every other variant equally likely instead of biasing the reroll.
    pub fn pick(&mut self, last: Option<u8>, count: u8) -> u8 {
        let mut index = self.below(u32::from(count)) as u8;
        if count > 1 && Some(index) == last {
            index = (index + 1 + self.below(u32::from(count - 1)) as u8) % count;
        }
        index
    }

    /// Level multiplier within ±1.5 dB, as the reference playback path does.
    pub fn level(&mut self) -> f32 {
        let unit = self.next() as f32 / u32::MAX as f32;
        10f32.powf((3.0 * unit - 1.5) / 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_never_repeat_and_stay_in_range() {
        let mut random = Random::new();
        let mut last = None;
        let mut seen = [0usize; 5];
        for _ in 0..500 {
            let variant = random.pick(last, RATIOS.len() as u8);
            assert!((variant as usize) < RATIOS.len());
            assert_ne!(Some(variant), last);
            seen[variant as usize] += 1;
            last = Some(variant);
        }
        assert!(seen.iter().all(|count| *count > 50), "{seen:?}");
    }

    #[test]
    fn a_single_variant_is_returned_and_the_level_stays_within_a_few_db() {
        let mut random = Random::new();
        assert_eq!(random.pick(Some(0), 1), 0);
        for _ in 0..1000 {
            let level = random.level();
            assert!((0.84..=1.2).contains(&level), "{level}");
        }
    }
}
