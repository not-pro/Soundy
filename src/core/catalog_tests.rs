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
        if let Some(index) = profile.release_index() {
            used.insert(index);
        }
    }
    for profile in MouseProfile::all() {
        for right in [false, true] {
            if let Some(index) = profile.sample_index(right) {
                used.insert(index);
            }
        }
    }
    for (index, name) in names.iter().enumerate() {
        assert!(used.contains(&index), "{name} is packaged but never routed");
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
    // A pair is one click per button branch: primary and extra buttons use `_1`,
    // the right button uses the longer `_2`.
    assert_eq!(MouseProfile(0).sample_index(false), None);
    assert_eq!(MouseProfile(0).sample_index(true), None);
    assert_eq!(
        names[MouseProfile(1).sample_index(false).unwrap()],
        "mouse_1_1.wav"
    );
    assert_eq!(
        names[MouseProfile(1).sample_index(true).unwrap()],
        "mouse_1_2.wav"
    );
    assert_eq!(
        names[MouseProfile(15).sample_index(false).unwrap()],
        "mouse_c_1.wav"
    );
    assert_eq!(
        names[MouseProfile(15).sample_index(true).unwrap()],
        "mouse_c_2.wav"
    );
    let bell = Profile::all()
        .find(|profile| profile.release_index().is_some())
        .expect("one profile ships a release sample");
    assert_eq!(names[bell.release_index().unwrap()], "t_bell_release.wav");
    assert_eq!(Profile(0).release_index(), None);
    assert!(
        Profile(0)
            .sample_indices()
            .all(|index| !names[index].ends_with("_release.wav"))
    );
    assert_eq!(Profile::from_index(255), Profile(0));
    assert_eq!(MouseProfile::from_index(255), MouseProfile(0));
}
