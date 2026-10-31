use super::*;
use std::collections::HashSet;

#[test]
fn catalog_covers_every_packaged_sound_and_preserves_routes() {
    let names: Vec<_> = files().collect();
    assert_eq!(names.len(), 240);
    assert_eq!(names.iter().copied().collect::<HashSet<_>>().len(), 240);
    for (category, count) in [
        (Text::Mechanical, 14),
        (Text::Electronic, 6),
        (Text::Fun, 11),
        (Text::Typewriters, 9),
    ] {
        assert_eq!(
            Profile::all()
                .filter(|profile| profile.category() == category)
                .count(),
            count
        );
    }
    validate(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds"),
        Language::English,
    )
    .unwrap();
    let mut used = HashSet::new();
    for profile in Profile::all() {
        for keycode in 0..128 {
            if let Some(index) = profile.sample_index(keycode) {
                used.insert(index);
            }
        }
    }
    for profile in MouseProfile::all() {
        if let Some(index) = profile.sample_index() {
            used.insert(index);
        }
    }
    // Key-up and mouse-up are not observed, so the release sample and the second
    // mouse variant of every pair are packaged but never routed.
    let unreachable = |name: &str| {
        name == "t_bell_release.wav" || (name.starts_with("mouse_") && name.ends_with("_2.wav"))
    };
    for (index, name) in names.iter().enumerate() {
        assert_eq!(
            used.contains(&index),
            !unreachable(name),
            "{name} routing changed"
        );
    }
    for profile in [Profile(0), Profile(1), Profile(2)] {
        let prefix = KEYBOARDS[profile.0 as usize].1;
        for (keycode, role) in [
            (49, "space"),
            (36, "key5"),
            (51, "key4"),
            (0, "key1"),
            (8, "key1"),
        ] {
            assert_eq!(
                names[profile.sample_index(keycode).unwrap()],
                format!("{prefix}_{role}.wav")
            );
        }
    }
    assert_eq!(MouseProfile(0).sample_index(), None);
    assert_eq!(
        names[MouseProfile(1).sample_index().unwrap()],
        "mouse_1_1.wav"
    );
    assert_eq!(
        names[MouseProfile(15).sample_index().unwrap()],
        "mouse_c_1.wav"
    );
    assert_eq!(Profile::from_index(255), Profile(0));
    assert_eq!(MouseProfile::from_index(255), MouseProfile(0));
}
