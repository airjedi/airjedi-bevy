use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::time::Instant;

use super::{AircraftListState, TrailHistory};
use crate::{MapState, ZoomState};

/// State for the aircraft detail panel
#[derive(Resource, Default)]
pub struct DetailPanelState {
    pub open: bool,
    /// Timestamp when the selected aircraft was first tracked
    pub track_start: Option<Instant>,
}

/// Resource for camera follow state
#[derive(Resource, Default)]
pub struct CameraFollowState {
    /// ICAO of the aircraft being followed (camera locked to this aircraft)
    pub following_icao: Option<String>,
}

/// Cached data for the detail panel display
pub struct DetailDisplayData {
    pub icao: String,
    pub callsign: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<i32>,
    pub heading: Option<f32>,
    pub velocity: Option<f64>,
    pub vertical_rate: Option<i32>,
    pub distance_nm: f64,
    pub track_points: usize,
    pub track_duration_secs: Option<u64>,
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

/// System to render the aircraft detail panel
pub fn render_detail_panel(
    mut contexts: EguiContexts,
    list_state: Res<AircraftListState>,
    mut detail_state: ResMut<DetailPanelState>,
    mut follow_state: ResMut<CameraFollowState>,
    map_state: Res<MapState>,
    aircraft_query: Query<(&crate::Aircraft, &TrailHistory)>,
) {
    // Only show panel if an aircraft is selected and panel is open
    let Some(selected_icao) = &list_state.selected_icao else {
        detail_state.open = false;
        detail_state.track_start = None;
        return;
    };

    if !detail_state.open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Find the selected aircraft
    let Some((aircraft, trail)) = aircraft_query.iter().find(|(a, _)| &a.icao == selected_icao) else {
        // Aircraft no longer exists
        detail_state.open = false;
        detail_state.track_start = None;
        return;
    };

    // Calculate display data
    let distance_nm = haversine_distance_nm(
        map_state.latitude,
        map_state.longitude,
        aircraft.latitude,
        aircraft.longitude,
    );

    let track_duration = detail_state.track_start.map(|start| start.elapsed().as_secs());
    let oldest_point_age = trail.points.front().map(|p| p.timestamp.elapsed().as_secs());

    // Define colors
    let panel_bg = egui::Color32::from_rgba_unmultiplied(25, 30, 35, 240);
    let border_color = egui::Color32::from_rgb(60, 80, 100);
    let label_color = egui::Color32::from_rgb(150, 150, 150);
    let value_color = egui::Color32::from_rgb(220, 220, 220);
    let callsign_color = egui::Color32::from_rgb(150, 220, 150);
    let highlight_color = egui::Color32::from_rgb(100, 200, 255);

    let panel_frame = egui::Frame::default()
        .fill(panel_bg)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::same(12));

