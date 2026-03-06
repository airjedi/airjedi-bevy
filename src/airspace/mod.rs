//! Airspace Boundaries Module
//!
//! Displays Class B/C/D airspace and restricted areas.
//! This module provides data structures and stubs for airspace visualization.
//!
//! ## Data Sources (for future implementation)
//! - FAA NASR (National Airspace System Resources)
//! - OpenAIP (open aviation database)
//! - FAA SUA (Special Use Airspace)
//! - VATSIM data (for virtual airspace)
//!
//! ## Implementation Notes
//! Airspace data is typically distributed in various formats:
//! - OpenAIP uses XML format
//! - FAA NASR uses various fixed-width and CSV formats
//! - GeoJSON is common for web applications
//!
//! The approach here defines the data structures needed, with stubs
//! for loading and rendering that can be filled in when data integration
//! is implemented.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::message::MessageReader;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use serde::{Deserialize, Serialize};

use crate::data_ingest::canonical::CanonicalRecord;
use crate::geo::CoordinateConverter;
use crate::render_layers::RenderCategory;
use crate::view3d::View3DState;

/// Classification of airspace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AirspaceClass {
    /// Class A airspace (18,000 MSL to FL600, requires IFR)
    ClassA,
    /// Class B airspace (around major airports)
    ClassB,
    /// Class C airspace (around medium airports with radar)
    ClassC,
    /// Class D airspace (around airports with control tower)
    ClassD,
    /// Class E airspace (controlled airspace, various altitudes)
    ClassE,
    /// Class G airspace (uncontrolled)
    #[default]
    ClassG,
    /// Restricted area (R-xxxx)
    Restricted,
    /// Prohibited area (P-xxxx)
    Prohibited,
    /// Warning area (W-xxxx)
    Warning,
    /// Military Operations Area (MOA)
    MOA,
    /// Alert area (A-xxxx)
    Alert,
    /// Temporary Flight Restriction
    TFR,
}

impl AirspaceClass {
    /// Get display name for this class
    pub fn display_name(&self) -> &'static str {
        match self {
            AirspaceClass::ClassA => "Class A",
            AirspaceClass::ClassB => "Class B",
            AirspaceClass::ClassC => "Class C",
            AirspaceClass::ClassD => "Class D",
            AirspaceClass::ClassE => "Class E",
            AirspaceClass::ClassG => "Class G",
            AirspaceClass::Restricted => "Restricted",
            AirspaceClass::Prohibited => "Prohibited",
            AirspaceClass::Warning => "Warning",
            AirspaceClass::MOA => "MOA",
            AirspaceClass::Alert => "Alert",
            AirspaceClass::TFR => "TFR",
        }
    }

    /// Get color for rendering this airspace type
    pub fn color(&self) -> Color {
        match self {
            AirspaceClass::ClassA => Color::srgba(1.0, 1.0, 1.0, 0.1), // White, subtle
            AirspaceClass::ClassB => Color::srgba(0.0, 0.4, 1.0, 0.3), // Blue
            AirspaceClass::ClassC => Color::srgba(0.5, 0.0, 0.5, 0.3), // Magenta
            AirspaceClass::ClassD => Color::srgba(0.0, 0.0, 1.0, 0.3), // Blue (dashed in real charts)
            AirspaceClass::ClassE => Color::srgba(0.5, 0.0, 0.5, 0.2), // Light magenta
            AirspaceClass::ClassG => Color::srgba(0.5, 0.5, 0.5, 0.1), // Gray
            AirspaceClass::Restricted => Color::srgba(1.0, 0.0, 0.0, 0.3), // Red
            AirspaceClass::Prohibited => Color::srgba(0.8, 0.0, 0.0, 0.4), // Dark red
            AirspaceClass::Warning => Color::srgba(1.0, 0.5, 0.0, 0.3), // Orange
            AirspaceClass::MOA => Color::srgba(0.6, 0.3, 0.0, 0.2), // Brown
            AirspaceClass::Alert => Color::srgba(1.0, 1.0, 0.0, 0.2), // Yellow
            AirspaceClass::TFR => Color::srgba(1.0, 0.0, 0.0, 0.4), // Red, prominent
        }
    }
}

