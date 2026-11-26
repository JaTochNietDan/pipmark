# Badgeify parity and verification ledger

Reference: Badgeify 1.15.0, public documentation and changelog inspected 2026-11-25.
Sources: https://docs.badgeify.app/ ; https://badgeify.app/changelog/

The goal is a complete native Rust recreation. This ledger tracks remaining work;
it does not narrow the goal to the current implementation.

- [ ] App discovery, adding/removing/enabling/reordering apps
- [ ] Native status items; left-click launch/toggle, right-click context menus
- [ ] Dock badge reading through Accessibility, including non-numeric badges
- [ ] Live updates, Dock restart, sleep/wake, permission recovery
- [ ] Native settings window matching the reference layout and polish
- [ ] Nested groups, aggregate unread counts, group menus and editing
- [ ] Text/dot/circle badges, count privacy, highlight color previews
- [ ] App/default/custom SVG icons, template detection, icon sizes
- [ ] Hide/dim/show rules for no unread and stopped apps, pin overrides
- [ ] Slack/Discord dot controls and running indicators
- [ ] Attention animation styles; stop timers when inactive
- [ ] Trash open/empty via native confirmation and visibility
- [ ] Launch at login, startup settings preference
- [ ] Settings import/export, automatic backups and recovery UI
- [ ] CLI list/status with documented JSON and permission/error semantics
- [ ] Language selection and localized native menus/dialogs
- [ ] Menu bar manager compatibility and command-drag ordering
- [ ] Reopen/settings recovery when status items are hidden
- [ ] Distribution bundle and update mechanism
- [ ] Visual verification in light/dark appearance
- [ ] Live behavior verified against Dock changes
- [ ] Release idle CPU measured with settings closed, with/without unread

Architecture: AppKit accessed directly from Rust using objc2. Native NSApplication
run loop, no embedded browser, no frame loop. Prefer AXObserver and workspace
notifications. Any necessary reconciliation timer must be measured and documented.
Do not claim event completeness until actual Dock notifications are verified.

Performance acceptance: target <=0.1% average idle CPU over 60 seconds after warmup,
no recurring rendering when state is unchanged. Measure process CPU time delta,
resident memory and wakeups if available. Record environment and monitoring mode.

## Validation log — 2026-11-25

- `mise exec -- cargo test`: 5 tests passed for visibility precedence, unread
  aggregation, nested ID validation, atomic save and previous-version backup.
- `mise run build`: release build and ad-hoc signed .app bundle succeeded.
- Native UI inspection: bundled app opened its settings window; Add Application
  opened NSOpenPanel and Cancel returned to settings successfully.
- Screenshot capture failed with ScreenCaptureKit error -3811. No visual fidelity
  claim is made from accessibility-tree inspection alone.
- Activity model is not yet wired into the runtime (dead-code warnings expected).
- Next: implement Dock Accessibility observation, verify real badge attributes and
  event support, and wire incremental status updates before measuring idle CPU.

## Dock and CLI implementation pass — 2026-11-25

- Added read-only AX Dock traversal, matching only application tiles by URL and
  bundle ID. CF values have owned lifetimes and type checks; AX calls have a
  250 ms messaging timeout. No name-based matching or notification database reads.
- Added `list` and `status --id` JSON commands, unknown-ID and permission errors.
  CLI integration test exercises the built executable with isolated settings.
- Added three-second reconciliation with timer tolerance and changed-state-only
  status updates. No reconciliation timer when no enabled items exist. This is
  polling, not proven AX event delivery. Moving reads off the UI thread and caching
  tile resolution remain performance/responsiveness work.
- Added separate left-click launch/hide and right-click menus; context Open always
  launches/activates. These paths compile but still require interaction validation.
- Text/dot badges and visibility model are wired to snapshots. Circle rendering,
  tinting, configuration controls and permission-loss UI remain incomplete.
- `mise exec -- cargo test`: 5 unit tests and 1 executable integration test passed.
- `mise run build`: succeeded without warnings.
- Live CLI probe explicitly returned `NO_PERMISSIONS`. Real unread updates, Dock
  restart recovery and performance under successful observation remain unverified.
  The app includes a link to Accessibility settings; it does not grant itself access.

## Appearance implementation pass — 2026-11-25

- Per-app native appearance editor now exposes text/dot/circle styles, count
  visibility, notification toggle, icon size, hide/dim/show rules, pinning and
  NSColorWell. Row and context-menu Settings open the corresponding item.
- Circle/dot rendering uses native AppKit drawing, only on changed state. Circle
  counts cap at 99; nonnumeric attention gets an empty circle. Hidden counts are
  also omitted from tooltips. Native text remains theme-adaptive in status buttons.
- `examples/render_badges.rs` generates a synthetic 3x3 render sheet (3,125,dot)
  through the actual renderer. Inspected target/badge-preview.png: dots, circle
  placement and white counts render correctly. This does not prove live Dock data.
- Native UI: pin preference changed, saved, and confirmed checked on reopening;
  target/qa-settings.json independently confirms persistence. Test uses an isolated
  config, not the user's settings. Color picker and other choices are not yet fully
  interaction-tested.
