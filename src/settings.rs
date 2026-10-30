use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject, Sel},
    sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSCopying, NSData, NSObjectProtocol, NSPoint, NSRect, NSRectEdge, NSSize,
    NSString, ns_string,
};

use crate::i18n::{Language, Text};

pub const GITHUB_URL: &str = "https://github.com/not-pro";
pub const TELEGRAM_URL: &str = "https://nbautin.t.me";
pub const VOLUME_TAG: isize = 100;
pub const STATUS_TAG: isize = 101;
pub const STATUS_DOT_TAG: isize = 102;

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn amber(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.48, 0.06, alpha)
}

fn rounded(frame: NSRect, radius: f64, fill: &NSColor, stroke: &NSColor) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(frame, radius, radius);
    fill.setFill();
    path.fill();
    stroke.setStroke();
    path.setLineWidth(1.0);
    path.stroke();
}

fn line(x: f64, y: f64, width: f64) {
    NSColor::colorWithWhite_alpha(1.0, 0.13).setStroke();
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(x, y));
    path.lineToPoint(NSPoint::new(x + width, y));
    path.stroke();
}

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
            for frame in [rect(22.0, 294.0, 436.0, 158.0), rect(22.0, 200.0, 436.0, 84.0), rect(22.0, 132.0, 436.0, 58.0), rect(22.0, 64.0, 436.0, 58.0)] {
                rounded(frame, 14.0, &NSColor::colorWithWhite_alpha(1.0, 0.055), &white);
            }
            line(23.0, 373.0, 434.0);
            line(22.0, 50.0, 436.0);
            rounded(rect(262.0, 143.0, 178.0, 36.0), 9.0, &NSColor::colorWithWhite_alpha(1.0, 0.04), &white);
            let selected = match self.ivars().language { Language::Russian => 0.0, Language::English => 1.0, Language::Chinese => 2.0 };
            rounded(rect(262.0 + selected * 59.0, 143.0, 60.0, 36.0), 9.0, &amber(0.45), &amber(1.0));
            rounded(rect(388.0, 81.0, 48.0, 25.0), 12.5, &*if self.ivars().login { amber(0.95) } else { NSColor::colorWithWhite_alpha(1.0, 0.18) }, &white);
            rounded(rect(if self.ivars().login { 413.0 } else { 390.0 }, 83.0, 21.0, 21.0), 10.5, &NSColor::whiteColor(), &NSColor::clearColor());
            for y in [392.0, 313.0] {
                rounded(rect(398.0, y, 37.0, 37.0), 18.5, &NSColor::colorWithWhite_alpha(1.0, 0.035), &NSColor::colorWithWhite_alpha(1.0, 0.5));
            }
            rounded(rect(426.0, 13.0, 28.0, 28.0), 14.0, &NSColor::colorWithWhite_alpha(1.0, 0.08), &NSColor::colorWithWhite_alpha(1.0, 0.4));
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
        msg_send![super(window), initWithContentRect: rect(0.0, 0.0, 480.0, 558.0), styleMask: NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::FullSizeContentView, backing: NSBackingStoreType::Buffered, defer: false]
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

fn label(view: &NSView, text: &str, frame: NSRect, size: f64) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), view.mtm());
    label.setFrame(frame);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    label.setTextColor(Some(&NSColor::colorWithWhite_alpha(1.0, 0.95)));
    view.addSubview(&label);
    label
}

fn symbol(name: &str) -> Retained<NSImage> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    )
    .expect("system symbol on macOS 13");
    image.setTemplate(true);
    image
}

fn icon(view: &NSView, name: &str, frame: NSRect) {
    let image = NSImageView::imageViewWithImage(&symbol(name), view.mtm());
    image.setFrame(frame);
    image.setContentTintColor(Some(&NSColor::whiteColor()));
    image.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    view.addSubview(&image);
}

