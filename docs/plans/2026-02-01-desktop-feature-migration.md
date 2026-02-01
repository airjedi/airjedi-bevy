# Desktop Feature Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate core tracker features (trails, aircraft list, aviation overlays) from airjedi-desktop to airjedi-bevy.

**Architecture:** Module-based organization with `src/aviation/`, `src/aircraft/`, and `src/data/` directories. Each feature is a Bevy plugin. Aviation data downloaded from OurAirports and cached locally. Trail rendering uses Bevy Gizmos for line drawing.

**Tech Stack:** Bevy 0.17, bevy_egui 0.38, bevy_slippy_tiles 0.10, reqwest (async), serde/csv parsing, chrono for timestamps.

---

## Phase 1: Data Infrastructure

### Task 1: Create Data Downloader Module

**Files:**
- Create: `src/data/mod.rs`
- Create: `src/data/downloader.rs`
- Modify: `src/main.rs:8` (add mod declaration)

**Step 1: Create module structure**

Create `src/data/mod.rs`:
```rust
pub mod downloader;

pub use downloader::*;
```

**Step 2: Create downloader skeleton**

Create `src/data/downloader.rs`:
```rust
use bevy::prelude::*;
use std::path::PathBuf;

/// Cache directory for aviation data
fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("airjedi-bevy")
}

/// Check if cached file exists and is fresh (< 7 days old)
pub fn is_cache_fresh(filename: &str) -> bool {
    let path = cache_dir().join(filename);
    if !path.exists() {
        return false;
    }

    match std::fs::metadata(&path) {
        Ok(meta) => {
            if let Ok(modified) = meta.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    return elapsed.as_secs() < 7 * 24 * 60 * 60; // 7 days
                }
            }
            false
        }
        Err(_) => false,
    }
}

/// Get the path to a cached file
pub fn cache_path(filename: &str) -> PathBuf {
    cache_dir().join(filename)
}

/// Ensure cache directory exists
pub fn ensure_cache_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir())
}
```

**Step 3: Add mod declaration to main.rs**

In `src/main.rs`, after line 8 (`mod config;`), add:
```rust
mod data;
```

**Step 4: Add dirs dependency to Cargo.toml**

Add to `[dependencies]`:
```toml
dirs = "6.0"
```

**Step 5: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 6: Commit**

```bash
git add src/data/ src/main.rs Cargo.toml
git commit -m "Add data downloader module skeleton"
```

---

### Task 2: Implement Async Download with Progress

**Files:**
- Modify: `src/data/downloader.rs`
- Modify: `Cargo.toml` (add csv dependency)

**Step 1: Add download function**

Add to `src/data/downloader.rs`:
```rust
use std::io::Write;

const OURAIRPORTS_BASE: &str = "https://davidmegginson.github.io/ourairports-data";

/// OurAirports data files
pub enum DataFile {
    Airports,
    Runways,
    Navaids,
}

impl DataFile {
    pub fn filename(&self) -> &'static str {
        match self {
            DataFile::Airports => "airports.csv",
            DataFile::Runways => "runways.csv",
            DataFile::Navaids => "navaids.csv",
        }
    }

    pub fn url(&self) -> String {
        format!("{}/{}", OURAIRPORTS_BASE, self.filename())
    }
}

/// Download a file from OurAirports (blocking, for use in async task)
pub fn download_file_blocking(file: &DataFile) -> Result<(), String> {
    ensure_cache_dir().map_err(|e| format!("Failed to create cache dir: {}", e))?;

    let url = file.url();
    info!("Downloading {} from {}", file.filename(), url);

    let response = reqwest::blocking::get(&url)
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    let bytes = response.bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let path = cache_path(file.filename());
    let mut file_handle = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    file_handle.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    info!("Downloaded {} ({} bytes)", file.filename(), bytes.len());
    Ok(())
}
```

**Step 2: Add csv dependency to Cargo.toml**

Add to `[dependencies]`:
```toml
csv = "1.3"
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/data/downloader.rs Cargo.toml
git commit -m "Add OurAirports file download functionality"
```

---

### Task 3: Create Aviation Data Types

**Files:**
- Create: `src/aviation/mod.rs`
- Create: `src/aviation/types.rs`
- Modify: `src/main.rs` (add mod declaration)

**Step 1: Create module structure**

Create `src/aviation/mod.rs`:
```rust
pub mod types;

pub use types::*;
```

**Step 2: Create aviation types**