/// A point defining part of an airspace boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirspacePoint {
    pub latitude: f64,
    pub longitude: f64,
}

/// Altitude specification for airspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AltitudeReference {
    /// Mean Sea Level (feet)
    MSL(i32),
    /// Above Ground Level (feet)
    AGL(i32),
    /// Flight Level (hundreds of feet)
    FL(u16),
    /// Surface (ground level)
    Surface,
    /// Unlimited (no upper limit)
    Unlimited,
}

impl AltitudeReference {
    /// Get displayable string
    pub fn display(&self) -> String {
        match self {
            AltitudeReference::MSL(ft) => format!("{} MSL", ft),
            AltitudeReference::AGL(ft) => format!("{} AGL", ft),
            AltitudeReference::FL(fl) => format!("FL{}", fl),
            AltitudeReference::Surface => "SFC".to_string(),
            AltitudeReference::Unlimited => "UNL".to_string(),
        }
    }
}

/// Complete airspace definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Airspace {
    /// Unique identifier (e.g., "KSFO_B", "R-2508")
    pub id: String,
    /// Display name
    pub name: String,
    /// Airspace classification
    pub class: AirspaceClass,
    /// Lower altitude limit
    pub floor: AltitudeReference,
    /// Upper altitude limit
    pub ceiling: AltitudeReference,
    /// Polygon points defining the boundary
    pub boundary: Vec<AirspacePoint>,
    /// Optional: controlling agency/frequency
    pub controlling_agency: Option<String>,
    /// Optional: frequency for communications
    pub frequency: Option<String>,
    /// Optional: times of operation (e.g., "0600-2200 local")
    pub operating_times: Option<String>,
}

impl Airspace {
    /// Check if a point is inside this airspace (horizontally)
    pub fn contains_point(&self, lat: f64, lon: f64) -> bool {
        // Ray casting algorithm for point-in-polygon
        if self.boundary.len() < 3 {
            return false;
        }

        let mut inside = false;
        let n = self.boundary.len();
        let mut j = n - 1;

        for i in 0..n {
            let yi = self.boundary[i].latitude;
            let xi = self.boundary[i].longitude;
            let yj = self.boundary[j].latitude;
            let xj = self.boundary[j].longitude;

            if ((yi > lat) != (yj > lat)) && (lon < (xj - xi) * (lat - yi) / (yj - yi) + xi) {
                inside = !inside;
            }
            j = i;
        }

        inside
    }

    /// Get the centroid of the airspace for labeling
    pub fn centroid(&self) -> (f64, f64) {
        if self.boundary.is_empty() {
            return (0.0, 0.0);
        }
        let sum_lat: f64 = self.boundary.iter().map(|p| p.latitude).sum();
        let sum_lon: f64 = self.boundary.iter().map(|p| p.longitude).sum();
        let n = self.boundary.len() as f64;
        (sum_lat / n, sum_lon / n)
    }
}

/// Resource storing loaded airspace data
#[derive(Resource, Default)]
pub struct AirspaceData {
    /// All loaded airspace definitions
    pub airspaces: Vec<Airspace>,
    /// Data source (for display)
    pub source: Option<String>,
    /// Whether data has been loaded
    pub loaded: bool,
    /// Whether data has changed and meshes need regeneration
    pub dirty: bool,
}

impl AirspaceData {
    /// Load airspace data from a file
    ///
    /// TODO: Implement actual file parsing for:
    /// - OpenAIP XML format
    /// - GeoJSON format
    /// - FAA NASR format
    pub fn load_from_file(&mut self, _path: &std::path::Path) -> Result<(), String> {
        // Stub implementation
        warn!("Airspace loading not yet implemented");
        Err("Airspace loading not yet implemented. Data sources to consider: OpenAIP, FAA NASR".to_string())
    }

