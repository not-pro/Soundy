use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject},
    sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSCopying, NSData, NSObjectProtocol, NSRect, NSRectEdge, NSSize, NSString,
    ns_string,
};

use crate::{
    consts,
    core::i18n::{Language, Text},
    utils::{amber, button, icon, label, line, rect, rounded, symbol_button},
};

struct GlassState {
    language: Language,
    login: bool,
}

define_class!(
    // SAFETY: All construction, drawing and interaction stay on the AppKit main thread.
    #[unsafe(super = NSWindow)]
    #[thread_kind = MainThreadOnly]
    struct GlassWindow;
    unsafe impl NSObjectProtocol for GlassWindow {}
    impl GlassWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool { true }
        #[unsafe(method(cancelOperation:))]
        fn cancel(&self, _sender: Option<&AnyObject>) { self.close(); }
    }
);

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = GlassState]
    struct Glass;
    unsafe impl NSObjectProtocol for Glass {}
    impl Glass {
        #[unsafe(method(tag))]
        fn content_tag(&self) -> isize { 200 }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let white = NSColor::colorWithWhite_alpha(1.0, 0.22);
            for frame in [rect(8.0, 139.0, 224.0, 72.0), rect(8.0, 85.0, 224.0, 48.0), rect(8.0, 53.0, 224.0, 28.0), rect(8.0, 21.0, 224.0, 28.0)] {
                rounded(frame, 8.0, &NSColor::colorWithWhite_alpha(1.0, 0.055), &white);
            }
            line(9.0, 175.0, 222.0);
            line(8.0, 18.0, 224.0);
            rounded(rect(86.0, 57.0, 138.0, 20.0), 5.0, &NSColor::colorWithWhite_alpha(1.0, 0.04), &white);
            let selected = match self.ivars().language { Language::Russian => 0.0, Language::English => 1.0, Language::Chinese => 2.0 };
            rounded(rect(86.0 + selected * 46.0, 57.0, 46.0, 20.0), 5.0, &amber(0.45), &amber(1.0));
            rounded(rect(193.0, 27.0, 30.0, 17.0), 8.5, &*if self.ivars().login { amber(0.95) } else { NSColor::colorWithWhite_alpha(1.0, 0.18) }, &white);
            rounded(rect(if self.ivars().login { 208.0 } else { 195.0 }, 29.0, 13.0, 13.0), 6.5, &NSColor::whiteColor(), &NSColor::clearColor());
            for y in [183.0, 147.0] {
                rounded(rect(207.0, y, 20.0, 20.0), 10.0, &NSColor::colorWithWhite_alpha(1.0, 0.035), &NSColor::colorWithWhite_alpha(1.0, 0.5));
            }
        }
    }
);

define_class!(
    #[unsafe(super = NSSliderCell)]
    #[thread_kind = MainThreadOnly]
    struct GlassSlider;
    unsafe impl NSObjectProtocol for GlassSlider {}
    impl GlassSlider {
        #[unsafe(method(drawBarInside:flipped:))]
        fn draw_bar(&self, frame: NSRect, _flipped: bool) {
            let bar = rect(frame.origin.x, frame.origin.y + (frame.size.height - 6.0) / 2.0, frame.size.width, 6.0);
            rounded(bar, 3.0, &NSColor::colorWithWhite_alpha(1.0, 0.17), &NSColor::colorWithWhite_alpha(1.0, 0.3));
            let fill = rect(bar.origin.x, bar.origin.y, bar.size.width * self.doubleValue() / 100.0, 6.0);
            if fill.size.width > 0.0 { rounded(fill, 3.0, &amber(1.0), &NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.70, 0.28, 1.0)); }
        }
        #[unsafe(method(drawKnob:))]
        fn draw_knob(&self, frame: NSRect) {
            rounded(frame, frame.size.width / 2.0, &NSColor::colorWithWhite_alpha(0.8, 0.65), &NSColor::whiteColor());
        }
    }
);

