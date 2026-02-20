use bevy::prelude::*;
use bevy_slippy_tiles::*;

use crate::constants;
use crate::geo;
use crate::map::{MapState, ZoomState};
use crate::view3d;
use crate::{clamp_latitude, clamp_longitude, Aircraft, AircraftLabel, ZoomDebugLogger};

// =============================================================================
// Components and Resources
// =============================================================================

/// Marker for the 3D camera that renders aircraft models.
#[derive(Component)]
pub(crate) struct AircraftCamera;

/// Marker for the primary 2D map camera (distinguishes it from the egui UI camera).
#[derive(Component)]
pub(crate) struct MapCamera;

/// Holds the shared Handle<bevy::pbr::ScatteringMedium> used by Atmosphere components.
/// Created at startup with ScatteringMedium::earthlike() defaults.
#[derive(Resource)]
pub struct AtmosphereMediumHandle(pub Handle<bevy::pbr::ScatteringMedium>);

// =============================================================================
// Plugin
// =============================================================================

pub(crate) struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            follow_aircraft.after(crate::adsb::sync_aircraft_from_adsb),
        )
        .add_systems(
            Update,
            update_camera_position
                .after(crate::input::handle_pan_drag)
                .after(crate::zoom::apply_camera_zoom)
                .after(follow_aircraft),
        )
        .add_systems(
            Update,
            sync_aircraft_camera
                .after(update_camera_position)
                .after(crate::zoom::apply_camera_zoom)
                .after(view3d::update_3d_camera),
        )
        .add_systems(
            Update,
            update_aircraft_positions
                .after(update_camera_position)
                .after(crate::adsb::sync_aircraft_from_adsb),
        )
        .add_systems(
            Update,
            scale_aircraft_and_labels.after(crate::zoom::apply_camera_zoom),
        )
        .add_systems(
            Update,
            update_aircraft_labels.after(update_aircraft_positions),
        );
    }
}

// =============================================================================
// Camera Systems
// =============================================================================

/// System to follow a selected aircraft (moves map center to aircraft position).
fn follow_aircraft(
    mut map_state: ResMut<MapState>,
    follow_state: Res<crate::aircraft::CameraFollowState>,
    aircraft_query: Query<&Aircraft>,
    time: Res<Time>,
) {
    let Some(ref following_icao) = follow_state.following_icao else {
        return;
    };

    // Find the aircraft we're following
    let Some(aircraft) = aircraft_query.iter().find(|a| &a.icao == following_icao) else {
        return;
    };

    // Lerp towards the aircraft position for smooth following
    let lerp_speed = 3.0; // How fast to catch up (higher = faster)
    let t = (lerp_speed * time.delta_secs()).min(1.0);

    let new_lat = map_state.latitude + (aircraft.latitude - map_state.latitude) * t as f64;
    let new_lon = map_state.longitude + (aircraft.longitude - map_state.longitude) * t as f64;

    map_state.latitude = clamp_latitude(new_lat);
    map_state.longitude = clamp_longitude(new_lon);
}

fn update_camera_position(
    map_state: Res<MapState>,
    tile_settings: Res<SlippyTilesSettings>,
    mut camera_query: Query<&mut Transform, With<MapCamera>>,
    logger: Option<Res<ZoomDebugLogger>>,
    view3d_state: Res<view3d::View3DState>,
) {
    // Don't fight with update_3d_camera during 3D mode or transitions
    if view3d_state.is_3d_active() || view3d_state.is_transitioning() {
        return;
    }

    let zoom_level = map_state.zoom_level;

    if let Ok(mut camera_transform) = camera_query.single_mut() {
        let reference_ll = LatitudeLongitudeCoordinates {
            latitude: tile_settings.reference_latitude,
            longitude: tile_settings.reference_longitude,
        };
        let reference_pixel = world_coords_to_world_pixel(
            &reference_ll,
            TileSize::Normal,
            zoom_level
        );

        let center_ll = LatitudeLongitudeCoordinates {
            latitude: map_state.latitude,
            longitude: map_state.longitude,
        };
        let center_pixel = world_coords_to_world_pixel(
            &center_ll,
            TileSize::Normal,
            zoom_level
        );

        let offset_x = center_pixel.0 - reference_pixel.0;
        let offset_y = center_pixel.1 - reference_pixel.1;

        if let Some(ref log) = logger {
            if map_state.is_changed() {
                log.log(&format!("=== CAMERA POS UPDATE (zoom: {}) ===", zoom_level.to_u8()));
                log.log(&format!("  center: ({:.6}, {:.6}) -> pixel ({:.2}, {:.2})",
                    map_state.latitude, map_state.longitude, center_pixel.0, center_pixel.1));
                log.log(&format!("  camera offset: ({:.2}, {:.2})", offset_x, offset_y));
            }
        }

        camera_transform.translation.x = offset_x as f32;
        camera_transform.translation.y = offset_y as f32;
    }
}

