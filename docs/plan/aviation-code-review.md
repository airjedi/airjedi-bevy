# Aviation Code Review

## 1. Duplicated Haversine / Geodesic Calculations (HIGH PRIORITY)

The haversine distance function is implemented **three separate times** with identical logic:

- `src/aircraft/detail_panel.rs:39-52` -- `haversine_distance_nm()`
- `src/aircraft/list_panel.rs:110-123` -- `haversine_distance_nm()`
- `src/tools/measurement.rs:49-62` -- `haversine_distance_nm()`

Additionally, `src/coverage/mod.rs:107-124` has its own inline haversine (as `CoverageState::calculate_range_nm`), and `src/coverage/mod.rs:90-104` has a bearing calculation duplicated from `src/tools/measurement.rs:65-75` (`calculate_bearing`).

**Recommendation:** Extract all geodesic math into a single `src/geo/` or `src/geo.rs` module with functions like:
- `haversine_distance_nm(lat1, lon1, lat2, lon2) -> f64`
- `initial_bearing(lat1, lon1, lat2, lon2) -> f64`
- `predict_position(lat, lon, heading_deg, speed_knots, minutes) -> (f64, f64)` (currently in `prediction.rs:31-66`)

The earth radius constant `3440.065` NM appears in 4 different files. It should be a single named constant.

---

## 2. Earth Radius and Aviation Constants Are Scattered (HIGH PRIORITY)

Magic constants appear across the codebase without centralization:

| Constant | Value | Locations |
|---|---|---|
| Earth radius (NM) | `3440.065` | `detail_panel.rs:40`, `list_panel.rs:111`, `measurement.rs:50`, `coverage/mod.rs:122`, `prediction.rs:48` |
| Feet-to-meters | `0.3048` | `export/mod.rs:121` |
| FL threshold | `18000` | `detail_panel.rs:165`, `list_panel.rs:460` |
| Ground traffic alt | `100` | `list_panel.rs:155` |
| Squawk codes | `7500/7600/7700` | `emergency.rs:7-9` (properly named, but not shared) |
| NM-to-KM | `1.852` | `measurement.rs:38` |

The `src/main.rs` has a `constants` module (lines 34-86) for map/UI constants, but aviation-specific constants are not gathered there. Squawk codes in `emergency.rs` are well-organized but isolated.

**Recommendation:** Create a `src/aviation_constants.rs` or add an `aviation` section to the existing constants module with:
- Earth radius in NM, KM, statute miles
- Unit conversion factors (ft-to-m, nm-to-km, etc.)
- Flight level threshold (18,000 ft)
- Standard squawk codes
- Speed thresholds for "stationary" aircraft

---

## 3. No Newtype Wrappers for Aviation Units (MEDIUM PRIORITY)

All aviation values are raw primitives (`f64`, `i32`, `f32`). This allows mixing units silently:

- Altitude is `Option<i32>` in feet -- but the KML export converts to meters inline (`src/export/mod.rs:121`)
- Speed is `Option<f64>` assumed to be knots everywhere
- Heading is `Option<f32>` in degrees
- Latitude/longitude are bare `f64`
- Distances are sometimes NM, sometimes KM -- the type doesn't distinguish

The `Aircraft` struct in `src/main.rs:278-298` documents units in comments, but there is no compile-time enforcement.

The `src/airspace/mod.rs` has a good `AltitudeReference` enum (`MSL(i32)`, `AGL(i32)`, `FL(u16)`, `Surface`, `Unlimited`) at line 102-114 that could serve as a model for how altitude should be handled more broadly, but it is only used within the airspace module.

**Recommendation:** Consider newtype wrappers:
```rust
pub struct Feet(pub i32);
pub struct Knots(pub f64);
pub struct Degrees(pub f32);  // heading/track
pub struct NauticalMiles(pub f64);
pub struct LatLon { pub lat: f64, pub lon: f64 }
```
This would prevent accidentally passing a kilometer value where NM is expected, and make unit conversions explicit. Start with `Feet` and `Knots` as they cross module boundaries most often.

---

## 4. Coordinate Conversion Boilerplate Is Heavily Repeated (HIGH PRIORITY)

The pattern of converting lat/lon to screen coordinates via `world_coords_to_world_pixel` relative to a reference point is repeated verbatim in at least **10 places**:

