use bevy::prelude::*;
use std::time::Instant;

use super::AircraftListState;
use crate::ZoomState;

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

/// Detail panel rendering is now integrated into the stacked right panel
/// (see `render_aircraft_list_panel` in list_panel.rs).
/// This system is kept as a no-op for the plugin registration; the actual
/// rendering happens inside the list panel's bottom section.
pub fn render_detail_panel(
    list_state: Res<AircraftListState>,
    mut detail_state: ResMut<DetailPanelState>,
) {
    // Clear state when no aircraft is selected
    if list_state.selected_icao.is_none() {
        detail_state.open = false;
        detail_state.track_start = None;
    }
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
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::MapCamera>>,
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