    egui::Window::new("Aircraft Details")
        .collapsible(false)
        .resizable(false)
        .frame(panel_frame)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-320.0, -10.0))
        .show(ctx, |ui| {
            // Header: ICAO and Callsign
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&aircraft.icao)
                        .color(highlight_color)
                        .size(16.0)
                        .strong()
                        .monospace(),
                );
                if let Some(ref callsign) = aircraft.callsign {
                    ui.label(
                        egui::RichText::new(callsign)
                            .color(callsign_color)
                            .size(14.0)
                            .strong(),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(egui::RichText::new("X").size(12.0)).clicked() {
                        detail_state.open = false;
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Position section
            egui::Grid::new("detail_grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Position").color(label_color).size(10.0));
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.4}, {:.4}",
                            aircraft.latitude, aircraft.longitude
                        ))
                        .color(value_color)
                        .size(11.0)
                        .monospace(),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new("Altitude").color(label_color).size(10.0));
                    let alt_text = aircraft
                        .altitude
                        .map(|a| {
                            if a >= 18000 {
                                format!("FL{:03}", a / 100)
                            } else {
                                format!("{} ft", a)
                            }
                        })
                        .unwrap_or_else(|| "---".to_string());
                    ui.label(
                        egui::RichText::new(alt_text)
                            .color(value_color)
                            .size(11.0)
                            .monospace(),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new("Speed").color(label_color).size(10.0));
                    let speed_text = aircraft
                        .velocity
                        .map(|v| format!("{} kts", v as i32))
                        .unwrap_or_else(|| "---".to_string());
                    ui.label(
                        egui::RichText::new(speed_text)
                            .color(value_color)
                            .size(11.0)
                            .monospace(),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new("Heading").color(label_color).size(10.0));
                    let heading_text = aircraft
                        .heading
                        .map(|h| format!("{:03}", h as i32))
                        .unwrap_or_else(|| "---".to_string());
                    ui.label(
                        egui::RichText::new(heading_text)
                            .color(value_color)
                            .size(11.0)
                            .monospace(),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Vertical Rate")
                            .color(label_color)
                            .size(10.0),
                    );
                    let vr_text = aircraft
                        .vertical_rate
                        .map(|vr| {
                            let symbol = if vr > 100 {
                                "+"
                            } else if vr < -100 {
                                ""
                            } else {
                                ""
                            };
                            format!("{}{} ft/min", symbol, vr)
                        })
                        .unwrap_or_else(|| "---".to_string());
                    ui.label(
                        egui::RichText::new(vr_text)
                            .color(value_color)
                            .size(11.0)
                            .monospace(),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Distance")
                            .color(label_color)
                            .size(10.0),
                    );
                    ui.label(
                        egui::RichText::new(format!("{:.1} nm", distance_nm))
                            .color(highlight_color)
                            .size(11.0)
                            .monospace(),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Track Points")
                            .color(label_color)
                            .size(10.0),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}", trail.points.len()))
                            .color(value_color)
                            .size(11.0)
                            .monospace(),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Track Duration")
                            .color(label_color)
                            .size(10.0),
                    );
                    let duration_text = oldest_point_age
                        .or(track_duration)
                        .map(|secs| {
                            let mins = secs / 60;
                            let secs = secs % 60;
                            format!("{}:{:02}", mins, secs)
                        })
                        .unwrap_or_else(|| "---".to_string());
                    ui.label(
                        egui::RichText::new(duration_text)
                            .color(value_color)
                            .size(11.0)
                            .monospace(),
                    );
                    ui.end_row();
                });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Action buttons
            ui.horizontal(|ui| {
                let is_following = follow_state.following_icao.as_ref() == Some(selected_icao);

                if ui.button("Center").clicked() {
                    // Centering will be handled by a separate system that reads an event
                    // For now, we'll use the follow state temporarily
                }

                let follow_text = if is_following { "Unfollow" } else { "Follow" };
                if ui.button(follow_text).clicked() {
                    if is_following {
                        follow_state.following_icao = None;
                    } else {
                        follow_state.following_icao = Some(selected_icao.clone());
                    }
                }
            });
        });
}

/// System to toggle detail panel with D key
pub fn toggle_detail_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut detail_state: ResMut<DetailPanelState>,
    list_state: Res<AircraftListState>,
) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        if list_state.selected_icao.is_some() {
            detail_state.open = !detail_state.open;
            if detail_state.open && detail_state.track_start.is_none() {
                detail_state.track_start = Some(Instant::now());
            }
        }
    }
}

/// System to open detail panel when aircraft is selected
pub fn open_detail_on_selection(
    list_state: Res<AircraftListState>,
    mut detail_state: ResMut<DetailPanelState>,
) {
    // Only trigger on change
    if !list_state.is_changed() {
        return;
    }

    if list_state.selected_icao.is_some() {
        detail_state.open = true;
        if detail_state.track_start.is_none() {
            detail_state.track_start = Some(Instant::now());
        }
    } else {
        detail_state.open = false;
        detail_state.track_start = None;
    }
}

/// System to detect clicks on aircraft sprites
pub fn detect_aircraft_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    aircraft_query: Query<(&crate::Aircraft, &Transform)>,
    mut list_state: ResMut<AircraftListState>,
    zoom_state: Res<ZoomState>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    // Convert cursor position to world coordinates
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    // Check each aircraft for click hit
    // Use a radius that accounts for the aircraft marker size and zoom
    let click_radius = 20.0 / zoom_state.camera_zoom;

    let mut closest_aircraft: Option<(String, f32)> = None;

    for (aircraft, transform) in aircraft_query.iter() {
        let aircraft_pos = Vec2::new(transform.translation.x, transform.translation.y);
        let distance = world_pos.distance(aircraft_pos);

        if distance < click_radius {
            if let Some((_, closest_dist)) = &closest_aircraft {
                if distance < *closest_dist {
                    closest_aircraft = Some((aircraft.icao.clone(), distance));
                }
            } else {
                closest_aircraft = Some((aircraft.icao.clone(), distance));
            }
        }
    }

    if let Some((icao, _)) = closest_aircraft {
        list_state.selected_icao = Some(icao);
    }
}
