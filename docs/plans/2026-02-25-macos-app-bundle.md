# macOS .app Bundle Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a double-clickable AirJedi.app bundle that runs from Finder or /Applications.

**Architecture:** A `macos/` subdirectory contains Makefile, scripts, and templates. The Makefile drives icon generation from the existing SVG, release builds, and .app bundle assembly. One code module (`src/paths.rs`) centralizes path resolution so the app finds assets inside the bundle and writes data to proper macOS locations.

**Tech Stack:** Makefile, bash scripts, rsvg-convert (brew), iconutil (macOS built-in), Bevy AssetPlugin configuration

---

### Task 1: Create macos/ directory structure

**Files:**
- Create: `macos/scripts/build-app.sh` (empty placeholder)
- Create: `macos/Makefile`

**Step 1: Create directory structure**

```bash
mkdir -p macos/scripts macos/icons
```

**Step 2: Create minimal Makefile skeleton**

Create `macos/Makefile`:

```makefile
# AirJedi macOS Application Bundle
# Usage: make app

APP_NAME := AirJedi
BUNDLE := build/$(APP_NAME).app
BINARY_NAME := airjedi_bevy
CARGO_TARGET := ../target/release/$(BINARY_NAME)
SVG_ICON := ../assets/airplane1.svg
ICONSET := icons/AppIcon.iconset
ICNS := icons/AppIcon.icns

.PHONY: app icons clean

app: icons
	@echo "Building $(APP_NAME).app..."
	./scripts/build-app.sh

icons: $(ICNS)

$(ICNS): $(SVG_ICON)
	@echo "Generating icons from SVG..."
	./scripts/gen-icons.sh

clean:
	rm -rf build
	rm -rf $(ICONSET)
	rm -f $(ICNS)
```

**Step 3: Commit**

```bash
git add macos/
git commit -m "Add macos/ directory with Makefile skeleton"
```

---

### Task 2: Icon generation script

**Files:**
- Create: `macos/scripts/gen-icons.sh`

**Step 1: Create the icon generation script**

Create `macos/scripts/gen-icons.sh`:

```bash
#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MACOS_DIR="$(dirname "$SCRIPT_DIR")"
SVG="$MACOS_DIR/../assets/airplane1.svg"
ICONSET="$MACOS_DIR/icons/AppIcon.iconset"
ICNS="$MACOS_DIR/icons/AppIcon.icns"

# Check for rsvg-convert
if ! command -v rsvg-convert &>/dev/null; then
    echo "Error: rsvg-convert not found. Install with: brew install librsvg"
    exit 1
fi

mkdir -p "$ICONSET"

# Generate all required icon sizes
# macOS requires these specific sizes and naming conventions
declare -a SIZES=(16 32 128 256 512)

for size in "${SIZES[@]}"; do
    double=$((size * 2))
    echo "  ${size}x${size} and ${size}x${size}@2x"
    rsvg-convert -w "$size" -h "$size" "$SVG" -o "$ICONSET/icon_${size}x${size}.png"
    rsvg-convert -w "$double" -h "$double" "$SVG" -o "$ICONSET/icon_${size}x${size}@2x.png"
done

echo "Converting iconset to icns..."
iconutil --convert icns --output "$ICNS" "$ICONSET"

echo "Icon generated: $ICNS"
```

**Step 2: Make it executable and test**

```bash
chmod +x macos/scripts/gen-icons.sh
cd macos && ./scripts/gen-icons.sh
```

Expected: `macos/icons/AppIcon.icns` is created. Verify with:
```bash
file macos/icons/AppIcon.icns
```
Expected output contains: `Mac OS X icon`

**Step 3: Add .gitignore for generated icons**

Create `macos/icons/.gitignore`:
```
AppIcon.iconset/
AppIcon.icns
```

**Step 4: Commit**

```bash
git add macos/scripts/gen-icons.sh macos/icons/.gitignore
git commit -m "Add icon generation script from SVG"
```

---

### Task 3: Info.plist template

**Files:**
- Create: `macos/Info.plist.template`

**Step 1: Create the Info.plist template**

Create `macos/Info.plist.template`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>AirJedi</string>
    <key>CFBundleDisplayName</key>
    <string>AirJedi</string>
    <key>CFBundleIdentifier</key>
    <string>com.airjedi.app</string>
    <key>CFBundleVersion</key>
    <string>__VERSION__</string>
    <key>CFBundleShortVersionString</key>
    <string>__VERSION__</string>
    <key>CFBundleExecutable</key>
    <string>airjedi_bevy</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
