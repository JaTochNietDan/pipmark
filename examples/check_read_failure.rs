//! Production read-result presentation checks using synthetic errors and activity.
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
        delegate.ivars().settings.borrow_mut().items = vec![Item {
            id: "group".into(),
            name: "Group".into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::Group { children: vec![] },
        }];
        delegate.ivars().settings.borrow_mut().language = crate::model::Language::English;
        delegate.rebuild_status();
        if let ItemKind::Group { children } =
            &mut delegate.ivars().settings.borrow_mut().items[0].kind
        {
            children.push(Item {
                id: "test".into(),
                name: "Test".into(),
                enabled: true,
                appearance: Default::default(),
                kind: ItemKind::App {
                    path: "/test.app".into(),
                },
            });
        }
        let button = delegate.ivars().status.borrow()[1].button(mtm).unwrap();
        delegate.apply_activity_result(Some(Err(dock::Error::Permission)));
        assert!(
            button
                .toolTip()
                .unwrap()
                .to_string()
                .contains("Accessibility permission")
        );
        let original_image = button.image().unwrap();
        delegate.apply_activity_result(Some(Err(dock::Error::Permission)));
        assert!(
            std::ptr::eq(&*original_image, &*button.image().unwrap()),
            "identical error must not redraw"
        );
        delegate.apply_activity_result(Some(Err(dock::Error::DockUnavailable)));
        assert!(
            button
                .toolTip()
                .unwrap()
                .to_string()
                .contains("Dock is not available")
        );
        delegate.apply_activity_result(Some(Ok(dock::Snapshot {
            apps: [(
                "test".into(),
                Activity {
                    running: true,
                    label: "7".into(),
                },
            )]
            .into(),
        })));
        assert_eq!(button.title().to_string(), " 7");
        assert!(button.toolTip().unwrap().to_string().contains("7 unread"));
        assert!(delegate.ivars().last_error.borrow().is_none());
        let original_status = delegate.ivars().status.borrow()[1].clone();
        {
            let mut settings = delegate.ivars().settings.borrow_mut();
            settings.items[0].appearance.show_count = false;
            settings.items[0].appearance.icon_size = 22;
        }
        delegate.refresh_appearance();
        assert!(
            std::ptr::eq(&*original_status, &*delegate.ivars().status.borrow()[1]),
            "appearance changes preserve native status identity"
        );
        assert_eq!(button.title().to_string(), "");
        assert_eq!(button.image().unwrap().size().width, 22.0);
        assert!(!button.toolTip().unwrap().to_string().contains("7"));
        assert!(delegate.ivars().last_snapshot.borrow().is_some());
        assert!(
            delegate.ivars().monitor.borrow().is_none(),
            "appearance edits must not start a scan"
        );
        delegate.apply_activity_result(Some(Err(dock::Error::Permission)));
        assert_eq!(button.title().to_string(), "");
        assert!(delegate.ivars().last_snapshot.borrow().is_none());
        assert!(delegate.ivars().last_activity.borrow().is_empty());
        delegate.ivars().settings.borrow_mut().items[0]
            .appearance
            .icon_size = 16;
        delegate.refresh_appearance();
        assert_eq!(button.image().unwrap().size().width, 16.0);
        assert!(
            button
                .toolTip()
                .unwrap()
                .to_string()
                .contains("Accessibility permission")
        );
        assert!(std::ptr::eq(
            &*original_status,
            &*delegate.ivars().status.borrow()[1]
        ));
        delegate.remove_appearance_observers();
        for status in delegate.ivars().status.borrow_mut().drain(..) {
            NSStatusBar::systemStatusBar().removeStatusItem(&status);
        }
        println!(
            "Initial failure, unchanged-error redraw suppression, changed errors, recovery and stale-count cleanup passed."
        );
    }
}
fn main() {
    app::check();
}
