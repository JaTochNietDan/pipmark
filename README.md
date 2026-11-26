# Pipmark

**Your app badges, at a glance.**

[Website](https://pipmark.io) · [Download DMG](https://pipmark.io/download)

Pipmark brings macOS app notification badges into the menu bar. Choose your apps,
group them together, and customize their icons, counts, and visibility—all in a
compact native interface built with Rust and AppKit.

- Real application icons and simple settings.
- Nested groups, custom SVG icons, and optional attention effects.
- No browser runtime or rendering loop; unchanged badges aren't redrawn.
- Native Sparkle update checks and signed update archives.

Requires macOS 13 or later; current builds support Apple silicon. Reading Dock
badges requires Accessibility access. Live badge monitoring still needs validation
with that permission; see [validation and remaining gaps](docs/PARITY.md).

## Build

```sh
mise run test
mise run build
open dist/Pipmark.app
```

Use `mise run dmg` for a Developer ID–signed disk image. Sparkle's framework
is downloaded at a pinned version with SHA-256 verification. See
[update and release setup](docs/UPDATES.md) for notarization and feed generation.
Tagged releases build, sign, notarize, and publish automatically on GitHub.
The Sparkle feed is live alongside the latest normal release.

Settings are stored in `~/Library/Application Support/Pipmark/settings.json`.
Existing Badgeify Native settings are reused if present. Set `PIPMARK_CONFIG` for
an isolated configuration. Import/export includes custom icons.

Pipmark is an independent project, unaffiliated with Badgeify.
