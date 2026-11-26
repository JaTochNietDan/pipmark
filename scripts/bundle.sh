#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/fetch_sparkle.py
cargo build --release
version="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
bundle="dist/Pipmark.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cargo run --quiet --release --example render_app_icon -- target/Pipmark.iconset
iconutil -c icns target/Pipmark.iconset -o "$bundle/Contents/Resources/Pipmark.icns"
cp target/release/pipmark "$bundle/Contents/MacOS/Pipmark"
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Pipmark</string>
<key>CFBundleDisplayName</key><string>Pipmark</string>
<key>CFBundleIdentifier</key><string>club.oleary.pipmark</string>
<key>CFBundleExecutable</key><string>Pipmark</string>
<key>CFBundleIconFile</key><string>Pipmark</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleLocalizations</key><array><string>en</string><string>de</string><string>ja</string><string>ko</string><string>zh-Hans</string><string>zh-Hant</string></array>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>SUFeedURL</key><string>https://github.com/JaTochNietDan/pipmark/releases/latest/download/appcast.xml</string>
<key>SUPublicEDKey</key><string>tQRge4Gw90SuaBRjufmPaMP0w2lfN7HWV5zGLCeYRfA=</string>
<key>SUAutomaticallyUpdate</key><false/>
<key>SUScheduledCheckInterval</key><integer>86400</integer>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
mkdir -p "$bundle/Contents/Frameworks"
ditto target/sparkle/Sparkle.framework "$bundle/Contents/Frameworks/Sparkle.framework"
cp target/sparkle/LICENSE "$bundle/Contents/Resources/Sparkle-LICENSE"
codesign --force --sign - "$bundle"
printf 'Built %s\n' "$bundle"
