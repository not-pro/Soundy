use super::*;

#[test]
fn running_engine_accepts_new_profile_formats_and_schedules_playback() {
    autoreleasepool(|_| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds");
        let mut player = Player::new();
        player.start(Language::English).unwrap();
        for (keyboard, mouse) in [(0, 1), (1, 2), (15, 15), (39, 0)] {
            let profile = Profile(keyboard);
            let mouse = MouseProfile(mouse);
            player
                .select_profiles(&root, profile, mouse, Language::English, Instant::now())
                .unwrap();
            for index in [profile.sample_index(0), mouse.sample_index(false)]
                .into_iter()
                .flatten()
            {
                player.play(index, 1, None);
                let sample = player.samples[index].as_ref().unwrap();
                assert!(sample.last_play.is_some());
                assert!(sample.last_variant.is_some(), "every press picks a variant");
            }
            // Key-up only sounds for a profile that ships a release sample.
            for index in profile
                .release_index()
                .into_iter()
                .chain(mouse.sample_indices())
            {
                player.play(index, 25, None);
                assert!(player.samples[index].as_ref().unwrap().last_play.is_some());
            }
        }
    });
}

#[test]
fn profile_cache_loads_only_selected_sounds_reuses_and_expires_them() {
    autoreleasepool(|_| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds");
        let now = Instant::now();
        let mut player = Player::new();
        let cached = |player: &Player| player.samples.iter().flatten().count();
        assert_eq!(cached(&player), 0);
        let first_count =
            Profile(0).sample_indices().count() + MouseProfile(1).sample_indices().count();
        let second_count =
            Profile(1).sample_indices().count() + MouseProfile(2).sample_indices().count();
        player
            .select_profiles(&root, Profile(0), MouseProfile(1), Language::English, now)
            .unwrap();
        assert_eq!(cached(&player), first_count);
        assert!(first_count < catalog::files().count());
        let first = Profile(0).sample_indices().next().unwrap();
        let buffer = &*player.samples[first].as_ref().unwrap().buffer as *const AVAudioPCMBuffer;
        player
            .select_profiles(
                &root,
                Profile(1),
                MouseProfile(2),
                Language::English,
                now + Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(cached(&player), first_count + second_count);
        player
            .select_profiles(
                &root,
                Profile(0),
                MouseProfile(1),
                Language::English,
                now + Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(
            &*player.samples[first].as_ref().unwrap().buffer as *const AVAudioPCMBuffer,
            buffer
        );
        player
            .select_profiles(
                &root,
                Profile(1),
                MouseProfile(2),
                Language::English,
                now + Duration::from_secs(3),
            )
            .unwrap();
        player.evict_expired(now + Duration::from_secs(12));
        assert_eq!(cached(&player), first_count + second_count);
        // A scheduled tail must finish even after the inactive cache entry expires.
        let tail = player.samples[first].as_mut().unwrap();
        tail.last_play = Some(now + Duration::from_secs(13));
        tail.duration = Duration::from_secs(2);
        player.evict_expired(now + Duration::from_secs(14));
        assert_eq!(cached(&player), second_count + 1);
        player.evict_expired(now + Duration::from_secs(16));
        assert_eq!(cached(&player), second_count);
        assert!(player.samples[first].is_none());
        for (index, pool) in player.pools.iter().enumerate() {
            assert!(
                !pool.is_some()
                    || player
                        .samples
                        .iter()
                        .flatten()
                        .any(|sample| sample.pool == index)
            );
        }
        // Active sounds never expire, including while the user is idle.
        player.evict_expired(now + Duration::from_secs(3600));
        assert_eq!(cached(&player), second_count);
        player
            .select_profiles(
                &root,
                Profile(0),
                MouseProfile(0),
                Language::English,
                now + Duration::from_secs(3601),
            )
            .unwrap();
        player.evict_expired(now + Duration::from_secs(3612));
        assert_eq!(cached(&player), Profile(0).sample_indices().count());
        // The decoder needs only the selected profile, even if unrelated
        // sounds are missing or malformed. --check still validates all WAVs.
        let partial =
            std::env::temp_dir().join(format!("soundy-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&partial).unwrap();
        for index in Profile(0).sample_indices() {
            let name = catalog::files().nth(index).unwrap();
            std::fs::copy(root.join(name), partial.join(name)).unwrap();
        }
        let unrelated = catalog::files()
            .nth(Profile(1).sample_indices().next().unwrap())
            .unwrap();
        std::fs::write(partial.join(unrelated), b"not a WAV").unwrap();
        let mut partial_player = Player::new();
        partial_player
            .select_profiles(
                &partial,
                Profile(0),
                MouseProfile(0),
                Language::English,
                now,
            )
            .unwrap();
        assert_eq!(cached(&partial_player), Profile(0).sample_indices().count());
        assert!(check(&partial, Language::English).is_err());
        std::fs::remove_dir_all(partial).unwrap();
    });
}

#[test]
fn pitched_variant_buffers_are_released_once_the_voice_goes_idle() {
    autoreleasepool(|_| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds");
        let mut player = Player::new();
        let index = Profile(0).sample_index(0).unwrap();
        player
            .select_profiles(
                &root,
                Profile(0),
                MouseProfile(1),
                Language::English,
                Instant::now(),
            )
            .unwrap();
        // Force a pitched variant: the picker never repeats the last variant index,
        // and RATIOS[2] is 1.0, so any other index resamples the buffer.
        player.samples[index].as_mut().unwrap().last_variant = Some(2);
        player.play(index, 50, None);
        let with_buffer = player
            .pools
            .iter()
            .flatten()
            .flat_map(|pool| pool.voices.iter())
            .filter(|voice| voice.buffer.is_some())
            .count();
        assert_eq!(with_buffer, 1);
        player.evict_expired(Instant::now() + Duration::from_secs(5));
        assert!(
            player
                .pools
                .iter()
                .flatten()
                .flat_map(|pool| pool.voices.iter())
                .all(|voice| voice.buffer.is_none())
        );
        assert!(
            player.samples[index].is_some(),
            "an active profile keeps its cached sample"
        );
    });
}
