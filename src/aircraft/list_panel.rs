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
            width: 304.0, // Match desktop panel width
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
    pub vertical_rate: Option<i32>,
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
                vertical_rate: a.vertical_rate,
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

/// Helper function to get altitude color based on altitude value
fn get_altitude_color(altitude: Option<i32>) -> (egui::Color32, &'static str) {
    match altitude {
        Some(alt) if alt >= 30000 => (egui::Color32::from_rgb(200, 100, 255), "▲"), // High - purple
        Some(alt) if alt >= 20000 => (egui::Color32::from_rgb(255, 150, 50), "▲"),  // Medium-high - orange
        Some(alt) if alt >= 10000 => (egui::Color32::from_rgb(200, 200, 100), "▲"), // Medium - yellow
        Some(_) => (egui::Color32::from_rgb(100, 200, 200), "▼"),                   // Low - cyan
        None => (egui::Color32::from_rgb(100, 100, 100), "─"),                      // Unknown - grey
    }
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

    // Define colors matching desktop theme
    let panel_bg = egui::Color32::from_rgba_unmultiplied(25, 30, 35, 230);
    let border_color = egui::Color32::from_rgb(60, 80, 100);
    let selected_bg = egui::Color32::from_rgba_unmultiplied(100, 140, 180, 26);
    let header_color = egui::Color32::from_rgb(150, 150, 150);
    let icao_color = egui::Color32::from_rgb(200, 220, 255);
    let callsign_color = egui::Color32::from_rgb(150, 220, 150);
    let callsign_selected_color = egui::Color32::from_rgb(255, 50, 50);
    let metrics_color = egui::Color32::from_rgb(170, 170, 170);
    let range_color = egui::Color32::from_rgb(100, 200, 255);
    let status_active = egui::Color32::from_rgb(100, 255, 100);

    // Create custom frame for the panel
    let panel_frame = egui::Frame::default()
        .fill(panel_bg)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::same(8));

    egui::SidePanel::right("aircraft_list_panel")
        .default_width(list_state.width)
        .resizable(true)
        .frame(panel_frame)
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Aircraft ({})", display_list.aircraft.len()))
                    .color(egui::Color32::from_rgb(200, 200, 200))
                    .size(14.0)
                    .strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(egui::RichText::new("X").size(12.0)).clicked() {
                        list_state.expanded = false;
                    }
                });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Sort dropdown
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Sort:")
                    .color(header_color)
                    .size(10.0));
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
                ui.label(egui::RichText::new("Search:")
                    .color(header_color)
                    .size(10.0));
                ui.text_edit_singleline(&mut list_state.search_text);
            });

            ui.add_space(4.0);

            // Filter popup
            if list_state.show_filter_popup {
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(35, 40, 45, 230))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Altitude (ft):")
                            .color(header_color)
                            .size(9.0));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut list_state.filters.min_altitude)
                                .range(0..=60000)
                                .prefix("Min: "));
                            ui.add(egui::DragValue::new(&mut list_state.filters.max_altitude)
                                .range(0..=60000)
                                .prefix("Max: "));
                        });

                        ui.label(egui::RichText::new("Speed (kts):")
                            .color(header_color)
                            .size(9.0));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut list_state.filters.min_speed)
                                .range(0.0..=600.0)
                                .prefix("Min: "));
                            ui.add(egui::DragValue::new(&mut list_state.filters.max_speed)
                                .range(0.0..=600.0)
                                .prefix("Max: "));
                        });

                        ui.label(egui::RichText::new("Distance (nm):")
                            .color(header_color)
                            .size(9.0));
                        ui.add(egui::DragValue::new(&mut list_state.filters.max_distance)
                            .range(0.0..=500.0)
                            .prefix("Max: "));

                        if ui.button("Close").clicked() {
                            list_state.show_filter_popup = false;
                        }
                    });
                ui.add_space(4.0);
            }

            // Aircraft count
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("TOTAL: {}", display_list.aircraft.len()))
                    .color(header_color)
                    .size(10.0)
                    .monospace());
            });

            ui.add_space(4.0);

            // Aircraft list with 3-row card layout
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;

                    for aircraft in &display_list.aircraft {
                        let is_selected = list_state.selected_icao.as_ref() == Some(&aircraft.icao);
                        let (alt_color, alt_indicator) = get_altitude_color(aircraft.altitude);

                        // Create card frame with selection highlighting (no outline)
                        let card_frame = if is_selected {
                            egui::Frame::none()
                                .fill(selected_bg)
                                .inner_margin(egui::Margin::symmetric(4, 2))
                        } else {
                            egui::Frame::none()
                                .inner_margin(egui::Margin::symmetric(4, 2))
                        };

                        let card_response = card_frame.show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;

                            // Row 1: Status + ICAO + Callsign + Altitude
                            ui.horizontal(|ui| {
                                // Status indicator (active since we don't have timestamps)
                                ui.label(egui::RichText::new("●")
                                    .color(status_active)
                                    .size(11.0));

                                // ICAO
                                ui.label(egui::RichText::new(&aircraft.icao)
                                    .color(icao_color)
                                    .size(10.5)
                                    .monospace()
                                    .strong());

                                // Callsign
                                if let Some(ref callsign) = aircraft.callsign {
                                    let cs_color = if is_selected {
                                        callsign_selected_color
                                    } else {
                                        callsign_color
                                    };
                                    ui.label(egui::RichText::new(format!("│ {}", callsign.trim()))
                                        .color(cs_color)
                                        .size(10.5)
                                        .strong());
                                }

                                // Altitude with indicator
                                if let Some(alt) = aircraft.altitude {
                                    let alt_text = if alt >= 18000 {
                                        format!("│ {} FL{:03}", alt_indicator, alt / 100)
                                    } else {
                                        format!("│ {} {}", alt_indicator, alt)
                                    };
                                    ui.label(egui::RichText::new(alt_text)
                                        .color(alt_color)
                                        .size(9.5)
                                        .monospace());
                                }
                            });

                            // Row 2: Speed + Heading + Range
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                if let Some(vel) = aircraft.velocity {
                                    ui.label(egui::RichText::new(format!("{:03}kt", vel as i32))
                                        .color(metrics_color)
                                        .size(8.0)
                                        .monospace());
                                }

                                if let Some(heading) = aircraft.heading {
                                    ui.label(egui::RichText::new(format!("{:03}°", heading as i32))
                                        .color(metrics_color)
                                        .size(8.0)
                                        .monospace());
                                }

                                ui.label(egui::RichText::new(format!("{:.1}nm", aircraft.distance))
                                    .color(range_color)
                                    .size(8.0)
                                    .monospace());
                            });

                            // Row 3: Vertical rate (if available)
                            if let Some(vr) = aircraft.vertical_rate {
                                ui.horizontal(|ui| {
                                    let (vr_color, vr_symbol) = if vr > 100 {
                                        (egui::Color32::from_rgb(100, 255, 100), "↑")
                                    } else if vr < -100 {
                                        (egui::Color32::from_rgb(255, 150, 100), "↓")
                                    } else {
                                        (egui::Color32::from_rgb(150, 150, 150), "─")
                                    };
                                    ui.label(egui::RichText::new(format!("{} {}ft/min", vr_symbol, vr))
                                        .color(vr_color)
                                        .size(7.5)
                                        .monospace());
                                });
                            }
                        });

                        // Handle click to select
                        if card_response.response.interact(egui::Sense::click()).clicked() {
                            list_state.selected_icao = Some(aircraft.icao.clone());
                        }

                        // Subtle separator line between items
                        ui.add_space(2.0);
                        let separator_color = egui::Color32::from_rgb(50, 55, 65);
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            rect.x_range(),
                            rect.top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(2.0);
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
