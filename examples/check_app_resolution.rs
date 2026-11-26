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
        assert!(std::env::var_os("BADGEIFY_CONFIG").is_some());
        assert!(!path.exists(), "use a fresh disposable config");
        let mtm = MainThreadMarker::new().unwrap();
        let _app = NSApplication::sharedApplication(mtm);
        let finder_path = "/System/Library/CoreServices/Finder.app";
        let url = resolve_application("com.apple.finder", "/missing/Finder.app").unwrap();
        assert_eq!(application_details(&url).unwrap().0, "com.apple.finder");
        assert_eq!(
            resolve_application("com.apple.finder", finder_path)
                .unwrap()
                .path()
                .unwrap()
                .to_string(),
            finder_path
        );
        assert!(resolve_application("test.badgeify.definitely-uninstalled", finder_path).is_none());
        let finder = Item {
            id: "com.apple.finder".into(),
            name: "Stale Finder name".into(),
            enabled: false,
            appearance: crate::model::Appearance {
                icon_size: 22,
                ..Default::default()
            },
            kind: ItemKind::App {
                path: "/missing/Finder.app".into(),
            },
        };
        let missing = Item {
            id: "test.badgeify.definitely-uninstalled".into(),
            name: "Preserved missing app".into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::App {
                path: "/missing/Application.app".into(),
            },
        };
        let delegate = Delegate::new(mtm);
        delegate.ivars().settings.borrow_mut().items = vec![Item {
            id: "group".into(),
            name: "My custom group".into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::Group {
                children: vec![finder, missing],
            },
        }];
        assert!(!delegate.refresh_application_metadata(Some("unrelated.app")));
        assert!(!path.exists());
        assert!(delegate.refresh_application_metadata(Some("com.apple.finder")));
        let settings = storage::read(&path).unwrap();
        assert_eq!(settings.items[0].name, "My custom group");
        let ItemKind::Group { children } = &settings.items[0].kind else {
            panic!()
        };
        assert_ne!(children[0].name, "Stale Finder name");
        assert!(!children[0].enabled);
        assert_eq!(children[0].appearance.icon_size, 22);
        assert_eq!(children[1].name, "Preserved missing app");
        assert!(matches!(&children[0].kind, ItemKind::App { path } if path == finder_path));
        let bytes = std::fs::read(&path).unwrap();
        assert!(!delegate.refresh_application_metadata(None));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        println!(
            "Native bundle lookup repairs stale paths, rejects mismatched IDs, preserves missing apps/preferences/groups, and only saves changed metadata."
        );
    }
}
fn main() {
    app::check();
}
