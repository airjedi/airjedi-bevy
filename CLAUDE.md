# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AirJedi is an aircraft map tracker built with Bevy 0.18 game engine and bevy_slippy_tiles for map rendering. The application displays aircraft positions on an interactive slippy map (OpenStreetMap-based) with pan, zoom, and real-time position tracking capabilities.

## Build and Run Commands

```bash
# Build the project
cargo build

# Run the application
cargo run

# Build with release optimizations
cargo build --release

# Run with release optimizations
cargo run --release

# Run with debug logging for airjedi
RUST_LOG=airjedi_bevy=debug cargo run

# Generate and open documentation
cargo doc --open
```

Note: The project uses custom optimization profiles in Cargo.toml - dev mode has opt-level 1 for the main crate but opt-level 3 for dependencies to improve compile-time performance.

## macOS Application Bundle

```bash
# Generate app icon from SVG (requires: brew install librsvg)
cd macos && make icons

# Build AirJedi.app bundle (release mode)
cd macos && make app

# Build and launch
cd macos && make run

# Clean build artifacts
cd macos && make clean
```

The `macos/` directory contains all macOS-specific build files. Assets are copied into `Contents/MacOS/assets/` inside the bundle (where Bevy's AssetPlugin looks). Tile cache uses `~/Library/Caches/airjedi/tiles/` in both development and bundle modes. The `src/paths.rs` module handles bundle-aware path resolution.

## Architecture

The application uses Bevy's ECS architecture with a plugin-based modular design. `src/main.rs` wires plugins together and defines shared constants, coordinate helpers, and the `setup_map` startup system.

### Module Map

| Module | Purpose |
|--------|---------|
| `src/main.rs` | App setup, plugin registration, constants, `setup_map` |
| `src/camera.rs` | Dual-camera system (MapCamera 2D + AircraftCamera 3D), aircraft position/label updates |
| `src/zoom.rs` | Two-tier zoom: continuous camera zoom + discrete tile zoom levels |
| `src/input.rs` | Pan/drag handling with Mercator projection conversion |
| `src/tiles.rs` | Tile lifecycle, multi-resolution bands, fade-in/out, 3D mesh quads |
| `src/tile_cache.rs` | Centralized tile cache in `~/Library/Caches/airjedi/tiles/`, symlinked into assets |
| `src/geo.rs` | `CoordinateConverter`, haversine distance, lat/lon-to-world helpers |
| `src/config.rs` | `AppConfig` persistence (TOML), basemap style, settings UI state |
| `src/theme.rs` | Catppuccin-based theming, egui style application |
| `src/dock.rs` | `egui_tiles`-based dock layout with tabbed panels |
| `src/toolbar.rs` | Top toolbar rendering |
| `src/statusbar.rs` | Bottom status bar (connection, zoom, coordinates) |
| `src/keyboard.rs` | Keyboard shortcuts and help overlay |
| `src/inspector.rs` | `bevy-inspector-egui` integration |
| `src/debug_panel.rs` | FPS, frame time, entity count diagnostics |
| `src/render_layers.rs` | `RenderCategory` constants for layer separation |
| `src/paths.rs` | Bundle-aware path resolution (dev vs macOS .app) |
| `src/map.rs` | `MapState` and `ZoomState` resource definitions |
| `src/units.rs` | Unit conversion helpers |
| `src/ui_panels.rs` | `UiPanelManager` and `PanelId` for panel visibility |
| `src/tools_window.rs` | Consolidated tools/settings window |

### Domain Modules (each has its own Plugin)

| Module | Plugin | Purpose |
|--------|--------|---------|
| `src/aircraft/` | `AircraftPlugin` | Components, trails, picking, prediction, staleness, altitude coloring, list/detail/stats panels, emergency alerts, type database, hanabi particle effects (optional feature) |
| `src/adsb/` | `AdsbPlugin` | Live ADS-B data via `adsb-client` crate, aircraft sync, 3D model loading |
| `src/aviation/` | `AviationPlugin` | Airport, navaid, and runway data loading and rendering |
| `src/view3d/` | `View3DPlugin` | 2D/3D mode transitions, orbit camera, atmosphere/sky/fog, sun/moon positioning, day/night cycle |
| `src/weather/` | `WeatherPlugin` | METAR fetching and weather indicators |
| `src/recording/` | `RecordingPlugin` | Flight recording and playback |
| `src/bookmarks/` | `BookmarksPlugin` | Map location bookmarks |
| `src/tools/` | `ToolsPlugin` | Measurement tools |
| `src/coverage/` | `CoveragePlugin` | ADS-B coverage visualization |
| `src/airspace/` | `AirspacePlugin` | Airspace boundary display |
| `src/data_sources/` | `DataSourcesPlugin` | Data source management |
| `src/export/` | `ExportPlugin` | Data export functionality |
| `src/data/` | — | Background data downloading |

### Dual Camera Architecture

The app uses three cameras:
1. **MapCamera** (`Camera2d`): Map tiles, sprites, text. Layers 0 + 2 (gizmos).
2. **AircraftCamera** (`Camera3d`): 3D aircraft models. Alpha-blends over MapCamera. Switches between orthographic (2D mode) and perspective (3D mode).
3. **UI Camera** (`Camera2d`, order 100): Dedicated egui camera, layer 11 only.

In 3D mode, Camera3d operates in Y-up space; Camera2d derives its transform via rotation for tile rendering.

### Key Resources

- `MapState`: Current map center (lat/lon) and discrete zoom level
- `ZoomState`: Continuous camera zoom (0.1x–10x)
- `View3DState`: 3D view mode, camera orbit (pitch/yaw/altitude), transition state
- `AppConfig`: Persisted settings (basemap, default location, zoom)
- `DockTreeState`: `egui_tiles` dock layout state

### Coordinate System

- Bevy world coordinates: origin at reference point, +X right, +Y up (2D) / +Y up (3D via rotation)
- Map tiles use Web Mercator projection (EPSG:3857)
- Latitude clamped to ±85.0511° (Mercator limit), longitude to ±180°
- Default center: Wichita, KS (37.6872, -97.3301)
- `CoordinateConverter` in `src/geo.rs` handles all lat/lon-to-world conversions

### System Ordering

Systems that modify `MapState::zoom_level` in 3D mode run in `ZoomSet::Change`. Position-dependent systems (aircraft, airports, navaids, camera) run `.after(ZoomSet::Change)`.

## Dependencies

Key dependencies (see `Cargo.toml` for full list):

- `bevy = "0.18"` with `jpeg` feature: Game engine
- `bevy_slippy_tiles`: Slippy map tile downloading/caching (local path, fork for 0.18)
- `bevy_egui = "0.39"` + `bevy-inspector-egui = "0.36"`: egui UI integration and entity inspector
- `egui_tiles = "0.14"`: Dock/tab layout system
- `egui-phosphor = "0.11"`: Icon font for UI
- `catppuccin = "2.6"` + `egui-aesthetix`: Theming
- `bevy_obj = "0.18"`: OBJ model loading
- `bevy_hanabi = "0.18"` (optional `hanabi` feature, default on): GPU particle effects for aircraft trails
- `bevy_brp_extras = "0.18"` (optional `brp` feature, default on): BRP extras for remote inspection, screenshots, input simulation
- `adsb-client`: Local crate for ADS-B SBS1 protocol parsing
- `tokio`: Async runtime for ADS-B network connections
- `reqwest`: HTTP client (blocking + json) for METAR and data downloads
- `serde` + `serde_json` + `toml`: Serialization for config and data
- `chrono`: Date/time handling for sun position and recordings
- `solar-positioning`: Sun elevation/azimuth calculations
- `csv`: Airport/navaid data parsing
- `dirs`: Platform-specific directory paths

## Live Data

Aircraft data comes from a live ADS-B feed via the `adsb-client` crate connecting to an SBS1 server (default: `98.186.33.60:30003`). Aircraft are synced in real-time with a 180-second staleness timeout and 250-mile max distance filter. Connection settings are in `src/main.rs::constants`.

## Map Tile Caching

Tiles are cached in `~/Library/Caches/airjedi/tiles/` (centralized), symlinked into `assets/tiles/` for Bevy's AssetPlugin. Naming format: `{zoom}.{x}.{y}.{tile_size}.tile.png`
- Clear cache via in-app button or `tile_cache::clear_tile_cache()`
- Cache can grow large with extensive panning/zooming across multiple zoom levels
- `tile_cache::remove_invalid_tiles()` runs at startup to clean corrupted files

## Bevy Remote Protocol (BRP)

The `brp` feature flag (default-on) enables runtime inspection and control of the app via the Bevy Remote Protocol. This allows AI coding assistants to query entities, inspect components, take screenshots, and simulate input through the `bevy_brp_mcp` MCP server.

- **Feature flag:** `brp` (disable with `--no-default-features -F hanabi`)
- **HTTP endpoint:** `localhost:15702` (default, not configurable)
- **MCP server:** Configured in `.mcp.json`, uses `bevy_brp_mcp` binary
- **macOS bundle:** BRP is excluded from release app bundles

## 3D Tile Rendering — Known Pitfalls

The 3D tile system in `src/tiles.rs` has several interacting subsystems that can cause visual flashing if modified incorrectly. Read this before changing tile display, zoom transitions, or culling logic.

### Architecture

- **Multi-resolution bands**: 3D mode requests tiles at 5 zoom levels (current, -1, -2, -3, -4) for perspective coverage, but only the current zoom level gets 3D mesh quads. Lower-zoom tiles exist as entities for tracking/transitions but are invisible in 3D.
- **Tile lifecycle**: Download → spawn entity (alpha 0) → fade in → mesh quad created → visible. Each step uses deferred commands, so there are 1-2 frame delays between steps.
- **The 300ms refresh timer** (`Tile3DRefreshTimer`) continuously re-requests tiles to fill the 3D view as the camera moves.

### Common Causes of Tile Flashing

1. **Z-fighting between zoom levels**: Never render mesh quads from multiple zoom levels simultaneously. `AlphaMode::Opaque` mesh quads at similar depths Z-fight unpredictably, especially at grazing angles near the horizon. Geometric depth separation and `StandardMaterial::depth_bias` both fail at grazing angles on Metal. The only reliable fix is single-zoom mesh quads.

2. **Spawn-cull-respawn cycle**: If the entity budget (`max_tile_entities`) is lower than the number of tiles the request system generates, budget culling removes visible tiles, their positions are cleared from `SpawnedTiles`, and the refresh timer re-requests them — creating perpetual flashing. Always ensure the budget exceeds the steady-state tile count (~1000-1200 in 3D).

3. **Zoom oscillation**: The altitude-adaptive zoom (`altitude_to_zoom_level`) uses hysteresis to prevent rapid switching. If hysteresis is too narrow, the zoom bounces between levels every 300ms, destroying and recreating mesh quads each time. Current values: 0.7 to upgrade, 0.6 to downgrade.

4. **Fade-in delay**: Tiles spawn at alpha 0 and fade in. In 3D mode, the fade speed is 30.0 (vs 3.0 in 2D) to minimize the gap when mesh quads are created. If set too slow, there's a visible gap during zoom transitions. If set to instant (alpha 1.0 on spawn), Bevy's default magenta checkerboard texture shows before the tile image loads.

5. **Stale tile accumulation**: If `animate_tile_fades` doesn't mark out-of-band tiles as dominated, tiles from old zoom levels accumulate indefinitely. The dominated check must mark tiles outside [current_zoom-4, current_zoom] for cleanup.

### Debugging Tile Issues

Add a periodic tile census to `animate_tile_fades` to see tile counts per zoom level:
```rust
// Temporary diagnostic — add inside animate_tile_fades when is_3d
use std::sync::atomic::{AtomicU32, Ordering};
static FRAME: AtomicU32 = AtomicU32::new(0);
if FRAME.fetch_add(1, Ordering::Relaxed) % 60 == 0 {
    let mut counts: HashMap<u8, (u32, u32)> = HashMap::new();
    for (_, fs, _, _) in tile_query.iter() {
        let e = counts.entry(fs.tile_zoom).or_default();
        e.0 += 1;
        if fs.alpha >= 1.0 { e.1 += 1; }
    }
    info!("TILE CENSUS zoom={} {:?} total={}", current_zoom, counts, tile_query.iter().count());
}
```

Run with `RUST_LOG=airjedi_bevy=info cargo run --release 2>tmp/tile_debug.log` and look for:
- **Total exceeding budget** → spawn-cull cycle
- **Tiles at alpha 0 that never become opaque** → texture loading or fade issue
- **Zoom changing every 300ms** → hysteresis too narrow
- **Tiles outside the band accumulating** → dominated check not working

### Bevy StandardMaterial::depth_bias Warning

`depth_bias` is cast to `i32` internally (`material.depth_bias as i32`). Fractional values like 0.001 or 0.01 are silently truncated to 0 and have no effect. Always use integer values if you need depth bias. Even with correct integer values, depth bias does not reliably prevent Z-fighting between horizontal coplanar mesh quads at grazing angles on Metal.