Create `src/aviation/types.rs`:
```rust
use bevy::prelude::*;
use serde::Deserialize;

/// Airport type from OurAirports
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AirportType {
    LargeAirport,
    MediumAirport,
    SmallAirport,
    Heliport,
    SeaplaneBase,
    Balloonport,
    Closed,
    #[serde(other)]
    Unknown,
}

/// Airport filter mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AirportFilter {
    /// All public airports
    All,
    /// Airports with scheduled service (default)
    #[default]
    FrequentlyUsed,
    /// Large international airports only
    MajorOnly,
}

/// Airport data from OurAirports
#[derive(Debug, Clone, Deserialize)]
pub struct Airport {
    pub id: i64,
    pub ident: String,
    #[serde(rename = "type")]
    pub airport_type: AirportType,
    pub name: String,
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    pub elevation_ft: Option<i32>,
    pub continent: Option<String>,
    pub iso_country: Option<String>,
    pub iso_region: Option<String>,
    pub municipality: Option<String>,
    pub scheduled_service: Option<String>,
    pub gps_code: Option<String>,
    pub iata_code: Option<String>,
    pub local_code: Option<String>,
}

impl Airport {
    /// Check if airport has scheduled service
    pub fn has_scheduled_service(&self) -> bool {
        self.scheduled_service.as_deref() == Some("yes")
    }

    /// Check if airport is a major (large) airport
    pub fn is_major(&self) -> bool {
        self.airport_type == AirportType::LargeAirport
    }

    /// Check if airport passes the current filter
    pub fn passes_filter(&self, filter: AirportFilter) -> bool {
        match filter {
            AirportFilter::All => matches!(
                self.airport_type,
                AirportType::LargeAirport | AirportType::MediumAirport | AirportType::SmallAirport
            ),
            AirportFilter::FrequentlyUsed => self.has_scheduled_service(),
            AirportFilter::MajorOnly => self.is_major(),
        }
    }

    /// Get color based on airport size
    pub fn color(&self) -> Color {
        match self.airport_type {
            AirportType::LargeAirport => Color::srgb(1.0, 0.2, 0.2),   // Red
            AirportType::MediumAirport => Color::srgb(1.0, 0.6, 0.2), // Orange
            _ => Color::srgb(0.6, 0.6, 0.6),                          // Gray
        }
    }
}

/// Runway data from OurAirports
#[derive(Debug, Clone, Deserialize)]
pub struct Runway {
    pub id: i64,
    pub airport_ref: i64,
    pub airport_ident: String,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub surface: Option<String>,
    pub lighted: Option<i32>,
    pub closed: Option<i32>,
    pub le_ident: Option<String>,
    pub le_latitude_deg: Option<f64>,
    pub le_longitude_deg: Option<f64>,
    pub le_elevation_ft: Option<i32>,
    pub le_heading_degT: Option<f64>,
    pub he_ident: Option<String>,
    pub he_latitude_deg: Option<f64>,
    pub he_longitude_deg: Option<f64>,
    pub he_elevation_ft: Option<i32>,
    pub he_heading_degT: Option<f64>,
}

impl Runway {
    /// Check if runway has valid coordinates for both ends
    pub fn has_valid_coords(&self) -> bool {
        self.le_latitude_deg.is_some()
            && self.le_longitude_deg.is_some()
            && self.he_latitude_deg.is_some()
            && self.he_longitude_deg.is_some()
    }

    /// Check if runway is closed
    pub fn is_closed(&self) -> bool {
        self.closed == Some(1)
    }
}

/// Navaid type
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NavaidType {
    Vor,
    #[serde(alias = "VOR-DME")]
    VorDme,
    Dme,
    Ndb,
    #[serde(alias = "NDB-DME")]
    NdbDme,
    Tacan,
    #[serde(alias = "VORTAC")]
    Vortac,
    #[serde(other)]
    Unknown,
}

/// Navaid data from OurAirports
#[derive(Debug, Clone, Deserialize)]
pub struct Navaid {
    pub id: i64,
    pub filename: Option<String>,
    pub ident: String,
    pub name: String,
    #[serde(rename = "type")]
    pub navaid_type: NavaidType,
    pub frequency_khz: Option<i32>,
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    pub elevation_ft: Option<i32>,
    pub iso_country: Option<String>,
    pub dme_frequency_khz: Option<i32>,
    pub dme_channel: Option<String>,
    pub dme_latitude_deg: Option<f64>,
    pub dme_longitude_deg: Option<f64>,
    pub dme_elevation_ft: Option<i32>,
    pub slaved_variation_deg: Option<f64>,
    pub magnetic_variation_deg: Option<f64>,
    #[serde(rename = "usageType")]
    pub usage_type: Option<String>,
    pub power: Option<String>,
    pub associated_airport: Option<String>,
}

impl Navaid {
    /// Get color based on navaid type
    pub fn color(&self) -> Color {
        match self.navaid_type {
            NavaidType::Vor | NavaidType::VorDme | NavaidType::Vortac => {
                Color::srgb(0.2, 0.6, 1.0) // Blue
            }
            NavaidType::Ndb | NavaidType::NdbDme => {
                Color::srgb(1.0, 0.6, 0.2) // Orange
            }
            NavaidType::Dme | NavaidType::Tacan => {
                Color::srgb(0.8, 0.2, 1.0) // Purple
            }
            NavaidType::Unknown => Color::srgb(0.5, 0.5, 0.5), // Gray
        }
    }

    /// Get frequency as string for display
    pub fn frequency_display(&self) -> String {
        if let Some(freq) = self.frequency_khz {
            if freq >= 1000 {
                format!("{:.2}", freq as f64 / 1000.0)
            } else {
                freq.to_string()
            }
        } else {
            String::new()
        }
    }
}
```

**Step 3: Add mod declaration to main.rs**

In `src/main.rs`, after the `mod data;` line, add:
```rust
mod aviation;
```

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/aviation/ src/main.rs
git commit -m "Add aviation data types for airports, runways, navaids"
```

---

### Task 4: Create Aviation Data Loader

**Files:**
- Create: `src/aviation/loader.rs`
- Modify: `src/aviation/mod.rs`

**Step 1: Create loader module**

Create `src/aviation/loader.rs`:
```rust
use bevy::prelude::*;
use std::collections::HashMap;

use crate::data::{cache_path, is_cache_fresh, download_file_blocking, DataFile};
use super::types::{Airport, Runway, Navaid};

/// Resource containing all aviation data
#[derive(Resource, Default)]
pub struct AviationData {
    pub airports: Vec<Airport>,
    pub runways: Vec<Runway>,
    pub navaids: Vec<Navaid>,
    /// Runways indexed by airport_ref for fast lookup
    pub runways_by_airport: HashMap<i64, Vec<usize>>,
    pub loading_state: LoadingState,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum LoadingState {
    #[default]
    NotStarted,
    Downloading,
    Parsing,
    Ready,
    Failed,
}

impl AviationData {
    /// Build runway index after loading
    pub fn build_runway_index(&mut self) {
        self.runways_by_airport.clear();
        for (idx, runway) in self.runways.iter().enumerate() {
            self.runways_by_airport
                .entry(runway.airport_ref)
                .or_default()
                .push(idx);
        }
    }

