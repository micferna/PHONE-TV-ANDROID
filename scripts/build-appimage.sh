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

# Get appimagetool if missing.
#
# Pinned to a tagged release with a recorded digest, not to `continuous`: this tool
# runs in the release workflow and its output is what users download, so an
# unverified binary fetched from a moving tag would put whoever can influence that
# tag inside our supply chain. Bump both the version and the digest together.
APPIMAGETOOL_VERSION="1.9.1"
case "$ARCH" in
    x86_64)  APPIMAGETOOL_SHA256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0" ;;
    aarch64) APPIMAGETOOL_SHA256="f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158" ;;
    armhf)   APPIMAGETOOL_SHA256="42b61cba5495d8aaf418a5c9a015a49b85ad92efabcbd3c341f1540440e4e23d" ;;
    i686)    APPIMAGETOOL_SHA256="7ad9ff47c203aae0149b18f6df9e3018b2e2f470ea644a0413e3ded39e9e3bdb" ;;
    *) echo "No pinned appimagetool digest for arch '$ARCH'" >&2; exit 1 ;;
esac

TOOL="$OUT_DIR/appimagetool-${APPIMAGETOOL_VERSION}-${ARCH}.AppImage"
if [ ! -x "$TOOL" ]; then
    URL="https://github.com/AppImage/appimagetool/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-${ARCH}.AppImage"
    echo "Downloading $URL"
    curl -fsSL "$URL" -o "$TOOL.part"
    if ! echo "${APPIMAGETOOL_SHA256}  $TOOL.part" | sha256sum -c - >/dev/null 2>&1; then
        echo "appimagetool checksum mismatch — refusing to run it" >&2
        echo "  expected: $APPIMAGETOOL_SHA256" >&2
        echo "  got:      $(sha256sum < "$TOOL.part" | cut -d' ' -f1)" >&2
        rm -f "$TOOL.part"
        exit 1
    fi
    mv "$TOOL.part" "$TOOL"
    chmod +x "$TOOL"
fi

ARCH="$ARCH" "$TOOL" "$APPDIR" "$OUT_DIR/phone-tv-${VERSION}-${ARCH}.AppImage"
echo "Built: $OUT_DIR/phone-tv-${VERSION}-${ARCH}.AppImage"
