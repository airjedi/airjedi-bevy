use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{TrailHistory, TrailConfig, altitude_color, age_opacity};
use crate::MapState;
use crate::view3d::View3DState;

/// System to draw flight trails using Gizmos.
/// In 2D mode, draws flat trails. In 3D mode, draws trails at altitude using Vec3 positions.
pub fn draw_trails(
    mut gizmos: Gizmos,
    config: Res<TrailConfig>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    view3d_state: Res<View3DState>,
    trail_query: Query<&TrailHistory>,
) {
    if !config.enabled {
        return;
    }

    let is_3d = view3d_state.is_3d_active();

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        map_state.zoom_level,
    );

    for trail in trail_query.iter() {
        if trail.points.len() < 2 {
            continue;
        }

        let mut prev_pos: Option<Vec3> = None;
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
                map_state.zoom_level,
            );

            let x = (point_pixel.0 - reference_pixel.0) as f32;
            let y = (point_pixel.1 - reference_pixel.1) as f32;
            let z = if is_3d {
                view3d_state.altitude_to_z(point.altitude.unwrap_or(0))
            } else {
                0.0
            };

            let pos = Vec3::new(x, y, z);

            let base_color = altitude_color(point.altitude);
            let color = base_color.with_alpha(opacity);

            if let Some(prev) = prev_pos {
                let draw_color = prev_color.unwrap_or(color);
                if is_3d {
                    gizmos.line(prev, pos, draw_color);
                } else {
                    gizmos.line_2d(prev.truncate(), pos.truncate(), draw_color);
                }
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
