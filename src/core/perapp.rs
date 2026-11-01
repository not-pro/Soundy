//! Per-application profile overrides: bundle ID → keyboard/mouse choice.
//!
//! Storage is one JSON string in the settings domain; a missing bundle ID means
//! "follow the global profiles", which keeps the override map and the global
//! settings independent. Keyboard `-1` is an explicit "no keyboard sound" for
//! that app, distinct from having no override at all.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Override {
    /// `-1` silences the keyboard for this app; `0..=39` picks a profile.
    pub keyboard: i16,
    /// `0` silences the mouse; `1..=15` picks a profile.
    pub mouse: u8,
}

pub type Map = HashMap<String, Override>;

/// The effective profiles for a frontmost app: `None` keyboard means silent.
pub fn resolve(map: &Map, bundle: Option<&str>, global_keyboard: u8, global_mouse: u8) -> (Option<u8>, u8) {
    match bundle.and_then(|bundle| map.get(bundle)) {
        Some(over) => (
            (over.keyboard >= 0).then_some(over.keyboard as u8),
            over.mouse,
        ),
        None => (Some(global_keyboard), global_mouse),
    }
}

pub fn save(map: &Map) -> String {
    serde_json::to_string(map).expect("override map serializes")
}

pub fn load(text: Option<&str>) -> Map {
    text.filter(|text| !text.is_empty())
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_bundle_and_unknown_bundle_follow_the_global_profiles() {
        let map = Map::new();
        assert_eq!(resolve(&map, None, 5, 3), (Some(5), 3));
        assert_eq!(resolve(&map, Some("com.example"), 5, 3), (Some(5), 3));
    }

    #[test]
    fn override_replaces_both_profiles_and_explicit_none_differs_from_absence() {
        let mut map = Map::new();
        map.insert(
            "com.chat".into(),
            Override {
                keyboard: -1,
                mouse: 2,
            },
        );
        // Explicit keyboard None silences keys but keeps the overridden mouse.
        assert_eq!(resolve(&map, Some("com.chat"), 5, 3), (None, 2));
        map.insert(
            "com.chat".into(),
            Override {
                keyboard: 7,
                mouse: 0,
            },
        );
        // The same bundle ID can instead pin a profile and silence the mouse.
        assert_eq!(resolve(&map, Some("com.chat"), 5, 3), (Some(7), 0));
    }

    #[test]
    fn map_round_trips_and_garbage_yields_empty() {
        let mut map = Map::new();
        map.insert("a".into(), Override { keyboard: -1, mouse: 0 });
        let restored = load(Some(&save(&map)));
        assert_eq!(restored, map);
        assert!(load(None).is_empty());
        assert!(load(Some("{oops")).is_empty());
    }
}
