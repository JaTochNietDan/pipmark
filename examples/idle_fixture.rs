//! Release idle fixture: production UI/monitor with synthetic unread data; no AX calls.
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
    use crate::{model::Appearance, monitor};
    pub fn run_fixture(animated: bool, duration: f64) {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mtm = MainThreadMarker::new().unwrap();
        let application = NSApplication::sharedApplication(mtm);
        application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        let delegate = Delegate::new(mtm);
        let reads = Arc::new(AtomicUsize::new(0));
        let styles = [
            AttentionStyle::None,
            AttentionStyle::Pulse,
            AttentionStyle::Blink,
            AttentionStyle::DoubleBlink,
        ];
        let items: Vec<_> = styles
            .into_iter()
            .enumerate()
            .map(|(index, style)| Item {
                id: format!("fixture.{index}"),
                name: format!("Idle fixture {index}"),
                enabled: true,
                appearance: Appearance {
                    attention: if animated {
                        style
                    } else {
                        AttentionStyle::None
                    },
                    badge_style: match index % 3 {
                        0 => BadgeStyle::Text,
                        1 => BadgeStyle::Dot,
                        _ => BadgeStyle::Circle,
                    },
                    ..Default::default()
                },
                kind: ItemKind::App {
                    path: "/System/Library/CoreServices/Finder.app".into(),
                },
            })
            .collect();
        let snapshot = dock::Snapshot {
            apps: items
                .iter()
                .map(|item| {
                    (
                        item.id.clone(),
                        Activity {
                            running: true,
                            label: "12".into(),
                        },
                    )
                })
                .collect(),
        };
        delegate.ivars().settings.borrow_mut().items = items;
        let targets = delegate.ivars().settings.borrow().monitored_apps();
        let counter = reads.clone();
        *delegate.ivars().monitor.borrow_mut() = Some(
            monitor::Monitor::with_factory(
                move || {
                    move || {
                        counter.fetch_add(1, Ordering::Relaxed);
                        Ok(snapshot.clone())
                    }
                },
                monitor::notify_ready,
            )
            .with_targets(targets),
        );
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &*delegate,
                sel!(dockReadCompleted:),
                Some(&s(monitor::READY_NOTIFICATION)),
                None,
            );
        }
        delegate.rebuild_status();
        objc2_foundation::NSRunLoop::currentRunLoop()
            .runUntilDate(&objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(5.0));
        assert_eq!(delegate.ivars().last_activity.borrow().len(), 4);
        let images: Vec<_> = delegate
            .ivars()
            .status
            .borrow()
            .iter()
            .skip(1)
            .map(|status| status.button(mtm).unwrap().image().unwrap())
            .collect();
        objc2_foundation::NSRunLoop::currentRunLoop().runUntilDate(
            &objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(duration - 5.0),
        );
        for (status, image) in delegate.ivars().status.borrow().iter().skip(1).zip(images) {
            assert!(
                std::ptr::eq(&*status.button(mtm).unwrap().image().unwrap(), &*image),
                "unchanged badge was rerasterized"
            );
        }
        let scans = reads.load(Ordering::Relaxed);
        assert!(
            scans >= (duration / 4.0) as usize && scans <= (duration / 3.0) as usize + 2,
            "unexpected scan count {scans}"
        );
        println!(
            "{}",
            serde_json::json!({"synthetic": true, "configured_apps": 4, "unread_each": 12, "animated": animated,
            "reduce_motion": NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion(), "scans": scans, "unchanged_images_reused": true})
        );
        unsafe {
            NSNotificationCenter::defaultCenter().removeObserver(&*delegate);
        }
        delegate.suspend_monitor();
        delegate.remove_appearance_observers();
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    app::run_fixture(
        args.get(1).is_some_and(|v| v == "animated"),
        args.get(2).unwrap().parse().unwrap(),
    );
}
