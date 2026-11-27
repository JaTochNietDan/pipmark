# Developing Pipmark

Pipmark is built with Rust and AppKit through objc2. It has no browser runtime
or rendering loop; unchanged badges are not redrawn. Development tooling is
pinned in `mise.toml`.

## Build and run

```sh
mise run test
mise run build
open dist/Pipmark.app
```

Use `mise run dmg` for a Developer ID–signed disk image. Sparkle's framework is
downloaded at a pinned version with SHA-256 verification. See
[releases and updates](UPDATES.md) for signing credentials, notarization, and
feed generation. Tagged releases build, sign, notarize, and publish on GitHub.

## Settings

Settings are stored in `~/Library/Application Support/Pipmark/settings.json`.
Existing Badgeify Native settings are reused if present. Set `PIPMARK_CONFIG` to
an absolute path for an isolated configuration. Import/export includes custom
icons.

## Validation

See the [capability and validation ledger](PARITY.md) for native UI checks,
performance measurements, and remaining gaps. Live Dock badge monitoring needs
validation with Accessibility access granted. Measurements taken without that
permission do not establish the cost of successful badge reads.
