//! Exercise the production KVO callback with synthetic unread state, without
//! changing system appearance or reading notifications.
#![allow(dead_code)]
#[path = "../src/archive.rs"]
mod archive;
#[path = "../src/attention.rs"]
mod attention;
#[path = "../src/dock.rs"]
mod dock;
#[path = "../src/icons.rs"]
mod icons;
#[path = "../src/lifecycle.rs"]
mod lifecycle;
#[path = "../src/localization.rs"]
mod localization;
#[path = "../src/login.rs"]
mod login;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/monitor.rs"]
mod monitor;
#[path = "../src/render.rs"]
mod render;
#[path = "../src/storage.rs"]
mod storage;
mod app {
    include!("../src/app.rs");
    pub fn check() {
        let mtm = MainThreadMarker::new().unwrap();
        let _app = NSApplication::sharedApplication(mtm);
        let delegate = Delegate::new(mtm);
        delegate.ivars().settings.borrow_mut().items.push(Item {
            id: "preview".into(),
            name: "Preview".into(),
            enabled: true,
            appearance: crate::model::Appearance {
                badge_style: BadgeStyle::Circle,
                ..Default::default()
            },
            kind: ItemKind::Group { children: vec![] },
        });
        delegate.rebuild_status();
        let button = delegate.ivars().status.borrow()[1].button(mtm).unwrap();
        let unread = Activity {
            running: true,
            label: "3".into(),
        };
        delegate
            .ivars()
            .last_activity
            .borrow_mut()
            .push(unread.clone());
        let mut pixels = Vec::new();
        for (name, file) in unsafe {
            [
                (NSAppearanceNameAqua, "target/appearance-light.png"),
                (NSAppearanceNameDarkAqua, "target/appearance-dark.png"),
            ]
        } {
            button.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
            // setAppearance synchronously invokes the registered production observer.
            let image = button.image().unwrap();
            assert_eq!(
                image.size().width,
                24.0,
                "callback must draw the unread circle"
            );
            let bitmap = NSBitmapImageRep::initWithData(
                NSBitmapImageRep::alloc(),
                &image.TIFFRepresentation().unwrap(),
            )
            .unwrap();
            let png = unsafe {
                bitmap.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &NSDictionary::new(),
                )
            }
            .unwrap();
            assert!(png.writeToFile_atomically(&s(file), true));
            pixels.push(png.to_vec());
        }
        assert_ne!(
            pixels[0], pixels[1],
            "template tint must change with appearance"
        );
        assert_eq!(delegate.ivars().last_activity.borrow()[0], unread);
        delegate.rebuild_status();
        // Retained old buttons must have been unregistered before status removal.
        button.setAppearance(None);
        delegate.remove_appearance_observers();
        for status in delegate.ivars().status.borrow_mut().drain(..) {
            NSStatusBar::systemStatusBar().removeStatusItem(&status);
        }
        println!(
            "Production appearance callback, cached unread redraw, and observer cleanup passed."
        );
    }
}
fn main() {
    app::check();
}
