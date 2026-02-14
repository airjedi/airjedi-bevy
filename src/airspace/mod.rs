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

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use serde::{Deserialize, Serialize};

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
    /// Whether to show labels
    pub show_labels: bool,
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
            show_labels: true,
        }
    }
}

/// Component for airspace boundary entities
#[derive(Component)]
pub struct AirspaceBoundary {
    pub airspace_id: String,
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

/// Plugin for airspace functionality
pub struct AirspacePlugin;

impl Plugin for AirspacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AirspaceData>()
            .init_resource::<AirspaceDisplayState>()
            .add_systems(Update, toggle_airspace_display);
        // Airspace panel is rendered via the consolidated Tools window (tools_window.rs)
    }
}