pub fn create(mtm: MainThreadMarker) -> Retained<NSWindow> {
    // Keep a standard titled window so macOS owns the shape, clipping and shadow.
    let window = GlassWindow::alloc(mtm).set_ivars(());
    let window: Retained<GlassWindow> = unsafe {
        msg_send![super(window), initWithContentRect: rect(0.0, 0.0, 240.0, 240.0), styleMask: NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::FullSizeContentView, backing: NSBackingStoreType::Buffered, defer: false]
    };
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    window.setTitlebarAppearsTransparent(true);
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
    window.setTitle(ns_string!("Soundy"));
    window.setMovableByWindowBackground(true);
    window.setRestorable(false);
    window.setOpaque(false);
    window.setHasShadow(true);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.center();
    Retained::into_super(window)
}

// Native controls reuse the tray's catalog and action handlers; no second settings model.
pub fn populate(
    window: &NSWindow,
    menu: &NSMenu,
    target: &AnyObject,
    language: Language,
    volume: u8,
    listening: bool,
) {
    let mtm = window.mtm();
    let login = menu
        .itemWithTitle(&NSString::from_str(language.text(Text::LaunchAtLogin)))
        .unwrap();
    let bounds = rect(0.0, 0.0, 240.0, 240.0);
    let view = Glass::alloc(mtm).set_ivars(GlassState {
        language,
        login: login.state() == 1,
    });
    let view: Retained<Glass> = unsafe { msg_send![super(view), initWithFrame: bounds] };
    let glass: Retained<NSView> = if let Some(class) = AnyClass::get(c"NSGlassEffectView") {
        // SAFETY: Available on macOS 26+. The SDK declares NSView inheritance,
        // initWithFrame:, style (NSInteger), and the strong contentView property.
        unsafe {
            let allocated: Allocated<NSView> = msg_send![class, alloc];
            let glass: Retained<NSView> = msg_send![allocated, initWithFrame: bounds];
            let _: () = msg_send![&*glass, setStyle: 1_isize]; // NSGlassEffectViewStyleClear
            let _: () = msg_send![&*glass, setContentView: &*view];
            glass
        }
    } else {
        let glass = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
        glass.setMaterial(NSVisualEffectMaterial::HUDWindow);
        glass.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        glass.setState(NSVisualEffectState::Active);
        glass.addSubview(&view);
        Retained::into_super(glass)
    };
    glass.setAppearance(
        NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
    );
    symbol_button(
        &view,
        "xmark",
        language.text(Text::CloseWindow),
        rect(216.0, 218.0, 16.0, 16.0),
        target,
        sel!(closeSettings:),
    );

    for (group, text, glyph, y) in [
        (0, Text::KeyboardSound, "keyboard", 175.0),
        (1, Text::MouseSound, "computermouse", 139.0),
    ] {
        icon(&view, glyph, rect(16.0, y + 10.0, 18.0, 16.0));
        let choices = menu
            .itemWithTitle(&NSString::from_str(language.text(text)))
            .unwrap()
            .submenu()
            .unwrap()
            .copy();
        let selected = choices.itemArray().iter().find(|item| item.state() == 1);
        for item in choices.itemArray() {
            unsafe {
                item.setAction(None);
                item.setTarget(None);
            }
        }
        let popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            rect(39.0, y + 6.0, 149.0, 24.0),
            false,
        );
        popup.setMenu(Some(&choices));
        popup.selectItem(selected.as_deref());
        popup.setBordered(false);
        if let Some(cell) = popup
            .cell()
            .and_then(|cell| cell.downcast::<NSPopUpButtonCell>().ok())
        {
            cell.setArrowPosition(NSPopUpArrowPosition::NoArrow);
        }
        icon(&view, "chevron.down", rect(188.0, y + 13.0, 8.0, 8.0));
        popup.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        popup.setContentTintColor(Some(&NSColor::whiteColor()));
        popup.setTag(group);
        popup.setAccessibilityLabel(Some(&NSString::from_str(language.text(text))));
        unsafe {
            popup.setTarget(Some(target));
            popup.setAction(Some(sel!(settingsChoice:)));
        }
        view.addSubview(&popup);
        let preview = symbol_button(
            &view,
            "play.fill",
            language.text(Text::Preview),
            rect(207.0, y + 8.0, 20.0, 20.0),
            target,
            sel!(settingsPreview:),
        );
        preview.setTag(group);
    }
    icon(&view, "speaker.wave.2", rect(16.0, 111.0, 15.0, 15.0));
    label(
        &view,
        language.text(Text::Volume),
        rect(38.0, 109.0, 130.0, 17.0),
        11.0,
    );
    let percentage = label(
        &view,
        &format!("{volume}%"),
        rect(180.0, 109.0, 44.0, 17.0),
        11.0,
    );
    percentage.setAlignment(NSTextAlignment::Right);
    percentage.setTag(consts::VOLUME_TAG);
    let slider = NSSlider::initWithFrame(NSSlider::alloc(mtm), rect(14.0, 87.0, 212.0, 22.0));
    let cell = GlassSlider::alloc(mtm).set_ivars(());
    let cell: Retained<GlassSlider> = unsafe { msg_send![super(cell), init] };
    slider.setCell(Some(&cell));
    slider.setMinValue(0.0);
    slider.setMaxValue(100.0);
    slider.setDoubleValue(volume.into());
    slider.setContinuous(true);
    slider.setAccessibilityLabel(Some(&NSString::from_str(language.text(Text::Volume))));
    unsafe {
        slider.setTarget(Some(target));
        slider.setAction(Some(sel!(settingsVolume:)));
    }
    view.addSubview(&slider);
    icon(&view, "globe", rect(16.0, 59.0, 16.0, 16.0));
    label(
        &view,
        language.text(Text::LanguageMenu),
        rect(38.0, 58.0, 46.0, 17.0),
        11.0,
    );
    for (index, (title, option)) in [
        ("RU", Language::Russian),
        ("EN", Language::English),
        ("中文", Language::Chinese),
    ]
    .into_iter()
    .enumerate()
    {
        let control = button(
            &view,
            title,
            rect(86.0 + index as f64 * 46.0, 57.0, 46.0, 20.0),
            target,
            sel!(settingsLanguage:),
        );
        control.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        control.setTag(option as isize);
        control.setAccessibilityLabel(Some(&NSString::from_str(option.name())));
        control.setButtonType(NSButtonType::PushOnPushOff);
        control.setState(if language == option { 1 } else { 0 });
    }
    icon(&view, "power", rect(16.0, 27.0, 16.0, 16.0));
    label(
        &view,
        language.text(Text::LaunchAtLogin),
        rect(38.0, 26.0, 152.0, 17.0),
        11.0,
    );
    let toggle = button(
        &view,
        "",
        rect(187.0, 23.0, 42.0, 24.0),
        target,
        sel!(settingsAction:),
    );
    toggle.setTag(menu.indexOfItem(&login));
    toggle.setAccessibilityRole(Some(ns_string!("AXCheckBox")));
    toggle.setAccessibilityLabel(Some(&login.title()));
    toggle.setButtonType(NSButtonType::PushOnPushOff);
    toggle.setState(login.state());
    let dot = label(&view, "●", rect(10.0, 1.0, 12.0, 16.0), 9.0);
    dot.setTag(consts::STATUS_DOT_TAG);
    dot.setHidden(!listening);
    let status = label(&view, "", rect(25.0, 0.0, 182.0, 17.0), 10.0);
    status.setTag(consts::STATUS_TAG);
    if !listening {
        if let Some(item) = menu
            .itemArray()
            .iter()
            .find(|item| item.action() == Some(sel!(permission:)))
        {
            let action = button(
                &view,
                language.text(Text::InputNeeded),
                rect(10.0, 0.0, 193.0, 18.0),
                target,
                sel!(settingsAction:),
            );
            action.setFont(Some(&NSFont::systemFontOfSize(10.0)));
            action.setToolTip(Some(&item.title()));
            action.setTag(menu.indexOfItem(&item));
            status.setHidden(true);
        }
    }
    symbol_button(
        &view,
        "ellipsis",
        language.text(Text::About),
        rect(211.0, 0.0, 22.0, 18.0),
        target,
        sel!(showAbout:),
    );
    window.setContentView(Some(&glass));
    sync_volume(window, language, volume);
}

