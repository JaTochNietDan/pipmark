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
    fn snapshot(delegate: &Delegate, name: &str, appearance: &NSAppearance) {
        let window = delegate.ivars().window.borrow().as_ref().unwrap().clone();
        window.setAppearance(Some(appearance));
        objc2_foundation::NSRunLoop::currentRunLoop().runUntilDate(
            &objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(0.05),
        );
        let view = window.contentView().unwrap();
        view.layoutSubtreeIfNeeded();
        view.displayIfNeeded();
        let bitmap = view
            .bitmapImageRepForCachingDisplayInRect(view.bounds())
            .unwrap();
        appearance.performAsCurrentDrawingAppearance(&block2::StackBlock::new(|| {
            view.cacheDisplayInRect_toBitmapImageRep(view.bounds(), &bitmap);
        }));
        let image = NSImage::initWithSize(NSImage::alloc(), view.bounds().size);
        appearance.performAsCurrentDrawingAppearance(&block2::StackBlock::new(|| {
            #[allow(deprecated)]
            image.lockFocus();
            NSColor::windowBackgroundColor().setFill();
            NSRectFill(view.bounds());
            unsafe {
                bitmap.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    view.bounds(),
                    NSRect::ZERO,
                    NSCompositingOperation::SourceOver,
                    1.0,
                    false,
                    None,
                );
            }
            #[allow(deprecated)]
            image.unlockFocus();
        }));
        let bitmap = NSBitmapImageRep::initWithData(
            NSBitmapImageRep::alloc(),
            &image.TIFFRepresentation().unwrap(),
        )
        .unwrap();
        let png = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }
        .unwrap();
        assert!(png.writeToFile_atomically(&s(&format!("target/{name}.png")), true));
    }
    pub fn render() {
        let mtm = MainThreadMarker::new().unwrap();
        let _application = NSApplication::sharedApplication(mtm);
        let delegate = Delegate::new(mtm);
        let app = |id: &str, name: &str, path: &str| Item {
            id: id.into(),
            name: name.into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::App { path: path.into() },
        };
        let mut messages = app(
            "com.apple.MobileSMS",
            "Messages",
            "/System/Applications/Messages.app",
        );
        messages.enabled = false;
        let mut group = Item {
            id: "communications".into(),
            name: "Communications".into(),
            enabled: true,
            appearance: Default::default(),
            kind: ItemKind::Group {
                children: vec![
                    app("com.apple.mail", "Mail", "/System/Applications/Mail.app"),
                    messages,
                ],
            },
        };
        if std::env::var_os("BADGEIFY_PREVIEW_LONG_NAMES").is_some() {
            group.name = "Communications — Projects, Clients, and Team Updates".into();
            if let ItemKind::Group { children } = &mut group.kind {
                children[0].name = "Mail — Project Correspondence and Shared Team Inbox".into();
            }
        }
        delegate.ivars().settings.borrow_mut().items = vec![
            app(
                "com.apple.finder",
                "Finder",
                "/System/Library/CoreServices/Finder.app",
            ),
            group.clone(),
            Item {
                id: "trash".into(),
                name: "Trash".into(),
                enabled: true,
                appearance: Default::default(),
                kind: ItemKind::Trash,
            },
        ];
        let locale = std::env::var("BADGEIFY_PREVIEW_LANGUAGE").unwrap_or_default();
        crate::localization::configure(match locale.as_str() {
            "de" => crate::model::Language::German,
            "ja" => crate::model::Language::Japanese,
            _ => crate::model::Language::English,
        });
        let mut light_preview = None;
        for (theme, appearance_name) in [
            ("light", unsafe { NSAppearanceNameAqua }),
            ("dark", unsafe { NSAppearanceNameDarkAqua }),
        ] {
            let appearance = NSAppearance::appearanceNamed(appearance_name).unwrap();
            delegate.show_settings();
            snapshot(
                &delegate,
                &format!("settings-list-{theme}{locale}"),
                &appearance,
            );
            if std::env::var_os("BADGEIFY_PREVIEW_SELECTION").is_some() {
                delegate.ivars().selected_rows.borrow_mut().clear();
                delegate.select_list_item(0, true);
                delegate.select_list_item(1, true);
                snapshot(
                    &delegate,
                    &format!("settings-selected-{theme}{locale}"),
                    &appearance,
                );
            }
            if std::env::var_os("BADGEIFY_PREVIEW_DROP").is_some() {
                let root = delegate
                    .ivars()
                    .window
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .contentView()
                    .unwrap()
                    .downcast::<ApplicationDropView>()
                    .unwrap();
                let target = delegate.ivars().list_rows.borrow()[1].1.clone();
                let point = root.convertPoint_fromView(NSPoint::new(10.0, 22.0), Some(&target));
                assert_eq!(
                    root.update_row_drop_at("com.apple.finder", point),
                    NSDragOperation::Move
                );
                snapshot(
                    &delegate,
                    &format!("settings-drop-{theme}{locale}"),
                    &appearance,
                );
            }
            *delegate.ivars().advanced_appearance.borrow_mut() =
                std::env::var_os("BADGEIFY_PREVIEW_ADVANCED").is_some();
            delegate.appearance_pane(group.clone(), 1, false, false);
            snapshot(
                &delegate,
                &format!("settings-group-{theme}{locale}"),
                &appearance,
            );
            let preview = || {
                delegate
                    .ivars()
                    .editor
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .style
                    .itemAtIndex(0)
                    .unwrap()
                    .image()
                    .unwrap()
                    .TIFFRepresentation()
                    .unwrap()
                    .to_vec()
            };
            if theme == "light" {
                light_preview = Some(preview());
            } else {
                assert_ne!(
                    preview(),
                    *light_preview.as_ref().unwrap(),
                    "preview must adapt to dark appearance"
                );
                let light = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameAqua }).unwrap();
                delegate
                    .ivars()
                    .window
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .setAppearance(Some(&light));
                assert_eq!(
                    preview(),
                    *light_preview.as_ref().unwrap(),
                    "preview must restore light colors without reopening its menu"
                );
            }
            delegate.appearance_pane(group.clone(), 1, false, true);
            snapshot(
                &delegate,
                &format!("settings-members-{theme}{locale}"),
                &appearance,
            );
        }
        println!(
            "Rendered main list, group settings, and group members in light and dark appearances."
        );
    }
}
fn main() {
    app::render();
}
