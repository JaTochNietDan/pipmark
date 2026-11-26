//! Dock Accessibility adapter: badge reads and explicit user-requested Trash actions.
//! No permission prompts or private databases.
//! CF Create/Copy results are owned by `Owned`; borrowed array entries are retained.
use crate::model::{Activity, Item, ItemKind, aggregate};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSBundle, NSString, NSURL};
use std::{
    collections::{HashMap, HashSet},
    ffi::{c_char, c_void},
    ptr,
};

type Ref = *const c_void;
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> Ref;
    fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
    fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
    fn AXUIElementPerformAction(element: Ref, action: Ref) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Ref);
    fn CFRetain(value: Ref) -> Ref;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(value: Ref) -> bool;
    fn CFURLGetTypeID() -> usize;
    fn CFStringCreateWithBytes(
        allocator: Ref,
        bytes: *const u8,
        len: isize,
        encoding: u32,
        external: bool,
    ) -> Ref;
    fn CFStringGetLength(value: Ref) -> isize;
    fn CFStringGetMaximumSizeForEncoding(len: isize, encoding: u32) -> isize;
    fn CFStringGetCString(value: Ref, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
    fn CFArrayGetCount(value: Ref) -> isize;
    fn CFArrayGetValueAtIndex(value: Ref, index: isize) -> Ref;
    fn CFURLCopyFileSystemPath(value: Ref, style: isize) -> Ref;
}
struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}
impl Owned {
    fn created(value: Ref) -> Option<Self> {
        if value.is_null() {
            None
        } else {
            Some(Self(value))
        }
    }
    fn string(text: &str) -> Self {
        Self::created(unsafe {
            CFStringCreateWithBytes(
                ptr::null(),
                text.as_ptr(),
                text.len() as isize,
                0x08000100,
                false,
            )
        })
        .expect("CFString allocation")
    }
    fn text(&self) -> Option<String> {
        unsafe {
            if CFGetTypeID(self.0) != CFStringGetTypeID() {
                return None;
            }
            let capacity =
                CFStringGetMaximumSizeForEncoding(CFStringGetLength(self.0), 0x08000100) + 1;
            if !(1..=1_048_576).contains(&capacity) {
                return None;
            }
            let mut bytes = vec![0u8; capacity as usize];
            if !CFStringGetCString(self.0, bytes.as_mut_ptr().cast(), capacity, 0x08000100) {
                return None;
            }
            let end = bytes.iter().position(|b| *b == 0)?;
            String::from_utf8(bytes[..end].to_vec()).ok()
        }
    }
    fn attribute(&self, name: &str) -> Result<Option<Self>, i32> {
        let attr = Self::string(name);
        let mut value = ptr::null();
        let code = unsafe { AXUIElementCopyAttributeValue(self.0, attr.0, &mut value) };
        match code {
            0 => Ok(Self::created(value)),
            -25205 | -25212 => Ok(None),
            code => Err(code),
        }
    }
    fn children(&self) -> Result<Vec<Self>, i32> {
        let Some(array) = self.attribute("AXChildren")? else {
            return Ok(vec![]);
        };
        unsafe {
            if CFGetTypeID(array.0) != CFArrayGetTypeID() {
                return Ok(vec![]);
            }
            Ok((0..CFArrayGetCount(array.0).min(2048))
                .filter_map(|i| Self::created(CFRetain(CFArrayGetValueAtIndex(array.0, i))))
                .collect())
        }
    }
    fn path(&self) -> Option<String> {
        unsafe {
            if CFGetTypeID(self.0) != CFURLGetTypeID() {
                return None;
            }
            Self::created(CFURLCopyFileSystemPath(self.0, 0))?.text()
        }
    }
}
#[derive(Debug)]
pub enum Error {
    Permission,
    DockUnavailable,
    TrashUnavailable,
    TrashActionUnavailable,
    Accessibility(i32),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Permission => write!(
                f,
                "Accessibility permission is required to read Dock badges"
            ),
            Self::DockUnavailable => write!(f, "The Dock is not available"),
            Self::TrashUnavailable => write!(f, "The Dock Trash item is not available"),
            Self::TrashActionUnavailable => {
                write!(f, "The Dock Empty Trash action is unavailable or disabled")
            }
            Self::Accessibility(code) => write!(f, "Dock Accessibility request failed ({code})"),
        }
    }
}
pub fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}
/// Ask the Dock to open its Trash, including trashed items on mounted volumes.
/// No Finder Automation permission or hard-coded home-directory path is needed.
pub fn open_trash() -> Result<(), Error> {
    let (_, tile) = trash_tile()?;
    perform(&tile, "AXPress")
}
fn perform(element: &Owned, name: &str) -> Result<(), Error> {
    let action = Owned::string(name);
    match unsafe { AXUIElementPerformAction(element.0, action.0) } {
        0 => Ok(()),
        code => Err(Error::Accessibility(code)),
    }
}
fn trash_tile() -> Result<(Owned, Owned), Error> {
    if !trusted() {
        return Err(Error::Permission);
    }
    let pid = NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .find(|app| {
            app.bundleIdentifier()
                .is_some_and(|id| id.to_string() == "com.apple.dock")
        })
        .map(|app| app.processIdentifier())
        .ok_or(Error::DockUnavailable)?;
    let root = Owned::created(unsafe { AXUIElementCreateApplication(pid) })
        .ok_or(Error::DockUnavailable)?;
    unsafe {
        AXUIElementSetMessagingTimeout(root.0, 0.25);
    }
    for list in root.children().map_err(Error::Accessibility)? {
        if list
            .attribute("AXRole")
            .map_err(Error::Accessibility)?
            .and_then(|v| v.text())
            .as_deref()
            != Some("AXList")
        {
            continue;
        }
        for tile in list.children().map_err(Error::Accessibility)? {
            if tile
                .attribute("AXSubrole")
                .map_err(Error::Accessibility)?
                .and_then(|v| v.text())
                .as_deref()
                != Some("AXTrashDockItem")
            {
                continue;
            }
            unsafe {
                AXUIElementSetMessagingTimeout(tile.0, 0.25);
            }
            return Ok((root, tile));
        }
    }
    Err(Error::TrashUnavailable)
}

