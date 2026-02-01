use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{AviationData, LoadingState, NavaidType};
use crate::MapState;

/// Component marking a navaid entity
#[derive(Component)]
pub struct NavaidMarker {
    pub navaid_id: i64,
}

/// Resource for navaid rendering state
#[derive(Resource)]
pub struct NavaidRenderState {
    pub show_navaids: bool,
}

impl Default for NavaidRenderState {
    fn default() -> Self {
        Self { show_navaids: false } // Off by default
    }
}

/// System to render navaids using Gizmos
pub fn draw_navaids(
    mut gizmos: Gizmos,
    aviation_data: Res<AviationData>,
    render_state: Res<NavaidRenderState>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
) {
    if aviation_data.loading_state != LoadingState::Ready {
        return;
    }
    if !render_state.show_navaids {
        return;
    }

    // Only show navaids at zoom 7+
    let zoom: u8 = map_state.zoom_level.to_u8();
    if zoom < 7 {
        return;
    }

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        map_state.zoom_level,
    );

    for navaid in &aviation_data.navaids {
        let navaid_ll = LatitudeLongitudeCoordinates {
            latitude: navaid.latitude_deg,
            longitude: navaid.longitude_deg,
        };
        let navaid_pixel = world_coords_to_world_pixel(
            &navaid_ll,
            TileSize::Normal,
            map_state.zoom_level,
        );

        let pos = Vec2::new(
            (navaid_pixel.0 - reference_pixel.0) as f32,
            (navaid_pixel.1 - reference_pixel.1) as f32,
        );

        let color = navaid.color();
        let size = 4.0;

        match navaid.navaid_type {
            NavaidType::Vor | NavaidType::VorDme | NavaidType::Vortac => {
                // Draw circle for VOR-type navaids
                gizmos.circle_2d(pos, size, color);
                // Draw radial lines
                for angle in [0.0_f32, 90.0, 180.0, 270.0] {
                    let rad = angle.to_radians();
                    let end = pos + Vec2::new(rad.cos(), rad.sin()) * (size + 3.0);
                    gizmos.line_2d(pos, end, color);
                }
            }
            NavaidType::Ndb | NavaidType::NdbDme => {
                // Draw diamond for NDB
                let points = [
                    pos + Vec2::new(0.0, size),
                    pos + Vec2::new(size, 0.0),
                    pos + Vec2::new(0.0, -size),
                    pos + Vec2::new(-size, 0.0),
                    pos + Vec2::new(0.0, size),
                ];
                for i in 0..4 {
                    gizmos.line_2d(points[i], points[i + 1], color);
                }
            }
            NavaidType::Dme | NavaidType::Tacan => {
                // Draw square for DME/TACAN
                let half = size / 2.0;
                let corners = [
                    pos + Vec2::new(-half, -half),
                    pos + Vec2::new(half, -half),
                    pos + Vec2::new(half, half),
                    pos + Vec2::new(-half, half),
                    pos + Vec2::new(-half, -half),
                ];
                for i in 0..4 {
                    gizmos.line_2d(corners[i], corners[i + 1], color);
                }
            }
            NavaidType::Unknown => {
                // Simple dot
                gizmos.circle_2d(pos, 2.0, color);
            }
        }
    }
}
