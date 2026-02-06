# Changelog

All notable changes to AirJedi are documented in this file.

## [Unreleased] - Major Feature Upgrade

This release represents a comprehensive enhancement of AirJedi with four phases of development, transforming it from a basic aircraft map tracker into a full-featured aviation monitoring application.

### Phase 1: Core Interaction Enhancements

#### Aircraft Detail Panel (`src/aircraft/detail_panel.rs`)
- Full aircraft information display including callsign, ICAO, altitude, heading, speed
- Vertical rate and squawk code display
- Real-time updates as aircraft data changes
- Panel toggle with `D` key or click on selected aircraft

#### Flight Following (Camera Lock) (`src/aircraft/list_panel.rs`, `src/keyboard.rs`)
- Lock camera to follow a selected aircraft
- Map automatically centers on followed aircraft as it moves
- Toggle follow mode with `F` key
- Visual indicator when following an aircraft

#### Emergency Squawk Detection (`src/aircraft/emergency.rs`)
- Automatic detection of emergency squawk codes (7500, 7600, 7700)
- Visual highlighting of emergency aircraft
- Emergency status display in detail panel and list

#### Keyboard Shortcuts (`src/keyboard.rs`)
- Comprehensive keyboard navigation:
  - `L` - Toggle aircraft list
  - `D` - Toggle detail panel
  - `S` - Toggle statistics
  - `B` - Toggle bookmarks
  - `M` - Measurement mode
  - `E` - Export panel
  - `V` - Coverage tracking
  - `3` - 3D view panel
  - `Esc` - Deselect/cancel follow
  - `F` - Follow selected aircraft
  - `C` - Center on selected
  - `+/-` - Zoom in/out
  - `H` - Help overlay
  - `R` - Reset view
  - `A` - Toggle airports
  - `T` - Toggle trails
  - `W` - Toggle weather
  - `Shift+A` - Toggle airspace
  - `Shift+D` - Data sources panel
  - `Shift+V` - Coverage stats
  - `Ctrl+R` - Record/stop recording

### Phase 2: Enhanced Visualization

#### Multiple Basemaps (`src/config.rs`)
- Four basemap styles:
  - CartoDB Dark (default)
  - CartoDB Light
  - OpenStreetMap
  - ESRI Satellite
- Style selection in settings panel
- Automatic tile cache management per style

#### Flight Path Predictions (`src/aircraft/prediction.rs`)
- Future position prediction based on current heading and speed
- Visual prediction line extending from aircraft
- Configurable prediction time window

#### Statistics Dashboard (`src/aircraft/stats_panel.rs`)
- Real-time aircraft count and distribution
- Altitude band statistics
- Flight category breakdown
- Message rate monitoring
- Toggle with `S` key

#### Advanced Aircraft Filters (`src/aircraft/list_panel.rs`)
- Filter by altitude range
- Filter by callsign pattern
- Filter by aircraft type
- Filter by emergency status
- Combine multiple filter criteria

### Phase 3: Data and Integrations

#### Weather Overlay (METAR) (`src/weather/`)
- METAR data display for airports
- Flight category coloring (VFR, MVFR, IFR, LIFR)
- Wind, visibility, and ceiling information
- Toggle with `W` key

#### Bookmarks/Favorites (`src/bookmarks/mod.rs`)
- Bookmark specific aircraft by ICAO address
- Bookmark map locations with name and zoom level
- Quick navigation to bookmarked locations
- Persistent storage in config file
- Toggle panel with `B` key

#### Historical Playback (`src/recording/`)
- Record flight sessions to NDJSON format
- Playback recorded sessions with speed control (0.5x, 1x, 2x, 4x)
- Pause and resume playback
- Progress bar and time display
- Record with `Ctrl+R`, manage via Recording panel

#### Distance/Bearing Tools (`src/tools/measurement.rs`)
- Click-to-measure mode for point-to-point distance
- Great circle distance calculation in nautical miles
- Bearing display between points
- Visual line overlay
- Toggle with `M` key

### Phase 4: Future Enhancements (Research & Prototyping)

#### Coverage Map (`src/coverage/mod.rs`)
- Visualize receiver coverage based on tracked aircraft
- 36-sector polar coverage tracking (10 degrees each)
- Maximum range per sector
- Coverage statistics panel
- Toggle tracking with `V` key
- Stats panel with `Shift+V`

