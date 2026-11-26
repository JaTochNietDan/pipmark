//! Main-thread native animation installation/cleanup smoke test; synthetic unread state.
#[path = "../src/attention.rs"]
mod attention;
#[allow(dead_code)]
#[path = "../src/model.rs"]
mod model;
use objc2_app_kit::*;
use objc2_foundation::{MainThreadMarker, NSString};
fn main() {
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let bar = NSStatusBar::systemStatusBar();
    let status = bar.statusItemWithLength(24.0);
    let button = status.button(mtm).unwrap();
    button.setTitle(&NSString::from_str("•"));
    let mut item = model::Item {
        id: "test".into(),
        name: "Test".into(),
        enabled: true,
        appearance: model::Appearance::default(),
        kind: model::ItemKind::Group { children: vec![] },
    };
    let unread = model::Activity {
        running: true,
        label: "3".into(),
    };
    let reduced = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
    for style in [
        model::AttentionStyle::Pulse,
        model::AttentionStyle::Blink,
        model::AttentionStyle::DoubleBlink,
    ] {
        item.appearance.attention = style;
        attention::apply(&button, &item, &unread);
        let active = button
            .layer()
            .and_then(|l| unsafe {
                l.animationForKey(&NSString::from_str("badgeify-unread-attention"))
            })
            .is_some();
        assert_eq!(active, !reduced);
        attention::apply(&button, &item, &model::Activity::default());
        assert!(
            button
                .layer()
                .and_then(|l| unsafe {
                    l.animationForKey(&NSString::from_str("badgeify-unread-attention"))
                })
                .is_none()
        );
    }
    bar.removeStatusItem(&status);
    println!(
        "Native attention install/clear passed; Reduce Motion={reduced}. Visual cadence and energy use not measured."
    );
}
