use std::{
    cell::{Cell, Ref, RefCell},
    sync::{Arc, Mutex, atomic::Ordering},
};

use crate::{
    consts,
    core::{
        audio::{Command, Controls, Worker},
        catalog::{MouseProfile, Profile},
        hotkeys::{self, Action as HotkeyAction, State as HotkeyState},
        i18n::{Language, Text},
        login, perapp,
    },
};

#[cfg(target_os = "macos")]
use crate::core::routes;

/// Platform-agnostic persistence backend for the ViewModel.
pub trait Persistence {
    fn profile(&self) -> u8;
    fn mouse_profile(&self) -> u8;
    fn volume(&self) -> u8;
    fn enabled(&self) -> Option<bool>;
    fn language(&self, fallback: Language) -> Language;
    fn launch_at_login(&self) -> bool;
    #[cfg(target_os = "macos")]
    fn stereo_pan(&self) -> Option<bool> {
        None
    }
    fn overrides(&self) -> perapp::Map {
        perapp::Map::new()
    }
    fn hotkeys(&self) -> HotkeyState {
        HotkeyState::default()
    }

    #[cfg(target_os = "macos")]
    fn route_mode_index(&self) -> u8 {
        0
    }

    #[cfg(target_os = "macos")]
    fn set_schema_version(&self, _version: isize) {}

    #[cfg(target_os = "macos")]
    fn remove_smoke_defaults(&self, _name: &objc2_foundation::NSString) {}

    fn save_profile(&self, profile: u8);
    fn save_mouse_profile(&self, profile: u8);
    fn save_volume(&self, volume: u8);
    fn save_enabled(&self, enabled: bool);
    fn save_language(&self, language: Language);
    fn save_launch_at_login(&self, enabled: bool);
    #[cfg(target_os = "macos")]
    fn save_stereo_pan(&self, _enabled: bool) {}
    fn save_overrides(&self, _overrides: &perapp::Map) {}
    fn save_hotkeys(&self, _hotkeys: &HotkeyState) {}