    /// Load sample airspace data for testing
    pub fn load_sample_data(&mut self) {
        // Sample Class B airspace around a fictional airport
        self.airspaces.push(Airspace {
            id: "SAMPLE_B".to_string(),
            name: "Sample Class B".to_string(),
            class: AirspaceClass::ClassB,
            floor: AltitudeReference::Surface,
            ceiling: AltitudeReference::FL(100),
            boundary: vec![
                AirspacePoint { latitude: 37.75, longitude: -97.40 },
                AirspacePoint { latitude: 37.75, longitude: -97.20 },
                AirspacePoint { latitude: 37.60, longitude: -97.20 },
                AirspacePoint { latitude: 37.60, longitude: -97.40 },
            ],
            controlling_agency: Some("Sample Approach".to_string()),
            frequency: Some("123.45".to_string()),
            operating_times: None,
        });

        // Sample restricted area
        self.airspaces.push(Airspace {
            id: "R-SAMPLE".to_string(),
            name: "Sample Restricted".to_string(),
            class: AirspaceClass::Restricted,
            floor: AltitudeReference::Surface,
            ceiling: AltitudeReference::FL(180),
            boundary: vec![
                AirspacePoint { latitude: 37.80, longitude: -97.50 },
                AirspacePoint { latitude: 37.80, longitude: -97.45 },
                AirspacePoint { latitude: 37.85, longitude: -97.45 },
                AirspacePoint { latitude: 37.85, longitude: -97.50 },
            ],
            controlling_agency: Some("Sample Military".to_string()),
            frequency: None,
            operating_times: Some("0800-1700 MON-FRI".to_string()),
        });

        self.loaded = true;
        self.dirty = true;
        self.source = Some("Sample Data".to_string());
        info!("Loaded {} sample airspace definitions", self.airspaces.len());
    }

    /// Find airspaces containing a point
    pub fn find_at_point(&self, lat: f64, lon: f64) -> Vec<&Airspace> {
        self.airspaces.iter().filter(|a| a.contains_point(lat, lon)).collect()
    }
}

/// Resource for airspace display settings
#[derive(Resource)]
pub struct AirspaceDisplayState {
    /// Whether to show airspace boundaries
    pub enabled: bool,
    /// Which classes to display
    pub show_class_b: bool,
    pub show_class_c: bool,
    pub show_class_d: bool,
    pub show_restricted: bool,
    pub show_moa: bool,
    pub show_tfr: bool,
    pub show_warning: bool,
    pub show_alert: bool,
    /// Whether to show labels
    pub show_labels: bool,
    /// Opacity for airspace meshes (0.0 - 1.0)
    pub opacity: f32,
    /// Optional altitude filter in feet (only show airspaces at this altitude)
    pub altitude_filter_ft: Option<i32>,
}

impl Default for AirspaceDisplayState {
    fn default() -> Self {
        Self {
            enabled: false,
            show_class_b: true,
            show_class_c: true,
            show_class_d: true,
            show_restricted: true,
            show_moa: false,
            show_tfr: true,
            show_warning: true,
            show_alert: true,
            show_labels: true,
            opacity: 0.3,
            altitude_filter_ft: None,
        }
    }
}

impl AirspaceDisplayState {
    pub fn is_class_visible(&self, class: &AirspaceClass) -> bool {
        if !self.enabled {
            return false;
        }
        match class {
            AirspaceClass::ClassB => self.show_class_b,
            AirspaceClass::ClassC => self.show_class_c,
            AirspaceClass::ClassD => self.show_class_d,
            AirspaceClass::Restricted => self.show_restricted,
            AirspaceClass::MOA => self.show_moa,
            AirspaceClass::Warning => self.show_warning,
            AirspaceClass::Alert => self.show_alert,
            AirspaceClass::TFR => self.show_tfr,
            _ => false,
        }
    }

    pub fn passes_altitude_filter(&self, floor_ft: Option<i32>, ceiling_ft: Option<i32>) -> bool {
        let Some(filter) = self.altitude_filter_ft else {
            return true;
        };
        let floor = floor_ft.unwrap_or(0);
        let ceiling = ceiling_ft.unwrap_or(60000);
        filter >= floor && filter <= ceiling
    }
}

/// Component for airspace boundary entities
#[derive(Component)]
pub struct AirspaceBoundary {
    pub airspace_id: String,
}

/// Marker for spawned airspace mesh entities.
#[derive(Component)]
pub struct AirspaceMeshMarker {
    pub airspace_id: String,
}

/// Tracks which airspaces have spawned meshes.
#[derive(Resource, Default)]
pub struct SpawnedAirspaces {
    pub ids: std::collections::HashSet<String>,
}

