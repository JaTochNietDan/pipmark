//! Synthetic running-indicator rendering checks; no Dock access.
#![allow(dead_code)]
#[path = "../src/model.rs"]
mod model;
#[path = "../src/render.rs"]
mod render;
use objc2::AnyThread;
use objc2_app_kit::*;
use objc2_foundation::*;
fn check(mtm: MainThreadMarker) {
    let sheet = NSImage::initWithSize(NSImage::alloc(), NSSize::new(180.0, 110.0));
    #[allow(deprecated)]
    sheet.lockFocus();
    NSColor::whiteColor().setFill();
    NSRectFill(NSRect::new(NSPoint::new(0.0, 0.0), sheet.size()));
    for (column, style) in [
        model::BadgeStyle::Text,
        model::BadgeStyle::Dot,
        model::BadgeStyle::Circle,
    ]
    .into_iter()
    .enumerate()
    {
        let mut item = model::Item {
            id: "test".into(),
            name: "Test".into(),
            enabled: true,
            kind: model::ItemKind::Group { children: vec![] },
            appearance: model::Appearance {
                running_indicator: true,
                badge_style: style,
                ..Default::default()
            },
        };
        for (row, (running, label)) in [(false, "3"), (true, "3"), (true, "")]
            .into_iter()
            .enumerate()
        {
            let state = model::Activity {
                running,
                label: label.into(),
            };
            let image = render::icon(&item, &state, mtm);
            if running {
                assert_eq!(image.size().height, 24.0);
            }
            if label.is_empty() {
                assert_eq!(image.size().width, 18.0, "no phantom unread badge");
            }
            image.drawInRect(NSRect::new(
                NSPoint::new(20.0 + column as f64 * 55.0, 76.0 - row as f64 * 34.0),
                image.size(),
            ));
        }
        item.appearance.notifications = false;
        for size in [16, 18, 22] {
            item.appearance.icon_size = size;
            let active = render::icon(
                &item,
                &model::Activity {
                    running: true,
                    label: "3".into(),
                },
                mtm,
            );
            assert_eq!(active.size(), NSSize::new(f64::from(size), 24.0));
            let stopped = render::icon(
                &item,
                &model::Activity {
                    running: false,
                    label: "3".into(),
                },
                mtm,
            );
            assert_eq!(
                stopped.size(),
                NSSize::new(f64::from(size), f64::from(size))
            );
            item.appearance.running_indicator = false;
            assert_eq!(
                render::icon(
                    &item,
                    &model::Activity {
                        running: true,
                        label: "3".into()
                    },
                    mtm
                )
                .size(),
                stopped.size()
            );
            item.appearance.running_indicator = true;
        }
    }
    #[allow(deprecated)]
    sheet.unlockFocus();
    let bitmap = NSBitmapImageRep::initWithData(
        NSBitmapImageRep::alloc(),
        &sheet.TIFFRepresentation().unwrap(),
    )
    .unwrap();
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    assert!(png.writeToFile_atomically(&NSString::from_str("target/running-preview.png"), true));
    println!(
        "Running/stopped indicator, notification independence, opt-out and all icon sizes passed."
    );
}
fn main() {
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let appearance = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameAqua) }.unwrap();
    appearance.performAsCurrentDrawingAppearance(&block2::StackBlock::new(|| check(mtm)));
}