    /// Get runways for an airport
    pub fn get_runways_for_airport(&self, airport_id: i64) -> Vec<&Runway> {
        self.runways_by_airport
            .get(&airport_id)
            .map(|indices| indices.iter().map(|&i| &self.runways[i]).collect())
            .unwrap_or_default()
    }
}

/// Load airports from cached CSV
fn load_airports() -> Result<Vec<Airport>, String> {
    let path = cache_path(DataFile::Airports.filename());
    let mut rdr = csv::Reader::from_path(&path)
        .map_err(|e| format!("Failed to open airports.csv: {}", e))?;

    let mut airports = Vec::new();
    for result in rdr.deserialize() {
        match result {
            Ok(airport) => airports.push(airport),
            Err(e) => {
                // Log but continue - some rows may have parsing issues
                warn!("Skipping airport row: {}", e);
            }
        }
    }
    info!("Loaded {} airports", airports.len());
    Ok(airports)
}

/// Load runways from cached CSV
fn load_runways() -> Result<Vec<Runway>, String> {
    let path = cache_path(DataFile::Runways.filename());
    let mut rdr = csv::Reader::from_path(&path)
        .map_err(|e| format!("Failed to open runways.csv: {}", e))?;

    let mut runways = Vec::new();
    for result in rdr.deserialize() {
        match result {
            Ok(runway) => runways.push(runway),
            Err(e) => {
                warn!("Skipping runway row: {}", e);
            }
        }
    }
    info!("Loaded {} runways", runways.len());
    Ok(runways)
}

/// Load navaids from cached CSV
fn load_navaids() -> Result<Vec<Navaid>, String> {
    let path = cache_path(DataFile::Navaids.filename());
    let mut rdr = csv::Reader::from_path(&path)
        .map_err(|e| format!("Failed to open navaids.csv: {}", e))?;

    let mut navaids = Vec::new();
    for result in rdr.deserialize() {
        match result {
            Ok(navaid) => navaids.push(navaid),
            Err(e) => {
                warn!("Skipping navaid row: {}", e);
            }
        }
    }
    info!("Loaded {} navaids", navaids.len());
    Ok(navaids)
}

/// System to initialize aviation data loading
pub fn start_aviation_data_loading(mut aviation_data: ResMut<AviationData>) {
    if aviation_data.loading_state != LoadingState::NotStarted {
        return;
    }

    aviation_data.loading_state = LoadingState::Downloading;

    // Check cache freshness and download if needed
    let files = [DataFile::Airports, DataFile::Runways, DataFile::Navaids];

    for file in &files {
        if !is_cache_fresh(file.filename()) {
            if let Err(e) = download_file_blocking(file) {
                error!("Failed to download {}: {}", file.filename(), e);
                aviation_data.loading_state = LoadingState::Failed;
                return;
            }
        }
    }

    aviation_data.loading_state = LoadingState::Parsing;

    // Load data from cache
    match (load_airports(), load_runways(), load_navaids()) {
        (Ok(airports), Ok(runways), Ok(navaids)) => {
            aviation_data.airports = airports;
            aviation_data.runways = runways;
            aviation_data.navaids = navaids;
            aviation_data.build_runway_index();
            aviation_data.loading_state = LoadingState::Ready;
            info!("Aviation data ready");
        }
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            error!("Failed to load aviation data: {}", e);
            aviation_data.loading_state = LoadingState::Failed;
        }
    }
}
```

**Step 2: Update aviation/mod.rs**

Replace `src/aviation/mod.rs`:
```rust
pub mod types;
pub mod loader;

pub use types::*;
pub use loader::*;
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/aviation/
git commit -m "Add aviation data loader with CSV parsing"
```

---

## Phase 2: Aviation Overlays

### Task 5: Create Airport Rendering System

**Files:**
- Create: `src/aviation/airports.rs`
- Modify: `src/aviation/mod.rs`

**Step 1: Create airport rendering system**

Create `src/aviation/airports.rs`:
```rust
use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{Airport, AirportFilter, AviationData, LoadingState};
use crate::config::AppConfig;

/// Component marking an airport entity
#[derive(Component)]
pub struct AirportMarker {
    pub airport_id: i64,
}

/// Component for airport labels
#[derive(Component)]
pub struct AirportLabel {
    pub airport_entity: Entity,
}

/// Resource for airport rendering state
#[derive(Resource)]
pub struct AirportRenderState {
    pub show_airports: bool,
    pub filter: AirportFilter,
    /// Viewport bounds for culling (min_lat, max_lat, min_lon, max_lon)
    pub viewport_bounds: Option<(f64, f64, f64, f64)>,
}

impl Default for AirportRenderState {
    fn default() -> Self {
        Self {
            show_airports: true,
            filter: AirportFilter::FrequentlyUsed,
            viewport_bounds: None,
        }
    }
}

/// Z-layer for airports (below aircraft, above tiles)
const AIRPORT_Z_LAYER: f32 = 5.0;
const AIRPORT_LABEL_Z_LAYER: f32 = 6.0;

/// System to spawn airport entities when data is ready
pub fn spawn_airports(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    aviation_data: Res<AviationData>,
    render_state: Res<AirportRenderState>,
    tile_settings: Res<SlippyTilesSettings>,
    existing_airports: Query<Entity, With<AirportMarker>>,
) {
    // Only run when data is ready and no airports exist yet
    if aviation_data.loading_state != LoadingState::Ready {
        return;
    }
    if !existing_airports.is_empty() {
        return;
    }
    if !render_state.show_airports {
        return;
    }

    info!("Spawning airport markers...");

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        tile_settings.zoom_level,
    );

    let mut count = 0;
    for airport in &aviation_data.airports {
        if !airport.passes_filter(render_state.filter) {
            continue;
        }

        let airport_ll = LatitudeLongitudeCoordinates {
            latitude: airport.latitude_deg,
            longitude: airport.longitude_deg,
        };
        let airport_pixel = world_coords_to_world_pixel(
            &airport_ll,
            TileSize::Normal,
            tile_settings.zoom_level,
        );

        let x = (airport_pixel.0 - reference_pixel.0) as f32;
        let y = -(airport_pixel.1 - reference_pixel.1) as f32;

        // Create airport marker (small square)
        let mesh = meshes.add(Rectangle::new(6.0, 6.0));
        let material = materials.add(ColorMaterial::from_color(airport.color()));

        commands.spawn((
            AirportMarker {
                airport_id: airport.id,
            },
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(x, y, AIRPORT_Z_LAYER),
            Visibility::Inherited,
        ));

        count += 1;
    }

    info!("Spawned {} airport markers", count);
}

/// System to update airport positions when map moves
pub fn update_airport_positions(
    tile_settings: Res<SlippyTilesSettings>,
    aviation_data: Res<AviationData>,
    mut airport_query: Query<(&AirportMarker, &mut Transform)>,
) {
    if aviation_data.loading_state != LoadingState::Ready {
        return;
    }

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        tile_settings.zoom_level,
    );

    // Build a lookup map for airports
    let airport_map: std::collections::HashMap<i64, &Airport> = aviation_data
        .airports
        .iter()
        .map(|a| (a.id, a))
        .collect();

    for (marker, mut transform) in airport_query.iter_mut() {
        if let Some(airport) = airport_map.get(&marker.airport_id) {
            let airport_ll = LatitudeLongitudeCoordinates {
                latitude: airport.latitude_deg,
                longitude: airport.longitude_deg,
            };
            let airport_pixel = world_coords_to_world_pixel(
                &airport_ll,
                TileSize::Normal,
                tile_settings.zoom_level,
            );

            transform.translation.x = (airport_pixel.0 - reference_pixel.0) as f32;
            transform.translation.y = -(airport_pixel.1 - reference_pixel.1) as f32;
        }
    }
}

