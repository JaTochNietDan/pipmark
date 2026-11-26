//! Reproducible synthetic badge render sheet; does not read user notifications.
#[path = "../src/icons.rs"]
mod icons;
#[allow(dead_code)]
#[path = "../src/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../src/render.rs"]
mod render;
use objc2::AnyThread;
use objc2_app_kit::*;
use objc2_foundation::*;
fn main() {
    let custom_icon = std::env::var("BADGEIFY_PREVIEW_SVG").ok().map(|path| {
        icons::import(
            std::path::Path::new(&path),
            std::path::Path::new("target/preview-icons"),
        )
        .unwrap()
    });
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(360.0, 100.0));
    #[allow(deprecated)]
    image.lockFocus();
    NSColor::windowBackgroundColor().setFill();
    NSBezierPath::bezierPathWithRect(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(360.0, 100.0),
    ))
    .fill();
    for (row, label) in ["3", "125", "•"].iter().enumerate() {
        for (column, style) in [
            model::BadgeStyle::Text,
            model::BadgeStyle::Dot,
            model::BadgeStyle::Circle,
        ]
        .iter()
        .enumerate()
        {
            let item = model::Item {
                id: "com.apple.finder".into(),
                name: "Finder".into(),
                enabled: true,
                kind: model::ItemKind::App {
                    path: "/System/Library/CoreServices/Finder.app".into(),
                },
                appearance: model::Appearance {
                    badge_style: *style,
                    custom_icon: custom_icon.clone(),
                    ..Default::default()
                },
            };
            let state = model::Activity {
                running: true,
                label: label.to_string(),
            };
            let icon = render::icon(&item, &state, mtm);
            icon.drawInRect(NSRect::new(
                NSPoint::new(20.0 + column as f64 * 110.0, 4.0 + row as f64 * 30.0),
                icon.size(),
            ));
            let text = NSAttributedString::initWithString(
                NSAttributedString::alloc(),
                &NSString::from_str(&render::title(&item, &state)),
            );
            text.drawAtPoint(NSPoint::new(
                45.0 + column as f64 * 110.0,
                10.0 + row as f64 * 30.0,
            ));
        }
    }
    #[allow(deprecated)]
    image.unlockFocus();
    let tiff = image.TIFFRepresentation().expect("TIFF representation");
    let bitmap = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &tiff).unwrap();
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/badge-preview.png".into());
    assert!(png.writeToFile_atomically(&NSString::from_str(&path), true));
}
