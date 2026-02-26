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

### Core Systems

The application uses Bevy's ECS (Entity-Component-System) architecture with these main systems:

1. **Map Management** (src/main.rs:113-130)
   - `setup_map`: Initializes camera and sends initial tile download requests
   - Map tiles are downloaded via `bevy_slippy_tiles` and cached in the `assets/` directory as `{zoom}.{x}.{y}.{tile_size}.tile.png`
   - Default center: London (51.5074, -0.1278) at zoom level 10
   - Tile endpoint: CartoDB dark_all basemap

2. **Zoom System** (src/main.rs:297-374)
   - Two-tier zoom: camera zoom (continuous, 0.1x to 10x) and tile zoom (discrete levels 0-19)
   - `handle_zoom`: Processes MouseWheel events, updates camera zoom, triggers tile level changes at thresholds (1.5x upgrade, 0.75x downgrade)
   - `apply_camera_zoom`: Applies zoom to OrthographicProjection scale
   - Supports both mouse wheel (line units) and trackpad (pixel units) with different sensitivity

3. **Pan/Drag System** (src/main.rs:225-295)
   - `handle_pan_drag`: Implements click-and-drag panning using Mercator projection calculations
   - Converts pixel deltas to lat/lon deltas based on current zoom level and latitude
   - Requests new tiles only after significant movement (>0.001 degrees ≈ 100m)

4. **Aircraft Rendering** (src/main.rs:400-439)
   - `update_aircraft_positions`: Converts lat/lon to screen coordinates using Mercator projection
   - `scale_aircraft_and_labels`: Scales markers and font sizes based on camera zoom
   - `update_aircraft_labels`: Positions text labels relative to aircraft with zoom-aware offsets
   - Aircraft are rendered at z=10, labels at z=11, map tiles at z=0

5. **UI and Cache Management** (src/main.rs:132-178, 441-528)
   - `setup_ui`: Creates attribution text, control instructions, and clear cache button
   - `clear_tile_cache`: Deletes all *.tile.png files from assets/ directory and forces fresh tile downloads

### Key Components

- `Aircraft`: Stores id, latitude, longitude, altitude, heading
- `AircraftLabel`: Links label entities to their aircraft entities
- `MapState`: Tracks current map center (lat/lon) and discrete zoom level
- `ZoomState`: Tracks continuous camera zoom (1.0 = normal) and min/max bounds
- `DragState`: Manages pan drag state and throttles tile requests

### Coordinate System

- Bevy world coordinates: origin at screen center, +X right, +Y up
- Map tiles use Web Mercator projection (EPSG:3857)
- Latitude clamped to ±85.0511° (Mercator limit)
- Longitude clamped to ±180°
- Pixel-to-degree conversion varies by zoom level and latitude (uses cosine correction)

## Dependencies

- `bevy = "0.18"`: Game engine providing ECS, rendering, input, and windowing
- `bevy_slippy_tiles`: Slippy map tile downloading and caching (git fork for 0.18 compatibility)
- `bevy_egui = "0.39"`: Immediate mode GUI integration for settings panel
- `serde`, `serde_json`: JSON serialization for aircraft data feeds
- `reqwest` with blocking feature: HTTP client for API calls

## Current Sample Data

The application spawns 3 sample aircraft around London (src/main.rs:186-223):
- BA123: London center, 35000 ft, heading 90°
- AA456: Southeast offset, 38000 ft, heading 180°
- LH789: Northwest offset, 32000 ft, heading 270°

These are hardcoded in `spawn_sample_aircraft` and should be replaced with real data feeds in production.

## Map Tile Caching

Tiles are cached in `assets/` with naming format: `{zoom}.{x}.{y}.{tile_size}.tile.png`
- Clear cache via in-app button or manually delete `assets/*.tile.png`
- Cache can grow large with extensive panning/zooming across multiple zoom levels
- No automatic cache size management currently implemented

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
