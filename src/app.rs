#[path = "updater.rs"]
mod updater;
use crate::{
    dock,
    model::{
        Activity, AttentionStyle, BadgeStyle, Item, ItemKind, Presentation, Settings, Visibility,
    },
    storage,
};
use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject, sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSBundle, NSDictionary, NSKeyValueObservingOptions, NSNotification,
    NSNotificationCenter, NSObject, NSObjectNSKeyValueObserverRegistration, NSObjectProtocol,
    NSPoint, NSRect, NSSize, NSString, NSTimer,
};
use std::cell::RefCell;

struct AppearanceEditor {
    id: String,
    is_defaults: bool,
    group_apps: bool,
    style: Retained<NSPopUpButton>,
    attention: Retained<NSPopUpButton>,
    size: Retained<NSPopUpButton>,
    empty: Retained<NSPopUpButton>,
    stopped: Retained<NSPopUpButton>,
    notifications: Retained<NSSwitch>,
    counts: Retained<NSSwitch>,
    running_indicator: Retained<NSSwitch>,
    activity_dot: Option<Retained<NSSwitch>>,
    pinned: Retained<NSSwitch>,
    color: Retained<NSColorWell>,
}
fn main_list_height(count: usize) -> f64 {
    if count == 0 {
        72.0
    } else {
        count.min(5) as f64 * 36.0
    }
}
fn surface_color(view: &NSView, grouped: bool) -> Retained<NSColor> {
    let dark = view
        .effectiveAppearance()
        .name()
        .to_string()
        .contains("Dark");
    NSColor::colorWithWhite_alpha(
        match (dark, grouped) {
            (false, false) => 1.0,
            (false, true) => 0.965,
            (true, false) => 0.105,
            (true, true) => 0.155,
        },
        1.0,
    )
}
struct SurfaceState {
    grouped: bool,
    row_height: f64,
    rows: usize,
}
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = SurfaceState]
    struct SettingsSurface;
    unsafe impl NSObjectProtocol for SettingsSurface {}
    impl SettingsSurface {
        #[unsafe(method(drawRect:))]
        fn draw_surface(&self, _: NSRect) {
            surface_color(self, self.ivars().grouped).setFill();
            if self.ivars().grouped {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), 12.0, 12.0).fill();
            } else { NSRectFill(self.bounds()); }
            NSColor::separatorColor().setFill();
            for row in 1..self.ivars().rows {
                let y = self.bounds().size.height - row as f64 * self.ivars().row_height;
                if y > 0.0 { NSRectFill(rect(12.0, y, self.bounds().size.width - 24.0, 0.5)); }
            }
        }
    }
);
impl SettingsSurface {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        grouped: bool,
        row_height: f64,
        rows: usize,
    ) -> Retained<Self> {
        let allocated = Self::alloc(mtm).set_ivars(SurfaceState {
            grouped,
            row_height,
            rows,
        });
        unsafe { msg_send![super(allocated), initWithFrame: frame] }
    }
}
const ROW_DRAG_TYPE: &str = "local.badgeify.configured-item";
struct RowState {
    delegate: objc2::rc::Weak<Delegate>,
}
define_class!(
    #[unsafe(super = NSButton)]
    #[thread_kind = MainThreadOnly]
    #[ivars = RowState]
    struct AppListButton;
    unsafe impl NSObjectProtocol for AppListButton {}
    unsafe impl NSDraggingSource for AppListButton {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn drag_operations(&self, _: &NSDraggingSession, context: NSDraggingContext) -> NSDragOperation {
            if context == NSDraggingContext::WithinApplication { NSDragOperation::Move } else { NSDragOperation::None }
        }
    }
    impl AppListButton {
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if event.modifierFlags().contains(NSEventModifierFlags::Control) {
                unsafe { let _: () = msg_send![super(self), mouseDown: event]; }
                return;
            }
            let Some(window) = self.window() else { return; };
            let origin = event.locationInWindow();
            while let Some(next) = window.nextEventMatchingMask(NSEventMask::LeftMouseDragged | NSEventMask::LeftMouseUp | NSEventMask::KeyDown) {
                if next.r#type() == NSEventType::KeyDown { if next.keyCode() == 53 { break; } else { continue; } }
                let point = next.locationInWindow();
                if next.r#type() == NSEventType::LeftMouseUp {
                    let local = self.convertPoint_fromView(point, None);
                    if local.x >= 0.0 && local.x <= self.bounds().size.width && local.y >= 0.0 && local.y <= self.bounds().size.height {
                        if let Some(delegate) = self.ivars().delegate.load() {
                            delegate.select_list_item(self.tag() as usize, event.modifierFlags().intersects(NSEventModifierFlags::Shift | NSEventModifierFlags::Command));
                        }
                    }
                    break;
                }
                if (point.x-origin.x).hypot(point.y-origin.y) >= 4.0 {
                    if let Some(delegate) = self.ivars().delegate.load() {
                        if let Some(id) = delegate.row_id(self.tag() as usize) {
                            let writer = NSPasteboardItem::new();
                            writer.setString_forType(&s(&id), &s(ROW_DRAG_TYPE));
                            let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), ProtocolObject::from_ref(&*writer));
                            unsafe { item.setDraggingFrame_contents(rect(0.0, 4.0, 18.0, 18.0), self.image().as_deref().map(|image| &***image)); }
                            self.beginDraggingSessionWithItems_event_source(&objc2_foundation::NSArray::from_retained_slice(&[item]), &next, ProtocolObject::from_ref(self));
                        }
                    }
                    break;
                }
            }
        }
        #[unsafe(method(drawRect:))]
        fn draw_row(&self, dirty: NSRect) {
            if self.state() == NSControlStateValueOn {
                NSColor::unemphasizedSelectedContentBackgroundColor().setFill();
                let bounds = self.bounds();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    rect(bounds.origin.x, bounds.origin.y + 2.0, bounds.size.width, bounds.size.height - 4.0), 5.0, 5.0
                ).fill();
            }
            unsafe { let _: () = msg_send![super(self), drawRect: dirty]; }
        }
    }
);
impl AppListButton {
    fn new(
        delegate: &Delegate,
        title: &str,
        frame: NSRect,
        action: objc2::runtime::Sel,
    ) -> Retained<NSButton> {
        let allocated = Self::alloc(delegate.mtm()).set_ivars(RowState {
            delegate: objc2::rc::Weak::new(delegate),
        });
        let button: Retained<Self> = unsafe { msg_send![super(allocated), initWithFrame: frame] };
        button.setTitle(&s(title));
        button.setBezelStyle(NSBezelStyle::Push);
        button.setContentTintColor(Some(&NSColor::labelColor()));
        unsafe {
            button.setTarget(Some(delegate));
            button.setAction(Some(action));
        }
        Retained::into_super(button)
    }
}
struct PreviewState {
    delegate: objc2::rc::Weak<Delegate>,
}
define_class!(
    #[unsafe(super = NSPopUpButton)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PreviewState]
    struct StylePreviewButton;
    unsafe impl NSObjectProtocol for StylePreviewButton {}
    impl StylePreviewButton {
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn appearance_changed(&self) {
            unsafe { let _: () = msg_send![super(self), viewDidChangeEffectiveAppearance]; }
            if let Some(delegate) = self.ivars().delegate.load() {
                if delegate.ivars().editor.try_borrow().is_ok() { delegate.update_style_previews(); }
            }
        }
    }
);
impl StylePreviewButton {
    fn new(delegate: &Delegate, frame: NSRect) -> Retained<NSPopUpButton> {
        let allocated = Self::alloc(delegate.mtm()).set_ivars(PreviewState {
            delegate: objc2::rc::Weak::new(delegate),
        });
        let control: Retained<Self> =
            unsafe { msg_send![super(allocated), initWithFrame: frame, pullsDown: false] };
        Retained::into_super(control)
    }
}
struct DropState {
    delegate: objc2::rc::Weak<Delegate>,
    marker: RefCell<Option<(String, bool, NSRect)>>,
}
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DropState]
    struct ApplicationDropView;
    unsafe impl NSObjectProtocol for ApplicationDropView {}
    impl ApplicationDropView {
        #[unsafe(method(drawRect:))]
        fn draw_drop_marker(&self, dirty: NSRect) {
            unsafe { let _: () = msg_send![super(self), drawRect: dirty]; }
            surface_color(self, false).setFill(); NSRectFill(self.bounds());
            surface_color(self, true).setFill();
            let visible = self.ivars().delegate.load().map(|delegate| main_list_height(delegate.ivars().settings.borrow().rows().len())).unwrap_or(72.0);
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(24.0,264.0-visible,512.0,visible+40.0),12.0,12.0).fill();
            if let Some((_,_,line)) = self.ivars().marker.borrow().as_ref() { NSColor::controlAccentColor().setFill(); NSRectFill(*line); }
        }
    }
    unsafe impl NSDraggingDestination for ApplicationDropView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            if sender.draggingPasteboard().stringForType(&s(ROW_DRAG_TYPE)).is_some() { return self.update_row_drop(sender); }
            if sender
                .draggingSourceOperationMask()
                .contains(NSDragOperation::Copy)
                && dropped_urls(&sender.draggingPasteboard())
                    .iter()
                    .any(|url| application_details(url).is_some())
            {
                NSDragOperation::Copy
            } else {
                NSDragOperation::None
            }
        }
        #[unsafe(method(draggingUpdated:))]
        fn dragging_updated(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            if sender.draggingPasteboard().stringForType(&s(ROW_DRAG_TYPE)).is_some() {
                if sender.draggingSourceOperationMask().contains(NSDragOperation::Move) {
                    self.autoscroll_row_drop(self.convertPoint_fromView(sender.draggingLocation(), None));
                }
                self.update_row_drop(sender)
            }
            else if sender.draggingSourceOperationMask().contains(NSDragOperation::Copy) && dropped_urls(&sender.draggingPasteboard()).iter().any(|url| application_details(url).is_some()) { NSDragOperation::Copy }
            else { NSDragOperation::None }
        }
        #[unsafe(method(wantsPeriodicDraggingUpdates))]
        fn periodic_drag_updates(&self) -> bool { true }
        #[unsafe(method(draggingEnded:))]
        fn dragging_ended(&self, _: &ProtocolObject<dyn NSDraggingInfo>) { self.clear_drop_marker(); }
        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _: Option<&ProtocolObject<dyn NSDraggingInfo>>) { self.clear_drop_marker(); }
        #[unsafe(method(performDragOperation:))]
        fn perform_drop(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            if let Some(id) = sender.draggingPasteboard().stringForType(&s(ROW_DRAG_TYPE)) {
                self.update_row_drop(sender);
                let target = self.ivars().marker.borrow_mut().take();
                NSView::setNeedsDisplay(self, true);
                return self.ivars().delegate.load().zip(target).is_some_and(|(delegate,(anchor,after,_))| delegate.move_list_row(&id.to_string(), &anchor, after)).into();
            }
            let urls = dropped_urls(&sender.draggingPasteboard());
            self.ivars().delegate.load().is_some_and(|delegate| {
                delegate
                    .add_application_urls(&objc2_foundation::NSArray::from_retained_slice(&urls))
            })
        }
    }
);
impl ApplicationDropView {
    fn clear_drop_marker(&self) {
        if self.ivars().marker.borrow_mut().take().is_some() {
            NSView::setNeedsDisplay(self, true);
        }
    }
    fn update_row_drop(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
        if !sender
            .draggingSourceOperationMask()
            .contains(NSDragOperation::Move)
        {
            self.clear_drop_marker();
            return NSDragOperation::None;
        }
        let Some(id) = sender.draggingPasteboard().stringForType(&s(ROW_DRAG_TYPE)) else {
            return NSDragOperation::None;
        };
        self.update_row_drop_at(
            &id.to_string(),
            self.convertPoint_fromView(sender.draggingLocation(), None),
        )
    }
    fn list_viewport(&self) -> Option<(Retained<NSScrollView>, NSRect)> {
        let delegate = self.ivars().delegate.load()?;
        let rows = delegate.ivars().list_rows.borrow();
        let scroll = rows.first()?.1.enclosingScrollView()?;
        let clip = scroll.contentView();
        let visible = self.convertRect_fromView(clip.bounds(), Some(&clip));
        Some((scroll, visible))
    }
    // AppKit supplies periodic updates only during a drag; no idle timer is needed.
    fn autoscroll_row_drop(&self, point: NSPoint) {
        let Some((scroll, viewport)) = self.list_viewport() else {
            return;
        };
        if !point_in_rect(point, viewport) {
            return;
        }
        let delta = if point.y < viewport.origin.y + 24.0 {
            -14.0
        } else if point.y > viewport.origin.y + viewport.size.height - 24.0 {
            14.0
        } else {
            return;
        };
        let Some(document) = scroll.documentView() else {
            return;
        };
        let visible = scroll.contentView().bounds();
        let bounds = document.bounds();
        let max_y =
            (bounds.origin.y + bounds.size.height - visible.size.height).max(bounds.origin.y);
        let y = (visible.origin.y + delta).clamp(bounds.origin.y, max_y);
        if y != visible.origin.y {
            document.scrollPoint(NSPoint::new(visible.origin.x, y));
        }
    }
    fn update_row_drop_at(&self, id: &str, point: NSPoint) -> NSDragOperation {
        let Some(delegate) = self.ivars().delegate.load() else {
            return NSDragOperation::None;
        };
        let target = if self
            .list_viewport()
            .is_some_and(|(_, viewport)| point_in_rect(point, viewport))
        {
            delegate
                .ivars()
                .list_rows
                .borrow()
                .iter()
                .find_map(|(anchor, row)| {
                    let local = row.convertPoint_fromView(point, Some(self));
                    let size = row.bounds().size;
                    if local.x < 0.0
                        || local.x > size.width
                        || local.y < 0.0
                        || local.y > size.height
                        || anchor == id
                    {
                        return None;
                    }
                    let after = local.y < size.height / 2.0;
                    let edge = self.convertPoint_fromView(
                        NSPoint::new(0.0, if after { 0.0 } else { size.height }),
                        Some(row),
                    );
                    Some((
                        anchor.clone(),
                        after,
                        rect(edge.x, edge.y - 1.0, size.width, 2.0),
                    ))
                })
        } else {
            None
        };
        let valid = target.filter(|(anchor, after, _)| {
            delegate
                .ivars()
                .settings
                .borrow()
                .clone()
                .move_relative(id, anchor, *after)
                .is_ok()
        });
        let operation = if valid.is_some() {
            NSDragOperation::Move
        } else {
            NSDragOperation::None
        };
        *self.ivars().marker.borrow_mut() = valid;
        NSView::setNeedsDisplay(self, true);
        operation
    }
    fn new(delegate: &Delegate, frame: NSRect) -> Retained<Self> {
        let allocated = Self::alloc(delegate.mtm()).set_ivars(DropState {
            delegate: objc2::rc::Weak::new(delegate),
            marker: RefCell::new(None),
        });
        let view: Retained<Self> = unsafe { msg_send![super(allocated), initWithFrame: frame] };
        view.registerForDraggedTypes(&objc2_foundation::NSArray::from_slice(&[
            unsafe { NSPasteboardTypeFileURL },
            &s(ROW_DRAG_TYPE),
        ]));
        view
    }
}
fn point_in_rect(point: NSPoint, bounds: NSRect) -> bool {
    point.x >= bounds.origin.x
        && point.x <= bounds.origin.x + bounds.size.width
        && point.y >= bounds.origin.y
        && point.y <= bounds.origin.y + bounds.size.height
}
fn dropped_urls(pasteboard: &NSPasteboard) -> Vec<Retained<objc2_foundation::NSURL>> {
    pasteboard
        .pasteboardItems()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let value = item.stringForType(unsafe { NSPasteboardTypeFileURL })?;
                    objc2_foundation::NSURL::URLWithString(&value)
                })
                .collect()
        })
        .unwrap_or_default()
}
fn application_details(url: &objc2_foundation::NSURL) -> Option<(String, String, String)> {
    if !url.isFileURL()
        || !url
            .pathExtension()
            .is_some_and(|ext| ext.to_string().eq_ignore_ascii_case("app"))
    {
        return None;
    }
    let bundle = NSBundle::bundleWithURL(url)?;
    let id = bundle.bundleIdentifier()?.to_string();
    let path = url.path()?.to_string();
    let name = ["CFBundleDisplayName", "CFBundleName"]
        .into_iter()
        .find_map(|key| {
            bundle
                .objectForInfoDictionaryKey(&s(key))?
                .downcast::<NSString>()
                .ok()
                .map(|name| name.to_string())
                .filter(|name| !name.trim().is_empty())
        })
        .unwrap_or_else(|| {
            std::path::Path::new(&path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    Some((id, path, name))
}
fn resolve_application(id: &str, path: &str) -> Option<Retained<objc2_foundation::NSURL>> {
    let matches = |url: &objc2_foundation::NSURL| {
        url.path()
            .is_some_and(|path| std::path::Path::new(&path.to_string()).is_dir())
            && application_details(url).is_some_and(|(found, _, _)| found == id)
    };
    let saved = objc2_foundation::NSURL::fileURLWithPath(&s(path));
    if matches(&saved) {
        return Some(saved);
    }
    let found = NSWorkspace::sharedWorkspace().URLForApplicationWithBundleIdentifier(&s(id))?;
    matches(&found).then_some(found)
}
fn refresh_application_items(items: &mut [Item], only: Option<&str>) -> bool {
    let mut changed = false;
    for item in items {
        match &mut item.kind {
            ItemKind::Group { children } => changed |= refresh_application_items(children, only),
            ItemKind::App { path } if only.is_none_or(|id| id == item.id) => {
                let Some(url) = resolve_application(&item.id, path) else {
                    continue;
                };
                let Some((_, resolved, name)) = application_details(&url) else {
                    continue;
                };
                if *path != resolved || item.name != name {
                    *path = resolved;
                    item.name = name;
                    changed = true;
                }
            }
            _ => {}
        }
    }
    changed
}
#[derive(Default)]
struct State {
    updater: RefCell<Option<updater::Updater>>,
    settings: RefCell<Settings>,
    selected_rows: RefCell<std::collections::HashSet<String>>,
    remove_button: RefCell<Option<Retained<NSButton>>>,
    list_rows: RefCell<Vec<(String, Retained<NSButton>)>>,
    lifecycle: RefCell<crate::lifecycle::Lifecycle>,
    editor: RefCell<Option<AppearanceEditor>>,
    advanced_appearance: RefCell<bool>,
    window: RefCell<Option<Retained<NSWindow>>>,
    status: RefCell<Vec<Retained<NSStatusItem>>>,
    menus: RefCell<Vec<Retained<NSMenu>>>,
    last_activity: RefCell<Vec<Activity>>,
    last_error: RefCell<Option<String>>,
    last_snapshot: RefCell<Option<dock::Snapshot>>,
    timer: RefCell<Option<Retained<NSTimer>>>,
    monitor: RefCell<Option<crate::monitor::Monitor>>,
    trash_action: RefCell<Option<crate::monitor::Action>>,
}
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = State]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSMenuDelegate for Delegate {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            self.update_group_menu(menu);
            let is_style = self.ivars().editor.borrow().as_ref().and_then(|e| e.style.menu()).is_some_and(|m| std::ptr::eq(&*m,menu));
            if is_style { self.update_style_previews(); }
        }
    }
    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn launched(&self, _: &NSNotification) {
            crate::localization::configure(crate::model::Language::System);
            match storage::load(&storage::config_path()) {
                Ok(s) => *self.ivars().settings.borrow_mut() = s,
                Err(e) => {
                    let alert = NSAlert::new(self.mtm());
                    alert.setMessageText(&t("Settings could not be loaded"));
                    alert.setInformativeText(&s(&crate::localization::format("Your file will be preserved. Restore the previous backup to continue.\n\n{error}", &[("error",&e)])));
                    alert.addButtonWithTitle(&t("Restore Backup")); alert.addButtonWithTitle(&t("Quit"));
                    if alert.runModal() != NSAlertFirstButtonReturn { NSApplication::sharedApplication(self.mtm()).terminate(None); return; }
                    match storage::restore_backup(&storage::config_path()) {
                        Ok(settings) => *self.ivars().settings.borrow_mut() = settings,
                        Err(error) => { self.alert("Backup could not be restored", &error); NSApplication::sharedApplication(self.mtm()).terminate(None); return; }
                    }
                }
            }
            unsafe { NSWorkspace::sharedWorkspace().notificationCenter().addObserver_selector_name_object(self,sel!(accessibilityChanged:),Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),None); }
            unsafe {
                let center=NSWorkspace::sharedWorkspace().notificationCenter();
                for name in [NSWorkspaceWillSleepNotification,NSWorkspaceDidWakeNotification,NSWorkspaceScreensDidSleepNotification,NSWorkspaceScreensDidWakeNotification,NSWorkspaceSessionDidBecomeActiveNotification,NSWorkspaceSessionDidResignActiveNotification] {
                    center.addObserver_selector_name_object(self,sel!(lifecycleChanged:),Some(name),None);
                }
                for name in [NSWorkspaceDidLaunchApplicationNotification, NSWorkspaceDidTerminateApplicationNotification] {
                    center.addObserver_selector_name_object(self,sel!(workspaceAppChanged:),Some(name),None);
                }
            }
            unsafe {
                NSNotificationCenter::defaultCenter().addObserver_selector_name_object(self, sel!(dockReadCompleted:), Some(&s(crate::monitor::READY_NOTIFICATION)), None);
            }
            match updater::Updater::start(self.mtm()) {
                Ok(value) => *self.ivars().updater.borrow_mut() = value,
                Err(error) => self.alert("Updates unavailable", &error),
            }
            self.refresh_application_metadata(None);
            self.rebuild_status();
            if self.ivars().settings.borrow().show_on_start { self.show_settings(); }
        }
        #[unsafe(method(applicationWillTerminate:))]
        fn terminating(&self,_: &NSNotification) {
            unsafe { NSWorkspace::sharedWorkspace().notificationCenter().removeObserver(self); }
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(self); }
            self.remove_appearance_observers();
            self.suspend_monitor();
            self.ivars().trash_action.borrow_mut().take();
        }
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn reopen(&self, _: &NSApplication, _: bool) -> bool { self.show_settings(); true }
    }
    impl Delegate {
        #[unsafe(method(workspaceAppChanged:))]
        fn workspace_app_notification(&self, notification: &NSNotification) {
            if self.ivars().lifecycle.borrow().suspended() { return; }
            let id = notification.userInfo()
                .and_then(|info| info.objectForKey(unsafe { NSWorkspaceApplicationKey }))
                .and_then(|app| app.downcast::<NSRunningApplication>().ok())
                .and_then(|app| app.bundleIdentifier());
            if let Some(id) = id {
                let id = id.to_string();
                if notification.name().isEqualToString(unsafe { NSWorkspaceDidLaunchApplicationNotification }) && self.refresh_application_metadata(Some(&id)) {
                    self.rebuild_status();
                }
                self.workspace_app_changed(&id);
            }
        }
        #[unsafe(method(exportSettings:))]
        fn export_settings(&self, _: &NSObject) {
            let panel = NSSavePanel::savePanel(self.mtm());
            panel.setTitle(Some(&t("Export Settings")));
            panel.setNameFieldStringValue(&s("Pipmark Settings.json"));
            if panel.runModal() != NSModalResponseOK { return; }
            let Some(path) = panel.URL().and_then(|u| u.path()) else { return; };
            if let Err(error) = crate::archive::Archive::from_settings(&self.ivars().settings.borrow()).and_then(|a| a.write(std::path::Path::new(&path.to_string()))) { self.alert("Export failed", &error); }
        }
        #[unsafe(method(importSettings:))]
        fn import_settings(&self, _: &NSObject) {
            let panel = NSOpenPanel::openPanel(self.mtm());
            panel.setTitle(Some(&t("Import Settings"))); panel.setCanChooseFiles(true); panel.setCanChooseDirectories(false);
            if panel.runModal() != NSModalResponseOK { return; }
            let Some(path) = panel.URL().and_then(|u| u.path()) else { return; };
            let archive = match crate::archive::Archive::read(std::path::Path::new(&path.to_string())) { Ok(s) => s, Err(e) => { self.alert("Cannot import settings", &e); return; } };
            let alert = NSAlert::new(self.mtm()); alert.setMessageText(&t("Replace current settings?"));
            alert.setInformativeText(&s(&crate::localization::format("Import {count} top-level items. Your current settings will be kept as the previous backup.", &[("count",&archive.settings.items.len().to_string())])));
            alert.addButtonWithTitle(&t("Import")); alert.addButtonWithTitle(&t("Cancel"));
            if alert.runModal() != NSAlertFirstButtonReturn { return; }
            let settings = match archive.install(&storage::config_path()) { Ok(s) => s, Err(e) => { self.alert("Import failed",&e); return; } };
            *self.ivars().settings.borrow_mut() = settings;
            self.rebuild_status(); self.show_settings();
        }
        #[unsafe(method(restoreSettings:))]
        fn restore_settings(&self, _: &NSObject) {
            let alert = NSAlert::new(self.mtm()); alert.setMessageText(&t("Restore previous settings?"));
            alert.setInformativeText(&t("The current file will be preserved in the settings folder."));
            alert.addButtonWithTitle(&t("Restore")); alert.addButtonWithTitle(&t("Cancel"));
            if alert.runModal() != NSAlertFirstButtonReturn { return; }
            match storage::restore_backup(&storage::config_path()) {
                Ok(settings) => { *self.ivars().settings.borrow_mut() = settings; self.rebuild_status(); self.show_settings(); }
                Err(e) => self.alert("Restore failed", &e),
            }
        }
        #[unsafe(method(listItemClicked:))]
        fn list_item_clicked(&self, sender: &NSButton) {
            let multiple = NSApplication::sharedApplication(self.mtm()).currentEvent()
                .is_some_and(|event| event.modifierFlags().intersects(NSEventModifierFlags::Shift | NSEventModifierFlags::Command));
            self.select_list_item(sender.tag() as usize, multiple);
        }
        #[unsafe(method(createGroup:))]
        fn create_group(&self, _: &NSObject) {
            let mtm = self.mtm();
            let candidates: Vec<_> = self.ivars().settings.borrow().rows().into_iter().map(|(item,depth)| (item.clone(),depth)).collect();
            let alert = NSAlert::new(mtm); alert.setMessageText(&t("New App Group"));
            alert.setInformativeText(&t("Name your group and choose apps or groups to include."));
            alert.addButtonWithTitle(&t("Create")); alert.addButtonWithTitle(&t("Cancel"));
            let list_height = (candidates.len() as f64 * 28.0).clamp(32.0,224.0);
            let accessory = NSView::initWithFrame(NSView::alloc(mtm),rect(0.0,0.0,340.0,list_height+42.0));
            let name = NSTextField::initWithFrame(NSTextField::alloc(mtm),rect(0.0,list_height+12.0,340.0,26.0));
            name.setStringValue(&s("New Group")); accessory.addSubview(&name);
            let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm),rect(0.0,0.0,340.0,list_height));
            scroll.setHasVerticalScroller(true); scroll.setAutohidesScrollers(true);
            let height = (candidates.len() as f64 * 28.0).max(list_height);
            let document = NSView::initWithFrame(NSView::alloc(mtm),rect(0.0,0.0,320.0,height));
            let mut choices = Vec::new();
            for (index,(item,depth)) in candidates.iter().enumerate() {
                let x = (*depth as f64 * 12.0).min(72.0);
                let y = height - (index+1) as f64 * 28.0;
                let icon = NSImageView::initWithFrame(NSImageView::alloc(mtm),rect(x,y+5.0,18.0,18.0));
                icon.setImage(Some(&self.list_icon(item))); document.addSubview(&icon);
                let control = unsafe { NSButton::checkboxWithTitle_target_action(&s(&item.name),None,None,mtm) };
                control.setFrame(rect(x+24.0,y,292.0-x,28.0));
                control.setState(if self.ivars().selected_rows.borrow().contains(&item.id) { NSControlStateValueOn } else { NSControlStateValueOff });
                document.addSubview(&control); choices.push((item.id.clone(),control));
            }
            if candidates.is_empty() { self.label(&document,crate::localization::text("Add apps to this group later."),rect(4.0,4.0,300.0,24.0),12.0,true); }
            scroll.setDocumentView(Some(&document)); accessory.addSubview(&scroll);
            document.scrollPoint(NSPoint::new(0.0,height-list_height));
            alert.setAccessoryView(Some(&accessory));
            if alert.runModal() != NSAlertFirstButtonReturn { return; }
            let selected = choices.into_iter().filter(|(_,control)| control.state()==NSControlStateValueOn).map(|(id,_)|id).collect();
            let mut settings = self.ivars().settings.borrow().clone();
            let id = format!("group-{}",objc2_foundation::NSUUID::UUID().UUIDString());
            match settings.create_group(id.clone(),name.stringValue().to_string(),&selected) {
                Ok(()) => {
                    self.apply_settings(settings);
                    if self.ivars().settings.borrow().rows().iter().any(|(item,_)| item.id == id) {
                        self.ivars().selected_rows.borrow_mut().clear();
                        self.update_list_selection();
                    }
                },
                Err(error) => self.alert("Cannot create group",&error),
            }
        }
        #[unsafe(method(groupApps:))]
        fn group_apps(&self, sender: &NSButton) { self.show_group_tab(sender.tag() as usize,true); }
        #[unsafe(method(groupSettings:))]
        fn group_settings(&self, sender: &NSButton) { self.show_group_tab(sender.tag() as usize,false); }
        #[unsafe(method(addGroupMembers:))]
        fn add_group_members(&self, _: &NSButton) { self.choose_group_members(); }
        #[unsafe(method(moveItem:))]
        fn move_item_action(&self, sender: &objc2::runtime::AnyObject) {
            let tag:isize=unsafe { msg_send![sender,tag] };
            let Some(id) = self.row_id(tag as usize) else { return; };
            let groups: Vec<(String,String)> = self.ivars().settings.borrow().rows().iter().filter(|(i,_)| matches!(i.kind,ItemKind::Group { .. }) && i.id != id).map(|(i,depth)| (i.id.clone(),format!("{}{}", "  ".repeat(*depth),i.name))).collect();
            let alert = NSAlert::new(self.mtm()); alert.setMessageText(&t("Move to Group"));
            alert.addButtonWithTitle(&t("Move")); alert.addButtonWithTitle(&t("Cancel"));
            let popup = NSPopUpButton::initWithFrame_pullsDown(NSPopUpButton::alloc(self.mtm()),rect(0.0,0.0,320.0,30.0),false);
            popup.addItemWithTitle(&t("Menu bar (no group)")); for (_,name) in &groups { popup.addItemWithTitle(&s(name)); }
            alert.setAccessoryView(Some(&popup)); if alert.runModal() != NSAlertFirstButtonReturn { return; }
            let parent = groups.get(popup.indexOfSelectedItem().wrapping_sub(1) as usize).map(|(id,_)|id.as_str());
            let mut settings = self.ivars().settings.borrow().clone();
            match settings.move_item(&id,parent) { Ok(()) => self.apply_settings(settings), Err(e) => self.alert("Cannot move item",&e) }
        }
        #[unsafe(method(chooseIcon:))]
        fn choose_icon(&self, sender: &objc2::runtime::AnyObject) {
            let tag: isize = unsafe { msg_send![sender, tag] };
            let Some(id) = self.row_id(tag as usize) else { return; };
            let panel = NSOpenPanel::openPanel(self.mtm()); panel.setTitle(Some(&t("Choose Custom SVG Icon")));
            panel.setCanChooseFiles(true); panel.setCanChooseDirectories(false);
            #[allow(deprecated)] panel.setAllowedFileTypes(Some(&objc2_foundation::NSArray::from_retained_slice(&[s("svg")])));
            if panel.runModal() != NSModalResponseOK { return; }
            let Some(path) = panel.URL().and_then(|u|u.path()) else { return; };
            let directory = storage::config_path().parent().unwrap().join("icons");
            match crate::icons::import(std::path::Path::new(&path.to_string()),&directory) {
                Ok(path) => {
                    let mut settings = self.ivars().settings.borrow().clone();
                    if let Some(item) = settings.item_mut(&id) { item.appearance.custom_icon=Some(path); }
                    self.apply_settings(settings);
                },
                Err(e) => self.alert("Cannot use this icon",&e),
            }
        }
        #[unsafe(method(resetIcon:))]
        fn reset_icon(&self, sender: &objc2::runtime::AnyObject) {
            let tag: isize = unsafe { msg_send![sender, tag] };
            let Some(id) = self.row_id(tag as usize) else { return; };
            let mut settings = self.ivars().settings.borrow().clone();
            if let Some(item) = settings.item_mut(&id) { item.appearance.custom_icon=None; }
            self.apply_settings(settings);
        }
        #[unsafe(method(moveEarlier:))]
        fn move_earlier(&self,sender: &objc2::runtime::AnyObject) { self.reorder_action(sender,true); }
        #[unsafe(method(moveLater:))]
        fn move_later(&self,sender: &objc2::runtime::AnyObject) { self.reorder_action(sender,false); }
        #[unsafe(method(renameGroup:))]
        fn rename_group(&self,sender: &NSTextField) {
            let id=self.ivars().editor.borrow().as_ref().map(|e|e.id.clone());
            let Some(id)=id else { return; };
            let name=sender.stringValue().to_string().trim().to_owned();
            if name.is_empty() { self.alert("Enter a group name","Group names cannot be blank."); return; }
            let mut settings=self.ivars().settings.borrow().clone();
            if let Some(item)=settings.item_mut(&id) { if matches!(item.kind,ItemKind::Group { .. }) { item.name=name; } }
            self.apply_settings(settings);
        }
        #[unsafe(method(lifecycleChanged:))]
        fn lifecycle_changed(&self,notification: &NSNotification) {
            use crate::lifecycle::Reason;
            let name=notification.name();
            let (reason,suspended)=unsafe {
                if name.isEqualToString(NSWorkspaceWillSleepNotification) { (Reason::System,true) }
                else if name.isEqualToString(NSWorkspaceDidWakeNotification) { (Reason::System,false) }
                else if name.isEqualToString(NSWorkspaceScreensDidSleepNotification) { (Reason::Display,true) }
                else if name.isEqualToString(NSWorkspaceScreensDidWakeNotification) { (Reason::Display,false) }
                else if name.isEqualToString(NSWorkspaceSessionDidResignActiveNotification) { (Reason::Session,true) }
                else { (Reason::Session,false) }
            };
            self.ivars().lifecycle.borrow_mut().set(reason,suspended);
            if self.ivars().lifecycle.borrow().suspended() { self.suspend_monitor(); }
            else { self.ivars().last_activity.borrow_mut().clear(); self.start_monitor(); self.refresh_activity(); }
        }
        #[unsafe(method(accessibilityChanged:))]
        fn accessibility_changed(&self,_: &NSNotification) {
            for status in self.ivars().status.borrow().iter() { if let Some(button)=status.button(self.mtm()) { crate::attention::clear(&button); } }
            self.refresh_appearance();
        }
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observed_appearance(&self, _: Option<&NSString>, button: &NSStatusBarButton, _: Option<&NSDictionary>, _: *mut std::ffi::c_void) {
            // Only our status buttons are registered, without Initial/Prior delivery.
            let index = button.tag() as usize;
            if let Some(item) = self.ivars().settings.borrow().items.get(index) {
                let state = self.ivars().last_activity.borrow().get(index).cloned().unwrap_or_default();
                button.setImage(Some(&crate::render::button_icon(item, &state, button, self.mtm())));
            }
        }
        #[unsafe(method(toggleItem:))]
        fn toggle_item(&self,sender: &NSMenuItem) {
            let Some(id)=self.row_id(sender.tag() as usize) else { return; };
            let mut settings=self.ivars().settings.borrow().clone();
            if let Some(item)=settings.item_mut(&id) { item.enabled=!item.enabled; }
            self.apply_settings(settings);
        }
        #[unsafe(method(dockReadCompleted:))]
        fn dock_read_completed(&self, _: &NSNotification) { self.receive_activity(false); self.finish_trash_action(); }
        #[unsafe(method(refreshBadges:))]
        fn refresh_badges(&self, _: &NSTimer) { self.refresh_activity(); }
        #[unsafe(method(statusClick:))]
        fn status_click(&self, sender: &NSButton) {
            let index = sender.tag() as usize;
            let event = NSApplication::sharedApplication(self.mtm()).currentEvent();
            let right = event.is_some_and(|e| e.r#type() == NSEventType::RightMouseUp || e.modifierFlags().contains(NSEventModifierFlags::Control));
            if right {
                let menu = self.ivars().menus.borrow().get(index).cloned();
                if let Some(menu) = menu { menu.popUpMenuPositioningItem_atLocation_inView(None, NSPoint::new(0.0, 0.0), Some(sender)); }
            } else { self.launch(index, true); }
        }
        #[unsafe(method(openAccessibility:))]
        fn open_accessibility(&self, _: &NSObject) {
            if let Some(url) = objc2_foundation::NSURL::URLWithString(&s("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")) { NSWorkspace::sharedWorkspace().openURL(&url); }
        }
        #[unsafe(method(toggleAdvanced:))]
        fn toggle_advanced(&self, _: &NSObject) {
            let expanded = *self.ivars().advanced_appearance.borrow();
            *self.ivars().advanced_appearance.borrow_mut() = !expanded;
            self.restore_current_editor();
        }
        #[unsafe(method(appearanceChanged:))]
        fn appearance_changed(&self, _: &NSObject) { self.save_appearance(); }
        #[unsafe(method(showDefaults:))]
        fn show_defaults(&self,_: &NSObject) {
            let appearance=self.ivars().settings.borrow().defaults.clone();
            let item=Item { id:String::new(),name:crate::localization::text("Defaults").into(),enabled:true,appearance,kind:ItemKind::Trash };
            self.appearance_pane(item,0,true,false);
        }
        #[unsafe(method(editItem:))]
        fn edit_item(&self, sender: &objc2::runtime::AnyObject) {
            let index: isize = unsafe { msg_send![sender, tag] };
            if index >= 0 { self.edit_appearance(index as usize); }
        }
        #[unsafe(method(showAbout:))]
        fn show_about(&self, _: &NSObject) {
            NSApplication::sharedApplication(self.mtm()).orderFrontStandardAboutPanel(None);
        }
        #[unsafe(method(showSettings:))]
        fn settings_action(&self, _: &NSObject) { self.show_settings(); }
        #[unsafe(method(addApp:))]
        fn add_app(&self, _: &NSObject) {
            let panel = NSOpenPanel::openPanel(self.mtm());
            panel.setCanChooseDirectories(false); panel.setCanChooseFiles(true); panel.setAllowsMultipleSelection(true);
            panel.setTitle(Some(&t("Choose Applications")));
            #[allow(deprecated)] panel.setAllowedFileTypes(Some(&objc2_foundation::NSArray::from_retained_slice(&[s("app")])));
            if panel.runModal() != NSModalResponseOK { return; }
            self.add_application_urls(&panel.URLs());
        }
        #[unsafe(method(addTrash:))]
        fn add_trash(&self, _: &NSObject) {
            let mut settings = self.ivars().settings.borrow().clone();
            if settings.rows().iter().any(|(item, _)| matches!(item.kind, ItemKind::Trash)) { return; }
            let mut appearance = settings.defaults.clone();
            appearance.pinned = true;
            settings.items.push(Item { id: "badgeify.trash".into(), name: crate::localization::text("Trash").into(), enabled: true, appearance, kind: ItemKind::Trash });
            self.apply_settings(settings);
        }
        #[unsafe(method(emptyTrash:))]
        fn empty_trash(&self, sender: &NSMenuItem) {
            let is_trash = self.ivars().settings.borrow().rows().get(sender.tag() as usize)
                .is_some_and(|(item, _)| matches!(item.kind, ItemKind::Trash));
            if is_trash { self.start_trash_action(true); }
        }
        #[unsafe(method(openApp:))]
        fn open_app(&self, sender: &NSMenuItem) {
            let item = self.ivars().settings.borrow().rows().get(sender.tag() as usize).map(|(i,_)| (*i).clone());
            match item {
                Some(item @ Item { kind: ItemKind::App { .. }, .. }) => self.open_application(&item, false),
                Some(Item { kind: ItemKind::Trash, .. }) => self.open_trash(),
                _ => {}
            }
        }
        #[unsafe(method(removeSelected:))]
        fn remove_selected(&self, _: &NSObject) {
            let selected = self.ivars().selected_rows.borrow().clone();
            if selected.is_empty() { return; }
            let mut settings = self.ivars().settings.borrow().clone();
            for id in selected { settings.remove_item(&id); }
            self.apply_settings(settings);
        }
        #[unsafe(method(removeApp:))]
        fn remove_app(&self, sender: &objc2::runtime::AnyObject) {
            let tag:isize=unsafe { msg_send![sender,tag] };
            if let Some(id) = self.row_id(tag as usize) {
                let mut settings = self.ivars().settings.borrow().clone();
                settings.remove_item(&id);
                self.apply_settings(settings);
            }
        }
        #[unsafe(method(toggleApp:))]
        fn toggle_app(&self, sender: &NSSwitch) {
            let Some(id) = self.row_id(sender.tag() as usize) else { return; };
            let mut settings = self.ivars().settings.borrow().clone();
            let Some(item) = settings.item_mut(&id) else { return; };
            let old = item.enabled;
            item.enabled = sender.state() == NSControlStateValueOn;
            if let Err(error) = self.commit_settings(settings) {
                sender.setState(if old { NSControlStateValueOn } else { NSControlStateValueOff });
                self.alert("Unable to save settings",&error);
            } else { self.rebuild_status(); self.show_settings(); }
        }
        #[unsafe(method(toggleLogin:))]
        fn toggle_login(&self,sender: &NSSwitch) {
            match crate::login::set_enabled(sender.state()==NSControlStateValueOn) {
                Ok(crate::login::Status::ApprovalRequired) => self.alert("Approve Launch at Login","macOS needs your approval. Open Login Items and enable Pipmark."),
                Ok(_) => (),
                Err(e) => self.alert("Unable to update Launch at Login",&e),
            }
            self.show_settings();
        }
        #[unsafe(method(openLoginSettings:))]
        fn open_login_settings(&self,_: &NSObject) { crate::login::open_settings(); }
        #[unsafe(method(chooseLanguage:))]
        fn choose_language(&self, sender: &NSMenuItem) {
            let Some(language) = crate::model::Language::ALL.get(sender.tag() as usize).copied() else { return; };
            let mut settings = self.ivars().settings.borrow().clone();
            settings.language = language;
            self.apply_settings(settings);
        }
        #[unsafe(method(toggleStartup:))]
        fn toggle_startup(&self, sender: &NSSwitch) {
            let mut settings = self.ivars().settings.borrow().clone();
            let old = settings.show_on_start;
            settings.show_on_start = sender.state() == NSControlStateValueOn;
            if let Err(error) = self.commit_settings(settings) {
                sender.setState(if old { NSControlStateValueOn } else { NSControlStateValueOff });
                self.alert("Unable to save settings",&error);
            }
        }
        #[unsafe(method(quit:))]
        fn quit(&self, _: &NSObject) { NSApplication::sharedApplication(self.mtm()).terminate(None); }
    }
);
fn s(text: &str) -> Retained<NSString> {
    NSString::from_str(text)
}
fn t(text: &str) -> Retained<NSString> {
    s(crate::localization::text(text))
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
impl Delegate {
    fn refresh_application_metadata(&self, only: Option<&str>) -> bool {
        let mut settings = self.ivars().settings.borrow().clone();
        if !refresh_application_items(&mut settings.items, only) {
            return false;
        }
        if let Err(error) = self.commit_settings(settings) {
            self.alert("Unable to save settings", &error);
            return false;
        }
        true
    }
    fn open_application(&self, item: &Item, toggle: bool) {
        let ItemKind::App { path } = &item.kind else {
            return;
        };
        let workspace = NSWorkspace::sharedWorkspace();
        if toggle {
            if let Some(app) = workspace.runningApplications().iter().find(|app| {
                app.bundleIdentifier()
                    .is_some_and(|id| id.to_string() == item.id)
            }) {
                if app.isActive() {
                    app.hide();
                    return;
                }
            }
        }
        let Some(url) = resolve_application(&item.id, path) else {
            self.alert(
                "Application unavailable",
                &crate::localization::format(
                    "Could not find {app}. Reinstall it or add its new location.",
                    &[("app", &item.name)],
                ),
            );
            return;
        };
        if !workspace.openURL(&url) {
            self.alert(
                "Application unavailable",
                &crate::localization::format("macOS could not open {app}.", &[("app", &item.name)]),
            );
        }
    }
    fn move_list_row(&self, id: &str, anchor: &str, after: bool) -> bool {
        let mut settings = self.ivars().settings.borrow().clone();
        if settings.move_relative(id, anchor, after).is_err() {
            return false;
        }
        if let Err(error) = self.commit_settings(settings) {
            self.alert("Unable to save settings", &error);
            return false;
        }
        self.rebuild_status();
        self.show_settings();
        true
    }
    fn select_list_item(&self, index: usize, multiple: bool) {
        let Some(id) = self.row_id(index) else {
            return;
        };
        if multiple {
            let mut selected = self.ivars().selected_rows.borrow_mut();
            if !selected.remove(&id) {
                selected.insert(id);
            }
            drop(selected);
            self.update_list_selection();
        } else {
            *self.ivars().selected_rows.borrow_mut() = [id].into();
            self.update_list_selection();
        }
    }
    fn update_list_selection(&self) {
        let selected = self.ivars().selected_rows.borrow();
        if let Some(button) = self.ivars().remove_button.borrow().as_ref() {
            button.setEnabled(!selected.is_empty());
        }
        for (id, button) in self.ivars().list_rows.borrow().iter() {
            let active = selected.contains(id);
            button.setBordered(false);
            NSView::setNeedsDisplay(button, true);
            button.setState(if active {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
    }
    fn add_application_urls(
        &self,
        urls: &objc2_foundation::NSArray<objc2_foundation::NSURL>,
    ) -> bool {
        let mut settings = self.ivars().settings.borrow().clone();
        let mut changed = false;
        for url in urls {
            let Some((id, path, name)) = application_details(&url) else {
                continue;
            };
            if settings.rows().iter().any(|(item, _)| item.id == id) {
                continue;
            }
            settings.items.push(Item {
                id,
                name,
                enabled: true,
                appearance: settings.defaults.clone(),
                kind: ItemKind::App { path },
            });
            changed = true;
        }
        if !changed {
            return false;
        }
        if let Err(error) = self.commit_settings(settings) {
            self.alert("Unable to save settings", &error);
            return false;
        }
        self.rebuild_status();
        self.show_settings();
        true
    }
    fn open_trash(&self) {
        self.start_trash_action(false);
    }
    fn start_trash_action(&self, empty: bool) {
        if self.ivars().trash_action.borrow().is_some() {
            return;
        }
        match crate::monitor::Action::start(
            move || {
                objc2::rc::autoreleasepool(|_| {
                    (if empty {
                        dock::empty_trash()
                    } else {
                        dock::open_trash()
                    })
                    .map_err(|e| e.to_string())
                })
            },
            crate::monitor::notify_ready,
        ) {
            Ok(action) => *self.ivars().trash_action.borrow_mut() = Some(action),
            Err(error) => self.alert("Cannot complete Trash action", &error.to_string()),
        }
    }
    fn finish_trash_action(&self) {
        let result = self
            .ivars()
            .trash_action
            .borrow()
            .as_ref()
            .and_then(|action| action.poll());
        if let Some(result) = result {
            // Release the guard before an alert enters AppKit's nested event loop.
            self.ivars().trash_action.borrow_mut().take();
            if let Err(error) = result {
                self.alert("Cannot complete Trash action", &error);
            }
        }
    }
    fn list_icon(&self, item: &Item) -> Retained<NSImage> {
        let source = match &item.kind {
            ItemKind::App { path } => NSWorkspace::sharedWorkspace().iconForFile(&s(path)),
            _ => NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &s(if matches!(item.kind, ItemKind::Trash) {
                    "trash"
                } else {
                    "folder"
                }),
                Some(&s(&item.name)),
            )
            .unwrap(),
        };
        let image: Retained<NSImage> = unsafe { msg_send![&*source, copy] };
        image.setSize(NSSize::new(18.0, 18.0));
        image
    }
    fn item_actions(&self, item: &Item, index: usize) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm());
        if let ItemKind::Group { children } = &item.kind {
            let row = NSMenuItem::new(self.mtm());
            row.setTitle(&t("Apps"));
            let submenu = NSMenu::new(self.mtm());
            self.group_menu(&submenu, children);
            row.setSubmenu(Some(&submenu));
            menu.addItem(&row);
            menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        }
        if matches!(item.kind, ItemKind::Trash) {
            self.menu_item(&menu, "Empty Trash…", sel!(emptyTrash:), index as isize, "");
        }
        self.menu_item(&menu, "Settings…", sel!(editItem:), index as isize, "");
        self.menu_item(
            &menu,
            if item.enabled { "Disable" } else { "Enable" },
            sel!(toggleItem:),
            index as isize,
            "",
        );
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        self.menu_item(&menu, "Move to Group…", sel!(moveItem:), index as isize, "");
        self.menu_item(
            &menu,
            "Move Earlier",
            sel!(moveEarlier:),
            index as isize,
            "",
        );
        self.menu_item(&menu, "Move Later", sel!(moveLater:), index as isize, "");
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        self.menu_item(
            &menu,
            if matches!(item.kind, ItemKind::Group { .. }) {
                "Ungroup"
            } else {
                "Remove"
            },
            sel!(removeApp:),
            index as isize,
            "",
        );
        menu
    }
    fn row_id(&self, index: usize) -> Option<String> {
        self.ivars()
            .settings
            .borrow()
            .rows()
            .get(index)
            .map(|(i, _)| i.id.clone())
    }
    fn reorder_action(&self, sender: &objc2::runtime::AnyObject, earlier: bool) {
        let tag: isize = unsafe { msg_send![sender, tag] };
        let Some(id) = self.row_id(tag as usize) else {
            return;
        };
        let mut settings = self.ivars().settings.borrow().clone();
        if settings.reorder(&id, earlier) {
            self.apply_settings(settings);
        }
    }
    fn apply_settings(&self, settings: Settings) {
        let tab = self
            .ivars()
            .editor
            .borrow()
            .as_ref()
            .is_some_and(|e| e.group_apps);
        let selected = self.ivars().editor.borrow().as_ref().map(|e| e.id.clone());
        if let Err(e) = self.commit_settings(settings) {
            self.alert("Unable to save settings", &e);
            self.restore_current_editor();
            return;
        }
        self.rebuild_status();
        let row = selected.and_then(|id| {
            self.ivars()
                .settings
                .borrow()
                .rows()
                .iter()
                .position(|(i, _)| i.id == id)
        });
        if let Some(row) = row {
            let item = self.ivars().settings.borrow().rows()[row].0.clone();
            self.appearance_pane(item, row, false, tab);
        } else {
            self.show_settings();
        }
    }
    fn group_menu(&self, menu: &NSMenu, children: &[Item]) {
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        if children.iter().all(|i| !i.enabled) {
            let empty = NSMenuItem::new(self.mtm());
            empty.setTitle(&t("No enabled apps"));
            empty.setEnabled(false);
            menu.addItem(&empty);
            return;
        }
        for child in children.iter().filter(|i| i.enabled) {
            let index = self
                .ivars()
                .settings
                .borrow()
                .rows()
                .iter()
                .position(|(i, _)| i.id == child.id)
                .unwrap_or(0);
            let row = NSMenuItem::new(self.mtm());
            row.setTitle(&s(&child.name));
            row.setImage(Some(&self.list_icon(child)));
            row.setTag(index as isize);
            unsafe {
                row.setRepresentedObject(Some(&s(&child.id)));
            }
            if let ItemKind::Group { children } = &child.kind {
                let submenu = NSMenu::new(self.mtm());
                self.group_menu(&submenu, children);
                row.setSubmenu(Some(&submenu));
            } else {
                unsafe {
                    row.setTarget(Some(self));
                    row.setAction(Some(sel!(openApp:)));
                }
            }
            menu.addItem(&row);
        }
    }
    fn update_group_menu(&self, menu: &NSMenu) {
        let settings = self.ivars().settings.borrow();
        let snapshot = self.ivars().last_snapshot.borrow();
        let rows = settings.rows();
        for row in menu.itemArray() {
            let Some(object) = row.representedObject() else {
                continue;
            };
            let Some(id) = object.downcast_ref::<NSString>() else {
                continue;
            };
            let Some((item, _)) = rows.iter().find(|(item, _)| item.id == id.to_string()) else {
                continue;
            };
            let state = snapshot
                .as_ref()
                .map(|s| s.display_activity(item))
                .unwrap_or_default();
            row.setTitle(&s(&crate::render::menu_title(item, &state)));
        }
    }
    fn show_group_tab(&self, index: usize, apps: bool) {
        let item = self
            .ivars()
            .settings
            .borrow()
            .rows()
            .get(index)
            .map(|(i, _)| (*i).clone());
        if let Some(item) = item {
            self.appearance_pane(item, index, false, apps);
        }
    }
    fn group_members_view(&self, view: &NSView, group: &Item) {
        let ItemKind::Group { children } = &group.kind else {
            return;
        };
        self.button(
            view,
            crate::localization::text("Add Apps…"),
            rect(0.0, 326.0, 130.0, 26.0),
            sel!(addGroupMembers:),
            0,
        );
        let scroll = NSScrollView::initWithFrame(
            NSScrollView::alloc(self.mtm()),
            rect(0.0, 0.0, 416.0, 314.0),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        let height = (children.len() as f64 * 28.0).max(314.0);
        let list = NSView::initWithFrame(NSView::alloc(self.mtm()), rect(0.0, 0.0, 396.0, height));
        let settings = self.ivars().settings.borrow();
        let rows = settings.rows();
        for (index, item) in children.iter().enumerate() {
            let row = rows.iter().position(|(i, _)| i.id == item.id).unwrap();
            let button = self.button(
                &list,
                &item.name,
                rect(4.0, height - (index + 1) as f64 * 28.0, 388.0, 28.0),
                sel!(editItem:),
                row as isize,
            );
            button.setBordered(false);
            button.setAlignment(NSTextAlignment::Left);
            button.setToolTip(Some(&s(&item.name)));
            button.setImage(Some(&self.list_icon(item)));
            button.setImagePosition(NSCellImagePosition::ImageLeft);
            button.setAlphaValue(if item.enabled { 1.0 } else { 0.45 });
            unsafe {
                button.setMenu(Some(&self.item_actions(item, row)));
            }
        }
        if children.is_empty() {
            self.label(
                &list,
                crate::localization::text("No apps in this group yet."),
                rect(12.0, 260.0, 372.0, 26.0),
                13.0,
                true,
            );
        }
        scroll.setDocumentView(Some(&list));
        view.addSubview(&scroll);
        list.scrollPoint(NSPoint::new(0.0, height - 314.0));
    }
    fn choose_group_members(&self) {
        let Some(id) = self.ivars().editor.borrow().as_ref().map(|e| e.id.clone()) else {
            return;
        };
        let settings = self.ivars().settings.borrow().clone();
        let Some(group) = settings
            .rows()
            .into_iter()
            .find(|(i, _)| i.id == id)
            .map(|(i, _)| i.clone())
        else {
            return;
        };
        let contained = Settings {
            items: vec![group],
            ..Settings::default()
        };
        let members: std::collections::HashSet<_> =
            contained.rows().iter().map(|(i, _)| i.id.clone()).collect();
        let candidates: Vec<_> = settings
            .rows()
            .into_iter()
            .filter(|(i, _)| matches!(i.kind, ItemKind::App { .. }) && !members.contains(&i.id))
            .map(|(i, _)| i.clone())
            .collect();
        if candidates.is_empty() {
            self.alert(
                "No other apps available",
                "Add an app in Manage Apps first, or move a group using its right-click menu.",
            );
            return;
        }
        let alert = NSAlert::new(self.mtm());
        alert.setMessageText(&t("Add Apps to Group"));
        alert.setInformativeText(&s(
            "Selected apps will move from their current location into this group.",
        ));
        alert.addButtonWithTitle(&t("Add"));
        alert.addButtonWithTitle(&t("Cancel"));
        let height = candidates.len() as f64 * 28.0;
        let visible = height.min(224.0);
        let scroll = NSScrollView::initWithFrame(
            NSScrollView::alloc(self.mtm()),
            rect(0.0, 0.0, 340.0, visible),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        let list = NSView::initWithFrame(NSView::alloc(self.mtm()), rect(0.0, 0.0, 320.0, height));
        let mut choices = Vec::new();
        for (index, item) in candidates.iter().enumerate() {
            let control = unsafe {
                NSButton::checkboxWithTitle_target_action(&s(&item.name), None, None, self.mtm())
            };
            let y = height - (index + 1) as f64 * 28.0;
            let icon = NSImageView::initWithFrame(
                NSImageView::alloc(self.mtm()),
                rect(0.0, y + 5.0, 18.0, 18.0),
            );
            icon.setImage(Some(&self.list_icon(item)));
            list.addSubview(&icon);
            control.setFrame(rect(24.0, y, 292.0, 28.0));
            list.addSubview(&control);
            choices.push((item.id.clone(), control));
        }
        scroll.setDocumentView(Some(&list));
        alert.setAccessoryView(Some(&scroll));
        list.scrollPoint(NSPoint::new(0.0, height - visible));
        if alert.runModal() != NSAlertFirstButtonReturn {
            return;
        }
        let mut next = settings;
        let mut changed = false;
        for (member, control) in choices {
            if control.state() == NSControlStateValueOn {
                changed = true;
                if let Err(error) = next.move_item(&member, Some(&id)) {
                    self.alert("Cannot move app", &error);
                    return;
                }
            }
        }
        if changed {
            self.apply_settings(next);
        }
    }
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(State::default());
        unsafe { msg_send![super(this), init] }
    }
    fn alert(&self, title: &str, detail: &str) {
        let a = NSAlert::new(self.mtm());
        a.setMessageText(&t(title));
        a.setInformativeText(&t(detail));
        a.runModal();
    }
    fn commit_settings(&self, settings: Settings) -> Result<(), String> {
        storage::save(&storage::config_path(), &settings)?;
        *self.ivars().settings.borrow_mut() = settings;
        Ok(())
    }
    fn menu_item(
        &self,
        menu: &NSMenu,
        title: &str,
        action: objc2::runtime::Sel,
        tag: isize,
        key: &str,
    ) {
        unsafe {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(self.mtm()),
                &t(title),
                Some(action),
                &s(key),
            );
            item.setTarget(Some(self));
            item.setTag(tag);
            menu.addItem(&item);
        }
    }
    fn remove_appearance_observers(&self) {
        for status in self.ivars().status.borrow().iter().skip(1) {
            if let Some(button) = status.button(self.mtm()) {
                unsafe {
                    button.removeObserver_forKeyPath(self, &s("effectiveAppearance"));
                }
            }
        }
    }
    fn add_update_menu(&self, menu: &NSMenu) {
        if let Some(updater) = self.ivars().updater.borrow().as_ref() {
            updater.add_menu_item(menu, self.mtm());
        }
    }
    fn rebuild_status(&self) {
        crate::localization::configure(self.ivars().settings.borrow().language);
        install_main_menu(self);
        self.remove_appearance_observers();
        self.ivars().last_error.borrow_mut().take();
        self.ivars().last_snapshot.borrow_mut().take();
        self.ivars().last_activity.borrow_mut().clear();
        self.ivars().menus.borrow_mut().clear();
        let bar = NSStatusBar::systemStatusBar();
        // Keep native identities (and user-positioned menu bar slots) for items
        // that survive a configuration change. Only obsolete items are removed.
        let mut previous = std::mem::take(&mut *self.ivars().status.borrow_mut()).into_iter();
        let root = previous
            .next()
            .unwrap_or_else(|| bar.statusItemWithLength(NSVariableStatusItemLength));
        let mut reusable: std::collections::HashMap<_, _> = previous
            .map(|status| (status.autosaveName().to_string(), status))
            .collect();
        root.setAutosaveName(Some(&s("pipmark-settings")));
        if let Some(b) = root.button(self.mtm()) {
            let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &s("app.badge"),
                Some(&s("Pipmark")),
            );
            b.setImage(image.as_deref());
            b.setToolTip(Some(&s("Pipmark")));
        }
        let menu = NSMenu::new(self.mtm());
        self.menu_item(&menu, "About Pipmark", sel!(showAbout:), 0, "");
        self.add_update_menu(&menu);
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        self.menu_item(
            &menu,
            "Pipmark Settings…",
            sel!(showSettings:),
            0,
            ",",
        );
        self.menu_item(&menu, "Add Application…", sel!(addApp:), 0, "");
        if !self
            .ivars()
            .settings
            .borrow()
            .rows()
            .iter()
            .any(|(item, _)| matches!(item.kind, ItemKind::Trash))
        {
            self.menu_item(&menu, "Add Trash", sel!(addTrash:), 0, "");
        }
        self.menu_item(&menu, "Default Appearance…", sel!(showDefaults:), 0, "");
        let language_index = menu.numberOfItems();
        self.menu_item(
            &menu,
            "Accessibility Settings…",
            sel!(openAccessibility:),
            0,
            "",
        );
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        self.menu_item(&menu, "Export Settings…", sel!(exportSettings:), 0, "");
        self.menu_item(&menu, "Import Settings…", sel!(importSettings:), 0, "");
        self.menu_item(
            &menu,
            "Restore Previous Settings…",
            sel!(restoreSettings:),
            0,
            "",
        );
        menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
        self.menu_item(&menu, "Quit Pipmark", sel!(quit:), 0, "q");
        let language_item = NSMenuItem::new(self.mtm());
        language_item.setTitle(&t("Language"));
        let languages = NSMenu::new(self.mtm());
        for (index, language) in crate::model::Language::ALL.into_iter().enumerate() {
            let row = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm()),
                    &t(language.name()),
                    Some(sel!(chooseLanguage:)),
                    &s(""),
                )
            };
            unsafe {
                row.setTarget(Some(self));
            }
            row.setTag(index as isize);
            row.setState(if language == self.ivars().settings.borrow().language {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            languages.addItem(&row);
        }
        language_item.setSubmenu(Some(&languages));
        menu.insertItem_atIndex(&language_item, language_index);
        root.setMenu(Some(&menu));
        self.ivars().status.borrow_mut().push(root);
        for (index, item) in self.ivars().settings.borrow().items.iter().enumerate() {
            let status = reusable
                .remove(&item.id)
                .unwrap_or_else(|| bar.statusItemWithLength(NSVariableStatusItemLength));
            status.setAutosaveName(Some(&s(&item.id)));
            if let Some(button) = status.button(self.mtm()) {
                crate::attention::clear(&button);
                button.setTitle(&s(""));
                button.setAlphaValue(1.0);
                button.setImage(Some(&crate::render::button_icon(
                    item,
                    &Activity::default(),
                    &button,
                    self.mtm(),
                )));
                button.setToolTip(Some(&s(&item.name)));
                button.setTag(index as isize);
                unsafe {
                    button.addObserver_forKeyPath_options_context(
                        self,
                        &s("effectiveAppearance"),
                        NSKeyValueObservingOptions::empty(),
                        std::ptr::null_mut(),
                    );
                }
                unsafe {
                    button.setTarget(Some(self));
                    button.setAction(Some(sel!(statusClick:)));
                }
                button.sendActionOn(NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp);
            }
            let menu = NSMenu::new(self.mtm());
            let row_index = self
                .ivars()
                .settings
                .borrow()
                .rows()
                .iter()
                .position(|(i, _)| i.id == item.id)
                .unwrap_or(0);
            if let ItemKind::Group { children } = &item.kind {
                self.group_menu(&menu, children);
            } else {
                self.menu_item(
                    &menu,
                    &crate::localization::format("Open {app}", &[("app", &item.name)]),
                    sel!(openApp:),
                    row_index as isize,
                    "",
                );
            }
            if matches!(item.kind, ItemKind::Trash) {
                self.menu_item(
                    &menu,
                    "Empty Trash…",
                    sel!(emptyTrash:),
                    row_index as isize,
                    "",
                );
            }
            menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
            self.menu_item(&menu, "Settings…", sel!(editItem:), row_index as isize, "");
            self.menu_item(&menu, "Quit Pipmark", sel!(quit:), 0, "q");
            self.ivars().menus.borrow_mut().push(menu);
            status.setVisible(item.enabled);
            self.ivars().status.borrow_mut().push(status);
        }
        for status in reusable.into_values() {
            bar.removeStatusItem(&status);
        }
        self.start_monitor();
        self.refresh_activity();
    }
    fn launch(&self, index: usize, toggle: bool) {
        let item = self.ivars().settings.borrow().items.get(index).cloned();
        let Some(item) = item else {
            return;
        };
        if matches!(item.kind, ItemKind::Trash) {
            self.open_trash();
        } else if matches!(item.kind, ItemKind::App { .. }) {
            self.open_application(&item, toggle);
        } else {
            // Release RefCell borrows before AppKit enters a nested menu event loop.
            let menu = self.ivars().menus.borrow().get(index).cloned();
            let button = self
                .ivars()
                .status
                .borrow()
                .get(index + 1)
                .and_then(|i| i.button(self.mtm()));
            if let (Some(menu), Some(button)) = (menu, button) {
                menu.popUpMenuPositioningItem_atLocation_inView(
                    None,
                    NSPoint::new(0.0, 0.0),
                    Some(&button),
                );
            }
        }
    }

    fn suspend_monitor(&self) {
        self.ivars().last_snapshot.borrow_mut().take();
        if let Some(timer) = self.ivars().timer.borrow_mut().take() {
            timer.invalidate();
        }
        self.ivars().monitor.borrow_mut().take();
        for status in self.ivars().status.borrow().iter() {
            if let Some(button) = status.button(self.mtm()) {
                crate::attention::clear(&button);
            }
        }
    }
    fn start_monitor(&self) {
        if self.ivars().lifecycle.borrow().suspended() {
            return;
        }
        let targets = self.ivars().settings.borrow().monitored_apps();
        if targets.is_empty() {
            self.ivars().monitor.borrow_mut().take();
            if let Some(timer) = self.ivars().timer.borrow_mut().take() {
                timer.invalidate();
            }
            return;
        }
        if self
            .ivars()
            .monitor
            .borrow()
            .as_ref()
            .is_none_or(|m| !m.watches(&targets))
        {
            *self.ivars().monitor.borrow_mut() = Some(crate::monitor::Monitor::new(targets));
        }
        if self.ivars().timer.borrow().is_some() {
            return;
        }
        // AXStatusLabel notification support differs across OS versions. Reconcile
        // at a modest cadence; only mutate status controls when activity changes.
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                3.0,
                self,
                sel!(refreshBadges:),
                None,
                true,
            )
        };
        timer.setTolerance(0.75);
        *self.ivars().timer.borrow_mut() = Some(timer);
    }
    fn refresh_activity(&self) {
        self.receive_activity(true);
    }
    fn workspace_app_changed(&self, id: &str) {
        if self.ivars().lifecycle.borrow().suspended() {
            return;
        }
        let targets = self.ivars().settings.borrow().monitored_apps();
        if targets.is_empty() || (id != "com.apple.dock" && !targets.contains(id)) {
            return;
        }
        let result = self
            .ivars()
            .monitor
            .borrow_mut()
            .as_mut()
            .and_then(|monitor| monitor.invalidate());
        self.apply_activity_result(result);
    }
    fn receive_activity(&self, request_scan: bool) {
        if self.ivars().lifecycle.borrow().suspended() {
            return;
        }
        if self.ivars().settings.borrow().items.is_empty() {
            return;
        }
        let result = self
            .ivars()
            .monitor
            .borrow_mut()
            .as_mut()
            .and_then(|m| if request_scan { m.tick() } else { m.poll() });
        self.apply_activity_result(result);
    }
    fn apply_activity_result(&self, result: Option<Result<dock::Snapshot, dock::Error>>) {
        let snapshot = match result {
            Some(Ok(snapshot)) => snapshot,
            Some(Err(error)) => {
                self.ivars().last_snapshot.borrow_mut().take();
                // Report initial failures too, without redrawing identical errors.
                let message = match error {
                    dock::Error::Permission => crate::localization::text(
                        "Accessibility permission is required to read Dock badges",
                    )
                    .to_owned(),
                    dock::Error::DockUnavailable => {
                        crate::localization::text("The Dock is not available").to_owned()
                    }
                    dock::Error::TrashUnavailable => {
                        crate::localization::text("The Dock Trash item is not available").to_owned()
                    }
                    dock::Error::TrashActionUnavailable => crate::localization::text(
                        "The Dock Empty Trash action is unavailable or disabled",
                    )
                    .to_owned(),
                    dock::Error::Accessibility(code) => crate::localization::format(
                        "Dock Accessibility request failed ({code})",
                        &[("code", &code.to_string())],
                    ),
                };
                let changed = self.ivars().last_error.borrow().as_ref() != Some(&message);
                if changed || !self.ivars().last_activity.borrow().is_empty() {
                    *self.ivars().last_error.borrow_mut() = Some(message.clone());
                    self.ivars().last_activity.borrow_mut().clear();
                    self.render_unavailable(Some(&message));
                }
                return;
            }
            None => return,
        };
        self.ivars().last_error.borrow_mut().take();
        let settings = self.ivars().settings.borrow();
        let mut last = self.ivars().last_activity.borrow_mut();
        let status = self.ivars().status.borrow();
        for (index, item) in settings.items.iter().enumerate() {
            let state = snapshot.display_activity(item);
            if last.get(index) == Some(&state) {
                continue;
            }
            if let Some(status) = status.get(index + 1) {
                let presentation = item.presentation(&state);
                status.setVisible(presentation != Presentation::Hidden);
                if let Some(button) = status.button(self.mtm()) {
                    button.setAlphaValue(if presentation == Presentation::Dimmed {
                        0.35
                    } else {
                        1.0
                    });
                    crate::attention::apply(&button, item, &state);
                    button.setImage(Some(&crate::render::button_icon(
                        item,
                        &state,
                        &button,
                        self.mtm(),
                    )));
                    button.setTitle(&s(&crate::render::title(item, &state)));
                    let tooltip = if state.has_unread() && item.appearance.notifications {
                        if item.appearance.show_count
                            && item.appearance.badge_style != BadgeStyle::Dot
                        {
                            crate::localization::format(
                                "{app} — {count} unread",
                                &[("app", &item.name), ("count", &state.label)],
                            )
                        } else {
                            crate::localization::format(
                                "{app} — Unread notifications",
                                &[("app", &item.name)],
                            )
                        }
                    } else {
                        item.name.clone()
                    };
                    button.setToolTip(Some(&s(&tooltip)));
                }
            }
            if index < last.len() {
                last[index] = state;
            } else {
                last.push(state);
            }
        }
        *self.ivars().last_snapshot.borrow_mut() = Some(snapshot);
    }
    /// Preferences affect pixels and presentation, not status-item identity.
    fn refresh_appearance(&self) {
        self.ivars().last_activity.borrow_mut().clear();
        let snapshot = self.ivars().last_snapshot.borrow().clone();
        if let Some(snapshot) = snapshot {
            self.apply_activity_result(Some(Ok(snapshot)));
        } else {
            let error = self.ivars().last_error.borrow().clone();
            self.render_unavailable(error.as_deref());
        }
    }
    fn render_unavailable(&self, error: Option<&str>) {
        let settings = self.ivars().settings.borrow();
        let status = self.ivars().status.borrow();
        for (index, item) in settings.items.iter().enumerate() {
            if let Some(status) = status.get(index + 1) {
                status.setVisible(item.enabled);
                if let Some(button) = status.button(self.mtm()) {
                    button.setTitle(&s(""));
                    button.setAlphaValue(1.0);
                    crate::attention::clear(&button);
                    button.setImage(Some(&crate::render::button_icon(
                        item,
                        &Activity::default(),
                        &button,
                        self.mtm(),
                    )));
                    let title = error
                        .map(|e| format!("{} — {}", item.name, e))
                        .unwrap_or_else(|| item.name.clone());
                    button.setToolTip(Some(&s(&title)));
                }
            }
        }
    }
    fn label(&self, view: &NSView, text: &str, frame: NSRect, size: f64, secondary: bool) {
        let label = NSTextField::labelWithString(&s(text), self.mtm());
        label.setFrame(frame);
        label.setFont(Some(&NSFont::systemFontOfSize(size)));
        if secondary {
            label.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        view.addSubview(&label);
    }
    fn button(
        &self,
        view: &NSView,
        title: &str,
        frame: NSRect,
        action: objc2::runtime::Sel,
        tag: isize,
    ) -> Retained<NSButton> {
        let b = if action == sel!(listItemClicked:) {
            AppListButton::new(self, title, frame, action)
        } else {
            unsafe {
                NSButton::buttonWithTitle_target_action(
                    &s(title),
                    Some(self),
                    Some(action),
                    self.mtm(),
                )
            }
        };
        b.setFrame(frame);
        b.setTag(tag);
        if action == sel!(createGroup:) {
            b.setToolTip(Some(&t(
                "Shift-click apps to select them, then create a group.",
            )));
        }
        view.addSubview(&b);
        b
    }
    fn edit_appearance(&self, index: usize) {
        let Some(item) = self
            .ivars()
            .settings
            .borrow()
            .rows()
            .get(index)
            .map(|(i, _)| (*i).clone())
        else {
            return;
        };
        self.appearance_pane(item, index, false, false);
    }
    fn appearance_pane(&self, item: Item, index: usize, is_defaults: bool, group_apps: bool) {
        let mtm = self.mtm();
        if self.ivars().window.borrow().is_none() {
            self.show_settings();
        }
        self.ivars().list_rows.borrow_mut().clear();
        let window = self.ivars().window.borrow().as_ref().unwrap().clone();
        let expanded = *self.ivars().advanced_appearance.borrow() && !group_apps;
        let supports_dot = is_defaults || item.supports_activity_dot();
        let extra = if expanded {
            if supports_dot { 192.0 } else { 148.0 }
        } else {
            0.0
        };
        let height = 600.0 + extra;
        let content = SettingsSurface::new(mtm, rect(0.0, 0.0, 560.0, height), false, 0.0, 0);
        if matches!(item.kind, ItemKind::Group { .. }) && !is_defaults {
            let name = NSTextField::initWithFrame(
                NSTextField::alloc(mtm),
                rect(24.0, height - 50.0, 280.0, 28.0),
            );
            name.setStringValue(&s(&item.name));
            name.setFont(Some(&NSFont::systemFontOfSize(18.0)));
            name.setBezeled(false);
            name.setDrawsBackground(false);
            name.setToolTip(Some(&s(&item.name)));
            unsafe {
                name.setTarget(Some(self));
                name.setAction(Some(sel!(renameGroup:)));
            }
            content.addSubview(&name);
            for (title, action, selected, x) in [
                ("Settings", sel!(groupSettings:), !group_apps, 326.0),
                ("Apps", sel!(groupApps:), group_apps, 434.0),
            ] {
                let button = self.button(
                    &content,
                    crate::localization::text(title),
                    rect(x, height - 50.0, 102.0, 26.0),
                    action,
                    index as isize,
                );
                button.setButtonType(NSButtonType::PushOnPushOff);
                button.setState(if selected {
                    NSControlStateValueOn
                } else {
                    NSControlStateValueOff
                });
            }
        } else {
            self.label(
                &content,
                &item.name,
                rect(24.0, height - 50.0, 512.0, 28.0),
                18.0,
                false,
            );
        }
        let form = NSView::initWithFrame(NSView::alloc(mtm), rect(24.0, 0.0, 512.0, height - 64.0));
        content.addSubview(&form);
        let card = |y, rows| {
            let card = SettingsSurface::new(
                mtm,
                rect(0.0, y, 512.0, rows as f64 * 44.0),
                true,
                44.0,
                rows,
            );
            form.addSubview(&card);
            card
        };
        let icons = card(height - 204.0, 3);
        let badges = card(height - 352.0, 3);
        let visibility = card(height - 456.0, 2);
        let advanced = card(100.0, if supports_dot { 4 } else { 3 });
        advanced.setHidden(!expanded);
        let check = |parent: &NSView, title: &str, y: f64, value: bool| {
            self.label(
                parent,
                crate::localization::text(title),
                rect(14.0, y + 5.0, 390.0, 22.0),
                14.0,
                false,
            );
            self.switch(
                parent,
                rect(450.0, y, 48.0, 28.0),
                sel!(appearanceChanged:),
                0,
                value,
                title,
            )
        };
        let popup = |parent: &NSView, label: &str, y: f64, names: &[&str], selected: isize| {
            self.label(
                parent,
                crate::localization::text(label),
                rect(14.0, y + 5.0, 260.0, 22.0),
                14.0,
                false,
            );
            let frame = rect(280.0, y, 220.0, 28.0);
            let control = if label == "Badge style" {
                StylePreviewButton::new(self, frame)
            } else {
                NSPopUpButton::initWithFrame_pullsDown(NSPopUpButton::alloc(mtm), frame, false)
            };
            for name in names {
                control.addItemWithTitle(&t(name));
            }
            control.selectItemAtIndex(selected);
            control.setBordered(false);
            control.setAlignment(NSTextAlignment::Right);
            control.setFont(Some(&NSFont::systemFontOfSize(13.0)));
            unsafe {
                control.setTarget(Some(self));
                control.setAction(Some(sel!(appearanceChanged:)));
            }
            parent.addSubview(&control);
            control
        };
        let pinned = check(&icons, "Pin to menu bar", 94.0, item.appearance.pinned);
        let size = popup(
            &icons,
            "Icon size",
            50.0,
            &["Small", "Normal", "Large"],
            match item.appearance.icon_size {
                0..=16 => 0,
                17..=19 => 1,
                _ => 2,
            },
        );
        self.label(
            &icons,
            crate::localization::text("Menu bar icon"),
            rect(14.0, 11.0, 260.0, 22.0),
            14.0,
            false,
        );
        if !is_defaults {
            self.symbol_button(
                &icons,
                "arrow.counterclockwise",
                "Default Icon",
                rect(298.0, 6.0, 28.0, 28.0),
                sel!(resetIcon:),
                index as isize,
            );
            self.button(
                &icons,
                crate::localization::text("Choose Icon…"),
                rect(338.0, 6.0, 162.0, 28.0),
                sel!(chooseIcon:),
                index as isize,
            );
        } else {
            self.label(
                &icons,
                crate::localization::text("Default Icon"),
                rect(330.0, 11.0, 170.0, 22.0),
                13.0,
                true,
            );
        }
        let counts = check(
            &badges,
            "Show notification count",
            94.0,
            item.appearance.show_count,
        );
        let style = popup(
            &badges,
            "Badge style",
            50.0,
            &["Text", "Dot", "Circle"],
            item.appearance.badge_style as isize,
        );
        self.set_style_previews(&style, &item);
        if let Some(menu) = style.menu() {
            menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        counts.setEnabled(item.appearance.badge_style != BadgeStyle::Dot);
        self.label(
            &badges,
            crate::localization::text("Highlight color"),
            rect(14.0, 11.0, 300.0, 22.0),
            14.0,
            false,
        );
        let color =
            NSColorWell::initWithFrame(NSColorWell::alloc(mtm), rect(456.0, 6.0, 42.0, 28.0));
        let [r, g, b] = item.appearance.color;
        color.setColor(&NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0));
        badges.addSubview(&color);
        unsafe {
            color.setTarget(Some(self));
            color.setAction(Some(sel!(appearanceChanged:)));
        }
        let empty = popup(
            &visibility,
            "No unread notifications",
            50.0,
            &["Show", "Dim", "Hide"],
            item.appearance.when_empty as isize,
        );
        let stopped = popup(
            &visibility,
            "App is not running",
            6.0,
            &["Show", "Dim", "Hide"],
            item.appearance.when_stopped as isize,
        );
        let top = if supports_dot { 138.0 } else { 94.0 };
        let notifications = check(
            &advanced,
            "Show notifications",
            top,
            item.appearance.notifications,
        );
        let attention = popup(
            &advanced,
            "Unread attention",
            top - 44.0,
            &["None", "Gentle pulse", "Blink", "Double blink"],
            item.appearance.attention as isize,
        );
        let running_indicator = check(
            &advanced,
            "Show running indicator",
            top - 88.0,
            item.appearance.running_indicator,
        );
        let activity_dot = supports_dot.then(|| {
            check(
                &advanced,
                "Show activity dots",
                6.0,
                item.appearance.show_activity_dot,
            )
        });
        let disclosure = self.button(
            &content,
            crate::localization::text("Advanced"),
            rect(24.0, height - 506.0, 140.0, 26.0),
            sel!(toggleAdvanced:),
            0,
        );
        disclosure.setBordered(false);
        disclosure.setAlignment(NSTextAlignment::Left);
        disclosure.setImage(
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &s(if expanded {
                    "chevron.down"
                } else {
                    "chevron.right"
                }),
                None,
            )
            .as_deref(),
        );
        disclosure.setImagePosition(NSCellImagePosition::ImageLeft);
        if group_apps {
            form.setHidden(true);
            disclosure.setHidden(true);
            let members = SettingsSurface::new(mtm, rect(24.0, 100.0, 512.0, 432.0), true, 0.0, 0);
            content.addSubview(&members);
            let inner = NSView::initWithFrame(NSView::alloc(mtm), rect(40.0, 20.0, 432.0, 380.0));
            members.addSubview(&inner);
            self.group_members_view(&inner, &item);
        }
        let done = self.button(
            &content,
            crate::localization::text("Done"),
            rect(444.0, 26.0, 92.0, 30.0),
            sel!(showSettings:),
            0,
        );
        done.setKeyEquivalent(&s("\r"));
        *self.ivars().editor.borrow_mut() = Some(AppearanceEditor {
            id: item.id,
            is_defaults,
            group_apps,
            style,
            attention,
            size,
            empty,
            stopped,
            notifications,
            counts,
            running_indicator,
            activity_dot,
            pinned,
            color,
        });
        window.setContentSize(NSSize::new(560.0, height));
        window.setContentView(Some(&content));
        self.update_style_previews();
        window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
    fn set_style_previews(&self, control: &NSPopUpButton, item: &Item) {
        control
            .effectiveAppearance()
            .performAsCurrentDrawingAppearance(&block2::StackBlock::new(|| {
                for (index, style) in [BadgeStyle::Text, BadgeStyle::Dot, BadgeStyle::Circle]
                    .into_iter()
                    .enumerate()
                {
                    if let Some(row) = control.itemAtIndex(index as isize) {
                        row.setImage(Some(&crate::render::style_preview(item, style, self.mtm())));
                    }
                }
            }));
    }
    fn update_style_previews(&self) {
        let editor = self.ivars().editor.borrow();
        let Some(editor) = editor.as_ref() else {
            return;
        };
        let settings = self.ivars().settings.borrow();
        let item = if editor.is_defaults {
            Some(Item {
                id: String::new(),
                name: "Defaults".into(),
                enabled: true,
                appearance: settings.defaults.clone(),
                kind: ItemKind::Group { children: vec![] },
            })
        } else {
            settings
                .rows()
                .into_iter()
                .find(|(i, _)| i.id == editor.id)
                .map(|(i, _)| i.clone())
        };
        if let Some(item) = item {
            self.set_style_previews(&editor.style, &item);
        }
    }
    fn restore_current_editor(&self) {
        let selection = self
            .ivars()
            .editor
            .borrow()
            .as_ref()
            .map(|e| (e.id.clone(), e.is_defaults, e.group_apps));
        if let Some((id, is_defaults, apps)) = selection {
            if is_defaults {
                let item = Item {
                    id: String::new(),
                    name: crate::localization::text("Defaults").into(),
                    enabled: true,
                    appearance: self.ivars().settings.borrow().defaults.clone(),
                    kind: ItemKind::Trash,
                };
                self.appearance_pane(item, 0, true, false);
                return;
            }
            let row = self
                .ivars()
                .settings
                .borrow()
                .rows()
                .into_iter()
                .enumerate()
                .find(|(_, (item, _))| item.id == id)
                .map(|(index, (item, _))| (index, item.clone()));
            if let Some((index, item)) = row {
                self.appearance_pane(item, index, false, apps);
                return;
            }
        }
        self.show_settings();
    }
    fn save_appearance(&self) {
        let editor = self.ivars().editor.borrow();
        let Some(AppearanceEditor {
            id,
            is_defaults,
            style,
            attention,
            size,
            empty,
            stopped,
            notifications,
            counts,
            running_indicator,
            activity_dot,
            pinned,
            color,
            ..
        }) = editor.as_ref()
        else {
            return;
        };
        let mut settings = self.ivars().settings.borrow().clone();
        {
            let a = if *is_defaults {
                &mut settings.defaults
            } else {
                let Some(current) = settings.item_mut(id) else {
                    return;
                };
                &mut current.appearance
            };
            a.badge_style = match style.indexOfSelectedItem() {
                1 => BadgeStyle::Dot,
                2 => BadgeStyle::Circle,
                _ => BadgeStyle::Text,
            };
            a.attention = match attention.indexOfSelectedItem() {
                1 => AttentionStyle::Pulse,
                2 => AttentionStyle::Blink,
                3 => AttentionStyle::DoubleBlink,
                _ => AttentionStyle::None,
            };
            a.icon_size = match size.indexOfSelectedItem() {
                0 => 16,
                2 => 22,
                _ => 18,
            };
            let visibility = |value| match value {
                1 => Visibility::Dim,
                2 => Visibility::Hide,
                _ => Visibility::Show,
            };
            a.when_empty = visibility(empty.indexOfSelectedItem());
            a.when_stopped = visibility(stopped.indexOfSelectedItem());
            a.notifications = notifications.state() == NSControlStateValueOn;
            a.show_count = counts.state() == NSControlStateValueOn;
            a.running_indicator = running_indicator.state() == NSControlStateValueOn;
            if let Some(control) = activity_dot {
                a.show_activity_dot = control.state() == NSControlStateValueOn;
            }
            a.pinned = pinned.state() == NSControlStateValueOn;
            if let Some(rgb) = color
                .color()
                .colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())
            {
                a.color = [
                    rgb.redComponent(),
                    rgb.greenComponent(),
                    rgb.blueComponent(),
                ];
            }
        }
        counts.setEnabled(style.indexOfSelectedItem() != 1);
        if let Err(error) = self.commit_settings(settings) {
            drop(editor);
            self.alert("Unable to save settings", &error);
            self.restore_current_editor();
            return;
        }
        self.update_style_previews();
        if !*is_defaults {
            self.refresh_appearance();
        }
    }
    fn switch(
        &self,
        view: &NSView,
        frame: NSRect,
        action: objc2::runtime::Sel,
        tag: isize,
        value: bool,
        label: &str,
    ) -> Retained<NSSwitch> {
        let control = NSSwitch::initWithFrame(NSSwitch::alloc(self.mtm()), frame);
        control.setState(if value {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        control.setTag(tag);
        control.setToolTip(Some(&t(label)));
        control.setAccessibilityLabel(Some(&t(label)));
        unsafe {
            control.setTarget(Some(self));
            control.setAction(Some(action));
        }
        view.addSubview(&control);
        control
    }
    fn symbol_button(
        &self,
        view: &NSView,
        symbol: &str,
        label: &str,
        frame: NSRect,
        action: objc2::runtime::Sel,
        tag: isize,
    ) -> Retained<NSButton> {
        let button = self.button(view, "", frame, action, tag);
        button.setBordered(false);
        button.setImage(
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &s(symbol),
                Some(&t(label)),
            )
            .as_deref(),
        );
        button.setImagePosition(NSCellImagePosition::ImageOnly);
        button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        button.setToolTip(Some(&t(label)));
        button.setAccessibilityLabel(Some(&t(label)));
        button
    }
    fn show_settings(&self) {
        self.ivars().editor.borrow_mut().take();
        self.ivars().list_rows.borrow_mut().clear();
        let valid: std::collections::HashSet<_> = self
            .ivars()
            .settings
            .borrow()
            .rows()
            .iter()
            .map(|(item, _)| item.id.clone())
            .collect();
        self.ivars()
            .selected_rows
            .borrow_mut()
            .retain(|id| valid.contains(id));
        let mtm = self.mtm();
        if self.ivars().window.borrow().is_none() {
            let window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    rect(0.0, 0.0, 680.0, 400.0),
                    NSWindowStyleMask::Titled
                        | NSWindowStyleMask::Closable
                        | NSWindowStyleMask::Miniaturizable,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };
            unsafe {
                window.setReleasedWhenClosed(false);
            }
            window.setTitle(&s("Pipmark"));
            window.center();
            *self.ivars().window.borrow_mut() = Some(window);
        }
        let w = self.ivars().window.borrow();
        let window = w.as_ref().unwrap();
        let view = ApplicationDropView::new(self, rect(0.0, 0.0, 560.0, 520.0));
        window.setContentSize(NSSize::new(560.0, 520.0));
        window.setContentView(Some(&view));
        self.label(
            &view,
            crate::localization::text("General"),
            rect(24.0, 474.0, 400.0, 28.0),
            20.0,
            false,
        );
        let general = SettingsSurface::new(mtm, rect(24.0, 364.0, 512.0, 88.0), true, 44.0, 2);
        view.addSubview(&general);
        self.label(
            &general,
            crate::localization::text("Launch at login"),
            rect(14.0, 54.0, 390.0, 22.0),
            14.0,
            false,
        );
        self.label(
            &general,
            crate::localization::text("Show settings on startup"),
            rect(14.0, 10.0, 390.0, 22.0),
            14.0,
            false,
        );
        let settings = self.ivars().settings.borrow();
        let login_status = crate::login::status();
        let login = self.switch(
            &general,
            rect(450.0, 50.0, 48.0, 28.0),
            sel!(toggleLogin:),
            0,
            login_status == crate::login::Status::Enabled,
            "Launch at login",
        );
        login.setEnabled(NSBundle::mainBundle().bundleIdentifier().is_some());
        self.switch(
            &general,
            rect(450.0, 6.0, 48.0, 28.0),
            sel!(toggleStartup:),
            0,
            settings.show_on_start,
            "Show settings on startup",
        );
        self.label(
            &view,
            crate::localization::text("Icons"),
            rect(32.0, 320.0, 400.0, 24.0),
            15.0,
            false,
        );
        let visible = main_list_height(settings.rows().len());
        let scroll = NSScrollView::initWithFrame(
            NSScrollView::alloc(mtm),
            rect(24.0, 300.0 - visible, 512.0, visible),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        let rows = settings.rows();
        let height = (rows.len() as f64 * 36.0).max(visible);
        let list = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 512.0, height));
        if rows.is_empty() {
            self.label(
                &list,
                crate::localization::text("Add an app to get started"),
                rect(18.0, 24.0, 470.0, 24.0),
                14.0,
                true,
            );
        }
        for (index, (item, depth)) in rows.iter().enumerate() {
            let y = height - (index + 1) as f64 * 36.0;
            let indent = (*depth as f64 * 14.0).min(84.0);
            self.label(
                &list,
                "⠿",
                rect(10.0 + indent, y + 7.0, 14.0, 20.0),
                12.0,
                true,
            );
            let row = self.button(
                &list,
                &item.name,
                rect(28.0 + indent, y, 368.0 - indent, 36.0),
                sel!(listItemClicked:),
                index as isize,
            );
            row.setButtonType(NSButtonType::PushOnPushOff);
            row.setBordered(false);
            row.setAlignment(NSTextAlignment::Left);
            row.setFont(Some(&NSFont::systemFontOfSize(14.0)));
            row.setToolTip(Some(&s(&item.name)));
            let icon = self.list_icon(item);
            icon.setSize(NSSize::new(22.0, 22.0));
            row.setImage(Some(&icon));
            row.setImagePosition(NSCellImagePosition::ImageLeft);
            unsafe {
                row.setMenu(Some(&self.item_actions(item, index)));
            }
            self.symbol_button(
                &list,
                "info.circle",
                "Settings…",
                rect(402.0, y + 4.0, 28.0, 28.0),
                sel!(editItem:),
                index as isize,
            );
            self.switch(
                &list,
                rect(450.0, y + 4.0, 48.0, 28.0),
                sel!(toggleApp:),
                index as isize,
                item.enabled,
                &item.name,
            );
            if index > 0 {
                let line =
                    NSBox::initWithFrame(NSBox::alloc(mtm), rect(14.0, y + 35.5, 484.0, 0.5));
                line.setBoxType(NSBoxType::Separator);
                line.setAlphaValue(0.35);
                list.addSubview(&line);
            }
            self.ivars()
                .list_rows
                .borrow_mut()
                .push((item.id.clone(), row));
        }
        scroll.setDocumentView(Some(&list));
        view.addSubview(&scroll);
        list.scrollPoint(NSPoint::new(0.0, height - visible));
        self.symbol_button(
            &view,
            "plus",
            "Add App…",
            rect(32.0, 267.0 - visible, 28.0, 28.0),
            sel!(addApp:),
            0,
        );
        let remove = self.symbol_button(
            &view,
            "minus",
            "Remove",
            rect(64.0, 267.0 - visible, 28.0, 28.0),
            sel!(removeSelected:),
            0,
        );
        *self.ivars().remove_button.borrow_mut() = Some(remove);
        self.symbol_button(
            &view,
            "folder",
            "New Group…",
            rect(96.0, 267.0 - visible, 28.0, 28.0),
            sel!(createGroup:),
            0,
        );
        let line = NSBox::initWithFrame(NSBox::alloc(mtm), rect(24.0, 299.5 - visible, 512.0, 0.5));
        line.setBoxType(NSBoxType::Separator);
        line.setAlphaValue(0.35);
        view.addSubview(&line);
        self.update_list_selection();
        if login_status == crate::login::Status::ApprovalRequired {
            self.button(
                &view,
                crate::localization::text("Approve in Login Items…"),
                rect(24.0, 28.0, 240.0, 26.0),
                sel!(openLoginSettings:),
                0,
            );
        }
        if !dock::trusted() {
            self.symbol_button(
                &view,
                "exclamationmark.circle",
                "Notification Access…",
                rect(504.0, 28.0, 28.0, 28.0),
                sel!(openAccessibility:),
                0,
            );
        }
        window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}