- `src/main.rs`: lines 976-994, 1220-1228, 1230-1244
- `src/aircraft/trail_renderer.rs:19-27`
- `src/aircraft/prediction.rs:109-132`
- `src/aircraft/emergency.rs:109-117`
- `src/aircraft/list_panel.rs:584-602`
- `src/aviation/airports.rs:65-73, 125-133`
- `src/aviation/navaids.rs:46-54`
- `src/aviation/runways.rs:49-57`

Every single rendering system has this ~10-line block:
```rust
let reference_ll = LatitudeLongitudeCoordinates {
    latitude: tile_settings.reference_latitude,
    longitude: tile_settings.reference_longitude,
};
let reference_pixel = world_coords_to_world_pixel(
    &reference_ll,
    TileSize::Normal,
    map_state.zoom_level,
);
```

**Recommendation:** Create a helper struct or resource:
```rust
pub struct CoordinateConverter {
    reference_pixel: (f64, f64),
    zoom_level: ZoomLevel,
}

impl CoordinateConverter {
    pub fn from_settings(tile_settings: &SlippyTilesSettings, zoom: ZoomLevel) -> Self { ... }
    pub fn latlon_to_world(&self, lat: f64, lon: f64) -> Vec2 { ... }
    pub fn world_to_latlon(&self, pos: Vec2) -> (f64, f64) { ... }
}
```

This would reduce each rendering system by 10-15 lines and centralize the projection logic.

---

## 5. The `Aircraft` Component Is in `main.rs`, Not the `aircraft` Module (MEDIUM PRIORITY)

The core `Aircraft` component struct is defined at `src/main.rs:278-298`, not in `src/aircraft/mod.rs`. All the aircraft sub-modules (`trails.rs`, `prediction.rs`, `emergency.rs`, etc.) reference it via `crate::Aircraft`.

Similarly, `AircraftLabel` (line 301-304), `MapState` (line 323-339), and `ZoomState` (line 356-379) are all in `main.rs` but used across many modules.

**Recommendation:** Move `Aircraft` and `AircraftLabel` to `src/aircraft/components.rs`, and `MapState`/`ZoomState` to `src/map/state.rs` or `src/map.rs`.

This would make `main.rs` (currently 1586 lines) significantly smaller and reduce cross-module coupling through `crate::`.

---

## 6. ADS-B Client Integration Is Tightly Coupled to `main.rs` (MEDIUM PRIORITY)

The entire ADS-B client setup, data syncing, and connection status display is in `main.rs` (lines 396-831). This includes:
- `AdsbAircraftData` resource (line 402-428)
- `setup_adsb_client` system (line 435-496)
- `sync_aircraft_from_adsb` system (line 683-784)
- `update_aircraft_label_text` system (line 787-798)
- `update_connection_status` system (line 801-831)

**Recommendation:** Extract to `src/adsb/` module:
- `src/adsb/mod.rs` -- plugin, resources
- `src/adsb/sync.rs` -- `sync_aircraft_from_adsb`, entity management
- `src/adsb/connection.rs` -- client setup, connection state

---

## 7. Altitude Display Logic Is Duplicated (LOW PRIORITY)

The "show FL above 18,000, feet below" logic appears in two places:
- `src/aircraft/detail_panel.rs:162-169`
- `src/aircraft/list_panel.rs:460-464`

The altitude color mapping also exists in two different forms:
- `src/aircraft/trails.rs:65-88` -- `altitude_color()` for trail rendering (continuous gradient)
- `src/aircraft/list_panel.rs:237-245` -- `get_altitude_color()` for list display (discrete bands)
- `src/aircraft/stats_panel.rs:37-63` -- `AltitudeBandStats` (yet another altitude classification)

**Recommendation:** Create a small `altitude` utility module with:
- `format_altitude(alt: i32) -> String` -- FL or feet formatting
- `altitude_color_continuous(alt: Option<i32>) -> Color` -- for trails
- `altitude_color_discrete(alt: Option<i32>) -> (Color, &str)` -- for UI
- `AltitudeBand` enum with classification logic

---

## 8. Weather Module Coordinate Handling Differs from Rest of App (MEDIUM PRIORITY)

In `src/weather/metar.rs:490-520`, `update_weather_indicator_positions` uses a different coordinate conversion approach than every other rendering system. It calculates positions relative to the map center rather than the tile reference point, and applies `zoom_state.camera_zoom` as a multiplier:

