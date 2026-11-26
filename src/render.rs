//! Render only when source activity or preferences change; no frame loop.
use crate::model::{Activity, BadgeStyle, Item};
use objc2::{AnyThread, rc::Retained, runtime::AnyObject};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSAttributedString, NSDictionary, NSPoint, NSRect, NSSize, NSString,
};

/// Compact native menu annotation, preserving each item's count privacy.
pub fn menu_title(item: &Item, state: &Activity) -> String {
    if !item.appearance.notifications || !state.has_unread() {
        return item.name.clone();
    }
    if item.appearance.show_count && item.appearance.badge_style != BadgeStyle::Dot {
        format!("{} ({})", item.name, state.label)
    } else {
        format!("{}  •", item.name)
    }
}
pub fn title(item: &Item, state: &Activity) -> String {
    if !item.appearance.notifications
        || !state.has_unread()
        || item.appearance.badge_style != BadgeStyle::Text
        || !item.appearance.show_count
    {
        return String::new();
    }
    format!(" {}", state.label)
}
pub fn circle_count(state: &Activity) -> String {
    let count = state.count();
    if count == 0 {
        String::new()
    } else {
        count.min(99).to_string()
    }
}
/// Resolve dynamic colors in the receiving menu bar button's appearance.
pub fn button_icon(
    item: &Item,
    state: &Activity,
    button: &NSStatusBarButton,
    mtm: MainThreadMarker,
) -> Retained<NSImage> {
    let output = std::cell::RefCell::new(None);
    button
        .effectiveAppearance()
        .performAsCurrentDrawingAppearance(&block2::StackBlock::new(|| {
            *output.borrow_mut() = Some(icon(item, state, mtm));
        }));
    output
        .into_inner()
        .expect("synchronous AppKit drawing block")
}
/// Synthetic menu preview; never embeds a real unread count.
pub fn style_preview(item: &Item, style: BadgeStyle, mtm: MainThreadMarker) -> Retained<NSImage> {
    let mut sample = item.clone();
    sample.appearance.badge_style = style;
    sample.appearance.show_count = true;
    sample.appearance.notifications = true;
    sample.appearance.running_indicator = false;
    sample.appearance.icon_size = 18;
    let state = Activity {
        running: true,
        label: "12".into(),
    };
    let icon = icon(&sample, &state, mtm);
    let preview = NSImage::initWithSize(NSImage::alloc(), NSSize::new(54.0, 24.0));
    #[allow(deprecated)]
    preview.lockFocus();
    icon.drawInRect(NSRect::new(
        NSPoint::new(2.0, (24.0 - icon.size().height) / 2.0),
        icon.size(),
    ));
    if icon.isTemplate() {
        NSColor::labelColor().setFill();
        NSRectFillUsingOperation(
            NSRect::new(NSPoint::new(0.0, 0.0), preview.size()),
            NSCompositingOperation::SourceAtop,
        );
    }
    if style == BadgeStyle::Text {
        let font = NSFont::systemFontOfSize(12.0);
        let color = NSColor::labelColor();
        let attrs = unsafe {
            NSDictionary::from_slices(
                &[NSFontAttributeName, NSForegroundColorAttributeName],
                &[&*font as &AnyObject, &*color as &AnyObject],
            )
        };
        let text = unsafe {
            NSAttributedString::initWithString_attributes(
                NSAttributedString::alloc(),
                &NSString::from_str("12"),
                Some(&attrs),
            )
        };
        text.drawAtPoint(NSPoint::new(25.0, (24.0 - text.size().height) / 2.0));
    }
    #[allow(deprecated)]
    preview.unlockFocus();
    preview
}
pub fn icon(item: &Item, state: &Activity, mtm: MainThreadMarker) -> Retained<NSImage> {
    let size = f64::from(item.appearance.icon_size);
    let base = item
        .appearance
        .custom_icon
        .as_ref()
        .and_then(|path| {
            let image =
                NSImage::initWithContentsOfFile(NSImage::alloc(), &NSString::from_str(path))?;
            image.setTemplate(
                std::path::Path::new(path)
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("template-")),
            );
            Some(image)
        })
        .unwrap_or_else(|| match &item.kind {
            crate::model::ItemKind::App { path } => {
                NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path))
            }
            _ => NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &NSString::from_str(if matches!(item.kind, crate::model::ItemKind::Trash) {
                    "trash"
                } else {
                    "folder"
                }),
                None,
            )
            .expect("system folder symbol"),
        });
    // Workspace and symbol images may be shared with other status items.
    let base: Retained<NSImage> = unsafe { objc2::msg_send![&*base, copy] };
    base.setSize(NSSize::new(size, size));
    let badge = item.appearance.notifications
        && state.has_unread()
        && item.appearance.badge_style != BadgeStyle::Text;
    let running = item.appearance.running_indicator && state.running;
    if !badge && !running {
        return base;
    }
    let circle = badge && item.appearance.badge_style == BadgeStyle::Circle;
    let count = if circle && item.appearance.show_count {
        circle_count(state)
    } else {
        String::new()
    };
    let diameter = if !circle {
        6.0
    } else if count.len() > 1 {
        16.0
    } else {
        12.0
    };
    let width = size
        + if !badge {
            0.0
        } else if circle {
            6.0
        } else {
            2.0
        };
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(width, 24.0));
    // This drawing occurs on AppKit's main thread only. Focus is always balanced.
    #[allow(deprecated)]
    image.lockFocus();
    base.drawInRect(NSRect::new(
        NSPoint::new(0.0, (24.0 - size) / 2.0 + if running { 1.0 } else { 0.0 }),
        NSSize::new(size, size),
    ));
    if base.isTemplate() {
        NSColor::labelColor().setFill();
        NSRectFillUsingOperation(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, 24.0)),
            NSCompositingOperation::SourceAtop,
        );
    }
    if badge {
        let [r, g, b] = item.appearance.color;
        NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0).setFill();
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(width - diameter, 24.0 - diameter),
            NSSize::new(diameter, diameter),
        ))
        .fill();
        if !count.is_empty() {
            let font = NSFont::boldSystemFontOfSize(9.0);
            let color = NSColor::whiteColor();
            let attrs = unsafe {
                NSDictionary::from_slices(
                    &[NSFontAttributeName, NSForegroundColorAttributeName],
                    &[&*font as &AnyObject, &*color as &AnyObject],
                )
            };
            let text = unsafe {
                NSAttributedString::initWithString_attributes(
                    NSAttributedString::alloc(),
                    &NSString::from_str(&count),
                    Some(&attrs),
                )
            };
            let bounds = text.size();
            text.drawAtPoint(NSPoint::new(
                width - diameter + (diameter - bounds.width) / 2.0,
                24.0 - diameter + (diameter - bounds.height) / 2.0,
            ));
        }
    }
    if running {
        NSColor::labelColor().setFill();
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(size / 2.0 - 1.0, 0.0),
            NSSize::new(2.0, 2.0),
        ))
        .fill();
    }
    #[allow(deprecated)]
    image.unlockFocus();
    let _ = mtm; // Marker makes the main-thread drawing requirement explicit.
    image
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circle_caps_counts_and_omits_non_numeric_labels() {
        assert_eq!(
            circle_count(&Activity {
                running: true,
                label: "125".into()
            }),
            "99"
        );
        assert_eq!(
            circle_count(&Activity {
                running: true,
                label: "•".into()
            }),
            ""
        );
        assert_eq!(
            circle_count(&Activity {
                running: true,
                label: "3".into()
            }),
            "3"
        );
    }
}