/// Sync Camera3d transform and projection to match Camera2d each frame.
/// This ensures 3D aircraft models are rendered with the same view as the 2D map.
///
/// Camera3d always clears to transparent (`Color::NONE`) so it gets a fresh
/// canvas each frame. Its rendered content alpha-composites on top of
/// Camera2d's output (tiles, sky, gizmos) without accumulating old frames.
fn sync_aircraft_camera(
    camera_2d: Query<(&Transform, &Projection), (With<MapCamera>, Without<AircraftCamera>)>,
    mut camera_3d: Query<(&mut Transform, &mut Projection), (With<AircraftCamera>, Without<Camera2d>)>,
) {
    let (Ok((t2, p2)), Ok((mut t3, mut p3))) = (camera_2d.single(), camera_3d.single_mut()) else {
        return;
    };
    *t3 = *t2;
    *p3 = p2.clone();
}

// =============================================================================
// Aircraft Rendering Systems
// =============================================================================

/// Keep aircraft and labels at constant screen size despite zoom changes.
fn scale_aircraft_and_labels(
    zoom_state: Res<ZoomState>,
    mut aircraft_query: Query<&mut Transform, (With<Aircraft>, Without<AircraftLabel>)>,
    mut label_query: Query<(&mut Transform, &mut TextFont), With<AircraftLabel>>,
    new_aircraft: Query<(), Added<Aircraft>>,
) {
    // Update scales when zoom changes or new aircraft are spawned
    if !zoom_state.is_changed() && new_aircraft.is_empty() {
        return;
    }

    let scale = constants::AIRCRAFT_MODEL_SCALE / zoom_state.camera_zoom;

    // Scale 3D aircraft models to maintain constant screen size
    for mut transform in aircraft_query.iter_mut() {
        transform.scale = Vec3::splat(scale);
    }

    // Scale label transforms to maintain constant screen size (labels are 2D, use 1/zoom)
    let label_scale = 1.0 / zoom_state.camera_zoom;
    for (mut transform, mut text_font) in label_query.iter_mut() {
        transform.scale = Vec3::splat(label_scale);
        text_font.font_size = constants::BASE_FONT_SIZE;
    }
}

fn update_aircraft_positions(
    map_state: Res<MapState>,
    tile_settings: Res<SlippyTilesSettings>,
    mut aircraft_query: Query<(&Aircraft, &mut Transform)>,
) {
    // Position aircraft RELATIVE to SlippyTilesSettings.reference
    let converter = geo::CoordinateConverter::new(&tile_settings, map_state.zoom_level);

    for (aircraft, mut transform) in aircraft_query.iter_mut() {
        let pos = converter.latlon_to_world(aircraft.latitude, aircraft.longitude);

        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        // Apply rotation for 3D model orientation:
        // GLB model has nose along +Z, wings along X, height along Y.
        // First rotate 180 around Z to flip the model right-side up (top faces camera).
        // Then rotate -90 around X to tilt nose from +Z to +Y (north on screen).
        // Finally heading rotation around Z orients the aircraft to its track angle.
        let base_rot = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_z(std::f32::consts::PI);
        if let Some(heading) = aircraft.heading {
            transform.rotation = Quat::from_rotation_z((-heading).to_radians()) * base_rot;
        } else {
            transform.rotation = base_rot;
        }
    }
}

fn update_aircraft_labels(
    zoom_state: Res<ZoomState>,
    aircraft_query: Query<&Transform, With<Aircraft>>,
    mut label_query: Query<(&AircraftLabel, &mut Transform), Without<Aircraft>>,
) {
    let world_space_offset = constants::LABEL_SCREEN_OFFSET / zoom_state.camera_zoom;

    for (label, mut label_transform) in label_query.iter_mut() {
        if let Ok(aircraft_transform) = aircraft_query.get(label.aircraft_entity) {
            label_transform.translation.x = aircraft_transform.translation.x + world_space_offset;
            label_transform.translation.y = aircraft_transform.translation.y + world_space_offset;
        }
    }
}