    #[cfg(target_os = "macos")]
    fn save_route_mode(&self, _mode: routes::Mode) {}
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InputStatus {
    Listening,
    PermissionDenied,
    Disconnected,
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginStatus {
    Disabled,
    Enabled,
    #[cfg(target_os = "macos")]
    RequiresApproval,
}

pub struct ViewModel {
    controls: Arc<Controls>,
    worker: RefCell<Option<Worker>>,
    persistence: RefCell<Box<dyn Persistence>>,
    language: Cell<Language>,
    global: Cell<(u8, u8)>,
    overrides: RefCell<perapp::Map>,
    frontmost: RefCell<Option<String>>,
    hotkeys: Arc<Mutex<HotkeyState>>,
    update_version: RefCell<Option<String>>,
    update_probe_finished: Cell<bool>,
    settings_open: Cell<bool>,
    about_open: Cell<bool>,
    enabled: Cell<bool>,
    volume: Cell<u8>,
    profile: Cell<Profile>,
    mouse_profile: Cell<MouseProfile>,
    input_status: RefCell<InputStatus>,
    smoke_test: bool,
    #[cfg(target_os = "macos")]
    stereo_pan: Cell<bool>,
    #[cfg(target_os = "macos")]
    route_mode: Cell<routes::Mode>,
    #[cfg(target_os = "macos")]
    output_class: Cell<routes::DeviceClass>,
}

#[allow(dead_code)]
impl ViewModel {
    pub fn new(
        persistence: Box<dyn Persistence>,
        controls: Arc<Controls>,
        worker: Option<Worker>,
        language: Language,
        smoke_test: bool,
    ) -> Self {
        let language = persistence.language(language);
        let profile = Profile::from_index(persistence.profile() as usize);
        let mouse_profile = MouseProfile::from_index(persistence.mouse_profile() as usize);
        let volume = persistence.volume().clamp(0, 100);
        let enabled = persistence.enabled().unwrap_or(true);
        #[cfg(target_os = "macos")]
        let launch_at_login = persistence.launch_at_login();

        controls.profile.store(profile.0, Ordering::Relaxed);
        controls
            .mouse_profile
            .store(mouse_profile.0, Ordering::Relaxed);
        controls.volume.store(volume, Ordering::Relaxed);
        #[cfg(target_os = "macos")]
        controls.enabled.store(enabled, Ordering::Relaxed);

        let global = (profile.0, mouse_profile.0);

        #[cfg(target_os = "macos")]
        let route_mode = routes::Mode::from_index(persistence.route_mode_index() as usize);
        #[cfg(target_os = "macos")]
        let output_class = routes::current_output_class();
        #[cfg(target_os = "macos")]
        controls
            .route_allows
            .store(route_mode.allows(output_class), Ordering::Relaxed);

        let overrides = persistence.overrides();
        let hotkeys = Arc::new(Mutex::new(persistence.hotkeys()));
        #[cfg(target_os = "macos")]
        let stereo_pan = persistence.stereo_pan().unwrap_or(false);
        #[cfg(target_os = "macos")]
        controls.stereo_pan.store(stereo_pan, Ordering::Relaxed);

        #[cfg(target_os = "macos")]
        if launch_at_login && login::status() == 0 {
            let _ = login::set_enabled(true);
        }

        Self {
            controls,
            worker: RefCell::new(worker),
            persistence: RefCell::new(persistence),
            language: Cell::new(language),
            global: Cell::new(global),
            overrides: RefCell::new(overrides),
            frontmost: RefCell::new(None),
            hotkeys,
            update_version: RefCell::new(None),
            update_probe_finished: Cell::new(false),
            settings_open: Cell::new(false),
            about_open: Cell::new(false),
            enabled: Cell::new(enabled),
            volume: Cell::new(volume),
            profile: Cell::new(profile),
            mouse_profile: Cell::new(mouse_profile),
            input_status: RefCell::new(InputStatus::Disconnected),
            smoke_test,
            #[cfg(target_os = "macos")]
            stereo_pan: Cell::new(stereo_pan),
            #[cfg(target_os = "macos")]
            route_mode: Cell::new(route_mode),
            #[cfg(target_os = "macos")]
            output_class: Cell::new(output_class),
        }
    }

    pub fn language(&self) -> Language {
        self.language.get()
    }

    pub fn profile(&self) -> Profile {
        self.profile.get()
    }

    pub fn mouse_profile(&self) -> MouseProfile {
        self.mouse_profile.get()
    }

    pub fn volume(&self) -> u8 {
        self.volume.get()
    }

    pub fn enabled(&self) -> bool {
        self.enabled.get()
    }

    #[cfg(target_os = "macos")]
    pub fn stereo_pan(&self) -> bool {
        self.stereo_pan.get()
    }

    #[cfg(target_os = "macos")]
    pub fn route_mode(&self) -> routes::Mode {
        self.route_mode.get()
    }

    #[cfg(target_os = "macos")]
    pub fn output_class(&self) -> routes::DeviceClass {
        self.output_class.get()
    }

    pub fn launch_at_login(&self) -> bool {
        self.persistence.borrow().launch_at_login()
    }

    pub fn login_status(&self) -> LoginStatus {
        match login::status() {
            login::ENABLED => LoginStatus::Enabled,
            #[cfg(target_os = "macos")]
            login::REQUIRES_APPROVAL => LoginStatus::RequiresApproval,
            _ => LoginStatus::Disabled,
        }
    }

    pub fn input_status(&self) -> InputStatus {
        self.input_status.borrow().clone()
    }

    pub fn set_input_status(&self, status: InputStatus) {
        *self.input_status.borrow_mut() = status;
    }

    pub fn update_version(&self) -> Option<String> {
        self.update_version.borrow().clone()
    }

    pub fn set_update_version(&self, version: Option<String>) {
        *self.update_version.borrow_mut() = version;
    }

    pub fn update_probe_finished(&self) -> bool {
        self.update_probe_finished.get()
    }

    pub fn set_update_probe_finished(&self) {
        self.update_probe_finished.set(true);
    }

    pub fn settings_open(&self) -> bool {
        self.settings_open.get()
    }

    pub fn about_open(&self) -> bool {
        self.about_open.get()
    }

    pub fn smoke_test(&self) -> bool {
        self.smoke_test
    }

    pub fn worker_active(&self) -> bool {
        self.worker.borrow().is_some()
    }

    pub fn worker_ref(&self) -> Ref<'_, Option<Worker>> {
        self.worker.borrow()
    }

    pub fn audio_sender(&self) -> Option<std::sync::mpsc::SyncSender<Command>> {
        self.worker
            .borrow()
            .as_ref()
            .map(|worker| worker.sender.clone())
    }

    pub fn persistence(&self) -> Ref<'_, Box<dyn Persistence>> {
        self.persistence.borrow()
    }