/// Timer for periodic airspace mesh refresh.
#[derive(Resource)]
pub struct AirspaceRefreshTimer(pub Timer);

impl Default for AirspaceRefreshTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}

/// System to toggle airspace display
pub fn toggle_airspace_display(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut display_state: ResMut<AirspaceDisplayState>,
    mut contexts: EguiContexts,
) {
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() {
            return;
        }
    }

    // Shift+A - Toggle airspace boundaries
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        if keyboard.just_pressed(KeyCode::KeyA) {
            display_state.enabled = !display_state.enabled;
            info!("Airspace display: {}", if display_state.enabled { "enabled" } else { "disabled" });
        }
    }
}

/// System to render airspace settings panel
pub fn render_airspace_panel(
    mut contexts: EguiContexts,
    mut display_state: ResMut<AirspaceDisplayState>,
    mut airspace_data: ResMut<AirspaceData>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Only show panel when airspace is enabled
    if !display_state.enabled {
        return;
    }

    egui::Window::new("Airspace")
        .collapsible(true)
        .resizable(false)
        .default_width(200.0)
        .show(ctx, |ui| {
            if !airspace_data.loaded {
                ui.label("No airspace data loaded");
                ui.separator();

                if ui.button("Load Sample Data").clicked() {
                    airspace_data.load_sample_data();
                }

                ui.label(
                    egui::RichText::new("Note: Full implementation requires\nintegration with FAA/OpenAIP data")
                        .size(11.0)
                        .color(egui::Color32::GRAY)
                );
            } else {
                if let Some(ref source) = airspace_data.source {
                    ui.label(format!("Source: {}", source));
                }
                ui.label(format!("{} airspaces loaded", airspace_data.airspaces.len()));

                ui.separator();
                ui.label("Display Options:");

                ui.checkbox(&mut display_state.show_class_b, "Class B");
                ui.checkbox(&mut display_state.show_class_c, "Class C");
                ui.checkbox(&mut display_state.show_class_d, "Class D");
                ui.checkbox(&mut display_state.show_restricted, "Restricted");
                ui.checkbox(&mut display_state.show_moa, "MOA");
                ui.checkbox(&mut display_state.show_tfr, "TFR");

                ui.separator();
                ui.checkbox(&mut display_state.show_labels, "Show Labels");
            }
        });
}

/// Consume NavigationDataUpdated messages and load airspace records into AirspaceData.
pub fn consume_airspace_data(
    mut nav_events: MessageReader<crate::data_ingest::NavigationDataUpdated>,
    mut airspace_data: ResMut<AirspaceData>,
) {
    for event in nav_events.read() {
        let airspaces: Vec<Airspace> = event
            .records
            .iter()
            .filter_map(|r| {
                if let CanonicalRecord::Airspace(info) = r {
                    Some(airspace_info_to_airspace(info))
                } else {
                    None
                }
            })
            .collect();

        if !airspaces.is_empty() {
            info!("Loaded {} airspace definitions from data ingest", airspaces.len());
            airspace_data.airspaces = airspaces;
            airspace_data.loaded = true;
            airspace_data.dirty = true;
            airspace_data.source = Some("FAA ADDS".to_string());
        }
    }
}

fn airspace_info_to_airspace(info: &crate::data_ingest::canonical::AirspaceInfo) -> Airspace {
    let class = match info.airspace_class.as_str() {
        "ClassB" => AirspaceClass::ClassB,
        "ClassC" => AirspaceClass::ClassC,
        "ClassD" => AirspaceClass::ClassD,
        "Restricted" => AirspaceClass::Restricted,
        "MOA" => AirspaceClass::MOA,
        "Warning" => AirspaceClass::Warning,
        "Alert" => AirspaceClass::Alert,
        "TFR" => AirspaceClass::TFR,
        _ => AirspaceClass::ClassG,
    };

    let floor = parse_altitude_ref(info.lower_limit_ft, info.lower_altitude_ref.as_deref());
    let ceiling = parse_altitude_ref(info.upper_limit_ft, info.upper_altitude_ref.as_deref());

    Airspace {
        id: info.name.clone(),
        name: info.name.clone(),
        class,
        floor,
        ceiling,
        boundary: info
            .polygon
            .iter()
            .map(|(lat, lon)| AirspacePoint {
                latitude: *lat,
                longitude: *lon,
            })
            .collect(),
        controlling_agency: None,
        frequency: None,
        operating_times: None,
    }
}

