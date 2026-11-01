#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

host=$(rustc -vV | sed -n 's/^host: //p')
target=${TARGET:-$host}
case "$target" in
    *windows*) ;;
    *)
        printf 'package-windows.sh needs a Windows target, not %s\n' "$target" >&2
        exit 1
        ;;
esac
case "$target" in
    *aarch64*) arch=arm64 ;;
    *) arch=x64 ;;
esac

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
python=${PYTHON:-$(command -v python3 || command -v python)}
[ -n "$python" ] || { echo "package-windows.sh needs python3 to write the ZIP" >&2; exit 1; }

cargo build --release --locked --target "$target"

archive="Soundy-v${version}-windows-${arch}"
bundle="dist/$archive/Soundy"
rm -rf "dist/$archive"
mkdir -p "$bundle"
cp "target/$target/release/soundy.exe" "$bundle/Soundy.exe"
cp assets/icons/Soundy.ico "$bundle/Soundy.ico"
cp README.md "$bundle/README.md"
# Only a native build can be executed here; cross builds are checked in CI.
if [ "$target" = "$host" ]; then
    "$bundle/Soundy.exe" --check
fi

"$python" - "dist/$archive.zip" "$bundle" <<'PY'
import pathlib, sys, zipfile

target, source = map(pathlib.Path, sys.argv[1:3])
with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as zip:
    for path in sorted(source.rglob("*")):
        if path.is_file():
            zip.write(path, path.relative_to(source.parent))
PY

cd dist
if command -v sha256sum >/dev/null 2>&1; then
    hash=$(sha256sum "$archive.zip" | cut -d' ' -f1)
else
    hash=$(shasum -a 256 "$archive.zip" | cut -d' ' -f1)
fi
printf '%s  %s\n' "$hash" "$archive.zip" > "$archive.zip.sha256"
printf 'Packaged %s.zip\n' "$archive"
