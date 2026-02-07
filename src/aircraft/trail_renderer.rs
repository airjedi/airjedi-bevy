use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{TrailHistory, TrailConfig, SessionClock, altitude_color, age_opacity};
use crate::MapState;
use crate::geo::CoordinateConverter;

/// System to draw flight trails using Gizmos
pub fn draw_trails(
    mut gizmos: Gizmos,
    config: Res<TrailConfig>,
    clock: Res<SessionClock>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    trail_query: Query<&TrailHistory>,
) {
    if !config.enabled {
        return;
    }

    let converter = CoordinateConverter::new(&tile_settings, map_state.zoom_level);

    for trail in trail_query.iter() {
        if trail.points.len() < 2 {
            continue;
        }

        let mut prev_pos: Option<Vec2> = None;
        let mut prev_color: Option<Color> = None;

        for point in trail.points.iter() {
            let opacity = age_opacity(
                clock.age_secs(point.timestamp),
                config.solid_duration_seconds,
                config.fade_duration_seconds,
            );

            if opacity <= 0.0 {
                prev_pos = None;
                continue;
            }

            let pos = converter.latlon_to_world(point.lat, point.lon);

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
    clock: Res<SessionClock>,
    mut trail_query: Query<&mut TrailHistory>,
) {
    for mut trail in trail_query.iter_mut() {
        trail.prune(config.max_age_seconds, &clock);
    }
}
