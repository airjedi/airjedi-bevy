# Phase 1: macOS .app Bundle

## Goal

Create a double-clickable `AirJedi.app` that can run from Finder or be dragged to `/Applications`.

## Project Structure

All macOS-specific build files live in a `macos/` subdirectory:

```
macos/
├── Makefile
├── Info.plist.template
├── icons/
│   └── AppIcon.iconset/     # Generated from airplane1.svg
└── scripts/
    └── build-app.sh
```

## Icon Pipeline

1. Use `rsvg-convert` (brew: `librsvg`) to render `assets/airplane1.svg` to PNGs at required sizes: 16, 32, 128, 256, 512, 1024 (plus @2x variants for 16, 32, 128, 256, 512).
2. Place PNGs in `macos/icons/AppIcon.iconset/` with Apple's naming convention (`icon_16x16.png`, `icon_16x16@2x.png`, etc.).
3. Use `iconutil --convert icns` to produce `AppIcon.icns`.
4. Makefile target: `make icons`.

## Info.plist

Template at `macos/Info.plist.template` with sed-substituted variables:

- `CFBundleName`: AirJedi
- `CFBundleIdentifier`: com.airjedi.app
- `CFBundleVersion`: extracted from `Cargo.toml` version field
- `CFBundleShortVersionString`: same as above
- `CFBundleExecutable`: airjedi_bevy
- `CFBundleIconFile`: AppIcon
- `NSHighResolutionCapable`: true
- `LSMinimumSystemVersion`: 13.0

## Bundle Assembly (build-app.sh)

1. Run `cargo build --release`.
2. Create `build/AirJedi.app/Contents/MacOS/`.
3. Create `build/AirJedi.app/Contents/Resources/`.
4. Copy release binary from `target/release/airjedi_bevy` to `Contents/MacOS/`.
5. Copy assets (fonts, models, images, tiles directory structure) to `Contents/Resources/assets/`.
6. Generate `Info.plist` from template into `Contents/`.
7. Copy `AppIcon.icns` to `Contents/Resources/`.

Output: `macos/build/AirJedi.app`

## Asset Path Code Change

Bevy resolves assets relative to the working directory by default. A small code change is needed so the app finds assets inside the bundle:

- On startup, check if the executable is inside a `.app` bundle by inspecting `std::env::current_exe()`.
- If so, derive the `Contents/Resources/` path and set it as the Bevy asset base path.
- Otherwise, use the default (current directory) for development builds.

This keeps `cargo run` working normally during development.

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make icons` | Render SVG to iconset and convert to .icns |
| `make app` | Build release binary and assemble .app bundle |
| `make run` | Build and launch the .app |
| `make clean` | Remove build/ directory |

## Dependencies

- `librsvg` (brew install) for SVG to PNG conversion
- `iconutil` (built into macOS) for iconset to icns conversion
- Standard Rust toolchain
