use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{AviationData, LoadingState};
use crate::MapState;

/// Component marking a runway entity
#[derive(Component)]
pub struct RunwayMarker {
    pub runway_id: i64,
    pub airport_ref: i64,
}

/// Resource for runway rendering state
#[derive(Resource)]
pub struct RunwayRenderState {
    pub show_runways: bool,
}

impl Default for RunwayRenderState {
    fn default() -> Self {
        Self { show_runways: true }
    }
}

const RUNWAY_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.7);

/// System to render runways using Gizmos
pub fn draw_runways(
    mut gizmos: Gizmos,
    aviation_data: Res<AviationData>,
    render_state: Res<RunwayRenderState>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
) {
    if aviation_data.loading_state != LoadingState::Ready {
        return;
    }
    if !render_state.show_runways {
        return;
    }

    // Only show runways at zoom 8+
    let zoom: u8 = map_state.zoom_level.to_u8();
    if zoom < 8 {
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

    for runway in &aviation_data.runways {
        if !runway.has_valid_coords() || runway.is_closed() {
            continue;
        }

        let le_lat = runway.le_latitude_deg.unwrap();
        let le_lon = runway.le_longitude_deg.unwrap();
        let he_lat = runway.he_latitude_deg.unwrap();
        let he_lon = runway.he_longitude_deg.unwrap();

        // Convert LE end to screen coordinates
        let le_ll = LatitudeLongitudeCoordinates {
            latitude: le_lat,
            longitude: le_lon,
        };
        let le_pixel = world_coords_to_world_pixel(
            &le_ll,
            TileSize::Normal,
            map_state.zoom_level,
        );

        // Convert HE end to screen coordinates
        let he_ll = LatitudeLongitudeCoordinates {
            latitude: he_lat,
            longitude: he_lon,
        };
        let he_pixel = world_coords_to_world_pixel(
            &he_ll,
            TileSize::Normal,
            map_state.zoom_level,
        );

        let start = Vec2::new(
            (le_pixel.0 - reference_pixel.0) as f32,
            (le_pixel.1 - reference_pixel.1) as f32,
        );
        let end = Vec2::new(
            (he_pixel.0 - reference_pixel.0) as f32,
            (he_pixel.1 - reference_pixel.1) as f32,
        );

        gizmos.line_2d(start, end, RUNWAY_COLOR);
    }
}
