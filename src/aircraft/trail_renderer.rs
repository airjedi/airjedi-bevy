use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{TrailHistory, TrailConfig, SessionClock};
use super::trails::{altitude_color, age_opacity};
use super::staleness::{staleness_opacity, aircraft_age_secs};
use crate::{Aircraft, MapState};
use crate::geo::CoordinateConverter;
use crate::view3d::View3DState;

/// System to draw flight trails using Gizmos.
/// In 2D mode, draws flat trails. In 3D mode, draws trails at altitude using Vec3 positions.
pub fn draw_trails(
    mut gizmos: Gizmos,
    config: Res<TrailConfig>,
    clock: Res<SessionClock>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    view3d_state: Res<View3DState>,
    trail_query: Query<(&TrailHistory, &Aircraft)>,
) {
    if !config.enabled {
        return;
    }

    let converter = CoordinateConverter::new(&tile_settings, map_state.zoom_level);
    let is_3d = view3d_state.is_3d_active();

    for (trail, aircraft) in trail_query.iter() {
        let stale_opacity = staleness_opacity(aircraft_age_secs(aircraft));

        if trail.points.len() < 2 {
            continue;
        }

        let mut prev_pos: Option<Vec3> = None;
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

            let xy = converter.latlon_to_world(point.lat, point.lon);
            let z = if is_3d {
                view3d_state.altitude_to_z(point.altitude.unwrap_or(0))
            } else {
                0.0
            };
            let pos = Vec3::new(xy.x, xy.y, z);

            let base_color = altitude_color(point.altitude);
            let color = base_color.with_alpha(opacity * stale_opacity);

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
    clock: Res<SessionClock>,
    mut trail_query: Query<&mut TrailHistory>,
) {
    for mut trail in trail_query.iter_mut() {
        trail.prune(config.max_age_seconds, &clock);
    }
}
