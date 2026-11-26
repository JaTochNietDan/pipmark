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
        use objc2_foundation::{NSDate, NSRunLoop};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mtm = MainThreadMarker::new().unwrap();
        let _app = NSApplication::sharedApplication(mtm);
        let delegate = Delegate::new(mtm);
        delegate.ivars().settings.borrow_mut().items = vec![Item {
            id: "group".into(),
            name: "Group".into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::Group { children: vec![] },
        }];
        delegate.rebuild_status(); // Empty group deliberately starts no timer/worker.
        if let ItemKind::Group { children } =
            &mut delegate.ivars().settings.borrow_mut().items[0].kind
        {
            children.push(Item {
                id: "test.app".into(),
                name: "Test".into(),
                enabled: true,
                appearance: Default::default(),
                kind: ItemKind::App {
                    path: "/test.app".into(),
                },
            });
        }
        let count = Arc::new(AtomicUsize::new(0));
        let reads = count.clone();
        *delegate.ivars().monitor.borrow_mut() = Some(crate::monitor::Monitor::with_factory(
            move || {
                move || {
                    reads.fetch_add(1, Ordering::SeqCst);
                    Ok(dock::Snapshot {
                        apps: [(
                            "test.app".into(),
                            Activity {
                                running: true,
                                label: "7".into(),
                            },
                        )]
                        .into(),
                    })
                }
            },
            crate::monitor::notify_ready,
        ));
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &*delegate,
                sel!(dockReadCompleted:),
                Some(&s(crate::monitor::READY_NOTIFICATION)),
                None,
            );
        }
        delegate.workspace_app_changed("unrelated.app");
        delegate
            .ivars()
            .lifecycle
            .borrow_mut()
            .set(crate::lifecycle::Reason::Session, true);
        delegate.workspace_app_changed("test.app");
        NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        delegate
            .ivars()
            .lifecycle
            .borrow_mut()
            .set(crate::lifecycle::Reason::Session, false);
        let started = std::time::Instant::now();
        delegate.workspace_app_changed("test.app");
        while delegate.ivars().last_activity.borrow().is_empty()
            && started.elapsed().as_secs_f64() < 2.0
        {
            NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.01));
        }
        assert_eq!(delegate.ivars().last_activity.borrow()[0].label, "7");
        let delivery_ms = started.elapsed().as_millis();
        NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.2));
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "completion must not create another scan"
        );
        assert!(delegate.ivars().timer.borrow().is_none());
        unsafe {
            NSNotificationCenter::defaultCenter().removeObserver(&*delegate);
        }
        delegate.suspend_monitor();
        delegate.remove_appearance_observers();
        for status in delegate.ivars().status.borrow_mut().drain(..) {
            NSStatusBar::systemStatusBar().removeStatusItem(&status);
        }
        println!(
            "Production AppKit completion callback delivered synthetic count in {delivery_ms} ms; one scan, no timer."
        );
    }
}
fn main() {
    app::check();
}
