//! Run from a disposable Pipmark.app copy so Sparkle sees a real application bundle.
#[path = "../src/updater.rs"]
mod updater;
use objc2::msg_send;
use objc2_app_kit::{NSApplication, NSMenu};
use objc2_foundation::MainThreadMarker;
fn main() {
    objc2::rc::autoreleasepool(|_| {
        let mtm = MainThreadMarker::new().unwrap();
        let _app = NSApplication::sharedApplication(mtm);
        let controller = updater::Updater::start(mtm).expect("Sparkle startup").expect("bundled updater");
        let menu = NSMenu::new(mtm);
        controller.add_menu_item(&menu, mtm);
        let item = menu.itemAtIndex(0).unwrap();
        assert_eq!(item.title().to_string(), "Check for Updates…");
        unsafe {
            let target = item.target().expect("Sparkle action target");
            let updater: objc2::rc::Retained<objc2_foundation::NSObject> = msg_send![&*target, updater];
            let can_check: bool = msg_send![&*updater, canCheckForUpdates];
            assert!(can_check, "Updater must accept user initiated checks after startup");
            let valid: bool = msg_send![&*target, validateMenuItem: &*item];
            assert!(valid);
        }
        println!("Sparkle started; update action target and menu validation passed");
    });
}
