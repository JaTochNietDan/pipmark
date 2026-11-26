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
        let path = storage::config_path();
        assert!(std::env::var_os("BADGEIFY_CONFIG").is_some() && !path.exists());
        let mtm = MainThreadMarker::new().unwrap();
        let _app = NSApplication::sharedApplication(mtm);
        let delegate = Delegate::new(mtm);
        delegate.ivars().settings.borrow_mut().items = ["First", "Second"]
            .map(|id| Item {
                id: id.into(),
                name: id.into(),
                enabled: true,
                appearance: Default::default(),
                kind: ItemKind::Group { children: vec![] },
            })
            .into();
        delegate.show_settings();
        assert!(
            !delegate
                .ivars()
                .remove_button
                .borrow()
                .as_ref()
                .unwrap()
                .isEnabled()
        );
        let document = unsafe { delegate.ivars().list_rows.borrow()[0].1.superview() }.unwrap();
        let toggle = document
            .subviews()
            .iter()
            .find_map(|view| {
                view.clone()
                    .downcast::<NSSwitch>()
                    .ok()
                    .filter(|v| v.tag() == 0)
            })
            .unwrap();
        toggle.setState(NSControlStateValueOff);
        unsafe {
            let _: () = msg_send![&*delegate, toggleApp: &*toggle];
        }
        assert!(!storage::read(&path).unwrap().items[0].enabled);
        delegate.select_list_item(1, false);
        assert!(
            delegate
                .ivars()
                .remove_button
                .borrow()
                .as_ref()
                .unwrap()
                .isEnabled()
        );
        assert!(delegate.ivars().editor.borrow().is_none());
        delegate.edit_appearance(1);
        assert!(!*delegate.ivars().advanced_appearance.borrow());
        let height = delegate
            .ivars()
            .window
            .borrow()
            .as_ref()
            .unwrap()
            .contentView()
            .unwrap()
            .frame()
            .size
            .height;
        unsafe {
            let _: () = msg_send![&*delegate, toggleAdvanced: &*NSObject::new()];
        }
        assert!(*delegate.ivars().advanced_appearance.borrow());
        assert!(
            delegate
                .ivars()
                .window
                .borrow()
                .as_ref()
                .unwrap()
                .contentView()
                .unwrap()
                .frame()
                .size
                .height
                > height
        );
        delegate.show_settings();
        unsafe {
            let _: () = msg_send![&*delegate, removeSelected: &*NSObject::new()];
        }
        let saved = storage::read(&path).unwrap();
        assert_eq!(saved.items.len(), 1);
        assert_eq!(saved.items[0].id, "First");
        delegate.remove_appearance_observers();
        println!(
            "Native row switches persist, selection enables removal, info opens details, Advanced expands, and removal preserves unselected items."
        );
    }
}
fn main() {
    app::check();
}
