#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked --offline
bundle="dist/Soundy.app"
mkdir -p "$bundle/Contents/MacOS"
cp target/release/soundy "$bundle/Contents/MacOS/soundy.new"
mv "$bundle/Contents/MacOS/soundy.new" "$bundle/Contents/MacOS/soundy"
cp macos/Info.plist "$bundle/Contents/Info.plist"
codesign --force --sign - --identifier dev.soundy.app "$bundle"
codesign --verify --strict "$bundle"
printf 'Built %s/dist/Soundy.app\n' "$PWD"
