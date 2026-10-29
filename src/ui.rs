use std::{
    cell::{OnceCell, RefCell},
    path::Path,
    sync::{Arc, atomic::Ordering},
};

use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem, NSWorkspace,
};
use objc2_core_graphics::{CGPreflightListenEventAccess, CGRequestListenEventAccess};
use objc2_foundation::{
    MainThreadMarker, NSData, NSNotification, NSObject, NSObjectProtocol, NSSize, NSString,
    NSTimer, NSURL, NSUserDefaults, ns_string,
};

use crate::{
    audio::{Command, Controls, Worker},
    catalog::{MouseProfile, Profile},
    input::InputTap,
};

struct State {
    controls: Arc<Controls>,
    worker: RefCell<Option<Worker>>,
    tap: RefCell<Option<InputTap>>,
    status: OnceCell<Retained<NSStatusItem>>,
    menu: OnceCell<Retained<NSMenu>>,
    defaults: Retained<NSUserDefaults>,
    smoke_test: bool,
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
                    &NSData::with_bytes(include_bytes!("../assets/icons/tray-template.png")),
                ).expect("embedded tray icon must decode");
                image.setSize(NSSize::new(18.0, 18.0));
                image.setTemplate(true);
                button.setImage(Some(&image));
                button.setAccessibilityLabel(Some(ns_string!("Soundy")));
                button.setToolTip(Some(ns_string!("Soundy — звуки клавиатуры и мыши")));
            }
            status.setMenu(Some(&menu));
            let _ = self.ivars().status.set(status);
            let _ = self.ivars().menu.set(menu);
            self.refresh();
            if self.ivars().smoke_test {
                // SAFETY: The target is retained by NSTimer until this one-shot invocation;
                // smokeFinish: has the matching NSTimer action signature below.
                unsafe {
                    NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                        0.25, self, sel!(smokeFinish:), None, false,
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

    unsafe impl NSMenuDelegate for Delegate {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) { self.refresh(); }
    }

    impl Delegate {
        #[unsafe(method(smokeFinish:))]
        fn smoke_finish(&self, _timer: &NSTimer) {
            let count = self.ivars().menu.get().map(|menu| menu.numberOfItems()).unwrap_or(0);
            let menu = self.ivars().menu.get().expect("menu created before timer");
            let keyboard_count = menu.itemWithTitle(ns_string!("Звук клавиатуры")).and_then(|item| item.submenu()).map(|menu| menu.numberOfItems());
            let mouse_count = menu.itemWithTitle(ns_string!("Звук мыши")).and_then(|item| item.submenu()).map(|menu| menu.numberOfItems());
            let permission_visible = menu.itemWithTitle(ns_string!("Разрешить мониторинг ввода…")).is_some();
            let tray_icon = self.ivars().status.get().and_then(|status| status.button(self.mtm())).and_then(|button| button.image());
            if tray_icon.is_some_and(|image| image.isTemplate() && image.size() == NSSize::new(18.0, 18.0)) && count >= 10 && keyboard_count == Some(40) && mouse_count == Some(16) && permission_visible != CGPreflightListenEventAccess() {
                println!("OK: AppKit launched, template tray icon and {count} menu items created; no input monitoring started");
            } else {
                eprintln!("Soundy: incomplete AppKit startup");
                std::process::exit(1);
            }
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }

        #[unsafe(method(toggle:))]
        fn toggle(&self, _sender: &NSMenuItem) {
            let controls = &self.ivars().controls;
            let enabled = !controls.enabled.load(Ordering::Relaxed);
            controls.enabled.store(enabled, Ordering::Relaxed);
            self.ivars().defaults.setBool_forKey(enabled, ns_string!("enabled"));
            self.refresh();
        }

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

        #[unsafe(method(previewMouse:))]
        fn preview_mouse(&self, _sender: &NSMenuItem) { self.play_mouse_preview(); }

        #[unsafe(method(selectVolume:))]
        fn select_volume(&self, sender: &NSMenuItem) {
            let volume = sender.tag().clamp(0, 100) as u8;
            self.ivars().controls.volume.store(volume, Ordering::Relaxed);
            self.ivars().defaults.setInteger_forKey(volume.into(), ns_string!("volume"));
            self.refresh();
        }

        #[unsafe(method(preview:))]
        fn preview(&self, _sender: &NSMenuItem) {
            self.play_preview();
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
    ) -> Retained<Self> {
        let defaults = NSUserDefaults::standardUserDefaults();
        if defaults.objectForKey(ns_string!("enabled")).is_some() {
            controls.enabled.store(
                defaults.boolForKey(ns_string!("enabled")),
                Ordering::Relaxed,
            );
        }
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
            defaults,
            smoke_test,
            worker: RefCell::new(Some(worker)),
            tap: RefCell::new(None),
            status: OnceCell::new(),
            menu: OnceCell::new(),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn refresh(&self) {
        let granted = CGPreflightListenEventAccess();
        let mut tap = self.ivars().tap.borrow_mut();
        if !granted || tap.as_ref().is_some_and(|tap| !tap.is_enabled()) {
            tap.take();
        }
        let mut input_error = None;
        if granted && tap.is_none() && !self.ivars().smoke_test {
            if let Some(worker) = self.ivars().worker.borrow().as_ref() {
                match InputTap::start(worker.sender.clone()) {
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
        let enabled = controls.enabled.load(Ordering::Relaxed);
        self.item(menu, "Звуки включены", Some(sel!(toggle:)), 0, enabled);
        if !granted {
            self.item(
                menu,
                "Разрешить мониторинг ввода…",
                Some(sel!(permission:)),
                0,
                false,
            );
        } else if !listening {
            self.item(
                menu,
                "Повторить подключение ввода…",
                Some(sel!(permission:)),
                0,
                false,
            );
        }
        if let Some(error) = input_error {
            eprintln!("Soundy: {error}");
            self.item(menu, &error, None, 0, false);
        }
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        let keyboard_menu = self.submenu(menu, "Звук клавиатуры");
        for profile in Profile::all() {
            self.item(
                &keyboard_menu,
                profile.name(),
                Some(sel!(selectProfile:)),
                isize::from(profile.0),
                controls.profile.load(Ordering::Relaxed) == profile.0,
            );
        }
        let mouse_menu = self.submenu(menu, "Звук мыши");
        for profile in MouseProfile::all() {
            self.item(
                &mouse_menu,
                &profile.name(),
                Some(sel!(selectMouse:)),
                isize::from(profile.0),
                controls.mouse_profile.load(Ordering::Relaxed) == profile.0,
            );
        }
        self.item(menu, "Послушать мышь", Some(sel!(previewMouse:)), 0, false);
        self.item(menu, "Послушать звук", Some(sel!(preview:)), 0, false);
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        let volume_menu = self.submenu(menu, "Громкость");
        for volume in [0, 10, 25, 50, 75, 100] {
            self.item(
                &volume_menu,
                &format!("{volume}%"),
                Some(sel!(selectVolume:)),
                volume,
                controls.volume.load(Ordering::Relaxed) as isize == volume,
            );
        }
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        self.item(menu, "Завершить Soundy", Some(sel!(quit:)), 0, false);
        if let Some(button) = self
            .ivars()
            .status
            .get()
            .and_then(|status| status.button(self.mtm()))
        {
            button.setTitle(&NSString::from_str(if !granted || !listening {
                " !"
            } else if enabled {
                ""
            } else {
                " Ⅱ"
            }));
            button.setToolTip(Some(&NSString::from_str(if !granted || !listening {
                "Soundy — мониторинг ввода недоступен"
            } else if enabled {
                "Soundy — звуки клавиатуры и мыши"
            } else {
                "Soundy — пауза"
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

pub fn run(root: &Path, smoke_test: bool) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("Soundy должен запускаться в основном потоке")?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let controls = Arc::new(Controls::default());
    let worker = Worker::start(root, controls.clone())?;
    let delegate = Delegate::new(mtm, controls, worker, smoke_test);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
    Ok(())
}

pub fn show_error(error: &str) {
    if let Some(mtm) = MainThreadMarker::new() {
        let alert = NSAlert::new(mtm);
        alert.setMessageText(ns_string!("Не удалось запустить Soundy"));
        alert.setInformativeText(&NSString::from_str(error));
        alert.runModal();
    }
}
