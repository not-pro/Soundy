#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
version=2.10.1
checksum=1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27
sdk="$PWD/target/sparkle-$version"
if [ ! -x "$sdk/bin/generate_appcast" ]; then
    mkdir -p target
    archive="$PWD/target/Sparkle-$version.tar.xz"
    curl --fail --location --silent --show-error --retry 2 \
        "https://github.com/sparkle-project/Sparkle/releases/download/$version/Sparkle-$version.tar.xz" \
        -o "$archive"
    printf '%s  %s\n' "$checksum" "$archive" | shasum -a 256 -c - > /dev/null
    mkdir -p "$sdk"
    tar -xf "$archive" -C "$sdk"
fi
printf '%s\n' "$sdk"
