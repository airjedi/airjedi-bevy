# Desktop Feature Migration Design

**Date:** 2026-02-01
**Status:** Approved
**Scope:** Core tracker features from airjedi-desktop to airjedi-bevy

## Overview

Migrate essential tracking features from the egui-based airjedi-desktop application to the Bevy-based airjedi-bevy application. Focus on core tracker functionality: flight trails, aircraft list panel, and aviation overlays (airports, runways, navaids).

## Features In Scope

1. **Flight Trails** - Altitude-colored, configurable fade history
2. **Aircraft List Panel** - Sortable, filterable egui sidebar
3. **Airport Overlay** - OurAirports data with filter modes
4. **Runway Overlay** - Displayed at higher zoom levels
5. **Navaid Overlay** - VOR/NDB/DME with type-based rendering

## Features Out of Scope

- Aircraft metadata (registration, photos)
- Multi-server support
- Weather overlay
- Video streaming
- SDR/Waterfall
- GPS auto-centering

---

## Architecture

### Module Structure

```
src/
├── main.rs              # Existing - add new systems
├── config.rs            # Existing - extend with new settings
├── aviation/
│   ├── mod.rs           # Aviation data module
│   ├── airports.rs      # Airport loading, filtering, rendering
│   ├── runways.rs       # Runway data and rendering
│   └── navaids.rs       # VOR/NDB/DME rendering
├── aircraft/
│   ├── mod.rs           # Aircraft module
│   ├── trails.rs        # Trail history and rendering
│   └── list_panel.rs    # egui aircraft list sidebar
└── data/
    └── downloader.rs    # OurAirports CSV download/cache
```

### Data Flow

1. On startup, check for cached aviation data in `~/.cache/airjedi-bevy/`
2. If missing/stale, download from OurAirports (airports.csv, runways.csv, navaids.csv)
3. Parse into Bevy resources (AirportData, RunwayData, NavaidData)
4. Render as sprites/shapes based on current viewport bounds and zoom level

---

## Flight Trails System

### Data Model

```rust
#[derive(Component)]
struct TrailHistory {
    points: VecDeque<TrailPoint>,  // Max 300 seconds of history
}

struct TrailPoint {
    lat: f64,
    lon: f64,
    altitude: Option<i32>,
    timestamp: Instant,
}
```

### Rendering Approach

- Each aircraft entity has a `TrailHistory` component (already collecting positions in adsb-client)
- Trail rendered as a polyline using Bevy's `Gizmos` or a custom mesh
- Altitude-based coloring: cyan (0-10k ft) → green → yellow → orange → purple (40k+ ft)
- Opacity fades based on age: 100% for first `solid_duration`, linear fade for `fade_duration`

### Configuration

```toml
[trails]
enabled = true
max_age_seconds = 300        # Total trail length
solid_duration_seconds = 225 # Full opacity portion
fade_duration_seconds = 75   # Fade-out portion
```

### Performance Considerations

- Only render trail points within viewport bounds
- Update trail meshes only when aircraft positions change
- Batch render all trails in single draw call if using mesh approach

---

## Aircraft List Panel

### UI Layout

egui sidebar on left side:

```
┌─ Aircraft (47) ─────────────────┐
│ [▼ Sort: Distance] [▼ Filter ▾] │
├─────────────────────────────────┤
│ 🔍 [Search callsign/ICAO...   ] │
├─────────────────────────────────┤
│ ● UAL123    12,500ft   15nm  →  │
│ ○ DAL456    35,000ft   23nm  ↗  │
│ ○ SWA789    8,200ft    31nm  ↓  │
│ ○ N12345    2,500ft    42nm  ←  │
│   ...                           │
└─────────────────────────────────┘
```

### Features

- Collapsible panel (toggle button, like settings)
- Click aircraft row to select (highlights on map, centers view)
- Sort dropdown: Distance, Altitude, Speed, Callsign
- Filter popover with sliders:
  - Altitude: 0-60,000 ft range
  - Speed: 0-600 kts range
  - Distance: 0-250 nm range
- Text search filters callsign and ICAO hex code
- Auto-scroll to selected aircraft
- Shows heading arrow indicator

### State Management

```rust
#[derive(Resource)]
struct AircraftListState {
    expanded: bool,
    sort_by: SortCriteria,
    sort_ascending: bool,
    filters: AircraftFilters,
    search_text: String,
    selected_icao: Option<String>,
}
```

---

## Aviation Data Overlay

### Data Sources

OurAirports CSV files:
- `airports.csv` - ~75k airports worldwide
- `runways.csv` - ~45k runways
- `navaids.csv` - ~12k navigation aids

### Caching

```
~/.cache/airjedi-bevy/
├── airports.csv      # Downloaded from OurAirports
├── runways.csv
├── navaids.csv
└── metadata.json     # Download timestamp, version
```

- Cache TTL: 7 days (configurable)
- Background download on startup if stale
- Fallback: continue with cached data if download fails

### Airport Rendering

- Filter by type: `large_airport`, `medium_airport`, `small_airport`
- Color by size: Red (large), Orange (medium), Gray (small)
- ICAO labels shown at zoom level 8+
- Three filter modes in settings: All, FrequentlyUsed (scheduled service), MajorOnly

### Runway Rendering

- Rendered as lines from threshold to threshold
- Visible at zoom level 8+ (only for visible airports)
- Color: White with slight transparency

### Navaid Rendering

- VOR: Blue circle with radial lines
- NDB: Orange diamond
- DME: Purple square
- Ident + frequency labels at zoom 9+

### Visibility Controls

```toml
[overlays]
show_airports = true
show_runways = true
show_navaids = false
airport_filter = "FrequentlyUsed"
```

---

## Implementation Plan

### Phase 1: Data Infrastructure

1. Create `data/downloader.rs` - OurAirports CSV download/cache system
2. Create aviation data parsers (airports, runways, navaids)
3. Add Bevy resources for aviation data

### Phase 2: Aviation Overlays

4. Airport rendering with zoom-based visibility
5. Runway rendering linked to airports
6. Navaid rendering with type-based icons
7. Add overlay toggles to settings panel

### Phase 3: Flight Trails

8. Extend TrailHistory from adsb-client data
9. Trail rendering system with altitude coloring
10. Trail configuration in settings panel

### Phase 4: Aircraft List Panel

11. egui sidebar with aircraft list
12. Sorting implementation
13. Filter controls (altitude, speed, distance, text)
14. Selection and map interaction

### Dependencies

- Phase 2 depends on Phase 1 (data loading)
- Phases 3 and 4 are independent of each other
- All phases can share the extended settings panel

### Estimated Scope

~2000-2500 lines of new code across modules

---

## Configuration Extensions

### Full config.toml Structure

```toml
[feed]
endpoint_url = "192.168.1.10:30003"
refresh_interval_ms = 1000

[map]
default_latitude = 37.6872
default_longitude = -97.3301
default_zoom = 10

[trails]
enabled = true
max_age_seconds = 300
solid_duration_seconds = 225
fade_duration_seconds = 75

[overlays]
show_airports = true
show_runways = true
show_navaids = false
airport_filter = "FrequentlyUsed"

[aircraft_list]
expanded = true
width = 350.0
sort_by = "distance"
sort_ascending = true

[cache]
aviation_data_ttl_days = 7
```