```rust
let rel_x = (px - cx) as f32 * zoom_state.camera_zoom;
let rel_y = -(py - cy) as f32 * zoom_state.camera_zoom;
```

Every other system (aircraft, airports, navaids, runways, trails) positions relative to `tile_settings.reference_latitude/longitude` and does NOT multiply by camera zoom. This is an inconsistency that likely causes weather indicators to drift relative to other map features when panning.

**Recommendation:** Align the weather indicator positioning with the same reference-point-based approach used everywhere else.

---

## 9. Aviation Data Loading Is Synchronous/Blocking on Startup (LOW-MEDIUM PRIORITY)

`src/aviation/loader.rs:108-145` runs `start_aviation_data_loading` as a Startup system. It downloads and parses CSV files synchronously, which blocks the main thread. The downloads are guarded by a 7-day cache (`src/data/downloader.rs:65-82`), but on cache miss, the entire UI will freeze.

**Recommendation:** Move the download/parse to a background thread (similar to the ADS-B client pattern), and use `Arc<Mutex<>>` or channels to communicate completion to the main thread. The `LoadingState` enum already supports this workflow.

---

## 10. No Validation of Aviation Data Ranges (LOW PRIORITY)

When syncing aircraft from ADS-B (`src/main.rs:706-724`), there is no validation of incoming values:
- Latitude could be outside [-90, 90]
- Longitude could be outside [-180, 180]
- Altitude could be negative or unreasonably high
- Heading could be outside [0, 360]
- Speed could be negative

The `clamp_latitude` and `clamp_longitude` functions exist in `main.rs:93-100` but are only applied to the map center, not to aircraft positions.

**Recommendation:** Add validation when ingesting ADS-B data, either in `sync_aircraft_from_adsb` or in the `adsb_client` crate itself.

---

## 11. Trail History Uses `std::time::Instant` (LOW PRIORITY)

`src/aircraft/trails.rs:11` uses `std::time::Instant` for trail point timestamps. This is fine for runtime, but means trails cannot be serialized/deserialized (Instant is not serializable). This limits the recording/playback system -- when recording includes trail data, only positions are saved, not the full trail history.

**Recommendation:** Consider using a relative timestamp (e.g., `f64` seconds since session start) that can be serialized, or use `Duration` from a session-start `Instant`.

---

## 12. Module Organization Summary

Current structure:
```
src/
  main.rs (1586 lines -- too large, contains Aircraft struct, ADS-B, map, zoom, tile logic)
  aircraft/  (well-organized, 7 sub-modules)
  aviation/  (well-organized, airports/runways/navaids/types)
  data/      (downloader only)
  weather/   (METAR with inconsistent coordinate handling)
  airspace/  (stub implementation, well-structured types)
  bookmarks/ (clean)
  recording/ (clean)
  tools/     (measurement)
  coverage/  (clean but has duplicated geo math)
  export/    (clean)
  config.rs  (large but cohesive)
  keyboard.rs
```

**Recommended refactoring targets in priority order:**
1. Extract shared geo/math utilities (haversine, bearing, coordinate conversion)
2. Move `Aircraft` component out of `main.rs` into `aircraft/components.rs`
3. Extract ADS-B integration from `main.rs` into `src/adsb/` module
4. Centralize aviation constants
5. Fix weather coordinate inconsistency
6. Consider newtype wrappers for commonly confused units

---

## Things Done Well

- **Squawk code handling** (`emergency.rs`): Clean enum with `from_squawk`, proper constants, good separation of detection vs rendering
- **Aviation data types** (`aviation/types.rs`): Well-structured with proper serde deserialization, filter logic, and display methods
- **Trail system** (`trails.rs` + `trail_renderer.rs`): Clean separation of data storage and rendering, good age-based pruning
- **Prediction math** (`prediction.rs:31-66`): Correct great-circle position prediction using proper spherical trigonometry
- **Airspace types** (`airspace/mod.rs`): Good data model with `AirspaceClass`, `AltitudeReference`, ray-casting point-in-polygon
- **Plugin pattern**: Consistent use of Bevy plugins for each feature area, clean system registration
- **ADS-B client integration**: Proper background thread with shared state via `Arc<Mutex<>>`, not blocking the main thread
