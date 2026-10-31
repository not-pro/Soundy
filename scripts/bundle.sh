#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
(cd assets/sounds && shasum -a 256 -c manifest.sha256 > /dev/null)
sparkle=$(sh scripts/setup-sparkle.sh)
cargo build --release --locked --offline
version=$(cargo metadata --locked --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
# Apple's bundle version fields use the numeric part of a Cargo SemVer.
bundle_version=${version%%-*}
bundle_version=${bundle_version%%+*}
bundle="dist/Soundy.app"
mkdir -p "$bundle/Contents/MacOS"
cp target/release/soundy "$bundle/Contents/MacOS/soundy.new"
mv "$bundle/Contents/MacOS/soundy.new" "$bundle/Contents/MacOS/soundy"
cp macos/Info.plist "$bundle/Contents/Info.plist"
mkdir -p "$bundle/Contents/Resources"
cp assets/icons/Soundy.icns "$bundle/Contents/Resources/Soundy.icns"
cp "$sparkle/LICENSE" "$bundle/Contents/Resources/Sparkle-LICENSE.txt"
ditto assets/sounds "$bundle/Contents/Resources/Sounds"
mkdir -p "$bundle/Contents/Frameworks"
ditto "$sparkle/Sparkle.framework" "$bundle/Contents/Frameworks/Sparkle.framework"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $bundle_version" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $bundle_version" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :SUFeedURL https://github.com/not-pro/Soundy/releases/latest/download/appcast-$(uname -m).xml" "$bundle/Contents/Info.plist"
# Give the ad-hoc signature an explicit designated requirement. The default ad-hoc
# requirement is derived from the code hash, so every release looks like a different
# app and macOS drops both the Input Monitoring approval and the login item on update.
# An identifier-based requirement is identical across builds, so the grants survive.
codesign --force --sign - --identifier dev.soundy.app \
    --requirements '=designated => identifier "dev.soundy.app"' "$bundle"
codesign --verify --deep --strict "$bundle"
printf 'Built %s/dist/Soundy.app\n' "$PWD"
