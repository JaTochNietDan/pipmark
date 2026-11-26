//! Measure translated compact-control labels using native font metrics.
#![allow(dead_code)]
#[path = "../src/localization.rs"]
mod localization;
#[path = "../src/model.rs"]
mod model;
use objc2::{AnyThread, runtime::AnyObject};
use objc2_app_kit::*;
use objc2_foundation::*;
fn main() {
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let font = NSFont::systemFontOfSize(13.0);
    let attrs =
        unsafe { NSDictionary::from_slices(&[NSFontAttributeName], &[&*font as &AnyObject]) };
    for language in model::Language::ALL {
        for (key, width) in [
            ("Badge style", 175.0),
            ("Icon size", 175.0),
            ("No unread notifications", 175.0),
            ("App is not running", 175.0),
            ("Unread attention", 175.0),
            ("Add App…", 114.0),
            ("New Group…", 106.0),
            ("Settings", 100.0),
            ("Apps", 100.0),
            ("Manage Apps…", 162.0),
        ] {
            let text = localization::translate_in(language, key);
            let measured = unsafe {
                NSAttributedString::initWithString_attributes(
                    NSAttributedString::alloc(),
                    &NSString::from_str(text),
                    Some(&attrs),
                )
            }
            .size()
            .width;
            assert!(
                measured <= width,
                "{language:?}: {text} needs {measured} points, available {width}"
            );
        }
    }
    println!(
        "Translated form labels and compact navigation fit native font measurements in all supported languages."
    );
}
