//! Trash item integration check. Requires a new disposable BADGEIFY_CONFIG path.
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
        use objc2_foundation::{NSArray, NSURL};
        let path = storage::config_path();
        assert!(std::env::var_os("BADGEIFY_CONFIG").is_some());
        assert!(!path.exists(), "use a fresh disposable config");
        let fixture = path.with_extension("fixture.app");
        assert!(!fixture.exists());
        std::fs::create_dir_all(fixture.join("Contents")).unwrap();
        std::fs::write(fixture.join("Contents/Info.plist"), br#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>test.badgeify.drop</string><key>CFBundleDisplayName</key><string>Visible App Name</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#).unwrap();
        let url = NSURL::fileURLWithPath(&s(&fixture.to_string_lossy()));
        let mtm = MainThreadMarker::new().unwrap();
        let _app = NSApplication::sharedApplication(mtm);
        let board = NSPasteboard::pasteboardWithUniqueName();
        let first = NSPasteboardItem::new();
        first.setString_forType(&url.absoluteString().unwrap(), unsafe {
            NSPasteboardTypeFileURL
        });
        let second = NSPasteboardItem::new();
        second.setString_forType(&s("https://example.com/remote.app"), unsafe {
            NSPasteboardTypeFileURL
        });
        let objects: [&ProtocolObject<dyn NSPasteboardWriting>; 2] = [
            ProtocolObject::from_ref(&*first),
            ProtocolObject::from_ref(&*second),
        ];
        assert!(board.writeObjects(&NSArray::from_slice(&objects)));
        let urls = dropped_urls(&board);
        assert_eq!(urls.len(), 2);
        assert!(application_details(&urls[1]).is_none());
        assert!(application_details(&NSURL::fileURLWithPath(&s("/tmp/not-an-app.txt"))).is_none());
        let delegate = Delegate::new(mtm);
        let drop_view = ApplicationDropView::new(&delegate, rect(0.0, 0.0, 680.0, 400.0));
        assert!(drop_view.ivars().delegate.load().is_some());
        let urls = NSArray::from_retained_slice(&urls);
        assert!(delegate.add_application_urls(&urls));
        let saved = storage::read(&path).unwrap();
        assert_eq!(saved.items.len(), 1);
        assert_eq!(saved.items[0].name, "Visible App Name");
        let bytes = std::fs::read(&path).unwrap();
        assert!(!delegate.add_application_urls(&urls));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        delegate.suspend_monitor();
        delegate.remove_appearance_observers();
        drop(delegate);
        assert!(drop_view.ivars().delegate.load().is_none());
        println!(
            "Private file pasteboard, mixed URL filtering, bundle display name, persistence, duplicates, and weak drop-view ownership passed."
        );
    }
}
fn main() {
    app::check();
}