fn button(
    view: &NSView,
    title: &str,
    frame: NSRect,
    target: &AnyObject,
    action: Sel,
) -> Retained<NSButton> {
    // SAFETY: The delegate stays alive and implements every supplied action.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(action),
            view.mtm(),
        )
    };
    button.setFrame(frame);
    button.setBordered(false);
    button.setFont(Some(&NSFont::systemFontOfSize(14.0)));
    button.setContentTintColor(Some(&NSColor::whiteColor()));
    view.addSubview(&button);
    button
}

fn symbol_button(
    view: &NSView,
    name: &str,
    title: &str,
    frame: NSRect,
    target: &AnyObject,
    action: Sel,
) -> Retained<NSButton> {
    let button = button(view, "", frame, target, action);
    button.setImage(Some(&symbol(name)));
    button.setImagePosition(NSCellImagePosition::ImageOnly);
    button.setAccessibilityLabel(Some(&NSString::from_str(title)));
    button.setToolTip(Some(&NSString::from_str(title)));
    button
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
    let bounds = rect(0.0, 0.0, 480.0, 558.0);
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
    let image = NSImage::initWithData(
        NSImage::alloc(),
        &NSData::with_bytes(include_bytes!("../assets/icons/soundy.png")),
    )
    .expect("app icon");
    let app_icon = NSImageView::imageViewWithImage(&image, mtm);
    app_icon.setFrame(rect(24.0, 466.0, 76.0, 76.0));
    view.addSubview(&app_icon);
    let title = label(&view, "Soundy", rect(110.0, 480.0, 270.0, 46.0), 32.0);
    title.setFont(Some(&NSFont::boldSystemFontOfSize(32.0)));
    symbol_button(
        &view,
        "xmark",
        language.text(Text::CloseWindow),
        rect(429.0, 510.0, 25.0, 25.0),
        target,
        sel!(closeSettings:),
    );

    for (group, text, heading, glyph, y) in [
        (0, Text::KeyboardSound, Text::Keyboard, "keyboard", 373.0),
        (1, Text::MouseSound, Text::Mouse, "computermouse", 294.0),
    ] {
        icon(&view, glyph, rect(48.0, y + 27.0, 32.0, 30.0));
        let caption = label(
            &view,
            language.text(heading),
            rect(112.0, y + 44.0, 260.0, 22.0),
            14.0,
        );
        caption.setTextColor(Some(&NSColor::colorWithWhite_alpha(1.0, 0.6)));
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
            rect(108.0, y + 15.0, 270.0, 30.0),
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
        icon(&view, "chevron.down", rect(363.0, y + 22.0, 12.0, 12.0));
        popup.setFont(Some(&NSFont::systemFontOfSize(18.0)));
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
            rect(399.0, y + 20.0, 35.0, 35.0),
            target,
            sel!(settingsPreview:),
        );
        preview.setTag(group);
    }
    icon(&view, "speaker.wave.2", rect(46.0, 246.0, 25.0, 25.0));
    label(
        &view,
        language.text(Text::Volume),
        rect(88.0, 245.0, 240.0, 25.0),
        15.0,
    );
    let percentage = label(
        &view,
        &format!("{volume}%"),
        rect(367.0, 245.0, 70.0, 25.0),
        15.0,
    );
    percentage.setAlignment(NSTextAlignment::Right);
    percentage.setTag(VOLUME_TAG);
    let slider = NSSlider::initWithFrame(NSSlider::alloc(mtm), rect(45.0, 213.0, 392.0, 26.0));
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
    icon(&view, "globe", rect(47.0, 149.0, 25.0, 25.0));
    label(
        &view,
        language.text(Text::LanguageMenu),
        rect(88.0, 148.0, 170.0, 25.0),
        15.0,
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
            rect(262.0 + index as f64 * 59.0, 143.0, 60.0, 36.0),
            target,
            sel!(settingsLanguage:),
        );
        control.setTag(option as isize);
        control.setAccessibilityLabel(Some(&NSString::from_str(option.name())));
        control.setButtonType(NSButtonType::PushOnPushOff);
        control.setState(if language == option { 1 } else { 0 });
    }
    icon(&view, "power", rect(47.0, 81.0, 25.0, 25.0));
    label(
        &view,
        language.text(Text::LaunchAtLogin),
        rect(88.0, 80.0, 292.0, 25.0),
        15.0,
    );
    let toggle = button(
        &view,
        "",
        rect(383.0, 76.0, 58.0, 35.0),
        target,
        sel!(settingsAction:),
    );
    toggle.setTag(menu.indexOfItem(&login));
    toggle.setAccessibilityRole(Some(ns_string!("AXCheckBox")));
    toggle.setAccessibilityLabel(Some(&login.title()));
    toggle.setButtonType(NSButtonType::PushOnPushOff);
    toggle.setState(login.state());
    let dot = label(&view, "●", rect(29.0, 16.0, 18.0, 22.0), 12.0);
    dot.setTag(STATUS_DOT_TAG);
    dot.setHidden(!listening);
    let status = label(&view, "", rect(51.0, 14.0, 360.0, 26.0), 14.0);
    status.setTag(STATUS_TAG);
    if !listening {
        if let Some(item) = menu
            .itemArray()
            .iter()
            .find(|item| item.action() == Some(sel!(permission:)))
        {
            let action = button(
                &view,
                &item.title().to_string(),
                rect(29.0, 13.0, 382.0, 29.0),
                target,
                sel!(settingsAction:),
            );
            action.setTag(menu.indexOfItem(&item));
            status.setHidden(true);
        }
    }
    symbol_button(
        &view,
        "ellipsis",
        language.text(Text::About),
        rect(426.0, 13.0, 28.0, 28.0),
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
            if label.tag() == VOLUME_TAG {
                label.setStringValue(&NSString::from_str(&format!("{volume}%")));
            }
            if label.tag() == STATUS_TAG {
                label.setStringValue(&NSString::from_str(language.text(if volume == 0 {
                    Text::SoundMuted
                } else {
                    Text::SoundEnabled
                })));
            }
            if label.tag() == STATUS_DOT_TAG {
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
    let height = 188.0 + extra.len() as f64 * 34.0;
    let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 280.0, height));
    let title = label(
        &view,
        "Soundy",
        rect(20.0, height - 40.0, 240.0, 25.0),
        19.0,
    );
    title.setFont(Some(&NSFont::boldSystemFontOfSize(19.0)));
    label(
        &view,
        &format!(
            "{} {}",
            language.text(Text::Version),
            env!("CARGO_PKG_VERSION")
        ),
        rect(20.0, height - 63.0, 240.0, 22.0),
        12.0,
    );
    for (index, (title, data)) in [
        (
            "GitHub · not-pro",
            include_bytes!("../assets/icons/github.pdf").as_slice(),
        ),
        (
            "Telegram · @nbautin",
            include_bytes!("../assets/icons/telegram.pdf").as_slice(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let link = button(
            &view,
            title,
            rect(20.0, height - 104.0 - index as f64 * 36.0, 240.0, 30.0),
            target,
            sel!(openSocial:),
        );
        link.setTag(index as isize);
        let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(data))
            .expect("bundled social icon");
        image.setSize(NSSize::new(19.0, 19.0));
        image.setTemplate(true);
        link.setImage(Some(&image));
        link.setImagePosition(NSCellImagePosition::ImageLeft);
        link.setAlignment(NSTextAlignment::Left);
    }
    for (position, (index, item)) in extra.into_iter().enumerate() {
        let action = button(
            &view,
            &item.title().to_string(),
            rect(20.0, height - 174.0 - position as f64 * 34.0, 240.0, 30.0),
            target,
            sel!(settingsAction:),
        );
        action.setTag(index as isize);
    }
    let quit = button(
        &view,
        language.text(Text::Quit),
        rect(20.0, 10.0, 240.0, 28.0),
        target,
        sel!(settingsAction:),
    );
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
    popover.setContentSize(NSSize::new(280.0, height));
    popover.showRelativeToRect_ofView_preferredEdge(sender.bounds(), sender, NSRectEdge::MaxY);
    popover
}
