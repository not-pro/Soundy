#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
sh scripts/bundle.sh
version=$(cargo metadata --locked --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
arch=$(uname -m)
bundle="dist/Soundy.app"
lipo "$bundle/Contents/MacOS/soundy" -verify_arch "$arch"
"$bundle/Contents/MacOS/soundy" --check

staging=$(mktemp -d "${TMPDIR:-/tmp}/soundy-installer.XXXXXX")
trap 'rm -rf "$staging"' EXIT HUP INT TERM
ditto "$bundle" "$staging/Soundy.app"
ln -s /Applications "$staging/Applications"
hdiutil create -volname Soundy -srcfolder "$staging" -fs HFS+ -format UDZO -ov \
    "dist/Soundy-v${version}-macos-${arch}.dmg"
hdiutil verify "dist/Soundy-v${version}-macos-${arch}.dmg"

cd dist
archive="Soundy-v${version}-macos-${arch}"
ditto -c -k --sequesterRsrc --keepParent Soundy.app "$archive.zip"
shasum -a 256 "$archive.dmg" > "$archive.dmg.sha256"
shasum -a 256 "$archive.zip" > "$archive.zip.sha256"
printf 'Packaged %s.dmg and %s.zip\n' "$archive" "$archive"
