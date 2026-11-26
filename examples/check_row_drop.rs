//! Native row-drop geometry and persistence check; no Dock access.
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
        let delegate = Delegate::new(mtm);
        let group = |id: &str| Item {
            id: id.into(),
            name: id.into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::Group { children: vec![] },
        };
        delegate.ivars().settings.borrow_mut().items =
            vec![group("First"), group("Second"), group("Third")];
        delegate.show_settings();
        let view = delegate
            .ivars()
            .window
            .borrow()
            .as_ref()
            .unwrap()
            .contentView()
            .unwrap()
            .downcast::<ApplicationDropView>()
            .unwrap();
        let second = delegate.ivars().list_rows.borrow()[1].1.clone();
        let above = view.convertPoint_fromView(NSPoint::new(10.0, 22.0), Some(&second));
        assert_eq!(
            view.update_row_drop_at("Third", above),
            NSDragOperation::Move
        );
        assert!(
            matches!(view.ivars().marker.borrow().as_ref(), Some((id, false, _)) if id == "Second")
        );
        let below = view.convertPoint_fromView(NSPoint::new(10.0, 5.0), Some(&second));
        assert_eq!(
            view.update_row_drop_at("First", below),
            NSDragOperation::Move
        );
        assert!(
            matches!(view.ivars().marker.borrow().as_ref(), Some((id, true, _)) if id == "Second")
        );
        assert_eq!(
            view.update_row_drop_at("Second", below),
            NSDragOperation::None
        );
        assert_eq!(
            view.update_row_drop_at("missing", below),
            NSDragOperation::None
        );
        assert_eq!(
            view.update_row_drop_at("First", NSPoint::new(10.0, 20.0)),
            NSDragOperation::None
        );
        assert!(view.ivars().marker.borrow().is_none());
        assert!(delegate.move_list_row("Third", "Second", false));
        let saved = storage::read(&path).unwrap();
        assert_eq!(
            saved
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["First", "Third", "Second"]
        );
        let before = std::fs::read(&path).unwrap();
        assert!(!delegate.move_list_row("First", "missing", false));
        assert_eq!(before, std::fs::read(&path).unwrap());
        // Exercise the actual clip view with enough rows to require scrolling.
        delegate.ivars().settings.borrow_mut().items = (0..40)
            .map(|index| group(&format!("Row {index}")))
            .collect();
        delegate.show_settings();
        let view = delegate
            .ivars()
            .window
            .borrow()
            .as_ref()
            .unwrap()
            .contentView()
            .unwrap()
            .downcast::<ApplicationDropView>()
            .unwrap();
        let (scroll, viewport) = view.list_viewport().unwrap();
        let top = scroll.contentView().bounds().origin.y;
        assert!(top > 0.0);
        let lower = NSPoint::new(viewport.origin.x + 40.0, viewport.origin.y + 4.0);
        let upper = NSPoint::new(lower.x, viewport.origin.y + viewport.size.height - 4.0);
        let middle = NSPoint::new(lower.x, viewport.origin.y + viewport.size.height / 2.0);
        view.autoscroll_row_drop(middle);
        view.autoscroll_row_drop(NSPoint::new(viewport.origin.x - 10.0, lower.y));
        assert_eq!(scroll.contentView().bounds().origin.y, top);
        view.autoscroll_row_drop(lower);
        assert_eq!(scroll.contentView().bounds().origin.y, top - 14.0);
        for _ in 0..100 {
            view.autoscroll_row_drop(lower);
        }
        assert_eq!(scroll.contentView().bounds().origin.y, 0.0);
        assert_eq!(
            view.update_row_drop_at("Row 0", lower),
            NSDragOperation::Move
        );
        assert!(
            matches!(view.ivars().marker.borrow().as_ref(), Some((id, _, _)) if id == "Row 39")
        );
        for _ in 0..100 {
            view.autoscroll_row_drop(upper);
        }
        assert_eq!(scroll.contentView().bounds().origin.y, top);
        assert_eq!(
            view.update_row_drop_at("Row 39", upper),
            NSDragOperation::Move
        );
        assert!(matches!(view.ivars().marker.borrow().as_ref(), Some((id, _, _)) if id == "Row 0"));
        // Bounds are derived from the real clip view rather than fixed window coordinates.
        scroll.setFrame(rect(50.0, 220.0, 500.0, 150.0));
        assert_eq!(
            view.update_row_drop_at("Row 0", lower),
            NSDragOperation::None
        );
        delegate.remove_appearance_observers();
        println!(
            "Native drop geometry, before/after markers, self/missing rejection, persisted reorder, and invalid-drop preservation, bounded edge scrolling, and resized viewport checks passed."
        );
    }
}
fn main() {
    app::check();
}