- UI automation loses the active alert after a popup choice (app sample confirms
  it remains in NSAlert's modal event loop). Checkbox save/reopen is verified;
  popup selection/save remains unverified. A dedicated settings detail pane is
  still needed for the requested visual parity, replacing this temporary editor.
- Seven tests pass (6 unit + 1 executable integration); release bundle builds.

## Import/export and recovery pass — 2026-11-25

- Added native menu actions and file dialogs for settings export/import and restore.
  Import validates the entire configuration before asking to replace current data;
  it writes a backup before applying it. No application restart is required.
- Strict import/backup reads reject missing files instead of interpreting them as
  an empty configuration. Global appearance defaults now receive validation too.
- Atomic writes use unique, exclusively created temporary files with mode 0600.
  Corrupt primary settings cannot rotate over a known-good backup. Restoration
  preserves the primary bytes in a separate timestamped recovery file.
- Startup corruption offers Restore Backup or Quit; it no longer leaves an app
  running without a menu. Native import/export interaction remains to be checked.
- Nine tests passed (8 unit + 1 executable integration). Added regressions for
  corrupt-primary backup preservation and missing backup/import handling.
- Added scripts/measure_idle.py for reproducible, isolated release measurements.
  It records whether the CLI could actually read Dock badges and does not conflate
  the denied-permission baseline with successful notification monitoring.

### Initial idle measurement

`docs/validation/idle-no-permission-2026-11-25.json`: release binary, one configured
app, settings closed, 5-second warmup, 60.013 seconds measured. CPU time increased
0.01 seconds (~0.017% average); peak sampled RSS 85.3 MiB. Accessibility was denied.
This proves only the denied-permission baseline, not successful Dock scanning,
unread rendering, observer behavior, or the final low-CPU acceptance requirement.

Settings now explains missing Accessibility permission and provides a direct
Notification Access button. This opens system settings without granting access.

## Nested groups pass — 2026-11-25

- Added group creation, hierarchical settings rows, move-to-group/root actions and
  recursive native NSMenu submenus. Appearance and enable/remove controls resolve
  nested items correctly. Group aggregate activity uses enabled descendants.
- Hierarchy moves validate a cloned candidate before applying it, rejecting cycles,
  nonexistent/non-group destinations and excessive depth. Removing a group promotes
  its children in place rather than deleting their configuration.
- Released menu/state borrows before entering native nested menu event loops.
- Eleven tests pass (10 unit + 1 CLI integration), including cycle rollback and
  preservation/order of children when removing a group. Release bundle builds.
- Native UI with isolated target/qa-groups.json: Work > Tools > Finder appeared;
  nested Finder Settings opened the correct editor. Move to menu-bar root returned
  successfully; saved JSON confirms Finder became a top-level item while Work and
  Tools remained. Popup destination selection and live subgroup-menu interaction
  still need verification. Multi-selection grouping, rename, drag reorder and
  reference-style group tabs remain incomplete.

## Background monitoring pass — 2026-11-25

- Dock reads now execute on a single background worker with an autorelease pool
  per scan. Main-thread timer ticks only receive completed results and enqueue at
  most one next request. Slow AX calls cannot block native menu/settings handling.
- Request and result channels are bounded; ticks while a scan is in progress do
  not queue more work. Removing all enabled items drops the monitor and timer.
  Shutdown does not join the worker on the UI thread; abandoned results cannot
  enter a later monitor instance.
- When an established snapshot fails, stale counts are cleared and the tooltip
  explains the read error. Enabled launchers remain available while status is
  unknown. Live permission-loss/recovery behavior still needs system validation.
- Thirteen tests pass (12 unit + 1 executable integration), including a reader
  deliberately blocked while 100 UI ticks return without overlapping scans.
- Refresh is still polling, not AX event driven. Completion is consumed at the
  next three-second timer tick, so badge latency may approach six seconds plus
  IPC time. Cached tile discovery and prompt completion delivery remain work.
- Requested user assistance enabling Accessibility for the built bundle so real
  Dock behavior and successful-monitoring performance can be measured. Remaining
  implementation work can continue independently of that permission.

Background-worker release baseline: 60.013 seconds, 0.01 CPU seconds (~0.017%),
85.7 MiB peak sampled RSS, one configured app, settings closed, permission denied.
Recorded in docs/validation/idle-worker-no-permission-2026-11-25.json. This remains
a denied-permission baseline and does not satisfy the successful-monitoring gate.

## Custom SVG icons pass — 2026-11-25

- Added Choose Custom Icon / Use Default Icon to per-item status menus. Imports
  parse and rasterize SVG through resvg into managed 64px PNG assets under the
  settings directory. Source deletion does not remove the imported icon.
- Rejects malformed, oversized (>2 MiB), empty and external-image-only artwork.
  SVG file/URL image resolution is disabled; no external files are embedded by
  following image hrefs. SVG text and embedded raster image feature coverage is
  not complete with the current resvg feature selection.
- Monochrome detection uses unpremultiplied raster colors with antialias tolerance;
  monochrome assets use native template rendering, multicolor assets retain color.
  Combined circle/dot icons tint template artwork with the native label color.
- Fifteen tests pass (14 unit + 1 executable integration). Release bundle builds.
- Synthetic custom SVG badge sheet generated through real import/render code and
  inspected at target/custom-badge-preview.png. Dot/circle template composites are
  visible in dark appearance. Standalone template adaptation relies on NSStatusBar
  rendering and is not established by the standalone drawing sheet. Live theme
  changes and actual menu-bar rendering remain to be verified.
- Remaining icon gaps: SVG source preservation / portable export, live theme-change
  invalidation for composite icons, richer built-in icon selection, settings-pane
  import control and native file-picker interaction validation.

## Portable icon archives pass — 2026-11-25

- Settings export now writes a versioned envelope containing configuration and
  embedded PNG assets, deduplicating icons shared by defaults/apps/groups.
- Import validates configuration, asset references and PNG dimensions/content
  before presenting the replacement confirmation. Asset IDs never become file
  paths; accepted imports receive fresh managed filenames. Template/color behavior
  survives the round trip. Legacy local settings are normalized into this format
  when their referenced icons are available.
- Source-path portability gap is closed for managed rasters. Original SVG sources
  are not yet retained. If materialization succeeds but configuration saving fails,
  unused managed files may remain; existing settings and backups stay intact.
- Seventeen tests pass (16 unit + 1 CLI integration), including export, deletion of
  original icon, import to a different directory and exact PNG-byte verification.
  Decoder checks reject invalid/oversized-dimension PNG input before pixel allocation.
- Native file-dialog import/export interaction and full cross-machine application
  path resolution remain unverified. This does not establish overall parity.

## Native detail-pane pass — 2026-11-25

- Replaced the modal appearance alert with an in-window editor and native material
  sidebar. App/group selection preserves hierarchy; dropdowns, checkboxes and color
  changes save immediately. Custom-icon controls are now available in the pane.
- Counts are disabled for dot style, including when reopening that style. Sidebar
  rows use app/group icons, left-aligned labels and selected-row-only borders.
- Native interaction verified: Finder appearance opens; Circle selected; navigate
  to Work and back; Finder retains Circle. This resolves the previous loss of the
  modal editor after a popup choice. All tests remain passing (17 total).
- Screenshot capture now succeeds. Inspected dark-mode pane: controls are readable
  and aligned, with sidebar and content separation. The subsequent sidebar icon /
  border refinement still needs a fresh screenshot. Light mode and reference
  fidelity remain unverified. Management still uses the earlier list view; group
  tabs, inline previews, rename, reorder and full settings integration remain work.

## Group rename and ordering pass — 2026-11-25

- Group names are editable in the detail-pane header. Enter commits a nonblank
  name. Configuration updates retain the selected item, including custom-icon
  updates, rather than returning unexpectedly to management.
- Management rows offer earlier/later controls within the current parent. Tests
  confirm boundaries and subtree preservation. This provides accessible ordering;
  drag-and-drop and native command-drag menu bar ordering still need validation.
- Eighteen tests pass (17 unit + 1 executable integration). Release bundle builds.
- Native interaction: renamed Work to Workspace, selected Finder's earlier button,
  and verified Finder precedes Workspace while Tools remains nested beneath it.
- Testing found Command-A was missing in the name field. Added a native Edit menu
  with responder-chain Undo/Cut/Copy/Paste/Select All actions. Compiled successfully;
  keyboard shortcut runtime revalidation remains pending.

## Native launch-at-login pass — 2026-11-25

- Added ServiceManagement SMAppService integration and a Launch at login control.
  Enabled/disabled/requires-approval states come from macOS, not a saved boolean.
  Register/unregister errors are surfaced; pending approval links to Login Items.
  Importing settings cannot silently enable startup registration.
- Local Apple SDK headers confirm API availability on macOS 13+, code-signing
  requirements and error semantics. The bundle's deployment target is macOS 13.
- Native UI check returned SMAppService NotFound for this development launch.
  No login preference was changed in validation. The control permits an explicit
  registration attempt from a bundled app and will report the actual error.
  Successful registration, approval/revocation and login launch remain unverified.
- Eighteen tests pass and the release bundle builds. Runtime regression check:
  Command-A followed by typing replaced Workspace with Work and persisted, proving
  the responder-chain Edit menu fixed the missing Select All behavior.

## Unread attention pass — 2026-11-25

- Added None/Gentle pulse/Blink/Double blink appearance preferences, defaulting to
  None for existing and new settings. Native opacity keyframe animations run on
  the status button layer; no per-frame Rust timer or icon rerasterization is used.
- Animations are removed on cleared unread state, disabled notifications, read
  failure, item removal and accessibility display-options changes. Reduce Motion
  suppresses them; its live notification invalidates the current presentation.
- Nineteen tests pass (18 unit + 1 CLI integration). Native main-thread smoke test
  `mise exec -- cargo run --example check_attention` confirmed animation install
  and removal for all three styles, with Reduce Motion=false on this machine.
- This is synthetic status-button validation, not proof of real Dock updates or
  visual cadence. Energy/CPU cost while animations run, display sleep handling,
  live Reduce Motion changes and reference animation timing remain unverified.

## Sleep/wake suspension pass — 2026-11-25

- Registered native workspace notifications for system sleep/wake, display
  sleep/wake and session activation/deactivation. Independent suspension reasons
  prevent one wake event from resuming work while another reason remains active.
- Suspension invalidates the polling timer, disconnects the background monitor and
  removes attention animations. The current scan may finish, but its result is
  discarded and no new scans are scheduled. Resume creates a fresh monitor and
  invalidates the previous presentation before requesting a new snapshot.
- Application termination removes workspace observers and stops monitoring.
- Added state-transition tests for overlapping and duplicate events. Actual hardware
  sleep/wake, Fast User Switching, and suspended CPU/energy remain unverified;
  validation does not force the user's display or machine to sleep.

## Dock discovery cache pass — 2026-11-25

- Background reader now retains Dock application AX elements and their resolved
  bundle IDs. Routine samples read status labels without repeating role, URL and
  bundle discovery for every tile. CLI queries still create a fresh reader.
- Discovery is rebuilt when the sorted PID/bundle-ID running-app signature changes
  (including Dock PID changes), after any read error, or after 30 seconds. The
  fallback covers Dock pin/unpin changes without app launches. New caches are
  published only after complete discovery; per-tile IPC timeout is set explicitly.
- Reader creation and destruction occur on its dedicated thread. No Send override
  was added for AX/CF pointers; only snapshots cross the bounded result channel.
- Twenty-one tests pass; release bundle builds. A fresh bundled CLI probe still
  returned NO_PERMISSIONS. Actual cache validity across Dock changes, latency and
  successful-read CPU performance are not yet established.

## Default appearance pane pass — 2026-11-25

- Added Defaults navigation and a Default Appearance menu action, exposing default
  badge/attention style, icon size, visibility, notification/count options, pinning
  and color. Defaults save immediately without rebuilding existing status items.
- Existing app/group preferences remain independent. Add Application and New Group
  already copy Settings.defaults when creating their new items.
- Native UI validation changed default pinning, then returned to Finder and found
  its existing pinning unchanged. Isolated saved JSON independently confirms
  defaults.pinned=true and Finder.appearance.pinned=false.
- Twenty-one tests pass; release bundle builds. Bulk applying defaults and global
  interaction/language settings remain separate unfinished work.

## CLI hierarchy semantics pass — 2026-11-25

- CLI list/status now use the shared hierarchy traversal rather than a separate
  flattening implementation. `shown` means a standalone status item: nested apps
  and subgroups report false while still exposing their activity and count.
- Added synthetic snapshot regression coverage for nested icon visibility and
  aggregate counts excluding disabled subgroups. Actual Dock permission remains
  required for live status queries; tests do not substitute synthetic live data.
- Dock discovery now treats an absent AXList as unavailable, rather than publishing
  a successful empty snapshot that could incorrectly clear unread counts.
- Twenty-three tests pass (22 unit + 1 executable integration). Runtime behavior
  during Dock startup/restart and full CLI/reference parity remain unverified.

## Compact application list pass — 2026-11-25

- Reduced the settings content size to 680×480 and simplified app management to
  32-point icon-and-name rows. The appearance sidebar uses 30-point rows with
  actual application icons and folder symbols for groups.
- Clicking a row opens appearance settings. Native context menus retain enable,
  move, reorder, and remove actions without placing buttons on every row.
- Icons are copied before sizing so list layout cannot resize the status icon.
- Release bundle builds. Native UI inspection confirmed icons, context menus,
  and the compact appearance form without clipping using an isolated QA config.
- Accessibility remains unavailable by user preference; live badge validation is
  deferred, and these styling checks do not establish full Badgeify parity.

## Targeted Dock monitoring pass — 2026-11-25

- Background readers now retain and query badge labels only for configured,
  enabled application leaves. Disabled ancestor groups exclude their descendants.
  Discovery still traverses Dock tiles periodically to resolve bundle IDs.
- Empty groups no longer keep a polling timer or reader thread alive. Changes to
  the enabled app set replace the reader; cosmetic edits retain the existing cache.
  CLI snapshots retain their independent all-app read behavior.
- Added regression coverage for nested groups, disabling ancestors, empty groups,
  and re-enabling a group while one descendant remains disabled. All 24 tests
  pass (23 unit + 1 integration); the signed release bundle builds successfully.
- Live AX IPC counts and CPU savings remain unmeasured without Accessibility.
  This reduces the work requested by the implementation, not proof of the final
  idle-performance target or event-delivery parity.

## Menu bar appearance refresh pass — 2026-11-25

- Status buttons observe their own effectiveAppearance. Changes redraw their
  cached activity immediately in that button's drawing appearance, without an
  additional timer, Dock read, or status-item rebuild. Observers are removed
  before rebuilding status items and on application termination.
- The renderer copies shared workspace/symbol images before resizing them, so
  one item's icon size cannot mutate another item's source image.
- `mise exec -- cargo run --example check_appearance` exercises the production
  delegate callback with an unread group, switches only its button between Aqua
  and Dark Aqua, verifies different rendered images and retained unread state,
  then rebuilds and checks detached-observer cleanup. The check passed; generated
  light/dark PNGs were inspected. System appearance and permissions were unchanged.
- All 24 tests pass. Full settings-window light/dark review, wallpaper-dependent
  menu bar contrast, and automatic system theme transition remain unverified.
- API references: [NSAppearance](https://developer.apple.com/documentation/appkit/nsappearance)
  and [effectiveAppearance](https://developer.apple.com/documentation/appkit/nsappearancecustomization/effectiveappearance).

## Slack/Discord activity dot control pass — 2026-11-25

- Added Show activity dots to Slack/Discord appearance and Defaults. Existing
  settings default to enabled, retaining previous behavior. Controls fit the
  compact form; unrelated apps do not show the app-specific option.
- Disabling filters dot-only labels (`.`, `•`, `●`, `·`) for the two stable app
  bundle IDs. Numeric badges and other nonnumeric labels are preserved. Filtered
  state drives visibility, attention and nested group aggregation, while CLI
  statusLabel/unread retain raw Dock data and shown reflects display policy.
- All 25 tests pass (24 unit + 1 integration); release bundle builds. Regression
  coverage includes group aggregation, running-state preservation, old defaults,
  raw CLI status, numbered badges and unrelated apps.
- Native UI inspection verified layout and toggle persistence using an isolated
  synthetic Slack entry; saved JSON confirms the app preference changed without
  changing defaults. No real Slack/Discord badge traffic or exact AX dot glyphs
  have been validated; other channels/bundle IDs remain to be checked.
- Reference: [visibility controls](https://docs.badgeify.app/features/icon-visibility)
  and [Discord toggle changelog](https://badgeify.app/changelog/archive/).

## Group menu detail pass — 2026-11-25

- Enabled group members now have app/folder icons in their native menu rows.
  NSMenuDelegate refreshes row titles on opening using the latest cached snapshot:
  unread labels for count-enabled items and a dot when counts are hidden. Nested
  groups show their aggregate activity; management rows remain unchanged.
- No extra polling or Dock read is added for menus. Snapshot caches clear on
  monitoring failure, suspension and settings rebuild, preventing stale menu
  counts from surviving a known read failure.
- `mise exec -- cargo run --example check_group_menu` passed. It invokes the
  production Objective-C menuNeedsUpdate selector against native NSMenu objects,
  checking nested aggregation, icons, count privacy, launch tags and missing-data
  cleanup. This validates the callback; it does not simulate actual menu opening
  or live Dock traffic. NSMenu.update alone did not invoke that callback while
  the test menu was closed, so the check calls the selector explicitly.
- All 25 tests pass and the release bundle builds. Visual inspection of the
  opened group menus and real-time badge changes remains pending.

## Group menu access from settings — 2026-11-25

- Group rows now include an Apps submenu in their context menu, using the same
  builder and update delegate as status menus. This exposes grouped launchers
  even when menu bar buttons are difficult to reach.
- Empty groups/subgroups display a disabled No enabled apps row instead of a
  blank submenu. The native menu smoke check now covers that state.
- Native UI interaction opened Work → Apps and exposed its Finder action, then
  dismissed the menu normally. Capture returned only the owning settings window,
  excluding floating menus; full menu visual fidelity remains unverified.
- A separate popup action was tried and removed because it did not remain open
  after the context menu action. The final implementation uses a standard submenu.

## Immediate worker completion delivery — 2026-11-25

- Completed Dock reads now enqueue a main-dispatch notification. The AppKit
  callback consumes the result immediately rather than waiting for the following
  reconciliation tick. Scan requests retain the three-second cadence/tolerance.
- Completion polling never requests another read; bounded channels still limit
  work to one in-flight request. Queued notifications capture no delegate or AX
  pointers, and retired-worker notifications cannot inject retired snapshots.
- All 26 tests pass (25 unit + 1 integration). Added a completion/scan separation
  regression. `check_monitor_delivery` exercises the production AppKit callback
  and renderer with synthetic activity: count delivered in 10 ms, one read and
  no timer. This measures completion dispatch, not real Dock detection latency.
- Release bundle builds. This remains periodic Dock scanning, not AXObserver
  notification delivery. Live badge detection and successful-read CPU are still
  pending Accessibility access.
- Fresh 60.027-second denied-permission baseline with one configured app and
  settings closed recorded no CPU-time increase at ps's 0.01-second resolution;
  peak sampled RSS was 83.91 MiB. See
  `docs/validation/idle-completion-delivery-2026-11-25.json`. This is below the
  measurement's resolution, not a claim of literally zero CPU or live-read cost.

## Running indicator pass — 2026-11-25

- Added a Show running indicator preference to item appearance and Defaults.
  A small appearance-aware dot sits below the icon while the app is running;
  groups use their existing any-member-running aggregate. Existing configurations
  default to off. The option uses existing snapshots and adds no timer.
- Running state is independent of notification visibility and unread badge style.
  The renderer composes the indicator only when needed, preserves icon width
  without unread badges, and retains stopped/disabled behavior.
- `mise exec -- cargo run --example check_running` passed for running/stopped,
  opt-out, disabled notifications and all three icon sizes. Inspected the generated
  render sheet: unread badges sit above/right, running dots sit below.
- All 26 tests pass; release bundle builds. The appearance form was tightened to
  fit the new checkbox in the existing compact window. Native visual inspection
  of that control was attempted but the Mac was locked. This layout check and
  live launch/quit behavior remain pending.

## Select members during group creation — 2026-11-25

- New App Group now combines naming with a scrollable native checklist of
  existing apps and groups, including icons and hierarchy indentation. Empty
  groups can still be created; Cancel makes no configuration change.
- Added transactional group creation to the model. Selected items retain order,
  enabled state and appearance; selecting an ancestor and its descendant carries
  the subtree once. New groups inherit Defaults. Unknown selections, invalid
  names, duplicate IDs and validation failures leave the original tree untouched.
- All 28 tests pass (27 unit + 1 integration), including overlap/order/preferences
  and atomic rejection coverage; release bundle builds. The new modal checklist
  has not yet been visually or interactively verified while the Mac is locked.
- Shift-selection grouping directly from the main list and drag-and-drop remain
  separate unfinished requirements; this implements the create-group-first flow.

## Failed-read presentation and recovery — 2026-11-25

- Initial monitoring errors now set informative item tooltips even when no read
  has succeeded. Changed errors refresh the explanation; repeated identical errors
  reuse the existing image rather than redraw. Items remain available as launchers.
- Successful snapshots clear error state and restore counts. Subsequent failures
  clear cached activity, group-menu snapshots and visible count text. Settings
  rebuilds invalidate the error cache so newly created controls receive an update.
- `check_read_failure` passed against native status buttons using production result
  handling: initial denial, identical-image reuse, changed error, success and later
  denial. This uses synthetic results and does not establish actual AX permission
  recovery behavior. No permissions were changed.

## Preserve status items during appearance edits — 2026-11-25

- Appearance preference saves now refresh existing status buttons from the cached
  raw snapshot, including nested-member preferences and group aggregates. They no
  longer remove/recreate native menu bar items or request a new Dock scan.
- With no successful snapshot, appearance edits update the fallback image while
  preserving the current error tooltip. Accessibility display-option changes also
  refresh cached presentation instead of requesting a read.
- Extended the native failure/recovery check to verify stable NSStatusItem
  identity, immediate icon-size/count-privacy changes, retained snapshot state,
  no monitor creation, and preserved failure explanation during fallback edits.
  The check and all 28 tests pass; release bundle builds.
- This establishes identity preservation for appearance edits. Actual command-drag
  ordering and compatibility with third-party menu bar managers remain unverified.

## Group Settings and Apps views — 2026-11-25

- Group detail now has Settings and Apps navigation. The Apps view presents
  immediate members as compact icon/name rows with the existing context actions;
  nested groups open their own detail view. Membership edits retain the Apps view.
- Add Apps presents an icon/checklist picker of apps outside the group's entire
  subtree. Selected apps move into the group in one saved update; failure leaves
  the live settings unchanged. Cancel/empty selection makes no change. Moving
  entire groups remains available through existing context menus.
- All 28 tests passed after the tab/member implementation; the final release
  bundle builds. UI validation was attempted with an isolated group plus another
  app, but the Mac remains locked. Tab switching, picker interaction, and visual
  spacing are unverified; an unlock request is pending.

## Rendered badge-style previews — 2026-11-25

- Badge style options now show rendered previews beside simple Text/Dot/Circle
  labels. Previews use the item's icon and a synthetic count of 12, independent
  of actual unread data. Dot/circle samples reflect the chosen highlight color.
- Previews refresh after preference changes and when their native menu opens,
  using the popup's effective drawing appearance. No background rendering timer
  was introduced.
- Generated and inspected `target/style-previews.png` via
  `mise exec -- cargo run --example render_style_previews`; all three styles and
  three highlight colors render correctly. All 28 tests pass; release builds.
- Native dropdown interaction/appearance remains unverified while the Mac is
  locked. Reference documentation describes theme-adaptive text while also
  claiming color support across styles; exact text-style highlight behavior still
  needs visual comparison. It was not changed based on that ambiguity.
- Reference: https://docs.badgeify.app/features/badge-styles and
  https://docs.badgeify.app/features/highlight-color.

## Language selection and core menu localization — 2026-11-25

- Added a persisted Language choice with System Default, English, German,
  Japanese, Korean, Simplified Chinese and Traditional Chinese. Existing settings
  follow the system's preferred supported language; explicit selections override
  it without changing any macOS preference.
- Embedded translations cover core status/application/Edit menus and common
  dialog action buttons. Native menus rebuild on selection. App names, paths,
  identifiers and CLI output are not translated. Unknown strings fall back to
  their original text; settings-page text and detailed messages remain incomplete.
- Added locale-tag/fallback and settings-serialization tests. Native smoke checks
  passed for German/Japanese root and application menus, one selected-language
  checkmark, and an app named Settings… remaining unchanged. No user configuration
  was written by the smoke check.
- Full translated settings layout, error/help text, OS-owned panel localization,
  and live language switching through the UI remain unfinished/unverified. The
  current catalog is a first localization pass, not full language parity.

## Localized compact settings controls — 2026-11-25

- Extended the catalog and explicit translation call sites for appearance labels,
  style/size/visibility/attention choices, notification/running options, group tabs,
  main list actions, startup controls and common empty-state/help labels.
- Dynamic app/group titles remain untranslated. The Defaults pseudo-item uses a
  localized heading; the group-name placeholder is translated without rewriting
  the stored name. Detailed errors, tooltips and some dialogs remain in English.
- Widened the top action buttons and group tabs within the existing window;
  corrected empty-state label widths to stay inside the list document.
- `check_localized_text` passed native font-width checks for compact form and
  navigation labels in every supported language. This validates text dimensions,
  not complete rendered-window layout or native control padding behavior.
- All 30 tests pass; release bundle builds. Visual language/layout checks remain
  pending while the Mac is locked.

## Localized dialogs and dynamic status text — 2026-11-25

- Added translations for recovery/import confirmations, grouping instructions,
  common failure titles, login approval instructions, app launch commands and
  unread/error tooltips. CLI errors retain their existing machine-facing contract.
- Dynamic messages use named placeholders with single-pass substitution. App
  names/error details containing braces are inserted literally, not reinterpreted
  as further template fields. Catalog tests check placeholder preservation in
  every translation; explicit cases cover Unicode and brace-containing names.
- Native failure/recovery checks still pass; release bundle builds. The fixture
  now selects English explicitly so its error-text assertions do not depend on
  the machine's system language.
- Backend technical error details and some tooltips/panel text still fall back to
  English. Native dialog sizing, OS-owned controls and full localization coverage
  remain to be validated; this does not complete language parity.

## Transactional UI settings changes — 2026-11-25

- Add/remove/enable/startup actions now save a proposed settings copy before
  publishing it to the live app. Previously several actions mutated live state
  first and continued rebuilding after a save error.
- Shared commit handling also backs appearance edits and group/settings updates.
  Failed startup/enable toggles restore their checkbox state; failed appearance
  or group-name edits reconstruct the editor from the unchanged saved state.
  Duplicate app additions no longer produce a redundant save/rebuild.
- `check_settings_commit` passed with a new isolated fixture: a corrupt primary
  caused a rejected commit without changing either memory or file bytes; a later
  successful commit published matching memory/disk state and retained the prior
  backup. No user settings were used.
- All 32 tests pass and release bundle builds. Interactive error-dialog/editor
  restoration still needs verification on the unlocked desktop.

### Compact application lists — 2026-11-25

Main management content reduced from 680×480 to 680×400 points; application, group-member, and sidebar rows now use 28-point spacing. Plain app-name rows retain actual 18-point macOS application icons and context menus. Scrollbars hide when unnecessary, and long main/sidebar lists start at the top. Appearance controls retain their 480-point content height. Release bundle built successfully with mise. Desktop remains locked, so the updated layout has not received live visual verification. No monitoring timers or background work added.

### SVG text and embedded raster images — 2026-11-25

Enabled resvg text, system-fonts, and raster-images features. Font discovery is lazy within each SVG import; vector-only icons do not load system fonts. Imported artwork is still normalized once to a managed 64×64 PNG, so status redraws and idle monitoring do not parse SVGs or scan fonts. Added pixel checks for text-only SVGs and embedded two-color PNGs, and a valid temporary image fixture proving external filesystem references are excluded. All 34 unit tests plus the CLI integration test pass. JPEG/GIF/WebP codecs are enabled but have not each received a dedicated fixture check. Live import-panel visual verification remains outstanding.

### Trash item and opening integration — 2026-11-25

Added a persisted, duplicate-safe Add Trash menu action, native trash symbols in lists/status menus, and left-click/Open handlers using the Dock AXTrashDockItem subrole and AXPress action. Newly added Trash is pinned visible. No Finder Automation permission is requested. The adapter checks Accessibility trust and reports errors without prompting. Native check_trash_item passed: persistence, duplicate prevention, image presence, removal of the Add Trash action, and no worker/timer for a Trash-only configuration. All 35 existing tests pass. Live Dock opening is unverified because Accessibility access remains declined. Empty Trash, full/empty detection, corresponding visibility controls, and CLI Trash status are still incomplete; this is partial implementation, not Trash parity. No destructive action was invoked. Reference: https://badgeify.app/changelog/ and Apple ApplicationServices Trash subrole/Press action documentation.

### Nonblocking Trash actions — 2026-11-25

Moved the Dock open action onto a one-shot worker. AppKit consumes completion through the existing main-queue notification; no action polling timer is installed. A pending-action guard suppresses repeated clicks, and completed actions are released before presenting an error alert. Termination drops the receiver without joining the worker. Added tests for nonblocking stalled actions, error delivery, and abandoned receivers. Native check_trash_item verifies repeated opens retain the original synthetic pending action and completion releases it. All 36 unit tests and one CLI integration test pass. Live Dock interaction remains unverified; no real Trash contents were read or modified by these checks.

### Native application file drops — 2026-11-25

The compact management content view now implements NSDraggingDestination for file-URL drops. Copy acceptance checks app bundle validity; drops share a transactional addition path with the file picker, skip duplicate bundle IDs, and reject remote URLs/non-app extensions. App names prefer localized CFBundleDisplayName/CFBundleName before the filename. The view holds a weak delegate reference. check_app_drop passed using a private pasteboard and a disposable app bundle: mixed URL filtering, display name, persisted addition, no write for duplicates, and weak-owner cleanup. macOS emitted a sandbox-extension warning for the private pasteboard, but the asserted URLs and saved result were correct. Actual Finder dragging, hover feedback, and window hit testing remain visually unverified while the desktop is locked. No recurring work was introduced.

Trash investigation: the installed Dock resources contain DockMenus/EMPTY_TRASH and Accessibility/AXTrashDockItem localization, but no empty/full localization signal was established. Do not infer live Trash state from these resources. Empty/full detection and native Empty Trash remain open.

### Workspace lifecycle refresh — 2026-11-25

Registered NSWorkspace launch/termination notifications, extracting the affected NSRunningApplication and filtering against enabled monitored leaves (or Dock). Relevant events invalidate the existing worker immediately; unrelated events and events during suspension request no scan. Monitor invalidation coalesces a burst during an in-flight read into one follow-up; timer ticks retain their existing bounded behavior and do not set the follow-up flag. All 37 unit tests and one CLI integration test pass, including a 100-event burst producing exactly two reads and then remaining idle. Native check_monitor_delivery passes with synthetic activity and the production completion callback, including ignored unrelated/suspended events and one relevant read with no timer. Release bundle rebuilt. Actual launch/quit Dock badges remain unverified without Accessibility access. Apple documents that launch notifications exclude background/LSUIElement apps, so this does not establish event completeness or replace reconciliation: https://developer.apple.com/documentation/appkit/nsworkspace/didlaunchapplicationnotification

### Settings render review and preview appearance fix — 2026-11-25

Added render_settings to capture the production main list, group appearance pane, and group-member pane with synthetic settings in Aqua/Dark Aqua. Views are cached into bitmaps and composited SourceOver onto their native window background; the default NSImageRep draw operation initially erased the background, so those initial captures were rejected. Inspected the corrected six fixture images under target/settings-{list,group,members}-{light,dark}.png. They show compact icon/name rows and readable controls without visible clipping in the English fixture. Offscreen capture cannot validate real menu positioning, desktop vibrancy, hit testing, or all translated/long-name layouts.

The render review found stale white badge-style preview artwork in a light settings window. Added a weak-delegate NSPopUpButton subclass that refreshes preview artwork on effective-appearance changes, plus a refresh after attachment to the settings window. No observer teardown or timer is required. render_settings now asserts the preview changes in dark mode and restores identical light-mode pixels on switching back without opening the menu.

### Long-name and localized layout review — 2026-11-25

Extended render_settings with German/Japanese and long-name fixtures via BADGEIFY_PREVIEW_LANGUAGE and BADGEIFY_PREVIEW_LONG_NAMES. Inspected German group settings/member panes and the Japanese main list. Native sidebar rows already truncate with ellipses; added full-name tooltips to main/sidebar/group rows and the group-name field. Corrected untranslated Add Apps in group membership and wired import/export/application/SVG file-panel titles through the localization catalog, adding all five language translations. The corrected German members render shows Apps hinzufügen… fitting its button. Editing a long group name retains the native scrolling text field. These fixtures do not prove every locale/control combination or live tooltip behavior. All 38 existing tests passed.

### Preserve native status-item identities — 2026-11-25

Status rebuilds now reuse the root item and surviving top-level items by their existing autosave names, creating/removing only changed entries. Reused buttons clear old animation/title/opacity, receive updated tags, icons, menus and visibility, and have appearance observers removed/re-registered once. check_status_reuse passed for rename, language, disable, row reorder, removal and addition, including pointer identity and stale-state checks. All 38 tests pass. Stable autosave names pre-existed this change; Apple documents them as the mechanism for saving/restoring item information. Actual Command-drag order across app restarts remains unverified and is not established by object-identity checks. Source: https://developer.apple.com/documentation/appkit/nsstatusitem/autosavename-swift.property

### Select apps before grouping — 2026-11-25

Main compact rows now toggle membership with Shift-click or Command-click, render a native selected state, and precheck those IDs in New Group. Ordinary clicks clear selection and open the existing editor. Selection survives a list refresh/cancel, prunes deleted IDs, and clears after a successful group commit. Added localized New Group tooltip guidance. check_list_selection passed for highlight state, toggling off, retention/pruning, selected-group model input, and ordinary editing. Inspected target/settings-selected-light.png produced by render_settings with BADGEIFY_PREVIEW_SELECTION=1. All 38 existing tests passed. Actual modifier-key delivery and modal confirmation/cancel remain unverified on the locked desktop. Reference: https://docs.badgeify.app/features/app-groups

### Stable selection-row alignment — 2026-11-25

Replaced selection-dependent NSButton borders with an AppListButton drawRect background using the native unemphasized selection color. The button remains borderless in both states, so icon/title insets no longer jump when toggled. Native content tint uses labelColor and adapts to light/dark mode. Added an image-rectangle equality check to check_list_selection, which passed. Inspected selected-row renders in both appearances; existing disabled opacity remains distinct. All 38 tests passed during this change; rendering and appearance round-trip checks passed after the final tint adjustment. No redraw timer or animation is involved.

Updated release idle baseline: docs/validation/idle-compact-selection-2026-11-25.json records Finder configured, settings closed, Accessibility denied, five-second warmup and 60.020 seconds measured. Cumulative process CPU delta was below the 0.01-second ps resolution (reported 0.00 seconds), with peak sampled RSS 84.06 MiB. This is not proof of zero CPU or successful live Dock monitoring performance. Updated README usage/runtime descriptions to reflect current selection, drop, completion-delivery and Trash limitations.

### App relocation and metadata refresh — 2026-11-25

App opening now validates the saved bundle identifier/path and falls back to NSWorkspace URLForApplicationWithBundleIdentifier if the path is stale or belongs to another app. Both status-click and menu Open use the resolver; unresolved/failed opens report localized errors. Startup and relevant launch notifications reconcile nested app names/paths and atomically save only changes. Missing apps, custom group names, disabled state and appearance preferences are retained. Suspended sessions ignore these notifications. No recurring metadata scan was added.

check_app_resolution passed using a stale Finder path, an unknown bundle ID pointed at Finder, and nested fixture settings: native lookup, ID validation, name/path updates, persistence, preference preservation, unrelated-event no-op and idempotence. All 38 tests passed. No apps were moved or launched by these checks. Actual relocation during a running session and renamed web-app metadata changes remain unverified; NSBundle caching and Launch Services registration timing may affect those cases. Source: https://developer.apple.com/documentation/appkit/nsworkspace/urlforapplication(withbundleidentifier:)

### Dragging compact list rows — 2026-11-25

Added a native internal drag type and NSDraggingSource tracking to the compact application rows, with before/after insertion feedback and transactional persistence. Moving beside an item adopts its parent and retains the moved subtree and preferences; moves beside descendants or missing targets are rejected without publishing settings. Modifier-click selection and control-click forwarding remain in the row handler. No idle timer was added.

All 39 unit tests and one CLI integration test pass. check_row_drop passed native coordinate conversion, before/after marker selection, self/missing/outside rejection, saved ordering and unchanged disk state on an invalid move. Reviewed settings-drop-light.png and settings-drop-dark.png: the native accent insertion marker is visible over the plain icon/name list. These checks exercise helper paths and rendering, not physical drag event delivery. Real dragging, Escape cancellation, modifier gestures and long-list autoscrolling remain unverified; explicit drag autoscrolling has not been implemented.

### Long-list drag scrolling — 2026-11-25

Internal row drags now scroll by 14 points per native drag update within a 24-point edge zone. Scrolling is confined to the actual clip-view viewport and clamped to document bounds; middle/outside positions do not scroll. Drop hit testing derives its viewport from the enclosing scroll view, replacing fixed window coordinates. Drag-end cleanup removes insertion feedback, including when a drag ends elsewhere. No application timer or idle work was added. AppKit supplies periodic draggingUpdated callbacks: https://developer.apple.com/documentation/appkit/nsdraggingdestination/wantsperiodicdraggingupdates()

Extended check_row_drop passed against a real 40-row NSScrollView: stationary helper calls reach both document ends, stop there, select the newly visible first/last drop target, ignore middle/outside positions, and reject coordinates outside a resized viewport. The check calls the production geometry/scrolling helpers; physical drag delivery, cancellation and scrolling cadence still require live desktop verification.

### Empty Trash action implementation — 2026-11-25

Added Empty Trash… to the Trash status and management menus. Explicit invocation uses the one-shot action worker, finds the Dock Trash tile, requests AXShowMenu, and searches the Dock menu tree for an enabled AXMenuItem matching EMPTY_TRASH from the installed DockMenus localization table. Matching tolerates only trailing ellipsis/whitespace, not arbitrary prefixes or menu positions. Missing localization, disabled/unknown enabled state, and missing actions fail closed. The shared pending-action guard prevents overlapping open/empty requests. macOS controls confirmation behavior; no Finder Automation or direct filesystem deletion is used.

All 40 unit tests plus the CLI integration test pass, including localized title/role/enabled matching. Native check_trash_item verifies the menu selector/tag and suppresses both open and empty requests while a synthetic action is pending, without invoking the real Dock action. Installed Dock resource lookup is covered by the native fixture. Live AXShowMenu discovery, menu cleanup on failure, macOS confirmation and emptying remain unverified because Accessibility access is declined. No real Trash was emptied. Full/empty detection, automatic disabled menu presentation and CLI Trash status remain incomplete. Reference: https://badgeify.app/changelog/ (1.14.0–1.14.2).

### Trash menu failure cleanup — 2026-11-25

Separated finding the open menu from selecting its action. Lookup first checks the requested Trash tile, then the Dock-level menu layout; unrelated application tiles are skipped. The action requires a unique enabled localized-title match. A guard sends AXCancel to the open menu on disabled/missing/ambiguous matches, child-read failures and failed AXPress, preserving the original error. Successful press disarms cleanup so subsequent system UI is not cancelled. This replaces cancellation incorrectly directed at the Dock tile.

All 43 unit tests and the CLI integration test pass. Synthetic trees exercise both menu locations, exclusion of unrelated app tiles, menu ordering, disabled/unknown state, missing and duplicate titles, read/press failures, cancellation targeting and successful-press behavior. These tests use the same traversal and cleanup code with a fake node adapter; they do not establish live Dock AX behavior. If menu discovery itself fails before obtaining a menu reference, cleanup remains unavailable. No real Dock menu or destructive action was invoked.

### Native window commands and recovery checks — 2026-11-25

Added localized Redo (Command-Shift-Z), Close (Command-W), and Minimize (Command-M) to the native application menu. Window commands use the responder chain and the Window menu is registered with NSApplication. Existing settings, quit and editing commands remain native.

check_window_recovery passed: hide every fixture status item, invoke the application reopen delegate, close settings with NSWindow.performClose, invoke reopen again, retain the same window object, leave status visibility unchanged, and start no monitor for an empty configuration. It also checks shortcut selectors/modifiers and nil-target Redo routing. The window server did not grant key/main-window status; attempting menu key delivery therefore did not close the window. Keyboard delivery is explicitly skipped in that environment and remains unverified, as do Finder/Launch Services reopen delivery and third-party menu-manager integration. All 43 unit tests and one CLI integration test pass.

### Import asset rollback and settings size consistency — 2026-11-25

Archive installation now owns newly generated icon paths until storage::save succeeds. Failure during reference resolution or configuration saving triggers best-effort deletion of those fresh files; existing icons and backup-referenced assets are never included. The UI publishes imported settings only after installation succeeds. This avoids orphaned icon files on ordinary failed imports; it is not a multi-file crash transaction, and cleanup can itself fail if filesystem permissions change.

Settings serialization now enforces the same 4 MiB limit as reads before rotating the previous backup. Reads are bounded to the limit plus one byte instead of loading an arbitrarily large file before rejecting it. All 45 unit tests and one CLI integration test pass. New failure fixtures verify unchanged primary/existing-icon bytes and removal of both newly staged assets after a forced backup-write failure, cleanup after an unresolved asset reference, and unchanged primary/previous backup after oversized-save rejection. The portable archive round trip still passes after deleting its original source image. Live import-panel interaction remains unverified.

### Synthetic unread idle measurements — 2026-11-25

Added a release-only executable fixture (examples/idle_fixture.rs) with four configured app items, fixed unread counts of 12, production status controls, the three-second reconciliation timer, bounded worker and main-queue completion delivery. The reader is synthetic and makes no AX calls. An initial image-identity assertion before warmup failed; a 12-second check and subsequent full runs pass after sampling identities at the end of the five-second warmup. This is consistent with startup appearance settling; no steady-state renderer change was made. The fixture verifies unchanged image identity and bounded scan counts.

Sequential 60-second measurements after warmup:

- Static unread: 0.01 seconds cumulative process CPU, approximately 0.0167% average; peak sampled RSS 62.95 MiB.
- Pulse/blink/double-blink styles configured: 0.02 seconds cumulative process CPU, approximately 0.0333% average; peak sampled RSS 62.64 MiB. Reduce Motion was false.
- Both fixtures completed 23 scans across their full approximately 66-second runtime and retained all four post-warmup images.

Reports: docs/validation/idle-synthetic-unread-{static,animated}-2026-11-25.json. CPU measurement resolution is 0.01 seconds. These results meet the process-CPU target only for synthetic data in this desktop session; they exclude successful AX IPC and do not establish visible-desktop compositor/GPU costs. All 45 unit tests and one CLI integration test pass. README documents reproduction commands.

Trash-state investigation remains unresolved: the installed Accessibility localization table labels AXTrashDockItem generically, and the consulted Foundation trash-directory documentation does not establish a complete Dock empty/full signal across mounted volumes. No guessed state was added. Live inspection remains necessary before claiming parity.

### Bundle icon and native About command — 2026-11-25

Added repository-owned vector artwork in assets/app-icon.svg and deterministic resvg iconset generation at build time. The bundle includes a validated ICNS container with ten standard raster representations (16 through 1024 pixels); reviewed the 256-pixel render. CFBundleIconFile now references that asset, replacing the generic Finder icon. No runtime icon generation was added. The application and status menus expose a localized About command invoking AppKit's standard panel. Bundle version metadata now comes from Cargo, and supported localizations are declared.

All 45 unit tests and one CLI integration test pass. mise run build succeeded; codesign --verify --deep --strict and plutil -lint pass. ICNS header/length, representation count and bundle version were checked. Actual Finder icon-cache refresh and the on-screen About panel remain unverified. Signing remains ad hoc; this does not establish distribution signing, notarization or update delivery.

### Verified local release archive — 2026-11-25

Added mise run package and scripts/package.py. The task builds the bundle, validates its signature, creates a versioned architecture-specific ZIP with ditto, extracts it into a temporary directory, verifies the extracted signature, compares executable hashes and bundle metadata, and runs the extracted CLI --help. It emits a SHA-256 sidecar and JSON metadata identifying architecture, minimum OS, signing mode and verification scope. No upload or updater connection is performed.

Packaging passed for dist/badgeify-native-0.1.0-arm64.zip (1,683,309 bytes, minimum macOS 13). The build is ad-hoc signed and notarization is not verified; this is a local test archive, not a public distribution claim. The release/update destination has been requested from the user and remains pending. Packaging follows Apple's ZIP guidance: https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution

### Current native regression and environment audit — 2026-11-25

Ran all sixteen check_* native examples with a separate temporary configuration for each. Fifteen passed initially; check_localization assumed Settings was the first item before About was added. Updated that fixture to locate commands by selector and verify both localized About and Settings titles. Its retry passed; the report retains the initial failure. All sixteen are now passing in docs/validation/native-regression-audit-2026-11-25.json. The window-recovery fixture explicitly reports unavailable keyboard delivery because no key window was granted. These are programmatic native fixtures, not physical user interaction.

Separated About from settings commands in the root menu and positioned Language after Default Appearance using the actual insertion index, keeping Add Application/Add Trash adjacent. Localization and native status-identity checks passed again after that change. Rebuilt and round-trip verified the local archive.

Current external-state evidence: computer-use inventory reports the Mac locked and unable to unlock; native app surfaces are unavailable. The built CLI's isolated Finder query exits 1 with NO_PERMISSIONS (docs/validation/permission-audit-2026-11-25.json). Accessibility remains unchanged as requested. The release/update URL question remains unanswered.

Completion is not established: real unread/permission recovery, Dock restart and wake behavior, physical menus/drag/key delivery, launch-at-login behavior, visible-desktop animation performance and menu-manager compatibility require live verification. Trash full/empty detection and CLI status remain incomplete without an established reliable signal. Distribution signing/notarization and update delivery remain incomplete. The existing fixtures and synthetic CPU measurements do not substitute for these requirements.

### Developer ID signed artifact — 2026-11-25

Read-only signing inventory found exactly one available Developer ID Application identity. Created a separate signed copy under dist/developer-id with hardened runtime and secure timestamp; strict code-signature verification passed. Added reproducible mise run package-signed, retaining the ad-hoc development build separately. The signed packager round trip verifies the original/extracted signatures, metadata, executable hash and extracted CLI.

Verified artifact: dist/badgeify-native-0.1.0-arm64-developer-id.zip (1,688,453 bytes). The installed notarytool resolves successfully. No notarization submission occurred: approval to upload this concrete archive to Apple and the existing keychain profile name have been requested. Release/feed destination also remains pending. Developer ID signing does not prove notarization or Gatekeeper acceptance. Signing evidence: docs/validation/developer-id-signing-2026-11-25.json; package metadata: dist/badgeify-native-0.1.0-arm64-developer-id.json.

### Blocked completion audit — 2026-11-25

Revalidated the persistent live-environment constraints after Developer ID signing: the signed executable also returns NO_PERMISSIONS for an isolated Finder status request, and computer-use inventory still reports the Mac locked with no native app surfaces. The signed archive still matches its recorded SHA-256. These constraints have persisted across the native regression, signing and current audit turns. No permission change or notarization upload was performed.

The goal is blocked, not achieved. Remaining grounded work requires an unlocked interactive desktop for physical UI/launch/login/menu-manager testing, and eventually authorized Dock observation for real badge delivery/recovery/performance and reliable Trash-state investigation. Accessibility remains deferred at the user's request. Trash full/empty behavior and CLI status are still incomplete; guessed state would not establish parity. Update delivery requires a user-controlled release/feed destination. Notarization requires approval for the concrete Apple upload and an existing keychain profile name; both questions remain pending. Current artifacts, native fixtures and synthetic performance results are preserved for resumption.

### Screenshot-directed settings redesign — 2026-11-25

Reworked the General window around the user's three supplied references: white/dark native surfaces, subtle rounded groups, native NSSwitch controls, 36-point icon/name rows with drag handles and info buttons, and a compact plus/minus/folder toolbar. Short lists size their background to their content; long lists scroll. Row clicks select for removal/grouping; info buttons open appearance settings. Main list and appearance controls keep their existing transactional persistence.

Appearance editing now uses three grouped sections for icon, badge and visibility controls, a Done action, and an Advanced disclosure for notification/attention/running/activity controls. Group membership remains available through Settings/Apps tabs. App status menus use concise parenthesized unread counts and a Settings/Quit footer; icon actions live in the appearance panel. These are AppKit views and native controls; no idle rendering loop was introduced. The editor is currently a window pane, not a modal sheet.

All 45 unit tests and the CLI test pass, plus 17 native checks including new switch/selection/removal/Advanced coverage. Light/dark renders were reviewed. The desktop became available during this turn: a separately configured signed preview was inspected through the live UI. Info opened Mail settings; notification-count toggling changed its accessible value and restored it; Advanced exposed the additional controls; Done returned to General; Mail selection enabled Remove; Messages' enable switch changed off/on; Shift-click cleared selection. Native active switches and Done rendered blue. The isolated preview was quit afterward. Accessibility permission for Dock reads was not changed. Full parity and notarization remain separate outstanding work.

## Pipmark rebrand and Sparkle — 2026-11-25

Visible app names and release artifacts now use Pipmark, bundle ID
`club.oleary.pipmark`. Existing settings and archive imports remain compatible.
Sparkle 2.10.1 is embedded and helpers are signed inside-out with Developer ID.
46 tests and the native reference-layout check pass. A disposable bundled
`check_updater` fixture successfully starts Sparkle, verifies its action target,
and passes native menu validation. The signed production app remains running
and loads Sparkle.framework in a process smoke check. CUA launch timed out, so
no click-through update-dialog verification is claimed. A signed appcast was
generated successfully. Public feed publication, notarization and a full
older-to-newer update installation remain pending.

### Published release validation

GitHub Actions built and published normal release v0.1.0. Apple notarization
accepted it; the downloaded archive passes its checksum, Sparkle signature, deep
code signature, stapled ticket, and Gatekeeper checks. The update feed is public.
A full older-to-newer installation test and live badge validation with granted
Accessibility remain unverified.
