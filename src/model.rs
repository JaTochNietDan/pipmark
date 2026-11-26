use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BadgeStyle {
    #[default]
    Text,
    Dot,
    Circle,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    #[default]
    Show,
    Dim,
    Hide,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttentionStyle {
    #[default]
    None,
    Pulse,
    Blink,
    DoubleBlink,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub badge_style: BadgeStyle,
    pub attention: AttentionStyle,
    pub show_count: bool,
    pub notifications: bool,
    pub show_activity_dot: bool,
    pub running_indicator: bool,
    pub pinned: bool,
    pub when_empty: Visibility,
    pub when_stopped: Visibility,
    pub icon_size: u16,
    pub color: [f64; 3],
    pub custom_icon: Option<String>,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            badge_style: BadgeStyle::Text,
            attention: AttentionStyle::None,
            show_count: true,
            notifications: true,
            show_activity_dot: true,
            running_indicator: false,
            pinned: false,
            when_empty: Visibility::Show,
            when_stopped: Visibility::Show,
            icon_size: 18,
            color: [1.0, 0.23, 0.19],
            custom_icon: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ItemKind {
    App { path: String },
    Group { children: Vec<Item> },
    Trash,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(flatten)]
    pub kind: ItemKind,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ko")]
    Korean,
    #[serde(rename = "zh-Hans")]
    SimplifiedChinese,
    #[serde(rename = "zh-Hant")]
    TraditionalChinese,
}
impl Language {
    pub const ALL: [Self; 7] = [
        Self::System,
        Self::English,
        Self::German,
        Self::Japanese,
        Self::Korean,
        Self::SimplifiedChinese,
        Self::TraditionalChinese,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::System => "System Default",
            Self::English => "English",
            Self::German => "Deutsch",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::SimplifiedChinese => "简体中文",
            Self::TraditionalChinese => "繁體中文",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub show_on_start: bool,
    pub language: Language,
    pub defaults: Appearance,
    pub items: Vec<Item>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            show_on_start: true,
            language: Language::System,
            defaults: Appearance::default(),
            items: vec![],
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    pub running: bool,
    pub label: String,
}
impl Activity {
    pub fn has_unread(&self) -> bool {
        !self.label.trim().is_empty() && self.label.trim() != "0"
    }
    pub fn count(&self) -> u64 {
        self.label
            .replace([',', '.', ' ', '\u{a0}'], "")
            .trim_end_matches('+')
            .parse()
            .unwrap_or(0)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
    Hidden,
    Dimmed,
    Visible,
}
impl Item {
    pub fn supports_activity_dot(&self) -> bool {
        matches!(self.kind, ItemKind::App { .. })
            && matches!(
                self.id.as_str(),
                "com.tinyspeck.slackmacgap" | "com.hnc.Discord"
            )
    }
    pub fn display_activity(&self, mut activity: Activity) -> Activity {
        if self.supports_activity_dot()
            && !self.appearance.show_activity_dot
            && matches!(activity.label.trim(), "." | "•" | "●" | "·")
        {
            activity.label.clear();
        }
        activity
    }

    pub fn presentation(&self, state: &Activity) -> Presentation {
        if !self.enabled {
            return Presentation::Hidden;
        }
        if self.appearance.pinned || state.has_unread() {
            return Presentation::Visible;
        }
        let empty = self.appearance.when_empty;
        let stopped = if state.running {
            Visibility::Show
        } else {
            self.appearance.when_stopped
        };
        if empty == Visibility::Hide || stopped == Visibility::Hide {
            Presentation::Hidden
        } else if empty == Visibility::Dim || stopped == Visibility::Dim {
            Presentation::Dimmed
        } else {
            Presentation::Visible
        }
    }
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Item> {
        if self.id == id {
            return Some(self);
        }
        if let ItemKind::Group { children } = &mut self.kind {
            return children.iter_mut().find_map(|i| i.find_mut(id));
        }
        None
    }
}
impl Settings {
    /// Only enabled app leaves contribute to visible app/group activity.
    pub fn monitored_apps(&self) -> HashSet<String> {
        fn visit(items: &[Item], ids: &mut HashSet<String>) {
            for item in items.iter().filter(|item| item.enabled) {
                match &item.kind {
                    ItemKind::App { .. } => {
                        ids.insert(item.id.clone());
                    }
                    ItemKind::Group { children } => visit(children, ids),
                    ItemKind::Trash => {}
                }
            }
        }
        let mut ids = HashSet::new();
        visit(&self.items, &mut ids);
        ids
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported settings version".into());
        }
        fn appearance(a: &Appearance) -> Result<(), String> {
            if !(12..=24).contains(&a.icon_size)
                || a.color
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err("Invalid appearance settings".into());
            }
            Ok(())
        }
        appearance(&self.defaults)?;
        fn visit(items: &[Item], ids: &mut HashSet<String>, depth: usize) -> Result<(), String> {
            if depth > 16 {
                return Err("Groups are nested too deeply".into());
            }
            for item in items {
                if item.id.is_empty() || !ids.insert(item.id.clone()) {
                    return Err("Item IDs must be unique and nonempty".into());
                }
                if item.name.trim().is_empty() {
                    return Err("Items need a name".into());
                }
                appearance(&item.appearance)?;
                if let ItemKind::Group { children } = &item.kind {
                    visit(children, ids, depth + 1)?;
                }
            }
            Ok(())
        }
        visit(&self.items, &mut HashSet::new(), 0)
    }
}
pub fn aggregate(states: impl Iterator<Item = Activity>) -> Activity {
    let mut result = Activity::default();
    let mut count = 0u64;
    let mut unread = false;
    for s in states {
        result.running |= s.running;
        unread |= s.has_unread();
        count = count.saturating_add(s.count());
    }
    result.label = if count > 0 {
        count.to_string()
    } else if unread {
        "•".into()
    } else {
        String::new()
    };
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item() -> Item {
        Item {
            id: "test".into(),
            name: "Test".into(),
            enabled: true,
            appearance: Appearance::default(),
            kind: ItemKind::Trash,
        }
    }
    #[test]
    fn unread_survives_stopped_hide() {
        let mut i = item();
        i.appearance.when_stopped = Visibility::Hide;
        assert_eq!(
            i.presentation(&Activity {
                running: false,
                label: "3".into()
            }),
            Presentation::Visible
        );
        assert_eq!(i.presentation(&Activity::default()), Presentation::Hidden);
    }
    #[test]
    fn pinned_overrides_rules_but_not_disabled() {
        let mut i = item();
        i.appearance.pinned = true;
        i.appearance.when_empty = Visibility::Hide;
        assert_eq!(i.presentation(&Activity::default()), Presentation::Visible);
        i.enabled = false;
        assert_eq!(i.presentation(&Activity::default()), Presentation::Hidden);
    }
    #[test]
    fn group_preserves_non_numeric_attention() {
        assert_eq!(
            aggregate(
                [Activity {
                    running: true,
                    label: "•".into()
                }]
                .into_iter()
            )
            .label,
            "•"
        );
        assert_eq!(
            aggregate(
                [
                    Activity {
                        running: true,
                        label: "1,234".into()
                    },
                    Activity {
                        running: false,
                        label: "8".into()
                    }
                ]
                .into_iter()
            )
            .label,
            "1242"
        );
    }
    #[test]
    fn rejects_duplicate_nested_ids() {
        let i = item();
        let mut group = item();
        group.id = "group".into();
        group.kind = ItemKind::Group {
            children: vec![i.clone()],
        };
        let s = Settings {
            items: vec![i, group],
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }
}

impl Settings {
    /// Group a selection atomically. Selected ancestors carry their whole subtree;
    /// selecting a descendant too must not detach it or duplicate it.
    pub fn create_group(
        &mut self,
        id: String,
        name: String,
        selected: &HashSet<String>,
    ) -> Result<(), String> {
        if selected
            .iter()
            .any(|id| !self.rows().iter().any(|(item, _)| &item.id == id))
        {
            return Err("A selected item no longer exists".into());
        }
        fn extract(items: &mut Vec<Item>, selected: &HashSet<String>, output: &mut Vec<Item>) {
            let mut index = 0;
            while index < items.len() {
                if selected.contains(&items[index].id) {
                    output.push(items.remove(index));
                } else {
                    if let ItemKind::Group { children } = &mut items[index].kind {
                        extract(children, selected, output);
                    }
                    index += 1;
                }
            }
        }
        let mut next = self.clone();
        let mut children = Vec::new();
        extract(&mut next.items, selected, &mut children);
        next.items.push(Item {
            id,
            name: name.trim().to_owned(),
            enabled: true,
            appearance: next.defaults.clone(),
            kind: ItemKind::Group { children },
        });
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn rows(&self) -> Vec<(&Item, usize)> {
        fn walk<'a>(items: &'a [Item], depth: usize, rows: &mut Vec<(&'a Item, usize)>) {
            for item in items {
                rows.push((item, depth));
                if let ItemKind::Group { children } = &item.kind {
                    walk(children, depth + 1, rows);
                }
            }
        }
        let mut rows = Vec::new();
        walk(&self.items, 0, &mut rows);
        rows
    }
    pub fn item_mut(&mut self, id: &str) -> Option<&mut Item> {
        self.items.iter_mut().find_map(|i| i.find_mut(id))
    }
    fn take(items: &mut Vec<Item>, id: &str) -> Option<Item> {
        if let Some(index) = items.iter().position(|i| i.id == id) {
            return Some(items.remove(index));
        }
        for item in items {
            if let ItemKind::Group { children } = &mut item.kind {
                if let Some(found) = Self::take(children, id) {
                    return Some(found);
                }
            }
        }
        None
    }
    /// Transactional move: reject cycles, missing destinations and excessive depth.
    pub fn move_item(&mut self, id: &str, parent: Option<&str>) -> Result<(), String> {
        let mut next = self.clone();
        let item = Self::take(&mut next.items, id).ok_or("Item no longer exists")?;
        if let Some(parent) = parent {
            let target = next
                .item_mut(parent)
                .ok_or("Cannot move an item into itself or a descendant")?;
            let ItemKind::Group { children } = &mut target.kind else {
                return Err("Destination is not a group".into());
            };
            children.push(item);
        } else {
            next.items.push(item);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// Move beside an anchor, preserving subtrees and adopting its parent atomically.
    pub fn move_relative(&mut self, id: &str, anchor: &str, after: bool) -> Result<(), String> {
        if id == anchor {
            return Ok(());
        }
        fn insert(
            items: &mut Vec<Item>,
            anchor: &str,
            after: bool,
            moved: &mut Option<Item>,
        ) -> bool {
            if let Some(index) = items.iter().position(|item| item.id == anchor) {
                items.insert(index + usize::from(after), moved.take().unwrap());
                return true;
            }
            for item in items {
                if let ItemKind::Group { children } = &mut item.kind {
                    if insert(children, anchor, after, moved) {
                        return true;
                    }
                }
            }
            false
        }
        let mut next = self.clone();
        let mut moved = Some(Self::take(&mut next.items, id).ok_or("Item no longer exists")?);
        if !insert(&mut next.items, anchor, after, &mut moved) {
            return Err("Cannot move beside a missing item or a descendant".into());
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// Move within the current parent; never detach a child or cross group boundaries.
    pub fn reorder(&mut self, id: &str, earlier: bool) -> bool {
        fn walk(items: &mut [Item], id: &str, earlier: bool) -> bool {
            if let Some(index) = items.iter().position(|i| i.id == id) {
                let next = if earlier {
                    index.checked_sub(1)
                } else {
                    index.checked_add(1).filter(|n| *n < items.len())
                };
                if let Some(next) = next {
                    items.swap(index, next);
                    return true;
                }
                return false;
            }
            for item in items {
                if let ItemKind::Group { children } = &mut item.kind {
                    if walk(children, id, earlier) {
                        return true;
                    }
                }
            }
            false
        }
        walk(&mut self.items, id, earlier)
    }
    /// Removing a group ungroups its children rather than removing their settings.
    pub fn remove_item(&mut self, id: &str) {
        fn remove(items: &mut Vec<Item>, id: &str) -> bool {
            if let Some(index) = items.iter().position(|i| i.id == id) {
                let item = items.remove(index);
                if let ItemKind::Group { children } = item.kind {
                    items.splice(index..index, children);
                }
                return true;
            }
            for item in items {
                if let ItemKind::Group { children } = &mut item.kind {
                    if remove(children, id) {
                        return true;
                    }
                }
            }
            false
        }
        remove(&mut self.items, id);
    }
}
#[cfg(test)]
mod hierarchy_tests {
    use super::*;
    #[test]
    fn relative_moves_preserve_subtrees_and_adopt_anchor_parent() {
        let mut settings = Settings {
            items: vec![
                group("a", vec![group("child", vec![])]),
                group("b", vec![group("target", vec![])]),
                group("c", vec![]),
            ],
            ..Default::default()
        };
        settings.items[0].appearance.icon_size = 22;
        settings.move_relative("a", "target", false).unwrap();
        assert_eq!(
            settings
                .rows()
                .iter()
                .map(|(i, d)| (i.id.as_str(), *d))
                .collect::<Vec<_>>(),
            vec![("b", 0), ("a", 1), ("child", 2), ("target", 1), ("c", 0)]
        );
        assert_eq!(settings.item_mut("a").unwrap().appearance.icon_size, 22);
        settings.move_relative("a", "c", true).unwrap();
        assert_eq!(
            settings
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "c", "a"]
        );
        assert!(settings.validate().is_ok());
    }
    #[test]
    fn relative_moves_reject_descendants_and_missing_targets_atomically() {
        let mut settings = Settings {
            items: vec![group("a", vec![group("child", vec![])])],
            ..Default::default()
        };
        let before = serde_json::to_value(&settings).unwrap();
        assert!(settings.move_relative("a", "child", false).is_err());
        assert!(settings.move_relative("child", "missing", true).is_err());
        assert_eq!(serde_json::to_value(&settings).unwrap(), before);
    }
    #[test]
    fn grouping_selection_preserves_subtrees_order_and_preferences() {
        let mut disabled = group("disabled", vec![]);
        disabled.enabled = false;
        disabled.appearance.icon_size = 22;
        let mut settings = Settings {
            items: vec![
                group("keep", vec![]),
                group("parent", vec![group("child", vec![])]),
                disabled,
            ],
            ..Default::default()
        };
        settings.defaults.running_indicator = true;
        settings
            .create_group(
                "new".into(),
                "  Work  ".into(),
                &["child".into(), "disabled".into(), "parent".into()].into(),
            )
            .unwrap();
        assert_eq!(
            settings
                .rows()
                .iter()
                .map(|(i, _)| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["keep", "new", "parent", "child", "disabled"]
        );
        assert_eq!(settings.items[1].name, "Work");
        assert!(settings.items[1].appearance.running_indicator);
        let disabled = settings.item_mut("disabled").unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.appearance.icon_size, 22);
        assert!(settings.validate().is_ok());
    }
    #[test]
    fn invalid_group_creation_is_atomic() {
        let mut settings = Settings {
            items: vec![group("existing", vec![])],
            ..Default::default()
        };
        let before = serde_json::to_value(&settings).unwrap();
        for (id, name, selected) in [
            ("existing", "Duplicate", vec![]),
            ("new", "   ", vec!["existing"]),
            ("new", "Missing", vec!["missing"]),
        ] {
            assert!(
                settings
                    .create_group(
                        id.into(),
                        name.into(),
                        &selected.into_iter().map(String::from).collect()
                    )
                    .is_err()
            );
            assert_eq!(serde_json::to_value(&settings).unwrap(), before);
        }
        settings
            .create_group("empty".into(), "Empty".into(), &HashSet::new())
            .unwrap();
        assert_eq!(settings.items.len(), 2);
    }
    #[test]
    fn monitoring_follows_enabled_ancestors_and_group_changes() {
        let app = |id: &str| Item {
            kind: ItemKind::App {
                path: format!("/{id}.app"),
            },
            ..group(id, vec![])
        };
        let mut settings = Settings {
            items: vec![
                app("root-app"),
                group(
                    "work",
                    vec![app("mail"), group("nested", vec![app("chat")])],
                ),
                group("empty", vec![]),
            ],
            ..Settings::default()
        };
        let ids = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<HashSet<_>>();
        assert_eq!(
            settings.monitored_apps(),
            ids(&["root-app", "mail", "chat"])
        );
        settings.item_mut("nested").unwrap().enabled = false;
        assert_eq!(settings.monitored_apps(), ids(&["root-app", "mail"]));
        settings.item_mut("work").unwrap().enabled = false;
        assert_eq!(settings.monitored_apps(), ids(&["root-app"]));
        settings.item_mut("root-app").unwrap().enabled = false;
        assert!(settings.monitored_apps().is_empty());
        settings.item_mut("work").unwrap().enabled = true;
        assert_eq!(settings.monitored_apps(), ids(&["mail"]));
    }
    fn group(id: &str, children: Vec<Item>) -> Item {
        Item {
            id: id.into(),
            name: id.into(),
            enabled: true,
            appearance: Appearance::default(),
            kind: ItemKind::Group { children },
        }
    }
    #[test]
    fn reorder_stays_within_parent_and_preserves_subtrees() {
        let mut s = Settings {
            items: vec![group(
                "root",
                vec![
                    group("a", vec![group("nested", vec![])]),
                    group("b", vec![]),
                ],
            )],
            ..Default::default()
        };
        assert!(!s.reorder("a", true));
        assert!(s.reorder("b", true));
        assert_eq!(
            s.rows()
                .iter()
                .map(|(i, _)| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root", "b", "a", "nested"]
        );
        assert!(!s.reorder("a", false));
        assert!(s.reorder("a", true));
        assert_eq!(s.rows()[2].0.id, "nested");
        assert!(s.validate().is_ok());
    }
    #[test]
    fn cycles_are_rejected_without_data_loss() {
        let mut s = Settings {
            items: vec![group("a", vec![group("b", vec![])])],
            ..Default::default()
        };
        let before = serde_json::to_string(&s).unwrap();
        assert!(s.move_item("a", Some("b")).is_err());
        assert_eq!(serde_json::to_string(&s).unwrap(), before);
        s.move_item("b", None).unwrap();
        assert_eq!(s.items.len(), 2);
        s.move_item("a", Some("b")).unwrap();
        assert_eq!(s.rows()[1].0.id, "a");
    }
    #[test]
    fn removing_group_preserves_children_and_order() {
        let mut s = Settings {
            items: vec![group("a", vec![group("b", vec![]), group("c", vec![])])],
            ..Default::default()
        };
        s.remove_item("a");
        assert_eq!(
            s.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["b", "c"]
        );
    }
}