/// System to toggle airport visibility based on zoom level
pub fn update_airport_visibility(
    tile_settings: Res<SlippyTilesSettings>,
    render_state: Res<AirportRenderState>,
    mut airport_query: Query<&mut Visibility, With<AirportMarker>>,
) {
    let zoom: u8 = tile_settings.zoom_level.into();
    let should_show = render_state.show_airports && zoom >= 6;

    for mut visibility in airport_query.iter_mut() {
        *visibility = if should_show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
```

**Step 2: Update aviation/mod.rs**

Add to `src/aviation/mod.rs`:
```rust
pub mod types;
pub mod loader;
pub mod airports;

pub use types::*;
pub use loader::*;
pub use airports::*;
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/aviation/
git commit -m "Add airport rendering system"
```

---

### Task 6: Create Runway Rendering System

**Files:**
- Create: `src/aviation/runways.rs`
- Modify: `src/aviation/mod.rs`

**Step 1: Create runway rendering system**

Create `src/aviation/runways.rs`:
```rust
use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{AviationData, LoadingState, Runway};

/// Component marking a runway entity
#[derive(Component)]
pub struct RunwayMarker {
    pub runway_id: i64,
    pub airport_ref: i64,
}

/// Resource for runway rendering state
#[derive(Resource, Default)]
pub struct RunwayRenderState {
    pub show_runways: bool,
}

impl Default for RunwayRenderState {
    fn default() -> Self {
        Self { show_runways: true }
    }
}

const RUNWAY_Z_LAYER: f32 = 4.0;
const RUNWAY_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.7);

/// System to render runways using Gizmos
pub fn draw_runways(
    mut gizmos: Gizmos,
    aviation_data: Res<AviationData>,
    render_state: Res<RunwayRenderState>,
    tile_settings: Res<SlippyTilesSettings>,
) {
    if aviation_data.loading_state != LoadingState::Ready {
        return;
    }
    if !render_state.show_runways {
        return;
    }

    // Only show runways at zoom 8+
    let zoom: u8 = tile_settings.zoom_level.into();
    if zoom < 8 {
        return;
    }

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        tile_settings.zoom_level,
    );

    for runway in &aviation_data.runways {
        if !runway.has_valid_coords() || runway.is_closed() {
            continue;
        }

        let le_lat = runway.le_latitude_deg.unwrap();
        let le_lon = runway.le_longitude_deg.unwrap();
        let he_lat = runway.he_latitude_deg.unwrap();
        let he_lon = runway.he_longitude_deg.unwrap();

        // Convert LE end to screen coordinates
        let le_ll = LatitudeLongitudeCoordinates {
            latitude: le_lat,
            longitude: le_lon,
        };
        let le_pixel = world_coords_to_world_pixel(
            &le_ll,
            TileSize::Normal,
            tile_settings.zoom_level,
        );

        // Convert HE end to screen coordinates
        let he_ll = LatitudeLongitudeCoordinates {
            latitude: he_lat,
            longitude: he_lon,
        };
        let he_pixel = world_coords_to_world_pixel(
            &he_ll,
            TileSize::Normal,
            tile_settings.zoom_level,
        );

        let start = Vec2::new(
            (le_pixel.0 - reference_pixel.0) as f32,
            -(le_pixel.1 - reference_pixel.1) as f32,
        );
        let end = Vec2::new(
            (he_pixel.0 - reference_pixel.0) as f32,
            -(he_pixel.1 - reference_pixel.1) as f32,
        );

        gizmos.line_2d(start, end, RUNWAY_COLOR);
    }
}
```

**Step 2: Update aviation/mod.rs**

```rust
pub mod types;
pub mod loader;
pub mod airports;
pub mod runways;

pub use types::*;
pub use loader::*;
pub use airports::*;
pub use runways::*;
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/aviation/
git commit -m "Add runway rendering system using Gizmos"
```

---

### Task 7: Create Navaid Rendering System

**Files:**
- Create: `src/aviation/navaids.rs`
- Modify: `src/aviation/mod.rs`

**Step 1: Create navaid rendering system**

Create `src/aviation/navaids.rs`:
```rust
use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{AviationData, LoadingState, Navaid, NavaidType};

/// Component marking a navaid entity
#[derive(Component)]
pub struct NavaidMarker {
    pub navaid_id: i64,
}

/// Resource for navaid rendering state
#[derive(Resource)]
pub struct NavaidRenderState {
    pub show_navaids: bool,
}

impl Default for NavaidRenderState {
    fn default() -> Self {
        Self { show_navaids: false } // Off by default
    }
}

const NAVAID_Z_LAYER: f32 = 5.5;

/// System to render navaids using Gizmos
pub fn draw_navaids(
    mut gizmos: Gizmos,
    aviation_data: Res<AviationData>,
    render_state: Res<NavaidRenderState>,
    tile_settings: Res<SlippyTilesSettings>,
) {
    if aviation_data.loading_state != LoadingState::Ready {
        return;
    }
    if !render_state.show_navaids {
        return;
    }

    // Only show navaids at zoom 7+
    let zoom: u8 = tile_settings.zoom_level.into();
    if zoom < 7 {
        return;
    }

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        tile_settings.zoom_level,
    );

    for navaid in &aviation_data.navaids {
        let navaid_ll = LatitudeLongitudeCoordinates {
            latitude: navaid.latitude_deg,
            longitude: navaid.longitude_deg,
        };
        let navaid_pixel = world_coords_to_world_pixel(
            &navaid_ll,
            TileSize::Normal,
            tile_settings.zoom_level,
        );

        let pos = Vec2::new(
            (navaid_pixel.0 - reference_pixel.0) as f32,
            -(navaid_pixel.1 - reference_pixel.1) as f32,
        );

        let color = navaid.color();
        let size = 4.0;

        match navaid.navaid_type {
            NavaidType::Vor | NavaidType::VorDme | NavaidType::Vortac => {
                // Draw circle for VOR-type navaids
                gizmos.circle_2d(pos, size, color);
                // Draw radial lines
                for angle in [0.0, 90.0, 180.0, 270.0] {
                    let rad = angle.to_radians();
                    let end = pos + Vec2::new(rad.cos(), rad.sin()) * (size + 3.0);
                    gizmos.line_2d(pos, end, color);
                }
            }
            NavaidType::Ndb | NavaidType::NdbDme => {
                // Draw diamond for NDB
                let points = [
                    pos + Vec2::new(0.0, size),
                    pos + Vec2::new(size, 0.0),
                    pos + Vec2::new(0.0, -size),
                    pos + Vec2::new(-size, 0.0),
                    pos + Vec2::new(0.0, size),
                ];
                for i in 0..4 {
                    gizmos.line_2d(points[i], points[i + 1], color);
                }
            }
            NavaidType::Dme | NavaidType::Tacan => {
                // Draw square for DME/TACAN
                let half = size / 2.0;
                let corners = [
                    pos + Vec2::new(-half, -half),
                    pos + Vec2::new(half, -half),
                    pos + Vec2::new(half, half),
                    pos + Vec2::new(-half, half),
                    pos + Vec2::new(-half, -half),
                ];
                for i in 0..4 {
                    gizmos.line_2d(corners[i], corners[i + 1], color);
                }
            }
            NavaidType::Unknown => {
                // Simple dot
                gizmos.circle_2d(pos, 2.0, color);
            }
        }
    }
}
```

**Step 2: Update aviation/mod.rs**

```rust
pub mod types;
pub mod loader;
pub mod airports;
pub mod runways;
pub mod navaids;

