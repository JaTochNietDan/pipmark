# Pipmark updates

Pipmark embeds Sparkle 2.10.1 (SHA-256 pinned in `scripts/fetch_sparkle.py`).
The Rust app loads its bundled framework on the main thread, retains the standard
updater controller, and lets Sparkle own scheduling, consent and update UI.
“Check for Updates…” targets Sparkle directly, including its menu validation.
Unbundled development/test binaries do not start the updater.

The feed is `https://github.com/JaTochNietDan/pipmark/releases/latest/download/appcast.xml`.
The endpoint is live and was verified against the published 0.1.0 archive.
Automatic checks use Sparkle's consent flow and a daily interval. Automatic
installation is disabled by default; users review available updates.

The local Ed25519 private key is kept in macOS Keychain under account
`club.oleary.pipmark`; the public key is embedded in Info.plist. Never commit the
private key. GitHub Actions signing credentials are provisioned as encrypted repository
secrets. The CI job uses a disposable keychain and removes it after signing.
Local signing uses the existing Keychain.

Release order:

1. Increment Cargo.toml version (also CFBundleVersion, used for update ordering).
2. `mise run package-signed` signs Sparkle helpers inside-out and the outer app.
3. Notarize the ZIP, staple the app, and repackage the stapled app.
4. `python3 scripts/appcast.py dist/pipmark-VERSION-arm64-developer-id.zip --download-prefix https://github.com/JaTochNietDan/pipmark/releases/download/vVERSION/`
5. Upload the final ZIP and appcast.xml to the same normal GitHub release.

Generate the feed only after final packaging: changing the archive invalidates
its Ed25519 signature. Every normal release must include appcast.xml because the
application uses GitHub's latest-release URL. Pre-releases do not advance it.
An actual older-to-newer installation test is still required before publishing.

Initial 0.1.0 notarization was accepted by Apple on 2026-11-25. The ticket was
stapled and Gatekeeper reports Notarized Developer ID.

GitHub Release run 45742615962 passed and published v0.1.0 as a normal release.
The public download passed SHA-256, Sparkle Ed25519, deep codesign, stapler, and
Gatekeeper verification. The website runs on Cloudflare Workers; GitHub hosts
source, build workflows, release archives, and the update feed.
