#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
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
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $bundle_version" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $bundle_version" "$bundle/Contents/Info.plist"
codesign --force --sign - --identifier dev.soundy.app "$bundle"
codesign --verify --strict "$bundle"
printf 'Built %s/dist/Soundy.app\n' "$PWD"
