use objc2::{
    MainThreadOnly,
    rc::Retained,
    runtime::{AnyObject, Sel},
};
use objc2_app_kit::*;
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

pub fn amber(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.48, 0.06, alpha)
}

pub fn rounded(frame: NSRect, radius: f64, fill: &NSColor, stroke: &NSColor) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(frame, radius, radius);
    fill.setFill();
    path.fill();
    stroke.setStroke();
    path.setLineWidth(1.0);
    path.stroke();
}

pub fn line(x: f64, y: f64, width: f64) {
    NSColor::colorWithWhite_alpha(1.0, 0.13).setStroke();
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(x, y));
    path.lineToPoint(NSPoint::new(x + width, y));
    path.stroke();
}

pub fn label(view: &NSView, text: &str, frame: NSRect, size: f64) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), view.mtm());
    label.setFrame(frame);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    label.setTextColor(Some(&NSColor::colorWithWhite_alpha(1.0, 0.95)));
    view.addSubview(&label);
    label
}

pub fn symbol(name: &str) -> Retained<NSImage> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    )
    .expect("system symbol on macOS 13");
    image.setTemplate(true);
    image
}

pub fn icon(view: &NSView, name: &str, frame: NSRect) {
    let image = NSImageView::imageViewWithImage(&symbol(name), view.mtm());
    image.setFrame(frame);
    image.setContentTintColor(Some(&NSColor::whiteColor()));
    image.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    view.addSubview(&image);
}

pub fn button(
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

pub fn symbol_button(
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
