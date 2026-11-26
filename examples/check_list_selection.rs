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
        let first_row = delegate.ivars().list_rows.borrow()[0].1.clone();
        let image_rect = first_row
            .cell()
            .unwrap()
            .imageRectForBounds(first_row.bounds());
        delegate.select_list_item(0, true);
        assert_eq!(
            first_row
                .cell()
                .unwrap()
                .imageRectForBounds(first_row.bounds()),
            image_rect
        );
        delegate.select_list_item(2, true);
        assert_eq!(delegate.ivars().selected_rows.borrow().len(), 2);
        for (index, (_, row)) in delegate.ivars().list_rows.borrow().iter().enumerate() {
            assert!(!row.isBordered());
            assert_eq!(
                row.state(),
                if index != 1 {
                    NSControlStateValueOn
                } else {
                    NSControlStateValueOff
                }
            );
        }
        delegate.select_list_item(0, true);
        assert_eq!(delegate.ivars().selected_rows.borrow().len(), 1);
        assert!(delegate.ivars().selected_rows.borrow().contains("Third"));
        delegate.show_settings();
        assert_eq!(
            delegate.ivars().list_rows.borrow()[2].1.state(),
            NSControlStateValueOn
        );
        let selected = delegate.ivars().selected_rows.borrow().clone();
        let mut proposed = delegate.ivars().settings.borrow().clone();
        proposed
            .create_group("new".into(), "Selected group".into(), &selected)
            .unwrap();
        let children = match &proposed.items.last().unwrap().kind {
            ItemKind::Group { children } => children,
            _ => panic!(),
        };
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, "Third");
        delegate.ivars().settings.borrow_mut().items.pop();
        delegate.show_settings();
        assert!(delegate.ivars().selected_rows.borrow().is_empty());
        delegate.select_list_item(0, true);
        delegate.select_list_item(1, false);
        assert_eq!(
            *delegate.ivars().selected_rows.borrow(),
            ["Second".into()].into()
        );
        assert!(delegate.ivars().editor.borrow().is_none());
        delegate.edit_appearance(1);
        assert_eq!(
            delegate.ivars().editor.borrow().as_ref().unwrap().id,
            "Second"
        );
        assert!(delegate.ivars().list_rows.borrow().is_empty());
        assert!(delegate.ivars().monitor.borrow().is_none());
        println!(
            "Native row highlighting, toggle-off, selection retention/pruning, grouping, and ordinary-click selection and info editing passed."
        );
    }
}
fn main() {
    app::check();
}
