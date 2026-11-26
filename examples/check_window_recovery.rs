//! Native window recovery and menu routing; no Dock reads or user settings.
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
        let application = NSApplication::sharedApplication(mtm);
        let delegate = Delegate::new(mtm);
        delegate.ivars().settings.borrow_mut().show_on_start = false;
        delegate.rebuild_status();
        for status in delegate.ivars().status.borrow().iter() {
            status.setVisible(false);
        }
        assert!(delegate.ivars().window.borrow().is_none());
        let reopened: bool = unsafe {
            msg_send![&*delegate, applicationShouldHandleReopen: &*application, hasVisibleWindows: false]
        };
        assert!(reopened);
        let window = delegate.ivars().window.borrow().as_ref().unwrap().clone();
        assert!(window.isVisible());
        window.makeKeyWindow();
        let menu = application.mainMenu().unwrap();
        let window_menu = menu
            .itemArray()
            .iter()
            .find_map(|item| {
                item.submenu().filter(|submenu| {
                    submenu
                        .itemArray()
                        .iter()
                        .any(|i| i.action() == Some(sel!(performClose:)))
                })
            })
            .unwrap();
        assert!(
            window_menu
                .itemArray()
                .iter()
                .any(|item| item.keyEquivalent().to_string() == "m"
                    && item.action() == Some(sel!(performMiniaturize:)))
        );
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown, NSPoint::new(0.0, 0.0), NSEventModifierFlags::Command, 0.0,
            window.windowNumber(), None, &s("w"), &s("w"), false, 13,
        ).unwrap();
        if window.isKeyWindow() {
            assert!(
                menu.performKeyEquivalent(&event),
                "Command-W should route to the native window"
            );
        } else {
            println!(
                "Keyboard delivery unavailable: window server did not grant key-window status."
            );
            assert!(
                window_menu
                    .itemArray()
                    .iter()
                    .any(|item| item.keyEquivalent().to_string() == "w"
                        && item.action() == Some(sel!(performClose:)))
            );
            window.performClose(None);
        }
        objc2_foundation::NSRunLoop::currentRunLoop()
            .runUntilDate(&objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(0.2));
        assert!(!window.isVisible());
        let reopened: bool = unsafe {
            msg_send![&*delegate, applicationShouldHandleReopen: &*application, hasVisibleWindows: false]
        };
        assert!(reopened);
        assert!(window.isVisible());
        assert!(std::ptr::eq(
            &*window,
            &**delegate.ivars().window.borrow().as_ref().unwrap()
        ));
        assert!(
            delegate
                .ivars()
                .status
                .borrow()
                .iter()
                .all(|item| !item.isVisible())
        );
        assert!(delegate.ivars().timer.borrow().is_none());
        assert!(delegate.ivars().monitor.borrow().is_none());
        let edit = menu
            .itemArray()
            .iter()
            .find_map(|item| {
                item.submenu().filter(|m| {
                    m.itemArray()
                        .iter()
                        .any(|i| i.action() == Some(sel!(redo:)))
                })
            })
            .unwrap();
        let redo = edit
            .itemArray()
            .iter()
            .find(|i| i.action() == Some(sel!(redo:)))
            .unwrap();
        assert_eq!(
            redo.keyEquivalentModifierMask(),
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        assert!(redo.target().is_none());
        window.close();
        delegate.remove_appearance_observers();
        println!(
            "Hidden-status reopen, native close, retained window recovery, shortcut bindings and no-monitor checks passed."
        );
    }
}
fn main() {
    app::check();
}