pub use types::*;
pub use loader::*;
pub use airports::*;
pub use runways::*;
pub use navaids::*;
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/aviation/
git commit -m "Add navaid rendering system using Gizmos"
```

---

### Task 8: Create Aviation Plugin and Integrate

**Files:**
- Create: `src/aviation/plugin.rs`
- Modify: `src/aviation/mod.rs`
- Modify: `src/main.rs`

**Step 1: Create aviation plugin**

Create `src/aviation/plugin.rs`:
```rust
use bevy::prelude::*;

use super::{
    AviationData, AirportRenderState, RunwayRenderState, NavaidRenderState,
    spawn_airports, update_airport_positions, update_airport_visibility,
    draw_runways, draw_navaids, start_aviation_data_loading,
};

pub struct AviationPlugin;

impl Plugin for AviationPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<AviationData>()
            .init_resource::<AirportRenderState>()
            .init_resource::<RunwayRenderState>()
            .init_resource::<NavaidRenderState>()
            .add_systems(Startup, start_aviation_data_loading)
            .add_systems(Update, (
                spawn_airports,
                update_airport_positions,
                update_airport_visibility,
                draw_runways,
                draw_navaids,
            ));
    }
}
```

**Step 2: Update aviation/mod.rs**

```rust
pub mod types;
pub mod loader;
pub mod airports;
pub mod runways;
pub mod navaids;
pub mod plugin;

pub use types::*;
pub use loader::*;
pub use airports::*;
pub use runways::*;
pub use navaids::*;
pub use plugin::*;
```

**Step 3: Add AviationPlugin to main.rs**

In `src/main.rs`, find the `App::new()` builder and add the plugin. After `ConfigPlugin`, add:
```rust
.add_plugins(aviation::AviationPlugin)
```

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 5: Run and test**

Run: `cargo run`
Expected: Application starts, downloads aviation data, shows airport markers

**Step 6: Commit**

```bash
git add src/aviation/ src/main.rs
git commit -m "Add AviationPlugin and integrate into main app"
```

---

## Phase 3: Flight Trails

### Task 9: Create Trail History Component

**Files:**
- Create: `src/aircraft/mod.rs`
- Create: `src/aircraft/trails.rs`
- Modify: `src/main.rs`

**Step 1: Create aircraft module**

Create `src/aircraft/mod.rs`:
```rust
pub mod trails;

pub use trails::*;
```

**Step 2: Create trails module**

Create `src/aircraft/trails.rs`:
```rust
use bevy::prelude::*;
use std::collections::VecDeque;
use std::time::Instant;

/// A single point in the trail history
#[derive(Clone, Debug)]
pub struct TrailPoint {
    pub lat: f64,
    pub lon: f64,
    pub altitude: Option<i32>,
    pub timestamp: Instant,
}

/// Component storing trail history for an aircraft
#[derive(Component, Default)]
pub struct TrailHistory {
    pub points: VecDeque<TrailPoint>,
}

/// Resource for trail configuration
#[derive(Resource)]
pub struct TrailConfig {
    pub enabled: bool,
    pub max_age_seconds: u64,
    pub solid_duration_seconds: u64,
    pub fade_duration_seconds: u64,
}

impl Default for TrailConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_age_seconds: 300,
            solid_duration_seconds: 225,
            fade_duration_seconds: 75,
        }
    }
}

impl TrailHistory {
    /// Add a new point to the trail
    pub fn add_point(&mut self, lat: f64, lon: f64, altitude: Option<i32>) {
        self.points.push_back(TrailPoint {
            lat,
            lon,
            altitude,
            timestamp: Instant::now(),
        });
    }

    /// Remove points older than max_age
    pub fn prune(&mut self, max_age_seconds: u64) {
        let cutoff = Instant::now() - std::time::Duration::from_secs(max_age_seconds);
        while let Some(front) = self.points.front() {
            if front.timestamp < cutoff {
                self.points.pop_front();
            } else {
                break;
            }
        }
    }
}

/// Get color for altitude (cyan at low, purple at high)
pub fn altitude_color(altitude: Option<i32>) -> Color {
    let alt = altitude.unwrap_or(0).max(0) as f32;

    // Altitude ranges: 0-10k cyan, 10k-20k green, 20k-30k yellow, 30k-40k orange, 40k+ purple
    let t = (alt / 40000.0).clamp(0.0, 1.0);

    if t < 0.25 {
        // Cyan to green
        let s = t / 0.25;
        Color::srgb(0.0, 1.0 - s * 0.5, 1.0 - s)
    } else if t < 0.5 {
        // Green to yellow
        let s = (t - 0.25) / 0.25;
        Color::srgb(s, 0.5 + s * 0.5, 0.0)
    } else if t < 0.75 {
        // Yellow to orange
        let s = (t - 0.5) / 0.25;
        Color::srgb(1.0, 1.0 - s * 0.4, 0.0)
    } else {
        // Orange to purple
        let s = (t - 0.75) / 0.25;
        Color::srgb(1.0 - s * 0.2, 0.6 - s * 0.6, s)
    }
}

