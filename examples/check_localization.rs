//! Native localization check; no user settings or system language changes.
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
        let app = NSApplication::sharedApplication(mtm);
        let delegate = Delegate::new(mtm);
        for (language, title) in [
            (
                crate::model::Language::German,
                "Pipmark-Einstellungen…",
            ),
            (crate::model::Language::Japanese, "Pipmarkの設定…"),
        ] {
            delegate.ivars().settings.borrow_mut().language = language;
            delegate.rebuild_status();
            let menu = delegate.ivars().status.borrow()[0].menu(mtm).unwrap();
            let command_title = |menu: &NSMenu, action| {
                menu.itemArray()
                    .iter()
                    .find(|item| item.action() == Some(action))
                    .expect("menu command")
                    .title()
                    .to_string()
            };
            assert_eq!(command_title(&menu, sel!(showSettings:)), title);
            assert_eq!(
                command_title(&menu, sel!(showAbout:)),
                crate::localization::translate_in(language, "About Pipmark")
            );
            let language_menu = menu
                .itemArray()
                .iter()
                .find_map(|item| {
                    item.submenu().filter(|submenu| {
                        submenu
                            .itemArray()
                            .iter()
                            .any(|row| row.action() == Some(sel!(chooseLanguage:)))
                    })
                })
                .unwrap();
            assert_eq!(
                language_menu
                    .itemArray()
                    .iter()
                    .filter(|row| row.state() == NSControlStateValueOn)
                    .count(),
                1
            );
            assert_eq!(
                language_menu
                    .itemAtIndex(language as isize)
                    .unwrap()
                    .state(),
                NSControlStateValueOn
            );
            let app_menu = app
                .mainMenu()
                .unwrap()
                .itemAtIndex(0)
                .unwrap()
                .submenu()
                .unwrap();
            assert_eq!(
                command_title(&app_menu, sel!(showSettings:)),
                crate::localization::translate_in(language, "Settings…")
            );
            assert_eq!(
                command_title(&app_menu, sel!(showAbout:)),
                crate::localization::translate_in(language, "About Pipmark")
            );
            let item = Item {
                id: "app".into(),
                name: "Settings…".into(),
                enabled: true,
                appearance: Default::default(),
                kind: ItemKind::App {
                    path: "/test.app".into(),
                },
            };
            delegate.ivars().settings.borrow_mut().items = vec![item.clone()];
            let group_menu = NSMenu::new(mtm);
            delegate.group_menu(&group_menu, &[item]);
            assert_eq!(
                group_menu.itemAtIndex(0).unwrap().title().to_string(),
                "Settings…",
                "app names must not be translated"
            );
            delegate.ivars().settings.borrow_mut().items.clear();
        }
        delegate.remove_appearance_observers();
        for status in delegate.ivars().status.borrow_mut().drain(..) {
            NSStatusBar::systemStatusBar().removeStatusItem(&status);
        }
        println!(
            "German/Japanese native menus, selected-language checks and app-name preservation passed."
        );
    }
}
fn main() {
    app::check();
}