pub fn content(window: &NSWindow) -> Retained<NSView> {
    window
        .contentView()
        .unwrap()
        .viewWithTag(200)
        .expect("settings content")
}

// Updating the existing labels keeps the native slider tracking during a drag.
pub fn sync_volume(window: &NSWindow, language: Language, volume: u8) {
    let root = content(window);
    for view in root.subviews() {
        if let Ok(label) = view.downcast::<NSTextField>() {
            if label.tag() == consts::VOLUME_TAG {
                label.setStringValue(&NSString::from_str(&format!("{volume}%")));
            }
            if label.tag() == consts::STATUS_TAG {
                label.setStringValue(&NSString::from_str(language.text(if volume == 0 {
                    Text::SoundMuted
                } else {
                    Text::SoundEnabled
                })));
            }
            if label.tag() == consts::STATUS_DOT_TAG {
                label.setTextColor(Some(&*if volume == 0 {
                    NSColor::colorWithWhite_alpha(1.0, 0.6)
                } else {
                    NSColor::colorWithSRGBRed_green_blue_alpha(0.45, 0.88, 0.38, 1.0)
                }));
            }
        }
    }
}

pub fn about(
    sender: &NSButton,
    target: &AnyObject,
    menu: &NSMenu,
    language: Language,
) -> Retained<NSPopover> {
    let mtm = sender.mtm();
    let extra: Vec<_> = menu.itemArray().iter().enumerate().filter(|(_, item)| matches!(item.action(), Some(action) if action == sel!(approveLogin:) || action == sel!(installUpdate:))).collect();
    let height = 112.0 + extra.len() as f64 * 24.0;
    let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 180.0, height));
    label(
        &view,
        &format!(
            "{} {}",
            language.text(Text::Version),
            env!("CARGO_PKG_VERSION")
        ),
        rect(12.0, height - 26.0, 156.0, 16.0),
        10.0,
    );
    for (index, (title, data)) in [
        (
            "GitHub · not-pro",
            include_bytes!("../../assets/icons/github.pdf").as_slice(),
        ),
        (
            "Telegram · @nbautin",
            include_bytes!("../../assets/icons/telegram.pdf").as_slice(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let link = button(
            &view,
            title,
            rect(12.0, height - 54.0 - index as f64 * 24.0, 156.0, 22.0),
            target,
            sel!(openSocial:),
        );
        link.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        link.setTag(index as isize);
        let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(data))
            .expect("bundled social icon");
        image.setSize(NSSize::new(14.0, 14.0));
        image.setTemplate(true);
        link.setImage(Some(&image));
        link.setImagePosition(NSCellImagePosition::ImageLeft);
        link.setAlignment(NSTextAlignment::Left);
    }
    for (position, (index, item)) in extra.into_iter().enumerate() {
        let action = button(
            &view,
            &item.title().to_string(),
            rect(12.0, height - 102.0 - position as f64 * 24.0, 156.0, 22.0),
            target,
            sel!(settingsAction:),
        );
        action.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        action.setToolTip(Some(&item.title()));
        action.setTag(index as isize);
    }
    let quit = button(
        &view,
        language.text(Text::Quit),
        rect(12.0, 8.0, 156.0, 20.0),
        target,
        sel!(settingsAction:),
    );
    quit.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    quit.setTag(menu.numberOfItems() - 1);
    let controller = NSViewController::new(mtm);
    controller.setView(&view);
    let popover = NSPopover::new(mtm);
    popover.setAppearance(
        NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
    );
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setAnimates(false);
    popover.setContentViewController(Some(&controller));
    popover.setContentSize(NSSize::new(180.0, height));
    popover.showRelativeToRect_ofView_preferredEdge(sender.bounds(), sender, NSRectEdge::MaxY);
    popover
}