/// Calculate opacity based on age
pub fn age_opacity(timestamp: Instant, solid_secs: u64, fade_secs: u64) -> f32 {
    let age = timestamp.elapsed().as_secs_f32();
    let solid = solid_secs as f32;
    let fade = fade_secs as f32;

    if age < solid {
        1.0
    } else if age < solid + fade {
        1.0 - (age - solid) / fade
    } else {
        0.0
    }
}
```

**Step 3: Add mod declaration to main.rs**

After `mod aviation;`, add:
```rust
mod aircraft;
```

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/aircraft/ src/main.rs
git commit -m "Add trail history component and altitude coloring"
```

---

### Task 10: Sync Trail History from ADS-B Data

**Files:**
- Modify: `src/main.rs` (sync_aircraft_from_adsb function)

**Step 1: Update Aircraft component**

Find the `Aircraft` struct in `src/main.rs` and add TrailHistory. Update the spawn code in `sync_aircraft_from_adsb` to:

1. Add `TrailHistory` component when spawning new aircraft
2. Update trail history when syncing existing aircraft

Find `sync_aircraft_from_adsb` and modify the spawn code to include TrailHistory:

After spawning the aircraft mesh, add:
```rust
aircraft::TrailHistory::default(),
```

And in the update loop, add trail point synchronization:
```rust
// After updating aircraft fields, sync trail history
if let Ok(mut trail) = trail_query.get_mut(entity) {
    // Copy position history from ADS-B data
    trail.points.clear();
    for point in &adsb_aircraft_data.position_history {
        trail.points.push_back(aircraft::TrailPoint {
            lat: point.lat,
            lon: point.lon,
            altitude: point.altitude,
            timestamp: std::time::Instant::now() -
                (chrono::Utc::now() - point.timestamp).to_std().unwrap_or_default(),
        });
    }
}
```

**Step 2: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Sync trail history from ADS-B position data"
```

---

### Task 11: Render Flight Trails

**Files:**
- Create: `src/aircraft/trail_renderer.rs`
- Modify: `src/aircraft/mod.rs`

**Step 1: Create trail renderer**

Create `src/aircraft/trail_renderer.rs`:
```rust
use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{TrailHistory, TrailConfig, altitude_color, age_opacity};

/// System to draw flight trails using Gizmos
pub fn draw_trails(
    mut gizmos: Gizmos,
    config: Res<TrailConfig>,
    tile_settings: Res<SlippyTilesSettings>,
    trail_query: Query<&TrailHistory>,
) {
    if !config.enabled {
        return;
    }

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        tile_settings.zoom_level,
    );

    for trail in trail_query.iter() {
        if trail.points.len() < 2 {
            continue;
        }

        let mut prev_pos: Option<Vec2> = None;
        let mut prev_color: Option<Color> = None;

        for point in trail.points.iter() {
            let opacity = age_opacity(
                point.timestamp,
                config.solid_duration_seconds,
                config.fade_duration_seconds,
            );

            if opacity <= 0.0 {
                prev_pos = None;
                continue;
            }

            let point_ll = LatitudeLongitudeCoordinates {
                latitude: point.lat,
                longitude: point.lon,
            };
            let point_pixel = world_coords_to_world_pixel(
                &point_ll,
                TileSize::Normal,
                tile_settings.zoom_level,
            );

            let pos = Vec2::new(
                (point_pixel.0 - reference_pixel.0) as f32,
                -(point_pixel.1 - reference_pixel.1) as f32,
            );

            let base_color = altitude_color(point.altitude);
            let color = base_color.with_alpha(opacity);

            if let Some(prev) = prev_pos {
                // Use gradient between previous and current color
                let draw_color = prev_color.unwrap_or(color);
                gizmos.line_2d(prev, pos, draw_color);
            }

            prev_pos = Some(pos);
            prev_color = Some(color);
        }
    }
}

/// System to prune old trail points
pub fn prune_trails(
    config: Res<TrailConfig>,
    mut trail_query: Query<&mut TrailHistory>,
) {
    for mut trail in trail_query.iter_mut() {
        trail.prune(config.max_age_seconds);
    }
}
```

**Step 2: Update aircraft/mod.rs**

```rust
pub mod trails;
pub mod trail_renderer;

pub use trails::*;
pub use trail_renderer::*;
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/aircraft/
git commit -m "Add trail rendering system with altitude coloring"
```

---

### Task 12: Create Aircraft Plugin

**Files:**
- Create: `src/aircraft/plugin.rs`
- Modify: `src/aircraft/mod.rs`
- Modify: `src/main.rs`

**Step 1: Create aircraft plugin**

Create `src/aircraft/plugin.rs`:
```rust
use bevy::prelude::*;

use super::{TrailConfig, draw_trails, prune_trails};

pub struct AircraftPlugin;

impl Plugin for AircraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<TrailConfig>()
            .add_systems(Update, (
                draw_trails,
                prune_trails,
            ));
    }
}
```

**Step 2: Update aircraft/mod.rs**

```rust
pub mod trails;
pub mod trail_renderer;
pub mod plugin;

pub use trails::*;
pub use trail_renderer::*;
pub use plugin::*;
```

**Step 3: Add AircraftPlugin to main.rs**

After `AviationPlugin`, add:
```rust
.add_plugins(aircraft::AircraftPlugin)
```

**Step 4: Build and test**

Run: `cargo build && cargo run`
Expected: Trails render behind aircraft

**Step 5: Commit**

```bash
git add src/aircraft/ src/main.rs
git commit -m "Add AircraftPlugin with trail rendering"
```

---

## Phase 4: Aircraft List Panel

### Task 13: Create Aircraft List State

**Files:**
- Create: `src/aircraft/list_panel.rs`
- Modify: `src/aircraft/mod.rs`

**Step 1: Create list panel module**

Create `src/aircraft/list_panel.rs`:
```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// Sort criteria for aircraft list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortCriteria {
    #[default]
    Distance,
    Altitude,
    Speed,
    Callsign,
}

impl SortCriteria {
    pub fn label(&self) -> &'static str {
        match self {
            SortCriteria::Distance => "Distance",
            SortCriteria::Altitude => "Altitude",
            SortCriteria::Speed => "Speed",
            SortCriteria::Callsign => "Callsign",
        }
    }
}

