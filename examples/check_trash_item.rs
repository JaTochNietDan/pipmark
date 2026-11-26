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
        let path = storage::config_path();
        assert!(
            std::env::var_os("BADGEIFY_CONFIG").is_some(),
            "use a disposable fixture"
        );
        assert!(!path.exists(), "fixture must be fresh");
        let title = dock::empty_trash_title().expect("installed Dock localized menu resource");
        assert!(!title.is_empty() && title != "EMPTY_TRASH");
        let mtm = MainThreadMarker::new().unwrap();
        let _application = NSApplication::sharedApplication(mtm);
        let delegate = Delegate::new(mtm);
        unsafe {
            let _: () = msg_send![&*delegate, addTrash: &*NSObject::new()];
        }
        let saved = storage::read(&path).unwrap();
        assert_eq!(saved.items.len(), 1);
        assert!(matches!(saved.items[0].kind, ItemKind::Trash));
        assert!(saved.items[0].appearance.pinned);
        assert!(saved.monitored_apps().is_empty());
        assert!(delegate.ivars().timer.borrow().is_none());
        assert!(delegate.ivars().monitor.borrow().is_none());
        assert_eq!(delegate.ivars().status.borrow().len(), 2);
        assert!(
            delegate.ivars().status.borrow()[1]
                .button(mtm)
                .unwrap()
                .image()
                .is_some()
        );
        unsafe {
            let _: () = msg_send![&*delegate, addTrash: &*NSObject::new()];
        }
        assert_eq!(delegate.ivars().settings.borrow().items.len(), 1);
        let menu = delegate.ivars().status.borrow()[0].menu(mtm).unwrap();
        assert!(
            !menu
                .itemArray()
                .iter()
                .any(|item| item.action() == Some(sel!(addTrash:)))
        );
        assert!(
            delegate.ivars().menus.borrow()[0]
                .itemArray()
                .iter()
                .any(|item| item.action() == Some(sel!(emptyTrash:)) && item.tag() == 0)
        );
        let (release, wait) = std::sync::mpsc::sync_channel(1);
        let (ready, notified) = std::sync::mpsc::sync_channel(1);
        *delegate.ivars().trash_action.borrow_mut() = Some(
            crate::monitor::Action::start(
                move || {
                    wait.recv().unwrap();
                    Ok(())
                },
                move || {
                    ready.send(()).unwrap();
                },
            )
            .unwrap(),
        );
        // While a synthetic open is pending, repeated clicks must not touch the Dock.
        for _ in 0..10 {
            delegate.open_trash();
            delegate.start_trash_action(true);
        }
        assert!(
            delegate
                .ivars()
                .trash_action
                .borrow()
                .as_ref()
                .unwrap()
                .poll()
                .is_none()
        );
        release.send(()).unwrap();
        notified
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        delegate.finish_trash_action();
        assert!(delegate.ivars().trash_action.borrow().is_none());
        println!(
            "Trash persists once, renders an icon, starts no idle monitor, suppresses duplicate opens, and releases completed actions."
        );
    }
}
fn main() {
    app::check();
}
