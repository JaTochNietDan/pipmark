//! Render synthetic badge-style choices with three highlight colors.
#![allow(dead_code)]
#[path = "../src/model.rs"]
mod model;
#[path = "../src/render.rs"]
mod render;
use objc2::AnyThread;
use objc2_app_kit::*;
use objc2_foundation::*;
fn main() {
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let sheet = NSImage::initWithSize(NSImage::alloc(), NSSize::new(240.0, 108.0));
    let appearance = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameAqua) }.unwrap();
    appearance.performAsCurrentDrawingAppearance(&block2::StackBlock::new(|| {
        #[allow(deprecated)]
        sheet.lockFocus();
        NSColor::whiteColor().setFill();
        NSRectFill(NSRect::new(NSPoint::new(0.0, 0.0), sheet.size()));
        for (row, color) in [[1.0, 0.23, 0.19], [0.0, 0.4, 1.0], [0.15, 0.7, 0.25]]
            .into_iter()
            .enumerate()
        {
            let item = model::Item {
                id: "preview".into(),
                name: "Preview".into(),
                enabled: true,
                appearance: model::Appearance {
                    color,
                    ..Default::default()
                },
                kind: model::ItemKind::Group { children: vec![] },
            };
            for (column, style) in [
                model::BadgeStyle::Text,
                model::BadgeStyle::Dot,
                model::BadgeStyle::Circle,
            ]
            .into_iter()
            .enumerate()
            {
                let preview = render::style_preview(&item, style, mtm);
                assert_eq!(preview.size(), NSSize::new(54.0, 24.0));
                preview.drawInRect(NSRect::new(
                    NSPoint::new(15.0 + column as f64 * 75.0, 76.0 - row as f64 * 32.0),
                    preview.size(),
                ));
            }
        }
        #[allow(deprecated)]
        sheet.unlockFocus();
    }));
    let bitmap = NSBitmapImageRep::initWithData(
        NSBitmapImageRep::alloc(),
        &sheet.TIFFRepresentation().unwrap(),
    )
    .unwrap();
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    assert!(png.writeToFile_atomically(&NSString::from_str("target/style-previews.png"), true));
}