fn parse_altitude_ref(ft: Option<i32>, code: Option<&str>) -> AltitudeReference {
    match (ft, code) {
        (_, Some("SFC")) => AltitudeReference::Surface,
        (_, Some("UNL")) => AltitudeReference::Unlimited,
        (Some(v), Some("AGL")) => AltitudeReference::AGL(v),
        (Some(v), Some("FL")) => AltitudeReference::FL(v as u16 / 100),
        (Some(v), _) => AltitudeReference::MSL(v),
        (None, _) => AltitudeReference::Surface,
    }
}

/// Build a wall mesh (vertical fence) from a polygon boundary between floor and ceiling.
fn build_wall_mesh(
    boundary: &[AirspacePoint],
    floor_z: f32,
    ceiling_z: f32,
    converter: &CoordinateConverter,
) -> Mesh {
    let n = boundary.len();
    if n < 2 {
        return Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    }

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 4);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n * 4);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(n * 6);

    for i in 0..n {
        let j = (i + 1) % n;

        let p0 = converter.latlon_to_world(boundary[i].latitude, boundary[i].longitude);
        let p1 = converter.latlon_to_world(boundary[j].latitude, boundary[j].longitude);

        // Y-up: x=east, y=altitude, z=-north
        let base_idx = positions.len() as u32;

        // Bottom-left, bottom-right, top-right, top-left
        positions.push([p0.x, floor_z, -p0.y]);
        positions.push([p1.x, floor_z, -p1.y]);
        positions.push([p1.x, ceiling_z, -p1.y]);
        positions.push([p0.x, ceiling_z, -p0.y]);

        // Normal pointing outward (cross product of edge x up)
        let edge = Vec3::new(p1.x - p0.x, 0.0, -(p1.y - p0.y));
        let up = Vec3::Y;
        let normal = edge.cross(up).normalize_or_zero();
        for _ in 0..4 {
            normals.push(normal.to_array());
        }

        uvs.push([0.0, 0.0]);
        uvs.push([1.0, 0.0]);
        uvs.push([1.0, 1.0]);
        uvs.push([0.0, 1.0]);

        // Two triangles per quad
        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);
        indices.push(base_idx);
        indices.push(base_idx + 2);
        indices.push(base_idx + 3);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Build a flat polygon mesh for 2D map rendering.