/// Filter settings for aircraft list
#[derive(Debug, Clone)]
pub struct AircraftFilters {
    pub min_altitude: i32,
    pub max_altitude: i32,
    pub min_speed: f64,
    pub max_speed: f64,
    pub max_distance: f64,
}

impl Default for AircraftFilters {
    fn default() -> Self {
        Self {
            min_altitude: 0,
            max_altitude: 60000,
            min_speed: 0.0,
            max_speed: 600.0,
            max_distance: 250.0,
        }
    }
}

/// State for the aircraft list panel
#[derive(Resource)]
pub struct AircraftListState {
    pub expanded: bool,
    pub width: f32,
    pub sort_by: SortCriteria,
    pub sort_ascending: bool,
    pub filters: AircraftFilters,
    pub search_text: String,
    pub selected_icao: Option<String>,
    pub show_filter_popup: bool,
}

impl Default for AircraftListState {
    fn default() -> Self {
        Self {
            expanded: true,
            width: 280.0,
            sort_by: SortCriteria::Distance,
            sort_ascending: true,
            filters: AircraftFilters::default(),
            search_text: String::new(),
            selected_icao: None,
            show_filter_popup: false,
        }
    }
}

/// Component to mark the aircraft list toggle button
#[derive(Component)]
pub struct AircraftListButton;
```

**Step 2: Update aircraft/mod.rs**

```rust
pub mod trails;
pub mod trail_renderer;
pub mod plugin;
pub mod list_panel;

pub use trails::*;
pub use trail_renderer::*;
pub use plugin::*;
pub use list_panel::*;
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/aircraft/
git commit -m "Add aircraft list panel state and types"
```

---

### Task 14: Create Aircraft List UI

**Files:**
- Modify: `src/aircraft/list_panel.rs`

**Step 1: Add egui rendering system**

Add to `src/aircraft/list_panel.rs`:
```rust
use crate::MapState;

/// Cached aircraft data for display
#[derive(Clone)]
pub struct AircraftDisplayData {
    pub icao: String,
    pub callsign: Option<String>,
    pub altitude: Option<i32>,
    pub velocity: Option<f64>,
    pub heading: Option<f32>,
    pub distance: f64,
}

/// Resource holding sorted/filtered aircraft for display
#[derive(Resource, Default)]
pub struct AircraftDisplayList {
    pub aircraft: Vec<AircraftDisplayData>,
}

/// System to render the aircraft list panel
pub fn render_aircraft_list_panel(
    mut contexts: EguiContexts,
    mut list_state: ResMut<AircraftListState>,
    display_list: Res<AircraftDisplayList>,
) {
    if !list_state.expanded {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::SidePanel::right("aircraft_list_panel")
        .default_width(list_state.width)
        .resizable(true)
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.heading(format!("Aircraft ({})", display_list.aircraft.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        list_state.expanded = false;
                    }
                });
            });

            ui.separator();

            // Sort dropdown
            ui.horizontal(|ui| {
                ui.label("Sort:");
                egui::ComboBox::from_id_salt("sort_by")
                    .selected_text(list_state.sort_by.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut list_state.sort_by, SortCriteria::Distance, "Distance");
                        ui.selectable_value(&mut list_state.sort_by, SortCriteria::Altitude, "Altitude");
                        ui.selectable_value(&mut list_state.sort_by, SortCriteria::Speed, "Speed");
                        ui.selectable_value(&mut list_state.sort_by, SortCriteria::Callsign, "Callsign");
                    });

                if ui.button(if list_state.sort_ascending { "↑" } else { "↓" }).clicked() {
                    list_state.sort_ascending = !list_state.sort_ascending;
                }

                if ui.button("Filter").clicked() {
                    list_state.show_filter_popup = !list_state.show_filter_popup;
                }
            });

            // Search box
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut list_state.search_text);
            });

            ui.separator();

            // Filter popup
            if list_state.show_filter_popup {
                ui.group(|ui| {
                    ui.label("Altitude (ft):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut list_state.filters.min_altitude)
                            .range(0..=60000)
                            .prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut list_state.filters.max_altitude)
                            .range(0..=60000)
                            .prefix("Max: "));
                    });

                    ui.label("Speed (kts):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut list_state.filters.min_speed)
                            .range(0.0..=600.0)
                            .prefix("Min: "));
                        ui.add(egui::DragValue::new(&mut list_state.filters.max_speed)
                            .range(0.0..=600.0)
                            .prefix("Max: "));
                    });

                    ui.label("Distance (nm):");
                    ui.add(egui::DragValue::new(&mut list_state.filters.max_distance)
                        .range(0.0..=500.0)
                        .prefix("Max: "));

                    if ui.button("Close").clicked() {
                        list_state.show_filter_popup = false;
                    }
                });
                ui.separator();
            }

            // Aircraft list
            egui::ScrollArea::vertical().show(ui, |ui| {
                for aircraft in &display_list.aircraft {
                    let is_selected = list_state.selected_icao.as_ref() == Some(&aircraft.icao);

                    let response = ui.selectable_label(
                        is_selected,
                        format!(
                            "{} {:>7} {:>5}nm",
                            aircraft.callsign.as_deref().unwrap_or(&aircraft.icao),
                            aircraft.altitude.map(|a| format!("{}ft", a)).unwrap_or_default(),
                            aircraft.distance as i32,
                        ),
                    );

                    if response.clicked() {
                        list_state.selected_icao = Some(aircraft.icao.clone());
                    }
                }
            });
        });
}

/// System to toggle aircraft list visibility
pub fn toggle_aircraft_list(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut list_state: ResMut<AircraftListState>,
) {
    if keyboard.just_pressed(KeyCode::KeyL) {
        list_state.expanded = !list_state.expanded;
    }
}
```

**Step 2: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src/aircraft/list_panel.rs
git commit -m "Add aircraft list panel UI rendering"
```

---

### Task 15: Populate Aircraft Display List

**Files:**
- Modify: `src/aircraft/list_panel.rs`
- Modify: `src/aircraft/plugin.rs`

**Step 1: Add display list population system**

