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
# Sign with a committed self-signed identity instead of ad-hoc ("-"). Ad-hoc signing
# derives its designated requirement from the code hash, so every update looks like a
# different app to macOS and drops both Input Monitoring approval and the login item.
# A stable certificate keeps the designated requirement identical across updates.
codesign_password=soundy
keychain_dir=$(mktemp -d "${TMPDIR:-/tmp}/soundy-sign.XXXXXX")
keychain="$keychain_dir/signing.keychain-db"
cleanup() { security delete-keychain "$keychain" > /dev/null 2>&1 || true; }
trap cleanup EXIT HUP INT TERM
security create-keychain -p "" "$keychain"
security unlock-keychain -p "" "$keychain"
security import certificates/soundy-codesign.p12 -k "$keychain" -P "$codesign_password" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "" "$keychain" > /dev/null
codesign --keychain "$keychain" --force --sign "Soundy Code Signing" --identifier dev.soundy.app "$bundle"
codesign --verify --deep --strict "$bundle"
printf 'Built %s/dist/Soundy.app\n' "$PWD"
