use crate::{
    dock::{self, Snapshot},
    model::{Item, ItemKind, Presentation},
    storage,
};
use serde_json::{Value, json};
fn kind(item: &Item) -> &'static str {
    match item.kind {
        ItemKind::App { .. } => "app",
        ItemKind::Group { .. } => "group",
        ItemKind::Trash => "trash",
    }
}
fn status_data(item: &Item, depth: usize, snapshot: &Snapshot) -> Value {
    let state = snapshot.activity(item);
    json!({"id":item.id,"type":kind(item),"name":item.name,"enabled":item.enabled,"isRunning":state.running,"shown":depth==0 && item.presentation(&snapshot.display_activity(item))!=Presentation::Hidden,"unread":state.count(),"statusLabel":state.label})
}
fn error(code: &str, message: impl ToString) -> Value {
    json!({"success":false,"error":{"code":code,"message":message.to_string()}})
}
pub fn execute(args: &[String]) -> Result<Value, Value> {
    let settings =
        storage::load(&storage::config_path()).map_err(|e| error("INVALID_SETTINGS", e))?;
    let items = settings.rows();
    match args.first().map(String::as_str) {
        Some("list") if args.len() == 1 => Ok(
            json!({"success":true,"data":items.iter().map(|(i,_)| json!({"id":i.id,"type":kind(i),"name":i.name,"enabled":i.enabled})).collect::<Vec<_>>()}),
        ),
        Some("status") if args.len() == 3 && args[1] == "--id" => {
            let (item, depth) = items
                .into_iter()
                .find(|(i, _)| i.id == args[2])
                .ok_or_else(|| {
                    error(
                        "NOT_FOUND",
                        format!("No app or group found with ID: {}", args[2]),
                    )
                })?;
            if matches!(item.kind, ItemKind::Trash) {
                return Err(error("UNSUPPORTED", "Trash status is not implemented yet"));
            }
            let snapshot = Snapshot::read().map_err(|e| {
                error(
                    match e {
                        dock::Error::Permission => "NO_PERMISSIONS",
                        _ => "RUNTIME_ERROR",
                    },
                    e,
                )
            })?;
            Ok(json!({"success":true,"data":status_data(item,depth,&snapshot)}))
        }
        _ => Err(error(
            "INVALID_ARGUMENT",
            "Usage: pipmark list | status --id <id>",
        )),
    }
}
pub fn run(args: &[String]) -> i32 {
    if args == ["--help"] || args == ["-h"] {
        println!(
            "Pipmark\n\nWithout arguments: start the native menu bar app.\nlist                     List configured apps and groups as JSON.\nstatus --id <id>         Read current Dock badge status as JSON."
        );
        return 0;
    }
    match execute(args) {
        Ok(value) => {
            println!("{value}");
            0
        }
        Err(value) => {
            eprintln!("{value}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Activity, Appearance};
    fn app(id: &str, enabled: bool) -> Item {
        Item {
            id: id.into(),
            name: id.into(),
            enabled,
            appearance: Appearance::default(),
            kind: ItemKind::App {
                path: "/test.app".into(),
            },
        }
    }
    #[test]
    fn suppressed_activity_dots_keep_raw_status_and_numbered_badges() {
        let mut slack = app("com.tinyspeck.slackmacgap", true);
        slack.appearance.show_activity_dot = false;
        slack.appearance.when_empty = crate::model::Visibility::Hide;
        let mut snapshot = Snapshot::default();
        for dot in [".", "•", "●", "·"] {
            snapshot.apps.insert(
                slack.id.clone(),
                Activity {
                    running: true,
                    label: dot.into(),
                },
            );
            let data = status_data(&slack, 0, &snapshot);
            assert_eq!(data["statusLabel"], dot);
            assert_eq!(data["shown"], false);
            assert!(snapshot.display_activity(&slack).running);
            let group = Item {
                kind: ItemKind::Group {
                    children: vec![slack.clone()],
                },
                ..app("group", true)
            };
            assert!(!snapshot.display_activity(&group).has_unread());
        }
        for label in ["3", "99+", "!"] {
            snapshot.apps.insert(
                slack.id.clone(),
                Activity {
                    running: true,
                    label: label.into(),
                },
            );
            assert_eq!(snapshot.display_activity(&slack).label, label);
            assert_eq!(status_data(&slack, 0, &snapshot)["shown"], true);
        }
        let old: Appearance = serde_json::from_str("{}").unwrap();
        assert!(
            old.show_activity_dot,
            "old settings retain existing behavior"
        );
        slack.id = "other.app".into();
        assert_eq!(
            slack
                .display_activity(Activity {
                    running: true,
                    label: "•".into()
                })
                .label,
            "•"
        );
        slack.id = "com.hnc.Discord".into();
        assert!(
            !slack
                .display_activity(Activity {
                    running: true,
                    label: "•".into()
                })
                .has_unread()
        );
    }
    #[test]
    fn nested_item_reports_activity_without_claiming_a_standalone_icon() {
        let item = app("app", true);
        let snapshot = Snapshot {
            apps: [(
                "app".into(),
                Activity {
                    running: true,
                    label: "12".into(),
                },
            )]
            .into(),
        };
        assert_eq!(status_data(&item, 0, &snapshot)["shown"], true);
        let nested = status_data(&item, 2, &snapshot);
        assert_eq!(nested["shown"], false);
        assert_eq!(nested["unread"], 12);
        assert_eq!(nested["isRunning"], true);
    }
    #[test]
    fn group_query_excludes_disabled_subtrees() {
        let nested = Item {
            id: "nested".into(),
            name: "Nested".into(),
            enabled: false,
            appearance: Appearance::default(),
            kind: ItemKind::Group {
                children: vec![app("b", true)],
            },
        };
        let group = Item {
            id: "group".into(),
            name: "Group".into(),
            enabled: true,
            appearance: Appearance::default(),
            kind: ItemKind::Group {
                children: vec![app("a", true), nested],
            },
        };
        let snapshot = Snapshot {
            apps: [
                (
                    "a".into(),
                    Activity {
                        running: true,
                        label: "4".into(),
                    },
                ),
                (
                    "b".into(),
                    Activity {
                        running: true,
                        label: "9".into(),
                    },
                ),
            ]
            .into(),
        };
        let data = status_data(&group, 0, &snapshot);
        assert_eq!(data["unread"], 4);
        assert_eq!(data["shown"], true);
    }
}