Add to `src/aircraft/list_panel.rs`:
```rust
use crate::Aircraft;

/// Calculate distance between two lat/lon points in nautical miles
fn haversine_distance_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 3440.065; // Earth radius in nautical miles

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();

    r * c
}

/// System to populate and sort the aircraft display list
pub fn update_aircraft_display_list(
    map_state: Res<MapState>,
    list_state: Res<AircraftListState>,
    aircraft_query: Query<&Aircraft>,
    mut display_list: ResMut<AircraftDisplayList>,
) {
    let center_lat = map_state.latitude;
    let center_lon = map_state.longitude;
    let search = list_state.search_text.to_lowercase();

    // Collect and filter aircraft
    let mut aircraft: Vec<AircraftDisplayData> = aircraft_query
        .iter()
        .filter_map(|a| {
            let distance = haversine_distance_nm(center_lat, center_lon, a.latitude, a.longitude);

            // Apply filters
            if distance > list_state.filters.max_distance {
                return None;
            }

            if let Some(alt) = a.altitude {
                if alt < list_state.filters.min_altitude || alt > list_state.filters.max_altitude {
                    return None;
                }
            }

            if let Some(vel) = a.velocity {
                if vel < list_state.filters.min_speed || vel > list_state.filters.max_speed {
                    return None;
                }
            }

            // Apply search filter
            if !search.is_empty() {
                let callsign_match = a.callsign.as_ref()
                    .map(|c| c.to_lowercase().contains(&search))
                    .unwrap_or(false);
                let icao_match = a.icao.to_lowercase().contains(&search);
                if !callsign_match && !icao_match {
                    return None;
                }
            }

            Some(AircraftDisplayData {
                icao: a.icao.clone(),
                callsign: a.callsign.clone(),
                altitude: a.altitude,
                velocity: a.velocity,
                heading: a.heading,
                distance,
            })
        })
        .collect();

    // Sort
    match list_state.sort_by {
        SortCriteria::Distance => {
            aircraft.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        }
        SortCriteria::Altitude => {
            aircraft.sort_by(|a, b| {
                a.altitude.unwrap_or(0).cmp(&b.altitude.unwrap_or(0))
            });
        }
        SortCriteria::Speed => {
            aircraft.sort_by(|a, b| {
                a.velocity.unwrap_or(0.0).partial_cmp(&b.velocity.unwrap_or(0.0)).unwrap()
            });
        }
        SortCriteria::Callsign => {
            aircraft.sort_by(|a, b| {
                let a_call = a.callsign.as_deref().unwrap_or(&a.icao);
                let b_call = b.callsign.as_deref().unwrap_or(&b.icao);
                a_call.cmp(b_call)
            });
        }
    }

    if !list_state.sort_ascending {
        aircraft.reverse();
    }

    display_list.aircraft = aircraft;
}
```

**Step 2: Update aircraft plugin**

Update `src/aircraft/plugin.rs`:
```rust
use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

use super::{
    TrailConfig, draw_trails, prune_trails,
    AircraftListState, AircraftDisplayList,
    render_aircraft_list_panel, toggle_aircraft_list, update_aircraft_display_list,
};

pub struct AircraftPlugin;

impl Plugin for AircraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<TrailConfig>()
            .init_resource::<AircraftListState>()
            .init_resource::<AircraftDisplayList>()
            .add_systems(Update, (
                draw_trails,
                prune_trails,
                toggle_aircraft_list,
                update_aircraft_display_list,
            ))
            .add_systems(EguiPrimaryContextPass, render_aircraft_list_panel);
    }
}
```

**Step 3: Build and test**

Run: `cargo build && cargo run`
Expected: Aircraft list panel shows on right side, press L to toggle

**Step 4: Commit**

```bash
git add src/aircraft/
git commit -m "Add aircraft list filtering, sorting, and display"
```

---

### Task 16: Add Selection Highlighting

**Files:**
- Modify: `src/main.rs` (update_aircraft_positions or similar)

**Step 1: Add selection visual feedback**

In the aircraft rendering code, check if aircraft is selected and apply different color:

```rust
// In update_aircraft_positions or similar system
if let Some(selected) = &list_state.selected_icao {
    if aircraft.icao == *selected {
        // Apply selection highlight (yellow ring or different color)
        // This depends on how markers are rendered
    }
}
```

**Step 2: Add click-to-center functionality**

When an aircraft is selected in the list, optionally center the map on it.

**Step 3: Build and test**

Run: `cargo build && cargo run`
Expected: Clicking aircraft in list highlights it on map

**Step 4: Commit**

```bash
git add src/main.rs src/aircraft/
git commit -m "Add aircraft selection highlighting"
```

---

### Task 17: Add Overlay Settings to Config Panel

**Files:**
- Modify: `src/config.rs`

**Step 1: Extend AppConfig**

Add overlay settings to `AppConfig`:
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OverlayConfig {
    pub show_airports: bool,
    pub show_runways: bool,
    pub show_navaids: bool,
    pub airport_filter: String, // "All", "FrequentlyUsed", "MajorOnly"
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrailsConfig {
    pub enabled: bool,
    pub max_age_seconds: u64,
    pub solid_duration_seconds: u64,
    pub fade_duration_seconds: u64,
}
```

**Step 2: Add to settings panel UI**

Add collapsible sections for Overlays and Trails in `render_settings_panel`.

**Step 3: Build and test**

Run: `cargo build && cargo run`
Expected: Settings panel includes overlay and trail toggles

**Step 4: Commit**

```bash
git add src/config.rs
git commit -m "Add overlay and trail settings to config panel"
```

---

### Task 18: Final Integration and Polish

**Files:**
- Modify: `src/main.rs`
- Modify: Various

**Step 1: Ensure all systems are properly ordered**

Review system ordering to prevent race conditions.

**Step 2: Add UI button for aircraft list toggle**

Add a button in the main UI to toggle the aircraft list panel.

**Step 3: Test full integration**

Run: `cargo run`
Expected:
- Airports appear on map at zoom 6+
- Runways appear at zoom 8+
- Navaids appear at zoom 7+ (when enabled)
- Trails render behind aircraft with altitude coloring
- Aircraft list shows, filters, and sorts correctly
- Settings persist across restarts

**Step 4: Final commit**

```bash
git add .
git commit -m "Complete desktop feature migration: trails, aircraft list, aviation overlays"
```

---

## Summary

| Phase | Tasks | Estimated Commits |
|-------|-------|-------------------|
| 1: Data Infrastructure | 1-4 | 4 |
| 2: Aviation Overlays | 5-8 | 4 |
| 3: Flight Trails | 9-12 | 4 |
| 4: Aircraft List Panel | 13-18 | 6 |
| **Total** | **18** | **18** |

Each task is a discrete unit of work with clear verification steps and commit points.
