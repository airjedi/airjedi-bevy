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

# Generate and open documentation
cargo doc --open
```

Note: The project uses custom optimization profiles in Cargo.toml - dev mode has opt-level 1 for the main crate but opt-level 3 for dependencies to improve compile-time performance.

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
