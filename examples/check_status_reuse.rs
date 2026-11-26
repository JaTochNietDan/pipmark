//! Native worker-to-AppKit delivery check with synthetic activity; no Dock access.
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
        let group = |id: &str| Item {
            id: id.into(),
            name: id.into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::Group { children: vec![] },
        };
        delegate.ivars().settings.borrow_mut().items = vec![group("first"), group("second")];
        delegate.rebuild_status();
        let original = delegate.ivars().status.borrow().clone();
        let first_button = original[1].button(mtm).unwrap();
        first_button.setTitle(&s(" 9"));
        first_button.setAlphaValue(0.35);
        {
            let mut settings = delegate.ivars().settings.borrow_mut();
            settings.items.swap(0, 1);
            settings.items[1].name = "Renamed first".into();
            settings.items[1].enabled = false;
            settings.language = crate::model::Language::German;
        }
        delegate.rebuild_status();
        {
            let updated = delegate.ivars().status.borrow();
            assert!(std::ptr::eq(&*original[0], &*updated[0]));
            assert!(std::ptr::eq(&*original[2], &*updated[1]));
            assert!(std::ptr::eq(&*original[1], &*updated[2]));
            assert_eq!(first_button.tag(), 1);
            assert!(first_button.title().is_empty());
            assert_eq!(first_button.alphaValue(), 1.0);
            assert!(!updated[2].isVisible());
            assert_eq!(updated[2].autosaveName().to_string(), "first");
        }
        delegate.ivars().settings.borrow_mut().items.remove(1);
        delegate
            .ivars()
            .settings
            .borrow_mut()
            .items
            .push(group("third"));
        delegate.rebuild_status();
        {
            let updated = delegate.ivars().status.borrow();
            assert_eq!(updated.len(), 3);
            assert!(std::ptr::eq(&*original[2], &*updated[1]));
            assert!(!std::ptr::eq(&*original[1], &*updated[2]));
            assert_eq!(updated[2].autosaveName().to_string(), "third");
        }
        delegate.remove_appearance_observers();
        assert!(delegate.ivars().timer.borrow().is_none());
        println!(
            "Status identities survive rename, language, disable, and row reorder; only removed/new items change identity, and stale button state is cleared."
        );
    }
}
fn main() {
    app::check();
}
