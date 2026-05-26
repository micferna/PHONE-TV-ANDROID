#!/usr/bin/env bash
# Build a portable Linux .AppImage for phone-tv.
# Requires: a fresh `cargo build --release` (binary at target/release/phone-tv).
# Downloads linuxdeploy on first run.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
ARCH="$(uname -m)"
BIN="target/release/phone-tv"
APPDIR="dist/AppDir"
OUT_DIR="dist"

if [ ! -x "$BIN" ]; then
    echo "Build the release binary first: cargo build --release" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"

install -m755 "$BIN" "$APPDIR/usr/bin/phone-tv"
install -m644 assets/phone-tv.desktop "$APPDIR/usr/share/applications/phone-tv.desktop"
# Also put .desktop at root (required by AppImage spec)
install -m644 assets/phone-tv.desktop "$APPDIR/phone-tv.desktop"

# Icon: try assets/icon.png, otherwise generate a placeholder
if [ -f assets/icon.png ]; then
    install -m644 assets/icon.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/phone-tv.png"
    install -m644 assets/icon.png "$APPDIR/phone-tv.png"
else
    # 1x1 transparent PNG placeholder (so AppImage tooling is happy)
    printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\rIDATx\x9cc\xf8\xcf\xc0\x00\x00\x00\x03\x00\x01[\xb1>D\x00\x00\x00\x00IEND\xaeB`\x82' \
        > "$APPDIR/usr/share/icons/hicolor/256x256/apps/phone-tv.png"
    cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/phone-tv.png" "$APPDIR/phone-tv.png"
fi

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/phone-tv" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Get appimagetool if missing
TOOL="$OUT_DIR/appimagetool.AppImage"
if [ ! -x "$TOOL" ]; then
    URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
    echo "Downloading $URL"
    curl -fsSL "$URL" -o "$TOOL"
    chmod +x "$TOOL"
fi

ARCH="$ARCH" "$TOOL" "$APPDIR" "$OUT_DIR/phone-tv-${VERSION}-${ARCH}.AppImage"
echo "Built: $OUT_DIR/phone-tv-${VERSION}-${ARCH}.AppImage"
