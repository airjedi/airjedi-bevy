#!/usr/bin/env bash
# Generate macOS .icns icon from airplane1.svg
# Requires: librsvg (brew install librsvg), iconutil (built into macOS)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MACOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="$(cd "$MACOS_DIR/.." && pwd)"

SVG_SOURCE="$ROOT_DIR/assets/airplane1.svg"
ICONSET_DIR="$MACOS_DIR/icons/AppIcon.iconset"
ICNS_OUTPUT="$MACOS_DIR/icons/AppIcon.icns"

# Check dependencies
if ! command -v rsvg-convert &>/dev/null; then
    echo "Error: rsvg-convert not found. Install with: brew install librsvg"
    exit 1
fi

if ! command -v iconutil &>/dev/null; then
    echo "Error: iconutil not found. This should be built into macOS."
    exit 1
fi

if [ ! -f "$SVG_SOURCE" ]; then
    echo "Error: SVG source not found at $SVG_SOURCE"
    exit 1
fi

# Create iconset directory
mkdir -p "$ICONSET_DIR"

# Generate icons at required sizes
# Standard sizes: 16, 32, 128, 256, 512
# @2x variants: 32(16@2x), 64(32@2x), 256(128@2x), 512(256@2x), 1024(512@2x)
declare -a SIZES=(
    "16:icon_16x16.png"
    "32:icon_16x16@2x.png"
    "32:icon_32x32.png"
    "64:icon_32x32@2x.png"
    "128:icon_128x128.png"
    "256:icon_128x128@2x.png"
    "256:icon_256x256.png"
    "512:icon_256x256@2x.png"
    "512:icon_512x512.png"
    "1024:icon_512x512@2x.png"
)

echo "Generating icon PNGs from $SVG_SOURCE..."

for entry in "${SIZES[@]}"; do
    size="${entry%%:*}"
    filename="${entry#*:}"
    echo "  ${size}x${size} -> $filename"
    rsvg-convert -w "$size" -h "$size" "$SVG_SOURCE" -o "$ICONSET_DIR/$filename"
done

# Convert iconset to icns
echo "Converting iconset to .icns..."
iconutil --convert icns --output "$ICNS_OUTPUT" "$ICONSET_DIR"

echo "Icon generated: $ICNS_OUTPUT"