fn install_main_menu(delegate: &Delegate) {
    let mtm = delegate.mtm();
    let app = NSApplication::sharedApplication(mtm);
    let main_menu = NSMenu::new(mtm);
    let app_item = NSMenuItem::new(mtm);
    let app_menu = NSMenu::new(mtm);
    delegate.add_update_menu(&app_menu);
    delegate.menu_item(&app_menu, "About Pipmark", sel!(showAbout:), 0, "");
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    delegate.menu_item(&app_menu, "Settings…", sel!(showSettings:), 0, ",");
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    delegate.menu_item(&app_menu, "Quit Pipmark", sel!(quit:), 0, "q");
    app_item.setSubmenu(Some(&app_menu));
    main_menu.addItem(&app_item);
    let edit_item = NSMenuItem::new(mtm);
    edit_item.setTitle(&t("Edit"));
    let edit_menu = NSMenu::new(mtm);
    edit_menu.setTitle(&t("Edit"));
    for (title, action, key) in [
        ("Undo", sel!(undo:), "z"),
        ("Redo", sel!(redo:), "Z"),
        ("Cut", sel!(cut:), "x"),
        ("Copy", sel!(copy:), "c"),
        ("Paste", sel!(paste:), "v"),
        ("Select All", sel!(selectAll:), "a"),
    ] {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &t(title),
                Some(action),
                &s(key),
            )
        };
        if action == sel!(redo:) {
            item.setKeyEquivalentModifierMask(
                NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
            );
        }
        // Nil target uses the responder chain, including the active field editor.
        edit_menu.addItem(&item);
    }
    edit_item.setSubmenu(Some(&edit_menu));
    main_menu.addItem(&edit_item);
    let window_item = NSMenuItem::new(mtm);
    window_item.setTitle(&t("Window"));
    let window_menu = NSMenu::new(mtm);
    window_menu.setTitle(&t("Window"));
    for (title, action, key) in [
        ("Close", sel!(performClose:), "w"),
        ("Minimize", sel!(performMiniaturize:), "m"),
    ] {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &t(title),
                Some(action),
                &s(key),
            )
        };
        window_menu.addItem(&item);
    }
    window_item.setSubmenu(Some(&window_menu));
    main_menu.addItem(&window_item);
    app.setWindowsMenu(Some(&window_menu));
    app.setMainMenu(Some(&main_menu));
}
pub fn run() {
    let mtm = MainThreadMarker::new().expect("Main thread required");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let delegate = Delegate::new(mtm);
    install_main_menu(&delegate);

    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}