    pub fn hotkeys(&self) -> Arc<Mutex<HotkeyState>> {
        self.hotkeys.clone()
    }

    pub fn open_settings(&self) {
        self.settings_open.set(true);
    }

    pub fn close_settings(&self) {
        self.settings_open.set(false);
        self.about_open.set(false);
    }

    pub fn set_about_open(&self, open: bool) {
        self.about_open.set(open);
    }

    pub fn select_profile(&self, profile: Profile) {
        self.profile.set(profile);
        self.global.set((profile.0, self.global.get().1));
        self.persistence.borrow().save_profile(profile.0);
        self.apply_profiles();
    }

    pub fn select_mouse(&self, profile: MouseProfile) {
        self.mouse_profile.set(profile);
        self.global.set((self.global.get().0, profile.0));
        self.persistence.borrow().save_mouse_profile(profile.0);
        self.apply_profiles();
    }

    pub fn set_volume(&self, volume: u8) {
        let volume = volume.clamp(0, 100);
        self.volume.set(volume);
        self.controls.volume.store(volume, Ordering::Relaxed);
        self.persistence.borrow().save_volume(volume);
    }

    pub fn bump_volume(&self, delta: i16) {
        let current = self.volume.get() as i16;
        self.set_volume((current + delta).clamp(0, 100) as u8);
    }

    pub fn select_language(&self, language: Language) {
        self.language.set(language);
        self.persistence.borrow().save_language(language);
    }

    pub fn toggle_enabled(&self) {
        let enabled = !self.enabled.get();
        self.enabled.set(enabled);
        #[cfg(target_os = "macos")]
        self.controls.enabled.store(enabled, Ordering::Relaxed);
        self.persistence.borrow().save_enabled(enabled);
    }

    #[cfg(target_os = "macos")]
    pub fn toggle_stereo(&self) {
        let on = !self.stereo_pan.get();
        self.stereo_pan.set(on);
        self.controls.stereo_pan.store(on, Ordering::Relaxed);
        self.persistence.borrow().save_stereo_pan(on);
    }

    #[cfg(target_os = "macos")]
    pub fn set_route_mode(&self, mode: routes::Mode) {
        self.route_mode.set(mode);
        self.controls
            .route_allows
            .store(mode.allows(self.output_class.get()), Ordering::Relaxed);
        self.persistence.borrow().save_route_mode(mode);
    }

    #[cfg(target_os = "macos")]
    pub fn set_output_class(&self, class: routes::DeviceClass) {
        self.output_class.set(class);
        self.controls
            .route_allows
            .store(self.route_mode.get().allows(class), Ordering::Relaxed);
    }

