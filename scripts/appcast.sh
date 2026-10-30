#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
: "${SPARKLE_PRIVATE_KEY:?SPARKLE_PRIVATE_KEY must contain the private update signing key}"
sdk=$(sh scripts/setup-sparkle.sh)
version=$(cargo metadata --locked --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
arch=$(uname -m)
repository=${GITHUB_REPOSITORY:-not-pro/Soundy}
archive="Soundy-v${version}-macos-${arch}"
staging=$(mktemp -d "${TMPDIR:-/tmp}/soundy-appcast.XXXXXX")
trap 'rm -rf "$staging"' EXIT HUP INT TERM
cp "dist/$archive.zip" "$staging/$archive.zip"
cp .github/release-notes.md "$staging/$archive.md"
printf '%s' "$SPARKLE_PRIVATE_KEY" | "$sdk/bin/generate_appcast" \
    --ed-key-file - --maximum-deltas 0 --embed-release-notes \
    --download-url-prefix "https://github.com/$repository/releases/download/v$version/" \
    -o "$PWD/dist/appcast-$arch.xml" "$staging"
# Verify both the generated feed and archive with Sparkle's own signing tool.
printf '%s' "$SPARKLE_PRIVATE_KEY" | "$sdk/bin/sign_update" \
    --ed-key-file - --verify "dist/appcast-$arch.xml"
signature=$(python3 -c 'import sys,xml.etree.ElementTree as E; print(E.parse(sys.argv[1]).find("./channel/item/enclosure").get("{http://www.andymatuschak.org/xml-namespaces/sparkle}edSignature"))' "dist/appcast-$arch.xml")
printf '%s' "$SPARKLE_PRIVATE_KEY" | "$sdk/bin/sign_update" \
    --ed-key-file - --verify "dist/$archive.zip" "$signature"