fn build_flat_polygon_mesh(
    boundary: &[AirspacePoint],
    converter: &CoordinateConverter,
) -> Mesh {
    let n = boundary.len();
    if n < 3 {
        return Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    }

    // Simple fan triangulation from centroid
    let positions_2d: Vec<Vec2> = boundary
        .iter()
        .map(|p| converter.latlon_to_world(p.latitude, p.longitude))
        .collect();

    let cx: f32 = positions_2d.iter().map(|p| p.x).sum::<f32>() / n as f32;
    let cy: f32 = positions_2d.iter().map(|p| p.y).sum::<f32>() / n as f32;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n + 1);
    let mut indices: Vec<u32> = Vec::with_capacity(n * 3);

    // Center vertex
    positions.push([cx, 0.01, -cy]); // slight Y offset above tiles
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for p in &positions_2d {
        positions.push([p.x, 0.01, -p.y]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.0, 0.0]);
    }

    for i in 0..n {
        let j = (i + 1) % n;
        indices.push(0); // center
        indices.push((i + 1) as u32);
        indices.push((j + 1) as u32);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn altitude_ref_to_ft(alt: &AltitudeReference) -> i32 {
    match alt {
        AltitudeReference::MSL(ft) => *ft,
        AltitudeReference::AGL(ft) => *ft,
        AltitudeReference::FL(fl) => *fl as i32 * 100,
        AltitudeReference::Surface => 0,
        AltitudeReference::Unlimited => 60000,
    }
}

/// Check if airspace centroid is within rendering range (~250nm).
fn is_in_range(airspace: &Airspace, camera_lat: f64, camera_lon: f64) -> bool {
    let (centroid_lat, centroid_lon) = airspace.centroid();
    let dlat = (centroid_lat - camera_lat).abs();
    let dlon = (centroid_lon - camera_lon).abs();
    // Rough degree-based check (~250nm = ~4 degrees at mid-latitudes)
    dlat < 4.0 && dlon < 4.0
}

/// System to spawn/despawn airspace meshes based on visibility and distance.
pub fn update_airspace_meshes(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<AirspaceRefreshTimer>,
    mut airspace_data: ResMut<AirspaceData>,
    display_state: Res<AirspaceDisplayState>,
    view3d_state: Option<Res<View3DState>>,
    map_state: Res<crate::map::MapState>,
    tile_settings: Res<bevy_slippy_tiles::SlippyTilesSettings>,
    mut spawned: ResMut<SpawnedAirspaces>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing_query: Query<(Entity, &AirspaceMeshMarker)>,
) {
    timer.0.tick(time.delta());
    let needs_refresh = timer.0.just_finished() || airspace_data.dirty;

    if !needs_refresh {
        return;
    }

    if airspace_data.dirty {
        airspace_data.dirty = false;
    }

    if !display_state.enabled || !airspace_data.loaded {
        // Despawn all if disabled
        for (entity, _) in existing_query.iter() {
            commands.entity(entity).despawn();
        }
        spawned.ids.clear();
        return;
    }

    let converter = CoordinateConverter::new(&tile_settings, map_state.zoom_level);
    let is_3d = view3d_state
        .as_ref()
        .is_some_and(|v| v.mode == crate::view3d::ViewMode::Perspective3D);

    let camera_lat = map_state.latitude;
    let camera_lon = map_state.longitude;

    // Despawn meshes for airspaces no longer visible
    for (entity, marker) in existing_query.iter() {
        let should_keep = airspace_data.airspaces.iter().any(|a| {
            a.id == marker.airspace_id
                && display_state.is_class_visible(&a.class)
                && is_in_range(a, camera_lat, camera_lon)
        });
        if !should_keep {
            commands.entity(entity).despawn();
            spawned.ids.remove(&marker.airspace_id);
        }
    }

    // Spawn meshes for newly visible airspaces
    for airspace in &airspace_data.airspaces {
        if spawned.ids.contains(&airspace.id) {
            continue;
        }
        if !display_state.is_class_visible(&airspace.class) {
            continue;
        }
        if !is_in_range(airspace, camera_lat, camera_lon) {
            continue;
        }

        let floor_ft = altitude_ref_to_ft(&airspace.floor);
        let ceiling_ft = altitude_ref_to_ft(&airspace.ceiling);

        if !display_state.passes_altitude_filter(Some(floor_ft), Some(ceiling_ft)) {
            continue;
        }

        let mut color = airspace.class.color();
        color.set_alpha(display_state.opacity);

        let material = materials.add(StandardMaterial {
            base_color: color,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            unlit: true,
            ..default()
        });

        let mesh = if is_3d {
            let v3d = view3d_state.as_ref().unwrap();
            let floor_z = v3d.altitude_to_z(floor_ft);
            let ceiling_z = v3d.altitude_to_z(ceiling_ft);
            build_wall_mesh(&airspace.boundary, floor_z, ceiling_z, &converter)
        } else {
            build_flat_polygon_mesh(&airspace.boundary, &converter)
        };

        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Visibility::default(),
            RenderLayers::layer(RenderCategory::AIRSPACE),
            AirspaceMeshMarker {
                airspace_id: airspace.id.clone(),
            },
        ));

        spawned.ids.insert(airspace.id.clone());
    }
}

/// Plugin for airspace functionality
pub struct AirspacePlugin;

impl Plugin for AirspacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AirspaceData>()
            .init_resource::<AirspaceDisplayState>()
            .init_resource::<SpawnedAirspaces>()
            .init_resource::<AirspaceRefreshTimer>()
            .add_systems(Update, (
                toggle_airspace_display,
                consume_airspace_data,
                update_airspace_meshes,
            ));
        // Airspace panel is rendered via the consolidated Tools window (tools_window.rs)
    }
}