fn same_menu_title(actual: &str, expected: &str) -> bool {
    fn normalized(value: &str) -> &str {
        value
            .trim()
            .trim_end_matches('…')
            .trim_end_matches("...")
            .trim_end()
    }
    !expected.is_empty() && normalized(actual) == normalized(expected)
}
/// Resolve the Dock's own system-language title, independent of our UI language.
pub(crate) fn empty_trash_title() -> Result<String, Error> {
    let bundle =
        NSBundle::bundleWithPath(&NSString::from_str("/System/Library/CoreServices/Dock.app"))
            .ok_or(Error::DockUnavailable)?;
    let title = bundle
        .localizedStringForKey_value_table(
            &NSString::from_str("EMPTY_TRASH"),
            None,
            Some(&NSString::from_str("DockMenus")),
        )
        .to_string();
    if title.is_empty() || title == "EMPTY_TRASH" {
        Err(Error::TrashActionUnavailable)
    } else {
        Ok(title)
    }
}
fn enabled_menu_action(role: &str, title: &str, enabled: Option<bool>, expected: &str) -> bool {
    role == "AXMenuItem" && enabled == Some(true) && same_menu_title(title, expected)
}
// Keep traversal and cleanup independent of IPC so failure paths can be tested
// without opening a real Dock menu or invoking a destructive action.
trait TrashMenuNode: Sized {
    fn text_attribute(&self, key: &str) -> Result<String, Error>;
    fn enabled(&self) -> Result<Option<bool>, Error>;
    fn nodes(&self) -> Result<Vec<Self>, Error>;
    fn action(&self, name: &str) -> Result<(), Error>;
}
impl TrashMenuNode for Owned {
    fn text_attribute(&self, key: &str) -> Result<String, Error> {
        Ok(self
            .attribute(key)
            .map_err(Error::Accessibility)?
            .and_then(|v| v.text())
            .unwrap_or_default())
    }
    fn enabled(&self) -> Result<Option<bool>, Error> {
        Ok(self
            .attribute("AXEnabled")
            .map_err(Error::Accessibility)?
            .and_then(|v| unsafe {
                (CFGetTypeID(v.0) == CFBooleanGetTypeID()).then(|| CFBooleanGetValue(v.0))
            }))
    }
    fn nodes(&self) -> Result<Vec<Self>, Error> {
        self.children().map_err(Error::Accessibility)
    }
    fn action(&self, name: &str) -> Result<(), Error> {
        perform(self, name)
    }
}
fn find_trash_menu<N: TrashMenuNode>(node: N, depth: usize) -> Result<Option<N>, Error> {
    if depth > 4 {
        return Ok(None);
    }
    let role = node.text_attribute("AXRole")?;
    if role == "AXMenu" {
        return Ok(Some(node));
    }
    if !matches!(role.as_str(), "AXApplication" | "AXList" | "AXDockItem") {
        return Ok(None);
    }
    if role == "AXDockItem" && node.text_attribute("AXSubrole")? != "AXTrashDockItem" {
        return Ok(None);
    }
    for child in node.nodes()?.into_iter().take(2048) {
        if let Some(menu) = find_trash_menu(child, depth + 1)? {
            return Ok(Some(menu));
        }
    }
    Ok(None)
}
struct OpenTrashMenu<N: TrashMenuNode> {
    node: N,
    completed: bool,
}
impl<N: TrashMenuNode> Drop for OpenTrashMenu<N> {
    fn drop(&mut self) {
        // AXCancel belongs to the open menu, not its Dock tile. Best effort:
        // retain the original lookup/press error if the menu already vanished.
        if !self.completed {
            let _ = self.node.action("AXCancel");
        }
    }
}
fn invoke_empty_action<N: TrashMenuNode>(menu: N, title: &str) -> Result<(), Error> {
    let mut guard = OpenTrashMenu {
        node: menu,
        completed: false,
    };
    let mut candidate = None;
    for child in guard.node.nodes()?.into_iter().take(64) {
        let role = child.text_attribute("AXRole")?;
        if role != "AXMenuItem" {
            continue;
        }
        let actual = child.text_attribute("AXTitle")?;
        if !same_menu_title(&actual, title) {
            continue;
        }
        if !enabled_menu_action(&role, &actual, child.enabled()?, title) || candidate.is_some() {
            return Err(Error::TrashActionUnavailable);
        }
        candidate = Some(child);
    }
    candidate
        .ok_or(Error::TrashActionUnavailable)?
        .action("AXPress")?;
    guard.completed = true;
    Ok(())
}
/// Invoke the Dock's action; macOS owns its confirmation UI and Trash semantics.
/// Call only following an explicit Empty Trash menu action. Never used by monitoring.
pub fn empty_trash() -> Result<(), Error> {
    let title = empty_trash_title()?;
    let (root, tile) = trash_tile()?;
    // AXShowMenu can time out while the Dock is tracking the newly opened menu.
    match perform(&tile, "AXShowMenu") {
        Ok(()) | Err(Error::Accessibility(-25204)) => (),
        Err(error) => return Err(error),
    }
    for _ in 0..10 {
        // Prefer the requested tile's menu. Some Dock versions expose menus at
        // application level instead, so retain that fallback.
        let tile_copy = Owned::created(unsafe { CFRetain(tile.0) }).unwrap();
        let root_copy = Owned::created(unsafe { CFRetain(root.0) }).unwrap();
        let menu = match find_trash_menu(tile_copy, 0)? {
            Some(menu) => Some(menu),
            None => find_trash_menu(root_copy, 0)?,
        };
        if let Some(menu) = menu {
            return invoke_empty_action(menu, &title);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(Error::TrashActionUnavailable)
}
#[cfg(test)]
mod trash_tests {
    use super::*;
    #[derive(Clone)]
    struct Node {
        id: &'static str,
        role: &'static str,
        subrole: &'static str,
        title: &'static str,
        enabled: Option<bool>,
        children: Vec<Node>,
        fail_read: bool,
        fail_press: bool,
        actions: std::rc::Rc<std::cell::RefCell<Vec<(&'static str, String)>>>,
    }
    impl Node {
        fn new(id: &'static str, role: &'static str) -> Self {
            Self {
                id,
                role,
                subrole: "",
                title: "",
                enabled: Some(true),
                children: vec![],
                fail_read: false,
                fail_press: false,
                actions: Default::default(),
            }
        }
        fn item(id: &'static str, title: &'static str, parent: &Node) -> Self {
            Self {
                title,
                actions: parent.actions.clone(),
                ..Self::new(id, "AXMenuItem")
            }
        }
    }
    impl TrashMenuNode for Node {
        fn text_attribute(&self, key: &str) -> Result<String, Error> {
            Ok(match key {
                "AXRole" => self.role,
                "AXSubrole" => self.subrole,
                "AXTitle" => self.title,
                _ => "",
            }
            .into())
        }
        fn enabled(&self) -> Result<Option<bool>, Error> {
            Ok(self.enabled)
        }
        fn nodes(&self) -> Result<Vec<Self>, Error> {
            if self.fail_read {
                Err(Error::Accessibility(-25202))
            } else {
                Ok(self.children.clone())
            }
        }
        fn action(&self, name: &str) -> Result<(), Error> {
            self.actions.borrow_mut().push((self.id, name.into()));
            if name == "AXPress" && self.fail_press {
                Err(Error::Accessibility(-25204))
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn trash_menu_is_found_at_tile_or_application_level_without_other_app_menus() {
        let menu = Node::new("menu", "AXMenu");
        let mut trash = Node::new("trash", "AXDockItem");
        trash.subrole = "AXTrashDockItem";
        trash.children.push(menu.clone());
        let mut other = trash.clone();
        other.subrole = "AXApplicationDockItem";
        assert!(find_trash_menu(other.clone(), 0).unwrap().is_none());
        let mut list = Node::new("list", "AXList");
        list.children = vec![other, trash];
        let mut root = Node::new("dock", "AXApplication");
        root.children.push(list);
        assert_eq!(
            find_trash_menu(root.clone(), 0).unwrap().unwrap().id,
            "menu"
        );
        root.children = vec![menu];
        assert_eq!(find_trash_menu(root, 0).unwrap().unwrap().id, "menu");
    }
    #[test]
    fn menu_failure_cancels_menu_and_never_presses_disabled_or_ambiguous_actions() {
        for failure in [
            "disabled",
            "unknown",
            "missing",
            "duplicate",
            "read",
            "press",
        ] {
            let mut menu = Node::new("menu", "AXMenu");
            let mut item = Node::item("empty", "Papierkorb entleeren…", &menu);
            item.enabled = match failure {
                "disabled" => Some(false),
                "unknown" => None,
                _ => Some(true),
            };
            item.fail_press = failure == "press";
            menu.fail_read = failure == "read";
            menu.children = if failure == "missing" {
                vec![]
            } else if failure == "duplicate" {
                vec![item.clone(), item]
            } else {
                vec![item]
            };
            let actions = menu.actions.clone();
            assert!(invoke_empty_action(menu, "Papierkorb entleeren").is_err());
            let expected = if failure == "press" {
                vec![("empty", "AXPress".into()), ("menu", "AXCancel".into())]
            } else {
                vec![("menu", "AXCancel".into())]
            };
            assert_eq!(*actions.borrow(), expected, "{failure}");
        }
    }
    #[test]
    fn matching_action_ignores_menu_position_and_does_not_cancel_successful_press() {
        let mut menu = Node::new("menu", "AXMenu");
        menu.children = vec![
            Node::item("open", "Open", &menu),
            Node::item("empty", "Empty Trash…", &menu),
        ];
        let actions = menu.actions.clone();
        invoke_empty_action(menu, "Empty Trash").unwrap();
        assert_eq!(*actions.borrow(), vec![("empty", "AXPress".into())]);
    }
    #[test]
    fn empty_action_requires_exact_localized_title_role_and_enabled_state() {
        for title in [
            "Empty Trash",
            "Papierkorb entleeren",
            "ゴミ箱を空にする",
            "清倒垃圾桶",
        ] {
            assert!(enabled_menu_action(
                "AXMenuItem",
                &format!("{title}…"),
                Some(true),
                title
            ));
            assert!(enabled_menu_action(
                "AXMenuItem",
                &format!("{title}..."),
                Some(true),
                title
            ));
            assert!(!enabled_menu_action(
                "AXMenuItem",
                title,
                Some(false),
                title
            ));
            assert!(!enabled_menu_action("AXMenuItem", title, None, title));
            assert!(!enabled_menu_action("AXDockItem", title, Some(true), title));
            assert!(!enabled_menu_action(
                "AXMenuItem",
                &format!("{title} now"),
                Some(true),
                title
            ));
        }
        assert!(!enabled_menu_action("AXMenuItem", "", Some(true), ""));
        assert!(!enabled_menu_action(
            "AXMenuItem",
            "Empty Trash",
            Some(true),
            "Papierkorb entleeren"
        ));
    }
}
#[derive(Default, Debug, Clone)]
pub struct Snapshot {
    pub apps: HashMap<String, Activity>,
}
impl Snapshot {
    pub fn read() -> Result<Self, Error> {
        Reader::default().read()
    }
    pub fn activity(&self, item: &Item) -> Activity {
        self.item_activity(item, false)
    }
    pub fn display_activity(&self, item: &Item) -> Activity {
        self.item_activity(item, true)
    }
    fn item_activity(&self, item: &Item, display: bool) -> Activity {
        match &item.kind {
            ItemKind::App { .. } => {
                let raw = self.apps.get(&item.id).cloned().unwrap_or_default();
                if display {
                    item.display_activity(raw)
                } else {
                    raw
                }
            }
            ItemKind::Group { children } => aggregate(
                children
                    .iter()
                    .filter(|i| i.enabled)
                    .map(|i| self.item_activity(i, display)),
            ),
            ItemKind::Trash => Activity::default(),
        }
    }
}

/// AX references live and are released on the reader thread. They never cross channels.
#[derive(Default)]
pub struct Reader {
    targets: Option<HashSet<String>>,
    tiles: Vec<(String, Owned)>,
    signature: Vec<(i32, String)>,
    discovered: Option<std::time::Instant>,
}
impl Reader {
    pub fn for_apps(targets: HashSet<String>) -> Self {
        Self {
            targets: Some(targets),
            ..Self::default()
        }
    }
    fn wants(&self, id: &str) -> bool {
        self.targets
            .as_ref()
            .is_none_or(|targets| targets.contains(id))
    }

    pub fn read(&mut self) -> Result<Snapshot, Error> {
        let result = self.sample();
        if result.is_err() {
            self.tiles.clear();
            self.discovered = None;
        }
        result
    }
    fn sample(&mut self) -> Result<Snapshot, Error> {
        if !trusted() {
            return Err(Error::Permission);
        }
        let running = NSWorkspace::sharedWorkspace().runningApplications();
        let mut signature: Vec<_> = running
            .iter()
            .filter_map(|app| {
                app.bundleIdentifier()
                    .map(|id| (app.processIdentifier(), id.to_string()))
            })
            .collect();
        signature.sort();
        let pid = signature
            .iter()
            .find(|(_, id)| id == "com.apple.dock")
            .map(|(pid, _)| *pid)
            .ok_or(Error::DockUnavailable)?;
        let expired = self
            .discovered
            .is_none_or(|time| time.elapsed() >= std::time::Duration::from_secs(30));
        if expired || signature != self.signature {
            // Build a replacement cache completely before publishing it.
            self.tiles = Self::discover(pid)?
                .into_iter()
                .filter(|(id, _)| self.wants(id))
                .collect();
            self.signature = signature.clone();
            self.discovered = Some(std::time::Instant::now());
        }
        let mut result = Snapshot::default();
        for (_, id) in signature {
            if !self.wants(&id) {
                continue;
            }
            result.apps.insert(
                id,
                Activity {
                    running: true,
                    label: String::new(),
                },
            );
        }
        for (id, tile) in &self.tiles {
            let label = tile
                .attribute("AXStatusLabel")
                .map_err(Error::Accessibility)?
                .and_then(|value| value.text())
                .unwrap_or_default();
            result.apps.entry(id.clone()).or_default().label = label;
        }
        Ok(result)
    }
    fn discover(pid: i32) -> Result<Vec<(String, Owned)>, Error> {
        let root = Owned::created(unsafe { AXUIElementCreateApplication(pid) })
            .ok_or(Error::DockUnavailable)?;
        unsafe {
            AXUIElementSetMessagingTimeout(root.0, 0.25);
        }
        let mut tiles = vec![];
        let mut found_list = false;
        for list in root.children().map_err(Error::Accessibility)? {
            if list
                .attribute("AXRole")
                .map_err(Error::Accessibility)?
                .and_then(|v| v.text())
                .as_deref()
                != Some("AXList")
            {
                continue;
            }
            found_list = true;
            for tile in list.children().map_err(Error::Accessibility)? {
                if tile
                    .attribute("AXSubrole")
                    .map_err(Error::Accessibility)?
                    .and_then(|v| v.text())
                    .as_deref()
                    != Some("AXApplicationDockItem")
                {
                    continue;
                }
                let Some(path) = tile
                    .attribute("AXURL")
                    .map_err(Error::Accessibility)?
                    .and_then(|v| v.path())
                else {
                    continue;
                };
                let url = NSURL::fileURLWithPath(&NSString::from_str(&path));
                let Some(id) = NSBundle::bundleWithURL(&url).and_then(|b| b.bundleIdentifier())
                else {
                    continue;
                };
                // Set the timeout on each retained element, not just its former parent.
                unsafe {
                    AXUIElementSetMessagingTimeout(tile.0, 0.25);
                }
                tiles.push((id.to_string(), tile));
            }
        }
        if !found_list {
            return Err(Error::DockUnavailable);
        }
        Ok(tiles)
    }
}