    pub fn toggle_login(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        let enable = !matches!(
            self.login_status(),
            LoginStatus::Enabled | LoginStatus::RequiresApproval
        );
        #[cfg(windows)]
        let enable = !matches!(self.login_status(), LoginStatus::Enabled);
        let result = login::set_enabled(enable);
        self.persistence.borrow().save_launch_at_login(enable);
        #[cfg(target_os = "macos")]
        if enable && matches!(self.login_status(), LoginStatus::RequiresApproval) {
            login::open_settings();
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        let result = result.map_err(|error| error.localizedDescription().to_string());
        result?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn approve_login(&self) {
        login::open_settings();
    }

    pub fn set_frontmost_app(&self, bundle: Option<String>) {
        if bundle != *self.frontmost.borrow() {
            *self.frontmost.borrow_mut() = bundle;
            self.apply_profiles();
        }
    }

    pub fn frontmost_app(&self) -> Option<String> {
        self.frontmost.borrow().clone()
    }

    pub fn active_override(&self) -> Option<perapp::Override> {
        self.frontmost
            .borrow()
            .as_ref()
            .and_then(|bundle| self.overrides.borrow().get(bundle).copied())
    }

    pub fn remove_override(&self) {
        let Some(frontmost) = self.frontmost.borrow().clone() else {
            return;
        };
        self.overrides.borrow_mut().remove(&frontmost);
        self.save_overrides();
        self.apply_profiles();
    }

    pub fn set_override(&self, keyboard: Option<i16>, mouse: Option<u8>) {
        let Some(frontmost) = self.frontmost.borrow().clone() else {
            return;
        };
        let (global_keyboard, global_mouse) = self.global.get();
        let existing = self.overrides.borrow().get(&frontmost).copied();
        let entry = perapp::Override {
            keyboard: keyboard
                .unwrap_or_else(|| existing.map_or(global_keyboard as i16, |over| over.keyboard)),
            mouse: mouse.unwrap_or_else(|| existing.map_or(global_mouse, |over| over.mouse)),
        };
        self.overrides.borrow_mut().insert(frontmost, entry);
        self.save_overrides();
        self.apply_profiles();
    }

    pub fn toggle_app_sound(&self) {
        let Some(frontmost) = self.frontmost.borrow().clone() else {
            return;
        };
        let mut map = self.overrides.borrow_mut();
        if map.remove(&frontmost).is_none() {
            map.insert(
                frontmost,
                perapp::Override {
                    keyboard: -1,
                    mouse: 0,
                },
            );
        }
        drop(map);
        self.save_overrides();
        self.apply_profiles();
    }

    pub fn start_recording_hotkey(&self, action: HotkeyAction) {
        let mut state = self.hotkeys.lock().expect("hotkey state not poisoned");
        state.recording = if state.recording == Some(action) {
            None
        } else {
            Some(action)
        };
    }

    pub fn record_hotkey(&self, action: HotkeyAction, combo: Option<hotkeys::Shortcut>) {
        let mut state = self.hotkeys.lock().expect("hotkey state not poisoned");
        state.recording = None;
        if let Some(shortcut) = combo {
            for other in HotkeyAction::ALL {
                if other != action && state.shortcuts[other.slot()] == Some(shortcut) {
                    state.shortcuts[other.slot()] = None;
                }
            }
            state.shortcuts[action.slot()] = Some(shortcut);
        }
        let saved = state.save();
        drop(state);
        self.persistence
            .borrow()
            .save_hotkeys(&HotkeyState::load(Some(&saved)));
    }

    pub fn reset_hotkeys(&self) {
        let mut state = self.hotkeys.lock().expect("hotkey state not poisoned");
        state.shortcuts = [None; 4];
        state.recording = None;
        let saved = state.save();
        drop(state);
        self.persistence
            .borrow()
            .save_hotkeys(&HotkeyState::load(Some(&saved)));
    }

    pub fn handle_hotkey(&self, action: HotkeyAction) {
        match action {
            HotkeyAction::Power => self.toggle_enabled(),
            HotkeyAction::VolumeUp => self.bump_volume(10),
            HotkeyAction::VolumeDown => self.bump_volume(-10),
            HotkeyAction::ToggleApp => self.toggle_app_sound(),
        }
    }

    pub fn shortcut_title(
        &self,
        action: HotkeyAction,
        key_label: impl Fn(u16) -> String,
    ) -> String {
        let language = self.language.get();
        let state = self.hotkeys.lock().expect("hotkey state not poisoned");
        if state.recording == Some(action) {
            return language.text(Text::PressKeys).into();
        }
        let label = match action {
            HotkeyAction::Power => Text::ActionPower,
            HotkeyAction::VolumeUp => Text::ActionVolumeUp,
            HotkeyAction::VolumeDown => Text::ActionVolumeDown,
            HotkeyAction::ToggleApp => Text::ActionToggleApp,
        };
        let combo = state.shortcuts[action.slot()]
            .map(|shortcut| shortcut.display(key_label))
            .unwrap_or_else(|| language.text(Text::NoShortcut).into());
        format!("{}: {}", language.text(label), combo)
    }

    pub fn play_preview(&self) {
        if let Some(worker) = self.worker.borrow().as_ref() {
            let _ = worker.sender.try_send(Command::Preview);
        }
    }

    pub fn play_mouse_preview(&self) {
        if let Some(worker) = self.worker.borrow().as_ref() {
            let _ = worker.sender.try_send(Command::PreviewMouse);
        }
    }

    pub fn restart_audio(&self) {
        if let Some(worker) = self.worker.borrow().as_ref() {
            let _ = worker.sender.try_send(Command::ResetAudio);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn notify_profiles_changed(&self) {
        if let Some(worker) = self.worker.borrow().as_ref() {
            let _ = worker.sender.try_send(Command::ProfilesChanged);
        }
    }

    pub fn keyboard_profiles_by_category(&self) -> Vec<(Text, Vec<Profile>)> {
        [
            Text::Mechanical,
            Text::Electronic,
            Text::Fun,
            Text::Typewriters,
        ]
        .into_iter()
        .map(|category| {
            (
                category,
                Profile::all()
                    .filter(|profile| profile.category() == category)
                    .collect(),
            )
        })
        .collect()
    }

    pub fn mouse_profiles(&self) -> Vec<MouseProfile> {
        MouseProfile::all().collect()
    }

    pub fn languages(&self) -> [Language; 3] {
        Language::ALL
    }

    pub fn volume_steps(&self) -> [u8; 6] {
        [0, 10, 25, 50, 75, 100]
    }

    #[cfg(target_os = "macos")]
    pub fn route_options(&self) -> Vec<(Text, routes::Mode)> {
        vec![
            (Text::RouteAlways, routes::Mode::Always),
            (Text::RouteHeadphones, routes::Mode::Headphones),
            (Text::RouteSpeakers, routes::Mode::Speakers),
        ]
    }

    pub fn hotkey_actions(&self) -> [HotkeyAction; 4] {
        HotkeyAction::ALL
    }

    pub fn version_string(&self) -> String {
        format!(
            "{} {}",
            self.language.get().text(Text::Version),
            env!("CARGO_PKG_VERSION")
        )
    }

    pub fn github_url(&self) -> &'static str {
        consts::github_url()
    }

    pub fn telegram_url(&self) -> &'static str {
        consts::telegram_url()
    }

    fn apply_profiles(&self) {
        let (keyboard, mouse) = perapp::resolve(
            &self.overrides.borrow(),
            self.frontmost.borrow().as_deref(),
            self.global.get().0,
            self.global.get().1,
        );
        if let Some(index) = keyboard {
            #[cfg(target_os = "macos")]
            self.controls.keyboard_off.store(false, Ordering::Relaxed);
            self.controls.profile.store(index, Ordering::Relaxed);
        }
        #[cfg(target_os = "macos")]
        if keyboard.is_none() {
            self.controls.keyboard_off.store(true, Ordering::Relaxed);
        }
        self.controls.mouse_profile.store(mouse, Ordering::Relaxed);
    }

    fn save_overrides(&self) {
        self.persistence
            .borrow()
            .save_overrides(&self.overrides.borrow());
    }
}

#[cfg(target_os = "macos")]
pub mod mac {
    use objc2::rc::Retained;
    use objc2_foundation::{NSString, NSUserDefaults, ns_string};

    use super::*;

    #[derive(Debug)]
    pub struct Persistence {
        defaults: Retained<NSUserDefaults>,
    }

    impl Persistence {
        pub fn new(defaults: Retained<NSUserDefaults>) -> Self {
            Self { defaults }
        }
    }

    impl super::Persistence for Persistence {
        fn profile(&self) -> u8 {
            Profile::from_index(self.defaults.integerForKey(ns_string!("profile")).max(0) as usize)
                .0
        }

        fn mouse_profile(&self) -> u8 {
            MouseProfile::from_index(
                self.defaults
                    .integerForKey(ns_string!("mouseProfile"))
                    .max(0) as usize,
            )
            .0
        }

        fn volume(&self) -> u8 {
            if self.defaults.objectForKey(ns_string!("volume")).is_some() {
                self.defaults
                    .integerForKey(ns_string!("volume"))
                    .clamp(0, 100) as u8
            } else {
                25
            }
        }

        fn enabled(&self) -> Option<bool> {
            self.defaults
                .objectForKey(ns_string!("enabled"))
                .map(|_| self.defaults.boolForKey(ns_string!("enabled")))
        }

        fn language(&self, _fallback: Language) -> Language {
            Language::load(&self.defaults)
        }

        fn launch_at_login(&self) -> bool {
            self.defaults.boolForKey(ns_string!("launchAtLogin"))
        }

        #[cfg(target_os = "macos")]
        fn stereo_pan(&self) -> Option<bool> {
            self.defaults
                .objectForKey(ns_string!("stereoKeyPosition"))
                .map(|_| self.defaults.boolForKey(ns_string!("stereoKeyPosition")))
        }

        fn route_mode_index(&self) -> u8 {
            self.defaults.integerForKey(ns_string!("audioRoute")).max(0) as u8
        }

        fn set_schema_version(&self, version: isize) {
            self.defaults
                .setInteger_forKey(version, ns_string!("schemaVersion"));
        }

        fn remove_smoke_defaults(&self, name: &NSString) {
            self.defaults.removePersistentDomainForName(name);
        }

        fn overrides(&self) -> perapp::Map {
            perapp::load(
                self.defaults
                    .stringForKey(ns_string!("perApp"))
                    .map(|text| text.to_string())
                    .as_deref(),
            )
        }

        fn hotkeys(&self) -> HotkeyState {
            HotkeyState::load(
                self.defaults
                    .stringForKey(ns_string!("hotkeys"))
                    .map(|text| text.to_string())
                    .as_deref(),
            )
        }

        fn save_profile(&self, profile: u8) {
            self.defaults
                .setInteger_forKey(isize::from(profile), ns_string!("profile"));
        }

        fn save_mouse_profile(&self, profile: u8) {
            self.defaults
                .setInteger_forKey(isize::from(profile), ns_string!("mouseProfile"));
        }

        fn save_volume(&self, volume: u8) {
            self.defaults
                .setInteger_forKey(volume.into(), ns_string!("volume"));
        }

        fn save_enabled(&self, enabled: bool) {
            self.defaults.setBool_forKey(enabled, ns_string!("enabled"));
        }

        fn save_language(&self, language: Language) {
            language.save(&self.defaults);
        }

        fn save_launch_at_login(&self, enabled: bool) {
            self.defaults
                .setBool_forKey(enabled, ns_string!("launchAtLogin"));
        }

        #[cfg(target_os = "macos")]
        fn save_stereo_pan(&self, enabled: bool) {
            self.defaults
                .setBool_forKey(enabled, ns_string!("stereoKeyPosition"));
        }

        fn save_route_mode(&self, mode: routes::Mode) {
            self.defaults
                .setInteger_forKey(mode as isize, ns_string!("audioRoute"));
        }

        fn save_overrides(&self, overrides: &perapp::Map) {
            let text = NSString::from_str(&perapp::save(overrides));
            // SAFETY: NSUserDefaults copies this immutable NSString property-list value.
            unsafe {
                self.defaults
                    .setObject_forKey(Some(&text), ns_string!("perApp"))
            };
        }

        fn save_hotkeys(&self, hotkeys: &HotkeyState) {
            let text = NSString::from_str(&hotkeys.save());
            // SAFETY: NSUserDefaults copies this immutable NSString property-list value.
            unsafe {
                self.defaults
                    .setObject_forKey(Some(&text), ns_string!("hotkeys"))
            };
        }
    }
}

#[cfg(windows)]
pub mod windows {
    use std::cell::Cell;

    use crate::core::settings::Config;

    use super::*;

    #[derive(Debug)]
    pub struct Persistence {
        config: Cell<Config>,
    }

    impl Persistence {
        pub fn new(config: Config) -> Self {
            Self {
                config: Cell::new(config),
            }
        }
    }

    impl super::Persistence for Persistence {
        fn profile(&self) -> u8 {
            self.config.get().profile
        }

        fn mouse_profile(&self) -> u8 {
            self.config.get().mouse_profile
        }

        fn volume(&self) -> u8 {
            self.config.get().volume
        }

        fn enabled(&self) -> Option<bool> {
            Some(self.config.get().enabled)
        }

        fn language(&self, _fallback: Language) -> Language {
            self.config.get().language
        }

        fn launch_at_login(&self) -> bool {
            self.config.get().launch_at_login
        }

        fn save_profile(&self, profile: u8) {
            let mut config = self.config.get();
            config.profile = profile;
            config.save();
            self.config.set(config);
        }

        fn save_mouse_profile(&self, profile: u8) {
            let mut config = self.config.get();
            config.mouse_profile = profile;
            config.save();
            self.config.set(config);
        }

        fn save_volume(&self, volume: u8) {
            let mut config = self.config.get();
            config.volume = volume;
            config.save();
            self.config.set(config);
        }

        fn save_enabled(&self, enabled: bool) {
            let mut config = self.config.get();
            config.enabled = enabled;
            config.save();
            self.config.set(config);
        }

        fn save_language(&self, language: Language) {
            let mut config = self.config.get();
            config.language = language;
            config.save();
            self.config.set(config);
        }

        fn save_launch_at_login(&self, enabled: bool) {
            let mut config = self.config.get();
            config.launch_at_login = enabled;
            config.save();
            self.config.set(config);
        }
    }
}
