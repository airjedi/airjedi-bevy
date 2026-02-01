use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::MapState;

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
            expanded: false, // Start collapsed
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
    aircraft_query: Query<&crate::Aircraft>,
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
                ui.label("Search:");
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

/// System to highlight selected aircraft with a ring
pub fn highlight_selected_aircraft(
    mut gizmos: Gizmos,
    list_state: Res<AircraftListState>,
    tile_settings: Res<bevy_slippy_tiles::SlippyTilesSettings>,
    map_state: Res<MapState>,
    aircraft_query: Query<&crate::Aircraft>,
) {
    let Some(selected_icao) = &list_state.selected_icao else {
        return;
    };

    // Find the selected aircraft
    let Some(aircraft) = aircraft_query.iter().find(|a| &a.icao == selected_icao) else {
        return;
    };

    use bevy_slippy_tiles::*;

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        map_state.zoom_level,
    );

    let aircraft_ll = LatitudeLongitudeCoordinates {
        latitude: aircraft.latitude,
        longitude: aircraft.longitude,
    };
    let aircraft_pixel = world_coords_to_world_pixel(
        &aircraft_ll,
        TileSize::Normal,
        map_state.zoom_level,
    );

    let pos = Vec2::new(
        (aircraft_pixel.0 - reference_pixel.0) as f32,
        (aircraft_pixel.1 - reference_pixel.1) as f32,
    );

    // Draw selection ring (yellow)
    let color = Color::srgb(1.0, 1.0, 0.0);
    gizmos.circle_2d(pos, 15.0, color);
    gizmos.circle_2d(pos, 18.0, color);
}
