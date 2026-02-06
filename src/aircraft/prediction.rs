use bevy::prelude::*;
use bevy_slippy_tiles::*;

use crate::{Aircraft, MapState};
use super::{AircraftListState, CameraFollowState};

/// Configuration for flight path prediction
#[derive(Resource)]
pub struct PredictionConfig {
    pub enabled: bool,
    /// Prediction time horizons in minutes
    pub horizons_minutes: Vec<f32>,
    /// Dash length in pixels
    pub dash_length: f32,
    /// Gap length in pixels
    pub gap_length: f32,
}

impl Default for PredictionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            horizons_minutes: vec![1.0, 5.0, 15.0],
            dash_length: 8.0,
            gap_length: 4.0,
        }
    }
}

/// Calculate a predicted position based on current position, heading, and speed
fn predict_position(
    lat: f64,
    lon: f64,
    heading_deg: f32,
    speed_knots: f64,
    minutes: f32,
) -> (f64, f64) {
    // Convert speed from knots to nautical miles per minute
    let nm_per_minute = speed_knots / 60.0;

    // Distance to travel in nautical miles
    let distance_nm = nm_per_minute * minutes as f64;

    // Convert heading to radians (0 = north, clockwise positive)
    let heading_rad = (heading_deg as f64).to_radians();

    // Earth radius in nautical miles
    let earth_radius_nm = 3440.065;

    // Calculate angular distance
    let angular_distance = distance_nm / earth_radius_nm;

    // Current position in radians
    let lat1 = lat.to_radians();
    let lon1 = lon.to_radians();

    // Calculate new latitude
    let lat2 = (lat1.sin() * angular_distance.cos()
        + lat1.cos() * angular_distance.sin() * heading_rad.cos()).asin();

    // Calculate new longitude
    let lon2 = lon1 + (heading_rad.sin() * angular_distance.sin() * lat1.cos())
        .atan2(angular_distance.cos() - lat1.sin() * lat2.sin());

    (lat2.to_degrees(), lon2.to_degrees())
}

/// System to draw flight path predictions using Gizmos
pub fn draw_predictions(
    mut gizmos: Gizmos,
    config: Res<PredictionConfig>,
    list_state: Res<AircraftListState>,
    follow_state: Res<CameraFollowState>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    aircraft_query: Query<&Aircraft>,
) {
    if !config.enabled {
        return;
    }

    // Get the aircraft to show prediction for (selected or followed)
    let target_icao = follow_state.following_icao.as_ref()
        .or(list_state.selected_icao.as_ref());

    let Some(target_icao) = target_icao else {
        return;
    };

    // Find the target aircraft
    let Some(aircraft) = aircraft_query.iter().find(|a| &a.icao == target_icao) else {
        return;
    };

    // Need both heading and velocity to predict
    let Some(heading) = aircraft.heading else {
        return;
    };
    let Some(velocity) = aircraft.velocity else {
        return;
    };

    // Skip if not moving significantly
    if velocity < 10.0 {
        return;
    }

    // Calculate reference point for world coordinate conversion
    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        map_state.zoom_level,
    );

    // Get current aircraft position in world coordinates
    let aircraft_ll = LatitudeLongitudeCoordinates {
        latitude: aircraft.latitude,
        longitude: aircraft.longitude,
    };
    let aircraft_pixel = world_coords_to_world_pixel(
        &aircraft_ll,
        TileSize::Normal,
        map_state.zoom_level,
    );
    let start_pos = Vec2::new(
        (aircraft_pixel.0 - reference_pixel.0) as f32,
        (aircraft_pixel.1 - reference_pixel.1) as f32,
    );

    // Draw prediction lines for each time horizon
    let mut prev_pos = start_pos;
    let mut prev_minutes = 0.0;

    // Colors for different time horizons: cyan -> blue -> purple
    let colors = [
        Color::srgba(0.0, 1.0, 1.0, 0.8),   // Cyan for T+1min
        Color::srgba(0.3, 0.5, 1.0, 0.6),   // Blue for T+5min
        Color::srgba(0.6, 0.3, 0.9, 0.4),   // Purple for T+15min
    ];

    for (i, &minutes) in config.horizons_minutes.iter().enumerate() {
        // Calculate predicted position
        let (pred_lat, pred_lon) = predict_position(
            aircraft.latitude,
            aircraft.longitude,
            heading,
            velocity,
            minutes,
        );

        let pred_ll = LatitudeLongitudeCoordinates {
            latitude: pred_lat,
            longitude: pred_lon,
        };
        let pred_pixel = world_coords_to_world_pixel(
            &pred_ll,
            TileSize::Normal,
            map_state.zoom_level,
        );
        let end_pos = Vec2::new(
            (pred_pixel.0 - reference_pixel.0) as f32,
            (pred_pixel.1 - reference_pixel.1) as f32,
        );

        // Get color for this segment
        let color = colors.get(i).copied().unwrap_or(colors[colors.len() - 1]);

        // Draw dashed line from previous position to this predicted position
        draw_dashed_line(&mut gizmos, prev_pos, end_pos, config.dash_length, config.gap_length, color);

        // Draw time marker (small circle) at prediction point
        gizmos.circle_2d(end_pos, 4.0, color);

        prev_pos = end_pos;
        prev_minutes = minutes;
    }

    // Draw small markers at intermediate positions (every minute for visual reference)
    let _ = prev_minutes; // silence unused warning
}

/// Draw a dashed line between two points
fn draw_dashed_line(
    gizmos: &mut Gizmos,
    start: Vec2,
    end: Vec2,
    dash_length: f32,
    gap_length: f32,
    color: Color,
) {
    let direction = end - start;
    let total_length = direction.length();

    if total_length < 0.1 {
        return;
    }

    let unit_dir = direction.normalize();

    let mut distance = 0.0;
    let mut drawing = true; // Start with a dash

    while distance < total_length {
        let segment_end = if drawing {
            (distance + dash_length).min(total_length)
        } else {
            (distance + gap_length).min(total_length)
        };

        if drawing {
            let p1 = start + unit_dir * distance;
            let p2 = start + unit_dir * segment_end;
            gizmos.line_2d(p1, p2, color);
        }

        distance = segment_end;
        drawing = !drawing;
    }
}
