use std::{
    cell::{Cell, OnceCell, RefCell},
    path::Path,
    sync::{Arc, atomic::Ordering},
};

use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSButton, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSPopUpButton, NSPopover, NSSlider,
    NSStatusBar, NSStatusItem, NSWindow, NSWindowDelegate, NSWorkspace,
};
use objc2_core_graphics::{CGPreflightListenEventAccess, CGRequestListenEventAccess};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSData, NSError, NSNotification, NSObject, NSObjectProtocol, NSSize,
    NSString, NSTimer, NSURL, NSUserDefaults, ns_string,
};

use crate::{
    consts,
    core::{
        audio::{Command, Controls, Worker},
        catalog::{MouseProfile, Profile},
        i18n::{Language, Text},
        input::InputTap,
        login,
        update::Updater,
    },
};

mod settings;

struct State {
    window: RefCell<Option<Retained<NSWindow>>>,
    about: RefCell<Option<Retained<NSPopover>>>,
    controls: Arc<Controls>,
    language: Cell<Language>,
    worker: RefCell<Option<Worker>>,
    tap: RefCell<Option<InputTap>>,
    status: OnceCell<Retained<NSStatusItem>>,
    menu: OnceCell<Retained<NSMenu>>,
    defaults: Retained<NSUserDefaults>,
    smoke_test: bool,
    updater: OnceCell<Updater>,
    update_version: RefCell<Option<String>>,
    update_probe_finished: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. All UI and input lifecycle
    // calls stay on the main thread. App keeps this delegate alive throughout run().
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = State]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_launch(&self, _notification: &NSNotification) {
            let mtm = self.mtm();
            let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("Soundy"));
            menu.setAutoenablesItems(false);
            menu.setDelegate(Some(ProtocolObject::from_ref(self)));
            let status = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
            if let Some(button) = status.button(mtm) {
                let image = NSImage::initWithData(
                    NSImage::alloc(),
                    &NSData::with_bytes(include_bytes!("../../assets/icons/tray-template.png")),
                ).expect("embedded tray icon must decode");
                image.setSize(NSSize::new(18.0, 18.0));
                image.setTemplate(true);
                button.setImage(Some(&image));
                button.setAccessibilityLabel(Some(ns_string!("Soundy")));
                button.setToolTip(Some(&NSString::from_str(self.ivars().language.get().text(Text::Tooltip))));
            }
            status.setMenu(Some(&menu));
            let _ = self.ivars().status.set(status);
            let _ = self.ivars().menu.set(menu);
            self.refresh();
            // Alternate feeds are accepted only by the explicit smoke-test mode.
            let test_feed = if self.ivars().smoke_test {
                std::env::var("SOUNDY_UPDATE_TEST_FEED").ok()
            } else { None };
            if !self.ivars().smoke_test || test_feed.is_some() {
                match Updater::start(self, test_feed.as_deref()) {
                    Ok(updater) => { let _ = self.ivars().updater.set(updater); }
                    Err(error) => eprintln!("Soundy update check: {error}"),
                }
            }
            if self.ivars().smoke_test {
                // SAFETY: The target is retained by NSTimer until this one-shot invocation;
                // smokeFinish: has the matching NSTimer action signature below.
                unsafe {
                    NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                        if test_feed.is_some() { 5.0 } else { 0.25 }, self, sel!(smokeFinish:), None, false,
                    );
                }
            }
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            self.ivars().tap.borrow_mut().take();
            self.ivars().worker.borrow_mut().take();
        }
    }

    unsafe impl NSWindowDelegate for Delegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            if let Some(about) = self.ivars().about.borrow_mut().take() { about.close(); }
            if let Some(window) = self.ivars().window.borrow_mut().take() {
                // Release after AppKit finishes the close event, not inside its callback.
                let _ = Retained::autorelease_ptr(window);
            }
        }
    }

    unsafe impl NSMenuDelegate for Delegate {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) { self.refresh(); }
    }

    impl Delegate {
        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: &AnyObject) {
            self.refresh();
            if self.ivars().window.borrow().is_none() {
                let window = settings::create(self.mtm());
                window.setDelegate(Some(ProtocolObject::from_ref(self)));
                *self.ivars().window.borrow_mut() = Some(window);
                self.refresh();
            }
            if let Some(window) = self.ivars().window.borrow().as_ref() {
                #[allow(deprecated)] // macOS 13 compatibility; activate() requires macOS 14.
                NSApplication::sharedApplication(self.mtm()).activateIgnoringOtherApps(true);
                window.makeKeyAndOrderFront(None);
            }
        }

        #[unsafe(method(settingsChoice:))]
        fn settings_choice(&self, sender: &NSPopUpButton) {
            if let Some(item) = sender.selectedItem() {
                match sender.tag() {
                    0 => self.select_profile(sel!(selectProfile:), &item),
                    1 => self.select_mouse(sel!(selectMouse:), &item),
                    2 => self.select_volume(sel!(selectVolume:), &item),
                    3 => self.select_language(sel!(selectLanguage:), &item),
                    _ => {},
                }
            }
        }

        #[unsafe(method(closeSettings:))]
        fn close_settings(&self, _sender: &AnyObject) {
            let window = self.ivars().window.borrow().clone();
            if let Some(window) = window { window.close(); }
        }

        #[unsafe(method(settingsPreview:))]
        fn settings_preview(&self, sender: &NSButton) {
            if sender.tag() == 0 { self.play_preview(); } else { self.play_mouse_preview(); }
        }

        #[unsafe(method(settingsLanguage:))]
        fn settings_language(&self, sender: &NSButton) {
            if let Some(language) = Language::ALL.get(sender.tag() as usize) {
                self.ivars().language.set(*language);
                language.save(&self.ivars().defaults);
                self.refresh();
            }
        }

        #[unsafe(method(settingsVolume:))]
        fn settings_volume(&self, sender: &NSSlider) {
            let volume = sender.doubleValue().round().clamp(0.0, 100.0) as u8;
            self.ivars().controls.volume.store(volume, Ordering::Relaxed);
            self.ivars().defaults.setInteger_forKey(volume.into(), ns_string!("volume"));
            if let Some(window) = self.ivars().window.borrow().as_ref() {
                settings::sync_volume(window, self.ivars().language.get(), volume);
            }
        }

        #[unsafe(method(showAbout:))]
        fn show_about(&self, sender: &NSButton) {
            if let Some(about) = self.ivars().about.borrow_mut().take() {
                let shown = about.isShown();
                about.close();
                if shown { return; }
            }
            let menu = self.ivars().menu.get().unwrap();
            *self.ivars().about.borrow_mut() = Some(settings::about(sender, self, menu, self.ivars().language.get()));
        }

        #[unsafe(method(openSocial:))]
        fn open_social(&self, sender: &NSButton) {
            let link = if sender.tag() == 0 { consts::github_url() } else { consts::telegram_url() };
            if let Some(url) = NSURL::URLWithString(&NSString::from_str(link)) {
                NSWorkspace::sharedWorkspace().openURL(&url);
            }
        }

        #[unsafe(method(settingsAction:))]
        fn settings_action(&self, sender: &NSButton) {
            if let Some(item) = self.ivars().menu.get().and_then(|menu| menu.itemAtIndex(sender.tag())) {
                if let Some(action) = item.action() {
                    // SAFETY: Use the same target, selector and sender as the tray item.
                    unsafe { NSApplication::sharedApplication(self.mtm()).sendAction_to_from(action, item.target().as_deref(), Some(&item)); }
                }
            }
        }

        #[unsafe(method(updater:didFindValidUpdate:))]
        fn update_found(&self, _updater: &AnyObject, item: &AnyObject) {
            // SAFETY: Sparkle passes SUAppcastItem, whose displayVersionString
            // property is a non-null NSString. Delegate callbacks are main-thread.
            let version: Retained<NSString> = unsafe { msg_send![item, displayVersionString] };
            *self.ivars().update_version.borrow_mut() = Some(version.to_string());
            self.refresh();
        }

        #[unsafe(method(installUpdate:))]
        fn install_update(&self, _sender: &NSMenuItem) {
            if let Some(updater) = self.ivars().updater.get() {
                updater.install();
            }
        }

        #[unsafe(method(updater:didFinishUpdateCycleForUpdateCheck:error:))]
        fn update_probe_finished(&self, _updater: &AnyObject, _check: isize, error: Option<&NSError>) {
            self.ivars().update_probe_finished.set(true);
            if let Some(error) = error {
                eprintln!("Soundy update check: {error}");
            }
        }

        #[unsafe(method(smokeFinish:))]
        fn smoke_finish(&self, _timer: &NSTimer) {
            let menu = self.ivars().menu.get().expect("menu created before timer");
            if let Ok(expected) = std::env::var("SOUNDY_UPDATE_TEST_EXPECTED") {
                assert!(self.ivars().updater.get().is_some(), "embedded updater must start");
                assert!(self.ivars().update_probe_finished.get(), "background probe must finish");
                let observed = self.ivars().update_version.borrow().clone();
                assert_eq!(observed.as_deref(), if expected == "none" { None } else { Some(expected.as_str()) });
                let app = NSApplication::sharedApplication(self.mtm());
                // SAFETY: NSApplication.windows is an NSArray of NSWindow objects.
                let windows: Retained<NSArray<AnyObject>> = unsafe { msg_send![&*app, windows] };
                for window in windows {
                    let visible: bool = unsafe { msg_send![&*window, isVisible] };
                    if visible {
                        let class = window.class().name().to_string_lossy();
                        let title: Retained<NSString> = unsafe { msg_send![&*window, title] };
                        let level: isize = unsafe { msg_send![&*window, level] };
                        println!("Smoke visible window: {class}, title={title}, level={level}");
                        assert!(class.contains("StatusBar"), "startup update checks must not display windows");
                    }
                }
                *self.ivars().update_version.borrow_mut() = None;
                self.refresh();
                println!("OK: background update probe: {expected}; no windows shown");
            }
            assert!(self.ivars().window.borrow().is_none(), "startup must remain tray-only");
            for language in Language::ALL {
                let title = self.ivars().language.get().text(Text::LanguageMenu);
                let language_menu = menu.itemWithTitle(&NSString::from_str(title)).unwrap().submenu().unwrap();
                self.select_language(sel!(selectLanguage:), &language_menu.itemAtIndex(language as isize).unwrap());
                assert_eq!(Language::load(&self.ivars().defaults), language, "language preference must persist");
                let submenu = |text| menu.itemWithTitle(&NSString::from_str(language.text(text))).unwrap().submenu().unwrap();
                let keyboard_menu = submenu(Text::KeyboardSound);
                assert_eq!(keyboard_menu.numberOfItems(), 47);
                let mut index = 0;
                for (category, count) in [(Text::Mechanical, 14), (Text::Electronic, 6), (Text::Fun, 11), (Text::Typewriters, 9)] {
                    if index > 0 {
                        assert!(keyboard_menu.itemAtIndex(index).unwrap().isSeparatorItem());
                        index += 1;
                    }
                    let heading = keyboard_menu.itemAtIndex(index).unwrap();
                    assert_eq!(heading.title().to_string(), language.text(category));
                    assert!(!heading.isEnabled());
                    index += 1;
                    for profile in Profile::all().filter(|profile| profile.category() == category) {
                        let item = keyboard_menu.itemAtIndex(index).unwrap();
                        assert_eq!(item.title().to_string(), profile.name());
                        assert_eq!(item.tag(), isize::from(profile.0));
                        assert_eq!(item.action(), Some(sel!(selectProfile:)));
                        index += 1;
                    }
                    assert_eq!(Profile::all().filter(|profile| profile.category() == category).count(), count);
                }
                assert_eq!(index, keyboard_menu.numberOfItems());
                let mouse_menu = submenu(Text::MouseSound);
                assert_eq!(mouse_menu.numberOfItems(), 16);
                for index in [0, 1, 15] {
                    assert_eq!(mouse_menu.itemAtIndex(index).unwrap().title().to_string(), language.mouse_name(index as u8));
                }
                let language_menu = submenu(Text::LanguageMenu);
                assert_eq!(language_menu.numberOfItems(), 3);
                for option in Language::ALL {
                    let item = language_menu.itemAtIndex(option as isize).unwrap();
                    assert_eq!(item.title().to_string(), option.name());
                    assert_eq!(item.state(), if option == language { 1 } else { 0 });
                }
                for text in [Text::LaunchAtLogin, Text::Volume, Text::Quit] {
                    assert!(menu.itemWithTitle(&NSString::from_str(language.text(text))).is_some());
                }
                let permission_visible = menu.itemWithTitle(&NSString::from_str(language.text(Text::AllowInput))).is_some();
                assert_ne!(permission_visible, CGPreflightListenEventAccess());
                let login_item = menu.itemWithTitle(&NSString::from_str(language.text(Text::LaunchAtLogin))).unwrap();
                assert_eq!(login_item.state() == 1, matches!(login::status(), login::ENABLED | login::REQUIRES_APPROVAL));
                // Smoke mode never starts the input tap, so Allow/Reconnect is present.
                assert_eq!(menu.numberOfItems(), 11 + if login::status() == login::REQUIRES_APPROVAL { 1 } else { 0 });
                *self.ivars().update_version.borrow_mut() = Some("99.0.0".into());
                self.refresh();
                let title = format!("{}: 99.0.0…", language.text(Text::UpdateAvailable));
                let update_item = menu.itemWithTitle(&NSString::from_str(&title)).unwrap();
                assert_eq!(update_item.action(), Some(sel!(installUpdate:)));
                assert!(update_item.isEnabled());
                *self.ivars().update_version.borrow_mut() = None;
                self.refresh();
                assert!(menu.itemWithTitle(&NSString::from_str(&title)).is_none());
                let button = self.ivars().status.get().unwrap().button(self.mtm()).unwrap();
                let image = button.image().unwrap();
                assert!(image.isTemplate() && image.size() == NSSize::new(18.0, 18.0));
                assert_eq!(button.toolTip().unwrap().to_string(), language.text(Text::InputUnavailable));
                let weak = objc2::rc::autoreleasepool(|_| {
                    self.open_settings(sel!(openSettings:), self);
                    let window = self.ivars().window.borrow().as_ref().unwrap().clone();
                    let weak = objc2::rc::Weak::from_retained(&window);
                    assert!(window.styleMask().contains(objc2_app_kit::NSWindowStyleMask::Titled), "system window must own corner clipping");
                    assert!(!window.isOpaque());
                    if let Some(class) = objc2::runtime::AnyClass::get(c"NSGlassEffectView") {
                        assert!(window.contentView().unwrap().isKindOfClass(class));
                    }
                    self.open_settings(sel!(openSettings:), self);
                    assert!(std::ptr::eq(&**self.ivars().window.borrow().as_ref().unwrap(), &*window), "reuse the visible window");
                    let view = settings::content(&window);
                    assert_eq!(view.bounds().size, NSSize::new(240.0, 240.0));
                    for child in view.subviews() {
                        let frame = child.frame();
                        assert!(frame.origin.x >= 0.0 && frame.origin.y >= 0.0 && frame.origin.x + frame.size.width <= 240.0 && frame.origin.y + frame.size.height <= 240.0, "compact controls must fit: {frame:?}");
                    }
                    assert!(!view.subviews().iter().filter_map(|view| view.downcast::<objc2_app_kit::NSTextField>().ok()).any(|label| label.stringValue().to_string() == "Soundy"));
                    let popups: Vec<_> = view.subviews().iter().filter_map(|view| view.downcast::<NSPopUpButton>().ok()).collect();
                    assert_eq!(popups.len(), 2);
                    for (popup, value) in popups.iter().zip([6, 15]) {
                        assert!(popup.selectItemWithTag(value));
                        // Exercise the actual NSControl target/action route.
                        unsafe { popup.sendAction_to(popup.action(), popup.target().as_deref()); }
                    }
                    assert_eq!(self.ivars().defaults.integerForKey(ns_string!("profile")), 6);
                    assert_eq!(self.ivars().defaults.integerForKey(ns_string!("mouseProfile")), 15);
                    let root = settings::content(&window);
                    let slider = root.subviews().iter().find_map(|view| view.downcast::<NSSlider>().ok()).unwrap();
                    for volume in [0.0, 37.0, 100.0, 25.0] {
                        slider.setDoubleValue(volume);
                        unsafe { slider.sendAction_to(slider.action(), slider.target().as_deref()); }
                        assert_eq!(self.ivars().defaults.integerForKey(ns_string!("volume")), volume as isize);
                        let percentage = root.subviews().iter().find_map(|view| view.downcast::<objc2_app_kit::NSTextField>().ok().filter(|label| label.tag() == consts::VOLUME_TAG)).unwrap();
                        assert_eq!(percentage.stringValue().to_string(), format!("{volume}%"));
                    }
                    for option in Language::ALL {
                        let root = settings::content(&window);
                        let control = root.subviews().iter().find_map(|view| view.downcast::<NSButton>().ok().filter(|button| button.action() == Some(sel!(settingsLanguage:)) && button.tag() == option as isize)).unwrap();
                        unsafe { control.sendAction_to(control.action(), control.target().as_deref()); }
                        assert_eq!(Language::load(&self.ivars().defaults), option);
                    }
                    self.ivars().language.set(language);
                    language.save(&self.ivars().defaults);
                    self.refresh();
                    let root = settings::content(&window);
                    let about = root.subviews().iter().find_map(|view| view.downcast::<NSButton>().ok().filter(|button| button.action() == Some(sel!(showAbout:)))).unwrap();
                    self.show_about(sel!(showAbout:), &about);
                    let popover = self.ivars().about.borrow().as_ref().unwrap().clone();
                    assert!(popover.isShown());
                    let content = popover.contentViewController().unwrap().view();
                    assert_eq!(content.bounds().size, NSSize::new(180.0, 112.0 + if login::status() == login::REQUIRES_APPROVAL { 24.0 } else { 0.0 }));
                    let links: Vec<_> = content.subviews().iter().filter_map(|view| view.downcast::<NSButton>().ok().filter(|button| button.action() == Some(sel!(openSocial:)))).collect();
                    assert_eq!(links.len(), 2);
                    assert!(links.iter().all(|button| button.image().is_some()));
                    assert!(content.subviews().iter().filter_map(|view| view.downcast::<objc2_app_kit::NSTextField>().ok()).any(|label| label.stringValue().to_string().contains(env!("CARGO_PKG_VERSION"))));
                    assert_eq!(consts::github_url(), "https://github.com/not-pro");
                    assert_eq!(consts::telegram_url(), "https://nbautin.t.me");
                    assert_eq!(self.ivars().defaults.integerForKey(ns_string!("volume")), 25);
                    assert_eq!(Language::load(&self.ivars().defaults), language);
                    self.close_settings(sel!(closeSettings:), self);
                    assert!(self.ivars().about.borrow().is_none());
                    assert!(self.ivars().window.borrow().is_none(), "close must clear the UI owner");
                    assert!(self.ivars().worker.borrow().is_some(), "audio must survive closing UI");
                    weak
                });
                assert!(weak.load().is_none(), "closed window must deallocate after the event pool drains");
                println!("OK: {} settings controls, persistence, reopen and UI deallocation", language.code());
                println!("OK: {} menu, language selection and saved preference; no input monitoring started", language.code());
            }
            self.ivars().defaults.removePersistentDomainForName(&smoke_defaults_name());
            if std::env::var_os("SOUNDY_SETTINGS_PREVIEW").is_some() {
                self.ivars().language.set(Language::Russian);
                self.open_settings(sel!(openSettings:), self);
                return;
            }
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }

        #[unsafe(method(toggleLogin:))]
        fn toggle_login(&self, _sender: &NSMenuItem) {
            let enable = !matches!(login::status(), login::ENABLED | login::REQUIRES_APPROVAL);
            let result = login::set_enabled(enable);
            self.refresh();
            if enable && login::status() == login::REQUIRES_APPROVAL {
                login::open_settings();
            } else if let Err(error) = result {
                let alert = NSAlert::new(self.mtm());
                let language = self.ivars().language.get();
                alert.setMessageText(&NSString::from_str(language.text(Text::LoginFailed)));
                alert.setInformativeText(&error.localizedDescription());
                alert.addButtonWithTitle(&NSString::from_str(language.text(Text::OK)));
                alert.runModal();
            }
        }

        #[unsafe(method(approveLogin:))]
        fn approve_login(&self, _sender: &NSMenuItem) { login::open_settings(); }

        #[unsafe(method(selectProfile:))]
        fn select_profile(&self, sender: &NSMenuItem) {
            let profile = Profile::from_index(sender.tag().max(0) as usize);
            self.ivars().controls.profile.store(profile.0, Ordering::Relaxed);
            self.ivars().defaults.setInteger_forKey(isize::from(profile.0), ns_string!("profile"));
            self.play_preview();
            self.refresh();
        }

        #[unsafe(method(selectMouse:))]
        fn select_mouse(&self, sender: &NSMenuItem) {
            let profile = MouseProfile::from_index(sender.tag().max(0) as usize);
            self.ivars().controls.mouse_profile.store(profile.0, Ordering::Relaxed);
            self.ivars().defaults.setInteger_forKey(isize::from(profile.0), ns_string!("mouseProfile"));
            self.play_mouse_preview();
            self.refresh();
        }

        #[unsafe(method(selectVolume:))]
        fn select_volume(&self, sender: &NSMenuItem) {
            let volume = sender.tag().clamp(0, 100) as u8;
            self.ivars().controls.volume.store(volume, Ordering::Relaxed);
            self.ivars().defaults.setInteger_forKey(volume.into(), ns_string!("volume"));
            self.refresh();
        }

        #[unsafe(method(permission:))]
        fn permission(&self, _sender: &NSMenuItem) {
            if !CGPreflightListenEventAccess() {
                CGRequestListenEventAccess();
                let url = NSURL::URLWithString(ns_string!("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"));
                if let Some(url) = url { NSWorkspace::sharedWorkspace().openURL(&url); }
            }
            self.refresh();
        }

        #[unsafe(method(selectLanguage:))]
        fn select_language(&self, sender: &NSMenuItem) {
            if let Some(&language) = usize::try_from(sender.tag()).ok().and_then(|index| Language::ALL.get(index)) {
                self.ivars().language.set(language);
                language.save(&self.ivars().defaults);
                self.refresh();
            }
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: &NSMenuItem) {
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

impl Delegate {
    fn play_preview(&self) {
        if let Some(worker) = self.ivars().worker.borrow().as_ref() {
            let _ = worker.sender.try_send(Command::Preview);
        }
    }

    fn play_mouse_preview(&self) {
        if let Some(worker) = self.ivars().worker.borrow().as_ref() {
            let _ = worker.sender.try_send(Command::PreviewMouse);
        }
    }

    fn new(
        mtm: MainThreadMarker,
        controls: Arc<Controls>,
        worker: Worker,
        smoke_test: bool,
        language: Language,
    ) -> Retained<Self> {
        let defaults = if smoke_test {
            NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&smoke_defaults_name()))
                .expect("smoke-test defaults suite")
        } else {
            NSUserDefaults::standardUserDefaults()
        };
        controls.profile.store(
            Profile::from_index(defaults.integerForKey(ns_string!("profile")).max(0) as usize).0,
            Ordering::Relaxed,
        );
        controls.mouse_profile.store(
            MouseProfile::from_index(
                defaults.integerForKey(ns_string!("mouseProfile")).max(0) as usize
            )
            .0,
            Ordering::Relaxed,
        );
        if defaults.objectForKey(ns_string!("volume")).is_some() {
            controls.volume.store(
                defaults.integerForKey(ns_string!("volume")).clamp(0, 100) as u8,
                Ordering::Relaxed,
            );
        }
        let this = Self::alloc(mtm).set_ivars(State {
            controls,
            window: RefCell::new(None),
            about: RefCell::new(None),
            language: Cell::new(language),
            defaults,
            smoke_test,
            worker: RefCell::new(Some(worker)),
            tap: RefCell::new(None),
            status: OnceCell::new(),
            menu: OnceCell::new(),
            updater: OnceCell::new(),
            update_version: RefCell::new(None),
            update_probe_finished: Cell::new(false),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn refresh(&self) {
        let language = self.ivars().language.get();
        let granted = CGPreflightListenEventAccess();
        let mut tap = self.ivars().tap.borrow_mut();
        if !granted || tap.as_ref().is_some_and(|tap| !tap.is_enabled()) {
            tap.take();
        }
        let mut input_error = None;
        if granted && tap.is_none() && !self.ivars().smoke_test {
            if let Some(worker) = self.ivars().worker.borrow().as_ref() {
                match InputTap::start(worker.sender.clone(), language) {
                    Ok(new_tap) => *tap = Some(new_tap),
                    Err(error) => input_error = Some(error),
                }
            }
        }
        let listening = tap.is_some();
        drop(tap);
        let Some(menu) = self.ivars().menu.get() else {
            return;
        };
        menu.removeAllItems();
        let controls = &self.ivars().controls;
        self.item(
            menu,
            language.text(Text::OpenSettings),
            Some(sel!(openSettings:)),
            0,
            false,
        );
        if !granted {
            self.item(
                menu,
                language.text(Text::AllowInput),
                Some(sel!(permission:)),
                0,
                false,
            );
        } else if !listening {
            self.item(
                menu,
                language.text(Text::RetryInput),
                Some(sel!(permission:)),
                0,
                false,
            );
        }
        if let Some(error) = input_error {
            eprintln!("Soundy: {error}");
            self.item(menu, &error, None, 0, false);
        }
        if menu.numberOfItems() > 0 {
            menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        }
        let keyboard_menu = self.submenu(menu, language.text(Text::KeyboardSound));
        for category in [
            Text::Mechanical,
            Text::Electronic,
            Text::Fun,
            Text::Typewriters,
        ] {
            if keyboard_menu.numberOfItems() > 0 {
                keyboard_menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
            }
            self.item(&keyboard_menu, language.text(category), None, 0, false);
            for profile in Profile::all().filter(|profile| profile.category() == category) {
                self.item(
                    &keyboard_menu,
                    profile.name(),
                    Some(sel!(selectProfile:)),
                    isize::from(profile.0),
                    controls.profile.load(Ordering::Relaxed) == profile.0,
                );
            }
        }
        let mouse_menu = self.submenu(menu, language.text(Text::MouseSound));
        for profile in MouseProfile::all() {
            self.item(
                &mouse_menu,
                &language.mouse_name(profile.0),
                Some(sel!(selectMouse:)),
                isize::from(profile.0),
                controls.mouse_profile.load(Ordering::Relaxed) == profile.0,
            );
        }
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        let volume_menu = self.submenu(menu, language.text(Text::Volume));
        for volume in [0, 10, 25, 50, 75, 100] {
            self.item(
                &volume_menu,
                &format!("{volume}%"),
                Some(sel!(selectVolume:)),
                volume,
                controls.volume.load(Ordering::Relaxed) as isize == volume,
            );
        }
        let language_menu = self.submenu(menu, language.text(Text::LanguageMenu));
        for option in Language::ALL {
            self.item(
                &language_menu,
                option.name(),
                Some(sel!(selectLanguage:)),
                option as isize,
                option == language,
            );
        }
        let login_status = login::status();
        self.item(
            menu,
            language.text(Text::LaunchAtLogin),
            Some(sel!(toggleLogin:)),
            0,
            matches!(login_status, login::ENABLED | login::REQUIRES_APPROVAL),
        );
        if login_status == login::REQUIRES_APPROVAL {
            self.item(
                menu,
                language.text(Text::ApproveLogin),
                Some(sel!(approveLogin:)),
                0,
                false,
            );
        }
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        if let Some(version) = self.ivars().update_version.borrow().as_ref() {
            self.item(
                menu,
                &format!("{}: {version}…", language.text(Text::UpdateAvailable)),
                Some(sel!(installUpdate:)),
                0,
                false,
            );
        }
        self.item(menu, language.text(Text::Quit), Some(sel!(quit:)), 0, false);
        if let Some(window) = self.ivars().window.borrow().as_ref() {
            if let Some(about) = self.ivars().about.borrow_mut().take() {
                about.close();
            }
            settings::populate(
                window,
                menu,
                self,
                language,
                controls.volume.load(Ordering::Relaxed),
                listening,
            );
        }
        if let Some(button) = self
            .ivars()
            .status
            .get()
            .and_then(|status| status.button(self.mtm()))
        {
            button.setTitle(&NSString::from_str(if !granted || !listening {
                " !"
            } else {
                ""
            }));
            button.setToolTip(Some(&NSString::from_str(if !granted || !listening {
                language.text(Text::InputUnavailable)
            } else {
                language.text(Text::Tooltip)
            })));
        }
    }

    fn submenu(&self, menu: &NSMenu, title: &str) -> Retained<NSMenu> {
        let item = self.item(menu, title, None, 0, false);
        item.setEnabled(true);
        let submenu = NSMenu::initWithTitle(NSMenu::alloc(self.mtm()), &NSString::from_str(title));
        submenu.setAutoenablesItems(false);
        item.setSubmenu(Some(&submenu));
        submenu
    }

    fn item(
        &self,
        menu: &NSMenu,
        title: &str,
        action: Option<objc2::runtime::Sel>,
        tag: isize,
        checked: bool,
    ) -> Retained<NSMenuItem> {
        // SAFETY: All selectors refer to methods defined on this retained main-thread delegate.
        let item = unsafe {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(self.mtm()),
                &NSString::from_str(title),
                action,
                ns_string!(""),
            );
            item.setTarget(Some(self));
            item
        };
        item.setTag(tag);
        item.setState(if checked { 1 } else { 0 });
        item.setEnabled(action.is_some());
        menu.addItem(&item);
        item
    }
}

pub fn run(root: &Path, smoke_test: bool, language: Language) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or(language.text(Text::MainThreadRequired))?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let controls = Arc::new(Controls::default());
    let worker = Worker::start(root, controls.clone(), language)?;
    let delegate = Delegate::new(mtm, controls, worker, smoke_test, language);
    if let Some(worker) = delegate.ivars().worker.borrow().as_ref() {
        let _ = worker.sender.try_send(Command::ProfilesChanged);
    }
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
    Ok(())
}

pub fn show_error(error: &str, language: Language) {
    if let Some(mtm) = MainThreadMarker::new() {
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str(language.text(Text::StartFailed)));
        alert.addButtonWithTitle(&NSString::from_str(language.text(Text::OK)));
        alert.setInformativeText(&NSString::from_str(error));
        alert.runModal();
    }
}

fn smoke_defaults_name() -> Retained<NSString> {
    NSString::from_str(&format!("dev.soundy.smoke.{}", std::process::id()))
}