```

**Step 2: Commit**

```bash
git add macos/Info.plist.template
git commit -m "Add Info.plist template for .app bundle"
```

---

### Task 4: Bundle-aware path resolution module

**Files:**
- Create: `src/paths.rs`
- Modify: `src/main.rs` (add `mod paths;`)
- Modify: `src/tile_cache.rs:213-216` (use `paths::assets_dir()`)
- Modify: `src/config.rs:198-202` (use `paths::config_dir()`)

This is the key code change. It centralizes path resolution so the app works both in development (`cargo run`) and from a `.app` bundle.

**Step 1: Write tests for path resolution**

Create `src/paths.rs` with tests first:

```rust
/// Centralized path resolution for development and .app bundle contexts.
///
/// In development (cargo run): paths resolve relative to current_dir().
/// In a .app bundle: assets resolve relative to the executable (Contents/MacOS/),
/// and writable data uses platform directories via the `dirs` crate.

use std::path::PathBuf;

/// Returns true if the executable is running inside a macOS .app bundle.
pub fn is_app_bundle() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        // In a bundle, the exe is at Something.app/Contents/MacOS/binary
        exe.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| s.ends_with(".app"))
                .unwrap_or(false)
        })
    } else {
        false
    }
}

/// Returns the base directory for Bevy assets.
///
/// - Bundle: `Contents/MacOS/` (parent of executable — Bevy resolves
///   its `assets/` folder relative to this)
/// - Development: current working directory
pub fn assets_base_dir() -> PathBuf {
    if is_app_bundle() {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Returns the path to the assets/tiles directory (or where the symlink should go).
///
/// - Bundle: `Contents/MacOS/assets/tiles`
/// - Development: `<cwd>/assets/tiles`
pub fn assets_tiles_dir() -> PathBuf {
    assets_base_dir().join("assets").join("tiles")
}

/// Returns the directory for configuration files.
///
/// - Bundle: `~/Library/Application Support/airjedi/`
/// - Development: current working directory
pub fn config_dir() -> PathBuf {
    if is_app_bundle() {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("airjedi")
    } else {
        std::env::current_dir().unwrap_or_default()
    }
}

/// Returns the directory for temporary/transient files (logs, recordings).
///
/// - Bundle: `~/Library/Application Support/airjedi/tmp/`
/// - Development: `<cwd>/tmp/`
pub fn tmp_dir() -> PathBuf {
    if is_app_bundle() {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("airjedi")
            .join("tmp")
    } else {
        std::env::current_dir()
            .map(|p| p.join("tmp"))
            .unwrap_or_else(|_| PathBuf::from("tmp"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_app_bundle_returns_false_in_tests() {
        // Test binaries are in target/debug/deps/, not inside a .app
        assert!(!is_app_bundle());
    }

    #[test]
    fn assets_base_dir_returns_cwd_in_dev() {
        // In non-bundle context, should return current dir
        let base = assets_base_dir();
        assert!(base.exists());
    }

    #[test]
    fn assets_tiles_dir_ends_with_expected_path() {
        let tiles = assets_tiles_dir();
        assert!(tiles.ends_with("assets/tiles"));
    }

    #[test]
    fn config_dir_returns_cwd_in_dev() {
        // In non-bundle context, should return current dir
        let cfg = config_dir();
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(cfg, cwd);
    }

    #[test]
    fn tmp_dir_ends_with_tmp() {
        let tmp = tmp_dir();
        assert!(tmp.ends_with("tmp"));
    }
}
```

**Step 2: Run the tests**

```bash
cargo test paths -- --nocapture
```

Expected: All 5 tests pass.

**Step 3: Register the module in main.rs**

Add `mod paths;` to `src/main.rs` after the existing module declarations (after line 30, `mod statusbar;`):

```rust
mod paths;
```

**Step 4: Run the tests again to confirm module registration**

```bash
cargo test paths -- --nocapture
```

Expected: All 5 tests still pass.

**Step 5: Commit**

```bash
git add src/paths.rs src/main.rs
git commit -m "Add bundle-aware path resolution module"
```

---

### Task 5: Update tile_cache.rs to use paths module

**Files:**
- Modify: `src/tile_cache.rs:213-217` (replace `assets_tiles_path()`)

**Step 1: Replace `assets_tiles_path()` with `paths::assets_tiles_dir()`**

In `src/tile_cache.rs`, change the `assets_tiles_path()` function (lines 213-217):

From:
```rust
fn assets_tiles_path() -> PathBuf {
    std::env::current_dir()
        .map(|path| path.join("assets").join("tiles"))
        .unwrap_or_else(|_| PathBuf::from("assets/tiles"))
}
```

To:
```rust
fn assets_tiles_path() -> PathBuf {
    crate::paths::assets_tiles_dir()
}
```

Also update `clear_legacy_tiles()` (lines 118-120):

From:
```rust
    let assets_path = std::env::current_dir()
        .map(|path| path.join("assets"))
        .unwrap_or_else(|_| PathBuf::from("assets"));
```

To:
```rust
    let assets_path = crate::paths::assets_base_dir().join("assets");
```

**Step 2: Run existing tests**

```bash
cargo test
```

Expected: All tests pass. No behavior change in development mode.

**Step 3: Commit**

```bash
git add src/tile_cache.rs
git commit -m "Use paths module for tile cache directory resolution"
```

---

### Task 6: Update config.rs to use paths module

**Files:**
- Modify: `src/config.rs:198-202`

**Step 1: Update config_path()**

In `src/config.rs`, change `config_path()` (lines 198-202):

From:
```rust
fn config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join(CONFIG_FILE)
}
```

To:
```rust
fn config_path() -> PathBuf {
    let dir = crate::paths::config_dir();
    // Ensure the config directory exists (relevant for bundle mode
    // where config_dir is ~/Library/Application Support/airjedi/)
    let _ = std::fs::create_dir_all(&dir);
    dir.join(CONFIG_FILE)
}
```

**Step 2: Run tests**

```bash
cargo test
```

Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/config.rs
git commit -m "Use paths module for config file resolution"
```

---

### Task 7: Update remaining current_dir() usages

**Files:**
- Modify: `src/main.rs:267-270, 283-289` (debug log paths)
- Modify: `src/recording/recorder.rs:84-86` (recording tmp dir)
- Modify: `src/export/mod.rs:396-398` (recording list tmp dir)
- Modify: `src/tools_window.rs:480-481` (recording UI tmp dir)

**Step 1: Update main.rs heartbeat log path (line 267)**

From:
```rust
    let log_path = std::env::current_dir()
        .ok()
        .map(|p| p.join("tmp/heartbeat.log"))
        .unwrap_or_else(|| std::path::PathBuf::from("tmp/heartbeat.log"));
```

To:
```rust
    let log_path = crate::paths::tmp_dir().join("heartbeat.log");
```

**Step 2: Update main.rs debug logger path (line 283)**

From:
```rust
    let log_path = std::env::current_dir()
        .ok()
        .map(|path| {
            let tmp_dir = path.join("tmp");
            // Ensure tmp directory exists
            let _ = std::fs::create_dir_all(&tmp_dir);
            tmp_dir.join("zoom_debug.log")
```

To:
```rust
    let log_path = {
            let tmp_dir = crate::paths::tmp_dir();
            let _ = std::fs::create_dir_all(&tmp_dir);
            tmp_dir.join("zoom_debug.log")
```

Note: Read the full function to get exact replacement boundaries. The structure around this may include closures.

**Step 3: Update recording/recorder.rs (line 84)**

From:
```rust
        let tmp_dir = std::env::current_dir()
            .map(|p| p.join("tmp"))
            .unwrap_or_else(|_| PathBuf::from("tmp"));
```

To:
```rust
        let tmp_dir = crate::paths::tmp_dir();
```

**Step 4: Update export/mod.rs (line 396)**

From:
```rust
    let tmp_dir = std::env::current_dir()
        .map(|p| p.join("tmp"))
        .unwrap_or_else(|_| PathBuf::from("tmp"));
```

To:
```rust
    let tmp_dir = crate::paths::tmp_dir();
```

**Step 5: Update tools_window.rs (line 480)**

From:
```rust
            if let Ok(cwd) = std::env::current_dir() {
                let tmp_dir = cwd.join("tmp");
```

To:
```rust
            {
                let tmp_dir = crate::paths::tmp_dir();
```

Note: The closing brace for the `if let Ok(cwd)` block also needs adjustment. Read the full block to confirm scope.

**Step 6: Verify no remaining current_dir() usages outside of paths.rs**

```bash
grep -rn 'current_dir' src/ --include='*.rs' | grep -v 'paths.rs'
```

Expected: No results.

**Step 7: Run all tests**

```bash
cargo test
```

Expected: All tests pass.

**Step 8: Run the app to verify it still works**

```bash
cargo run
```

Expected: App launches, tiles load, config loads normally.

**Step 9: Commit**

```bash
git add src/main.rs src/recording/recorder.rs src/export/mod.rs src/tools_window.rs
git commit -m "Replace all current_dir() with paths module"
```

---

### Task 8: Build script for .app bundle assembly

**Files:**
- Create: `macos/scripts/build-app.sh`

**Step 1: Create the build script**

Create `macos/scripts/build-app.sh`:

```bash
#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MACOS_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$MACOS_DIR")"

APP_NAME="AirJedi"
BUNDLE="$MACOS_DIR/build/${APP_NAME}.app"
BINARY_NAME="airjedi_bevy"
ICNS="$MACOS_DIR/icons/AppIcon.icns"

# Extract version from Cargo.toml
VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Building $APP_NAME v$VERSION"

# Build release binary
echo "==> Building release binary..."
(cd "$PROJECT_DIR" && cargo build --release)

# Verify binary exists
BINARY="$PROJECT_DIR/target/release/$BINARY_NAME"
if [ ! -f "$BINARY" ]; then
    echo "Error: Release binary not found at $BINARY"
    exit 1
fi

# Verify icon exists
if [ ! -f "$ICNS" ]; then
    echo "Error: AppIcon.icns not found. Run 'make icons' first."
    exit 1
fi

# Clean previous build
rm -rf "$BUNDLE"

# Create bundle structure
echo "==> Creating app bundle..."
mkdir -p "$BUNDLE/Contents/MacOS"
mkdir -p "$BUNDLE/Contents/Resources"

# Copy binary
cp "$BINARY" "$BUNDLE/Contents/MacOS/$BINARY_NAME"

# Copy assets (excluding tiles symlink/cache and .tile. files)
echo "==> Copying assets..."
mkdir -p "$BUNDLE/Contents/MacOS/assets"

# Copy non-tile assets
rsync -a \
    --exclude='tiles' \
    --exclude='*.tile.*' \
    "$PROJECT_DIR/assets/" "$BUNDLE/Contents/MacOS/assets/"

# Generate Info.plist from template
echo "==> Generating Info.plist..."
sed "s/__VERSION__/$VERSION/g" \
    "$MACOS_DIR/Info.plist.template" > "$BUNDLE/Contents/Info.plist"

# Copy icon
cp "$ICNS" "$BUNDLE/Contents/Resources/AppIcon.icns"

# Show result
echo ""
echo "==> Built: $BUNDLE"
du -sh "$BUNDLE"
echo ""
echo "Run with: open $BUNDLE"
```

**Step 2: Make executable**

```bash
chmod +x macos/scripts/build-app.sh
```

**Step 3: Commit**

```bash
git add macos/scripts/build-app.sh
git commit -m "Add .app bundle assembly script"
```

---

### Task 9: Test the full build pipeline

**Step 1: Generate icons**

```bash
cd macos && make icons
```

Expected: `macos/icons/AppIcon.icns` is created.

**Step 2: Build the .app bundle**

```bash
cd macos && make app
```

Expected: `macos/build/AirJedi.app` is created. Output shows the bundle size.

**Step 3: Verify bundle structure**

```bash
ls -la macos/build/AirJedi.app/Contents/
ls -la macos/build/AirJedi.app/Contents/MacOS/
ls -la macos/build/AirJedi.app/Contents/Resources/
ls -la macos/build/AirJedi.app/Contents/MacOS/assets/
```

Expected:
- `Contents/Info.plist` exists
- `Contents/MacOS/airjedi_bevy` exists and is executable
- `Contents/Resources/AppIcon.icns` exists
- `Contents/MacOS/assets/` contains fonts/, models/, airplane.glb, airplane1.png, airplane1.svg

**Step 4: Verify Info.plist has correct version**

```bash
plutil -p macos/build/AirJedi.app/Contents/Info.plist
```

Expected: CFBundleVersion and CFBundleShortVersionString are "0.1.0".

**Step 5: Launch the app from Finder**

```bash
open macos/build/AirJedi.app
```

Expected: App launches, map tiles load (from ~/Library/Caches/airjedi/tiles), aircraft appear.

**Step 6: Verify tile cache works in bundle mode**

Check that the tile cache symlink is created inside the bundle's assets dir:

```bash
ls -la macos/build/AirJedi.app/Contents/MacOS/assets/tiles
```

Expected: Symlink pointing to ~/Library/Caches/airjedi/tiles.

**Step 7: If any issues, fix and re-test before committing**

**Step 8: Add build output to .gitignore**

Add to root `.gitignore` (or create `macos/.gitignore`):

```
macos/build/
```

**Step 9: Commit**

```bash
git add macos/.gitignore
git commit -m "Add .gitignore for macOS build output"
```

---

### Task 10: Update CLAUDE.md with macOS build instructions

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Add macOS build section to CLAUDE.md**

Add after the "Build and Run Commands" section:

```markdown
## macOS Application Bundle

```bash
# Generate app icon from SVG (requires: brew install librsvg)
cd macos && make icons

# Build AirJedi.app bundle (release mode)
cd macos && make app

# Launch the built app
open macos/build/AirJedi.app

# Clean build artifacts
cd macos && make clean
```

The `macos/` directory contains all macOS-specific build files. Assets are copied into `Contents/MacOS/assets/` inside the bundle. Tile cache uses `~/Library/Caches/airjedi/tiles/` in both development and bundle modes.
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "Add macOS build instructions to CLAUDE.md"
```
