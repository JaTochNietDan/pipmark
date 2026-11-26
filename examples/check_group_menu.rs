//! Native group-menu update regression with synthetic activity; no notifications read.
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
        let app = |id: &str| Item {
            id: id.into(),
            name: id.into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::App {
                path: "/System/Library/CoreServices/Finder.app".into(),
            },
        };
        let mut hidden_count = app("Private");
        hidden_count.appearance.show_count = false;
        let group = Item {
            id: "Group".into(),
            name: "Group".into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::Group {
                children: vec![app("Mail"), hidden_count],
            },
        };
        delegate.ivars().settings.borrow_mut().items = vec![group.clone()];
        let menu = NSMenu::new(mtm);
        delegate.group_menu(&menu, &[group]);
        delegate.menu_item(&menu, "Settings…", sel!(showSettings:), 0, "");
        *delegate.ivars().last_snapshot.borrow_mut() = Some(dock::Snapshot {
            apps: [
                (
                    "Mail".into(),
                    Activity {
                        running: true,
                        label: "4".into(),
                    },
                ),
                (
                    "Private".into(),
                    Activity {
                        running: true,
                        label: "8".into(),
                    },
                ),
            ]
            .into(),
        });
        unsafe {
            let _: () = msg_send![&*delegate, menuNeedsUpdate: &*menu];
        }
        let row = menu.itemAtIndex(0).unwrap();
        assert_eq!(row.title().to_string(), "Group (12)");
        assert!(row.image().is_some());
        assert_eq!(
            menu.itemAtIndex(1).unwrap().title().to_string(),
            "Settings…"
        );
        let submenu = row.submenu().unwrap();
        unsafe {
            let _: () = msg_send![&*delegate, menuNeedsUpdate: &*submenu];
        }
        assert_eq!(
            submenu.itemAtIndex(0).unwrap().title().to_string(),
            "Mail (4)"
        );
        assert_eq!(
            submenu.itemAtIndex(1).unwrap().title().to_string(),
            "Private  •"
        );
        assert_eq!(submenu.itemAtIndex(0).unwrap().tag(), 1);
        assert!(submenu.itemAtIndex(0).unwrap().image().is_some());
        delegate.ivars().last_snapshot.borrow_mut().take();
        unsafe {
            let _: () = msg_send![&*delegate, menuNeedsUpdate: &*menu];
        }
        unsafe {
            let _: () = msg_send![&*delegate, menuNeedsUpdate: &*submenu];
        }
        assert_eq!(row.title().to_string(), "Group");
        assert_eq!(submenu.itemAtIndex(0).unwrap().title().to_string(), "Mail");
        let empty_menu = NSMenu::new(mtm);
        delegate.group_menu(&empty_menu, &[]);
        assert_eq!(empty_menu.numberOfItems(), 1);
        assert_eq!(
            empty_menu.itemAtIndex(0).unwrap().title().to_string(),
            "No enabled apps"
        );
        assert!(!empty_menu.itemAtIndex(0).unwrap().isEnabled());
        println!(
            "Native menu update callback, nested aggregates, icons, privacy, action tags and missing-data cleanup passed."
        );
    }
}
fn main() {
    app::check();
}
