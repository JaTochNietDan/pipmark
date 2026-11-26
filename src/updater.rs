//! Sparkle owns its scheduling and native UI; no application polling timer.
use objc2::{msg_send, rc::Retained, runtime::AnyClass, sel};
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSBundle, NSObject, NSString};

pub struct Updater {
    controller: Retained<NSObject>,
    _framework: Retained<NSBundle>,
}
impl Updater {
    pub fn start(_mtm: MainThreadMarker) -> Result<Option<Self>, String> {
        let main = NSBundle::mainBundle();
        // Command-line development and test binaries have no bundled updater.
        if main.bundleIdentifier().as_deref().map(|id| id.to_string()).as_deref() != Some("club.oleary.pipmark") {
            return Ok(None);
        }
        let path = format!("{}/Contents/Frameworks/Sparkle.framework", main.bundlePath());
        let framework = NSBundle::bundleWithPath(&NSString::from_str(&path))
            .ok_or("Sparkle.framework is missing")?;
        unsafe {
            framework.loadAndReturnError().map_err(|e| e.to_string())?;
            let class = AnyClass::get(c"SPUStandardUpdaterController").ok_or("Sparkle controller is unavailable")?;
            let allocated: objc2::rc::Allocated<NSObject> = msg_send![class, alloc];
            let controller: Retained<NSObject> = msg_send![allocated,
                initWithStartingUpdater: false,
                updaterDelegate: Option::<&NSObject>::None,
                userDriverDelegate: Option::<&NSObject>::None];
            let updater: Retained<NSObject> = msg_send![&*controller, updater];
            let mut error: *mut objc2_foundation::NSError = std::ptr::null_mut();
            let started: bool = msg_send![&*updater, startUpdater: &mut error];
            if !started {
                return Err(if error.is_null() { "Sparkle could not start".into() } else { (*error).to_string() });
            }
            Ok(Some(Self { controller, _framework: framework }))
        }
    }
    pub fn add_menu_item(&self, menu: &NSMenu, mtm: MainThreadMarker) {
        use objc2::MainThreadOnly;
        unsafe {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm), &NSString::from_str("Check for Updates…"),
                Some(sel!(checkForUpdates:)), &NSString::from_str(""));
            // Sparkle validates availability while a check or installation is active.
            item.setTarget(Some(&*self.controller));
            menu.addItem(&item);
        }
    }
}
