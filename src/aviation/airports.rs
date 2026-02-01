use bevy::prelude::*;
use bevy_slippy_tiles::*;

use super::{Airport, AirportFilter, AviationData, LoadingState};
use crate::MapState;

/// Component marking an airport entity
#[derive(Component)]
pub struct AirportMarker {
    pub airport_id: i64,
}

/// Component for airport labels
#[derive(Component)]
pub struct AirportLabel {
    pub airport_entity: Entity,
}

/// Resource for airport rendering state
#[derive(Resource)]
pub struct AirportRenderState {
    pub show_airports: bool,
    pub filter: AirportFilter,
    /// Viewport bounds for culling (min_lat, max_lat, min_lon, max_lon)
    pub viewport_bounds: Option<(f64, f64, f64, f64)>,
}

impl Default for AirportRenderState {
    fn default() -> Self {
        Self {
            show_airports: true,
            filter: AirportFilter::FrequentlyUsed,
            viewport_bounds: None,
        }
    }
}

/// Z-layer for airports (below aircraft, above tiles)
const AIRPORT_Z_LAYER: f32 = 5.0;

/// System to spawn airport entities when data is ready
pub fn spawn_airports(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    aviation_data: Res<AviationData>,
    render_state: Res<AirportRenderState>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    existing_airports: Query<Entity, With<AirportMarker>>,
) {
    // Only run when data is ready and no airports exist yet
    if aviation_data.loading_state != LoadingState::Ready {
        return;
    }
    if !existing_airports.is_empty() {
        return;
    }
    if !render_state.show_airports {
        return;
    }

    info!("Spawning airport markers...");

    let reference_ll = LatitudeLongitudeCoordinates {
        latitude: tile_settings.reference_latitude,
        longitude: tile_settings.reference_longitude,
    };
    let reference_pixel = world_coords_to_world_pixel(
        &reference_ll,
        TileSize::Normal,
        map_state.zoom_level,
    );

    let mut count = 0;
    for airport in &aviation_data.airports {
        if !airport.passes_filter(render_state.filter) {
            continue;
        }

        let airport_ll = LatitudeLongitudeCoordinates {
            latitude: airport.latitude_deg,
            longitude: airport.longitude_deg,
        };
        let airport_pixel = world_coords_to_world_pixel(
            &airport_ll,
            TileSize::Normal,
            map_state.zoom_level,
        );

        let x = (airport_pixel.0 - reference_pixel.0) as f32;
        let y = (airport_pixel.1 - reference_pixel.1) as f32;

        // Create airport marker (small square)
        let mesh = meshes.add(Rectangle::new(6.0, 6.0));
        let material = materials.add(ColorMaterial::from_color(airport.color()));

        commands.spawn((
            AirportMarker {
                airport_id: airport.id,
            },
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(x, y, AIRPORT_Z_LAYER),
            Visibility::Inherited,
        ));

        count += 1;
    }

    info!("Spawned {} airport markers", count);
}

/// System to update airport positions when map moves
pub fn update_airport_positions(
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    aviation_data: Res<AviationData>,
    mut airport_query: Query<(&AirportMarker, &mut Transform)>,
) {
    if aviation_data.loading_state != LoadingState::Ready {
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

    // Build a lookup map for airports
    let airport_map: std::collections::HashMap<i64, &Airport> = aviation_data
        .airports
        .iter()
        .map(|a| (a.id, a))
        .collect();

    for (marker, mut transform) in airport_query.iter_mut() {
        if let Some(airport) = airport_map.get(&marker.airport_id) {
            let airport_ll = LatitudeLongitudeCoordinates {
                latitude: airport.latitude_deg,
                longitude: airport.longitude_deg,
            };
            let airport_pixel = world_coords_to_world_pixel(
                &airport_ll,
                TileSize::Normal,
                map_state.zoom_level,
            );

            transform.translation.x = (airport_pixel.0 - reference_pixel.0) as f32;
            transform.translation.y = (airport_pixel.1 - reference_pixel.1) as f32;
        }
    }
}

/// System to toggle airport visibility based on zoom level
pub fn update_airport_visibility(
    map_state: Res<MapState>,
    render_state: Res<AirportRenderState>,
    mut airport_query: Query<&mut Visibility, With<AirportMarker>>,
) {
    let zoom: u8 = map_state.zoom_level.to_u8();
    let should_show = render_state.show_airports && zoom >= 6;

    for mut visibility in airport_query.iter_mut() {
        *visibility = if should_show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