#### Airspace Boundaries (`src/airspace/mod.rs`)
- Data structures for Class A/B/C/D/E/G airspace
- Support for restricted, prohibited, warning, MOA, alert, and TFR areas
- Altitude floor/ceiling definitions
- Sample airspace data for testing
- Toggle with `Shift+A`
- Stub for FAA/OpenAIP data integration

#### Multiple Data Sources (`src/data_sources/mod.rs`)
- Support for configuring multiple ADS-B feeds
- Data source status tracking
- Aircraft data merging from multiple sources
- Priority-based source selection
- Panel with `Shift+D`

#### Export/Import (`src/export/mod.rs`)
- Export recorded sessions to KML (Google Earth)
- Export to CSV for spreadsheet analysis
- Export to GeoJSON for map visualization
- Load and convert recorded NDJSON files
- Toggle with `E` key

#### 3D View Mode (`src/view3d/mod.rs`)
- Research stub for 3D perspective view
- Camera position calculation
- Geographic to 3D coordinate conversion
- Documentation of implementation approach
- Toggle panel with `3` key

### Core Improvements

#### Aviation Data (`src/aviation/`)
- Airport database loading from CSV
- Runway information display
- Navaid (VOR, NDB, etc.) overlay
- Configurable overlay visibility
- Efficient viewport-based rendering

#### Flight Trails (`src/aircraft/trails.rs`, `src/aircraft/trail_renderer.rs`)
- Historical position trail visualization
- Altitude-based color coding
- Configurable trail duration (default 5 minutes)
- Fade-out effect for older points
- Toggle with `T` key

#### Configuration System (`src/config.rs`)
- TOML-based configuration file
- Settings UI panel (Escape key)
- Feed endpoint configuration
- Map default settings
- Overlay visibility persistence
- Bookmark storage

### Technical Notes

- Built with Bevy 0.18 game engine
- ECS (Entity-Component-System) architecture
- Mercator projection for map display
- bevy_slippy_tiles for tile management
- bevy_egui for UI panels
- adsb_client for live ADS-B data

### Data Sources Documentation

For full implementation of certain features, external data sources are required:

**Airspace Data:**
- FAA NASR (National Airspace System Resources)
- OpenAIP (open aviation database)
- FAA SUA (Special Use Airspace)

**Weather Data:**
- Aviation Weather Center (aviationweather.gov)
- NOAA METAR services

**Terrain Data (for 3D view):**
- SRTM elevation data
- Mapbox terrain tiles
- USGS elevation service

### File Structure

```
src/
  main.rs           - Application entry point and core systems
  config.rs         - Configuration management and settings UI
  keyboard.rs       - Keyboard shortcut handling
  aircraft/
    mod.rs          - Aircraft module exports
    plugin.rs       - Aircraft plugin registration
    list_panel.rs   - Aircraft list UI and filtering
    detail_panel.rs - Selected aircraft detail panel
    stats_panel.rs  - Statistics dashboard
    emergency.rs    - Emergency squawk detection
    prediction.rs   - Flight path prediction
    trails.rs       - Trail data structures
    trail_renderer.rs - Trail visualization
  aviation/
    mod.rs          - Aviation data module
    types.rs        - Airport, runway, navaid types
    loader.rs       - CSV data loading
    airports.rs     - Airport rendering
    runways.rs      - Runway rendering
    navaids.rs      - Navaid rendering
  weather/
    mod.rs          - Weather plugin
    metar.rs        - METAR parsing and display
  bookmarks/
    mod.rs          - Bookmarks panel and storage
  recording/
    mod.rs          - Recording plugin
    recorder.rs     - Session recording
    player.rs       - Session playback
  tools/
    mod.rs          - Tools plugin
    measurement.rs  - Distance/bearing measurement
  coverage/
    mod.rs          - Coverage map tracking
  airspace/
    mod.rs          - Airspace boundaries (stub)
  data_sources/
    mod.rs          - Multiple data sources (stub)
  export/
    mod.rs          - Export to KML/CSV/GeoJSON
  view3d/
    mod.rs          - 3D view mode (research)
  data/
    mod.rs          - Data utilities
    downloader.rs   - Data file downloading
```
