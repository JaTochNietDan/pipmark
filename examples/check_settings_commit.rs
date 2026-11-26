//! Production settings commit check. Requires a new disposable BADGEIFY_CONFIG path.
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
        let path = std::path::PathBuf::from(
            std::env::var_os("BADGEIFY_CONFIG")
                .expect("set BADGEIFY_CONFIG to a disposable fixture path"),
        );
        let mtm = MainThreadMarker::new().unwrap();
        let delegate = Delegate::new(mtm);
        let original = serde_json::to_value(&*delegate.ivars().settings.borrow()).unwrap();
        let mut proposed = delegate.ivars().settings.borrow().clone();
        proposed.show_on_start = false;
        proposed.language = crate::model::Language::German;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("fixture must not already exist");
        drop(file);
        std::fs::write(&path, b"broken fixture").unwrap();
        assert!(delegate.commit_settings(proposed.clone()).is_err());
        assert_eq!(
            serde_json::to_value(&*delegate.ivars().settings.borrow()).unwrap(),
            original
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"broken fixture");
        storage::export(&path, &Settings::default()).unwrap();
        delegate.commit_settings(proposed.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&*delegate.ivars().settings.borrow()).unwrap(),
            serde_json::to_value(&proposed).unwrap()
        );
        assert_eq!(
            serde_json::to_value(storage::read(&path).unwrap()).unwrap(),
            serde_json::to_value(&proposed).unwrap()
        );
        assert!(
            storage::read(&path.with_extension("json.backup"))
                .unwrap()
                .show_on_start
        );
        println!(
            "Failed settings commit preserves live state and disk; successful commit publishes saved state and retains backup."
        );
    }
}
fn main() {
    app::check();
}
