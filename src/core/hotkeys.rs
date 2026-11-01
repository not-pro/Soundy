//! Global hotkeys for power, volume, and the per-app toggle.
//!
//! A shortcut is a physical keycode plus a normalized modifier mask (the four
//! standard modifiers only). KeyBell's exact default combos were not recovered,
//! so Soundy ships none and records them on demand; the recording and matching
//! rules here are the policy layer the input tap consults before any sound.

use serde::{Deserialize, Serialize};

pub const MOD_CONTROL: u16 = 1 << 0;
pub const MOD_OPTION: u16 = 1 << 1;
pub const MOD_SHIFT: u16 = 1 << 2;
pub const MOD_COMMAND: u16 = 1 << 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    pub keycode: u16,
    pub modifiers: u16,
}

impl Shortcut {
    pub const ESCAPE: u16 = 53;

    /// Stable string form for storage, e.g. "55+8" (keycode + normalized mask).
    pub fn encode(self) -> String {
        format!("{}+{}", self.keycode, self.modifiers)
    }

    pub fn decode(text: &str) -> Option<Self> {
        let (keycode, modifiers) = text.split_once('+')?;
        Some(Self {
            keycode: keycode.parse().ok()?,
            modifiers: modifiers.parse().ok()?,
        })
    }

    /// Human-readable combo such as "⌃⇧A"; used in menu titles.
    pub fn display(self, key_label: impl Fn(u16) -> String) -> String {
        let mut text = String::new();
        if self.modifiers & MOD_CONTROL != 0 {
            text.push('⌃');
        }
        if self.modifiers & MOD_OPTION != 0 {
            text.push('⌥');
        }
        if self.modifiers & MOD_SHIFT != 0 {
            text.push('⇧');
        }
        if self.modifiers & MOD_COMMAND != 0 {
            text.push('⌘');
        }
        text.push_str(&key_label(self.keycode));
        text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Power,
    VolumeUp,
    VolumeDown,
    ToggleApp,
}

impl Action {
    pub const ALL: [Self; 4] = [
        Self::Power,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::ToggleApp,
    ];

    pub fn slot(self) -> usize {
        self as usize
    }
}

/// Shared, mutable hotkey state: the configured shortcuts plus the action
/// currently waiting for its recording keystroke, if any.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// Index follows `Action::ALL`; `None` means the action has no shortcut.
    pub shortcuts: [Option<Shortcut>; 4],
    pub recording: Option<Action>,
}

impl State {
    /// Exact match on keycode and the four standard modifiers; the first
    /// configured action wins. Callers must normalize the modifier mask first —
    /// Caps Lock, Fn, and the numeric-pad flag never reach this comparison.
    pub fn matched_by(&self, keycode: u16, modifiers: u16) -> Option<Action> {
        if self.recording.is_some() {
            return None;
        }
        Action::ALL.into_iter().find(|action| {
            self.shortcuts[action.slot()].is_some_and(|shortcut| {
                shortcut.keycode == keycode && shortcut.modifiers == modifiers
            })
        })
    }

    pub fn save(&self) -> String {
        let encoded: Vec<Option<String>> = self
            .shortcuts
            .iter()
            .map(|shortcut| shortcut.map(Shortcut::encode))
            .collect();
        serde_json::to_string(&encoded).expect("hotkey state serializes")
    }

    pub fn load(text: Option<&str>) -> Self {
        let mut state = Self::default();
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            return state;
        };
        let Ok(decoded) = serde_json::from_str::<Vec<Option<String>>>(text) else {
            return state;
        };
        for (slot, value) in decoded.iter().take(4).enumerate() {
            state.shortcuts[slot] = value.as_deref().and_then(Shortcut::decode);
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(keycode: u16, modifiers: u16) -> Option<Shortcut> {
        Some(Shortcut { keycode, modifiers })
    }

    #[test]
    fn matching_ignores_everything_but_the_four_standard_modifiers() {
        let mut state = State::default();
        state.shortcuts[Action::Power.slot()] = shortcut(9, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(
            state.matched_by(9, MOD_CONTROL | MOD_SHIFT),
            Some(Action::Power)
        );
        // The caller normalizes the flags; the matcher compares exactly what it gets.
        assert_eq!(state.matched_by(9, MOD_CONTROL | MOD_SHIFT | 0x8000), None);
        assert_eq!(
            state.matched_by(9, MOD_CONTROL | MOD_SHIFT | 0x1000 | 0x2000 | 0x4000),
            None
        );
        assert_eq!(state.matched_by(10, MOD_CONTROL | MOD_SHIFT), None);
        assert_eq!(state.matched_by(9, MOD_CONTROL), None);
    }

    #[test]
    fn recording_suppresses_matching_and_esc_cancels_by_convention() {
        let mut state = State::default();
        state.shortcuts[Action::VolumeUp.slot()] = shortcut(24, MOD_OPTION);
        state.recording = Some(Action::VolumeDown);
        assert_eq!(state.matched_by(24, MOD_OPTION), None);
        state.recording = None;
        assert_eq!(state.matched_by(24, MOD_OPTION), Some(Action::VolumeUp));
    }

    #[test]
    fn state_round_trips_through_storage_and_survives_garbage() {
        let mut state = State::default();
        state.shortcuts[Action::Power.slot()] = shortcut(55, MOD_COMMAND);
        state.shortcuts[Action::ToggleApp.slot()] = shortcut(53, 0);
        let restored = State::load(Some(&state.save()));
        assert_eq!(restored, state);
        assert_eq!(State::load(None), State::default());
        assert_eq!(State::load(Some("not json")), State::default());
        assert_eq!(
            State::load(Some("[\"7+3\",null]")).shortcuts[0]
                .unwrap()
                .keycode,
            7
        );
    }
}
