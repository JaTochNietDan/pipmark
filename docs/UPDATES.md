# Pipmark updates and releases

Pipmark embeds Sparkle 2.10.1, downloaded with a pinned SHA-256. The Rust app
loads its bundled framework on the main thread and retains Sparkle's standard
updater controller. Sparkle owns scheduling, consent, native update UI, and menu
validation; the app adds no updater polling loop. Unbundled development binaries
do not start the updater.

Automatic checks use Sparkle's consent flow and a daily interval. Automatic
installation is disabled by default. “Check for Updates…” is available in the
app and menu-bar menus.

The live feed is:
`https://github.com/JaTochNietDan/pipmark/releases/latest/download/appcast.xml`.
Every normal release must include this asset. Pre-releases do not advance it.

## Release cycle

1. Increment Cargo.toml's version. This also sets CFBundleVersion for update ordering.
2. Push a matching `vVERSION` tag.
3. GitHub tests and builds the app, signs Sparkle helpers inside-out, and notarizes
   the app. It staples Apple's ticket before packaging.
4. The job creates a DMG containing Pipmark and an Applications shortcut, signs
   and notarizes the DMG, and staples its ticket.
5. It generates the final DMG checksum and Sparkle Ed25519 signature, then
   publishes a normal GitHub release. The ZIP remains a secondary download.

Locally, `mise run dmg` creates a signed disk image. It does not notarize it.
`scripts/release.py` performs the complete packaging/notarization flow using
`PIPMARK_NOTARY_PROFILE` and optional `PIPMARK_NOTARY_KEYCHAIN`.

The local Ed25519 key is stored in macOS Keychain under account
`club.oleary.pipmark`. Only its public key is embedded in the app. GitHub Actions
credentials are encrypted repository secrets; CI uses and deletes a disposable
keychain. Never commit signing credentials.

Generate the feed only after final packaging/stapling. Any archive change
invalidates its Sparkle signature. `scripts/appcast.py` signs the finalized DMG,
using the Keychain locally or `PIPMARK_SPARKLE_KEY_FILE` in CI.

## Verification

The first normal release passed GitHub Actions and Apple notarization. The
published DMG passed SHA-256, Sparkle Ed25519, stapled-ticket and Gatekeeper
checks. A read-only mount verified the enclosed signed app and Applications
shortcut. A native fixture verified updater startup, action targeting and menu
validation. A full older-to-newer installation test remains unverified.

## Website

Cloudflare Worker `pipmark-site` serves the site. Terraform in the ops repository
manages pipmark.io and www.pipmark.io. `/download` selects the latest release's
Apple-silicon DMG, falling back to the release page if GitHub's API is unavailable.
GitHub hosts source, builds, releases, and the update feed; no GitHub Pages is used.
