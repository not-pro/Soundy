use objc2::{
    AnyThread, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::AnyObject, sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSCopying, NSData, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
    ns_string,
};

use crate::i18n::{Language, Text};

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn amber(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.43, 0.08, alpha)
}

define_class!(
    // SAFETY: Drawing and view construction are confined to the AppKit main thread.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    struct OrangeGlass;
    unsafe impl NSObjectProtocol for OrangeGlass {}
    impl OrangeGlass {
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let gradient = NSGradient::initWithStartingColor_endingColor(
                NSGradient::alloc(),
                &NSColor::colorWithSRGBRed_green_blue_alpha(0.25, 0.07, 0.02, 0.92),
                &NSColor::colorWithSRGBRed_green_blue_alpha(0.80, 0.30, 0.06, 0.63),
            ).expect("two valid gradient colors");
            gradient.drawInRect_angle(self.bounds(), 65.0);
            for frame in [rect(20.0, 252.0, 440.0, 238.0), rect(20.0, 50.0, 440.0, 186.0)] {
                let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(frame, 18.0, 18.0);
                NSColor::colorWithWhite_alpha(1.0, 0.08).setFill();
                path.fill();
                amber(0.5).setStroke();
                path.setLineWidth(1.0);
                path.stroke();
            }
        }
    }
);

pub fn create(mtm: MainThreadMarker) -> Retained<NSWindow> {
    // SAFETY: The retained window is owned by State; AppKit must not release it again.
    let window = unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect(0.0, 0.0, 480.0, 650.0),
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::FullSizeContentView,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        window
    };
    window.setTitle(ns_string!("Soundy"));
    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    window.setTitlebarAppearsTransparent(true);
    window.setMovableByWindowBackground(true);
    window.setRestorable(false);
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.center();
    window
}

fn label(view: &NSView, text: &str, frame: NSRect, size: f64) {
    let label = NSTextField::labelWithString(&NSString::from_str(text), view.mtm());
    label.setFrame(frame);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    label.setTextColor(Some(&NSColor::colorWithWhite_alpha(1.0, 0.95)));
    view.addSubview(&label);
}

// Build only on demand. Menu copies reuse the catalog, translations and saved
// selections; every action goes through the same handlers as the tray menu.
pub fn populate(window: &NSWindow, menu: &NSMenu, target: &AnyObject, language: Language) {
    let mtm = window.mtm();
    let bounds = rect(0.0, 0.0, 480.0, 650.0);
    let glass = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
    glass.setMaterial(NSVisualEffectMaterial::HUDWindow);
    glass.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    glass.setState(NSVisualEffectState::Active);
    glass.setAppearance(
        NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
    );
    let view = OrangeGlass::alloc(mtm).set_ivars(());
    let view: Retained<OrangeGlass> = unsafe { msg_send![super(view), initWithFrame: bounds] };
    glass.addSubview(&view);
    let image = NSImage::initWithData(
        NSImage::alloc(),
        &NSData::with_bytes(include_bytes!("../assets/icons/soundy.png")),
    )
    .expect("app icon");
    let icon = NSImageView::imageViewWithImage(&image, mtm);
    icon.setFrame(rect(26.0, 514.0, 102.0, 102.0));
    view.addSubview(&icon);
    label(&view, "Soundy", rect(144.0, 558.0, 290.0, 44.0), 34.0);
    label(
        &view,
        &format!(
            "{} · {}",
            language.text(Text::OpenSettings).trim_end_matches('…'),
            env!("CARGO_PKG_VERSION")
        ),
        rect(146.0, 530.0, 290.0, 22.0),
        12.0,
    );

    for (group, text) in [
        Text::KeyboardSound,
        Text::MouseSound,
        Text::Volume,
        Text::LanguageMenu,
    ]
    .into_iter()
    .enumerate()
    {
        let source = menu
            .itemWithTitle(&NSString::from_str(language.text(text)))
            .unwrap()
            .submenu()
            .unwrap();
        let choices = source.copy();
        let selected = choices.itemArray().iter().find(|item| item.state() == 1);
        for item in choices.itemArray() {
            // Control action dispatches the selected NSMenuItem to the shared handler.
            unsafe {
                item.setAction(None);
                item.setTarget(None);
            }
        }
        let y = 440.0 - group as f64 * 54.0;
        label(&view, language.text(text), rect(36.0, y, 170.0, 22.0), 13.0);
        let popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            rect(208.0, y - 3.0, 238.0, 28.0),
            false,
        );
        popup.setMenu(Some(&choices));
        popup.selectItem(selected.as_deref());
        popup.setTag(group as isize);
        popup.setAccessibilityLabel(Some(&NSString::from_str(language.text(text))));
        unsafe {
            popup.setTarget(Some(target));
            popup.setAction(Some(sel!(settingsChoice:)));
        }
        view.addSubview(&popup);
    }
    let mut y = 198.0;
    for (index, item) in menu.itemArray().iter().enumerate() {
        let Some(action) = item.action() else {
            continue;
        };
        if action == sel!(openSettings:) || item.hasSubmenu() {
            continue;
        }
        // SAFETY: target implements settingsAction: with the NSButton sender signature.
        let button = unsafe {
            if action == sel!(toggleLogin:) {
                NSButton::checkboxWithTitle_target_action(
                    &item.title(),
                    Some(target),
                    Some(sel!(settingsAction:)),
                    mtm,
                )
            } else {
                NSButton::buttonWithTitle_target_action(
                    &item.title(),
                    Some(target),
                    Some(sel!(settingsAction:)),
                    mtm,
                )
            }
        };
        button.setFrame(rect(36.0, y, 408.0, 28.0));
        button.setTag(index as isize);
        button.setState(item.state());
        button.setBezelColor(Some(&amber(0.8)));
        view.addSubview(&button);
        y -= 34.0;
    }
    label(
        &view,
        language.text(Text::WindowHint),
        rect(26.0, 18.0, 432.0, 20.0),
        11.0,
    );
    window.setContentView(Some(&glass));
}
