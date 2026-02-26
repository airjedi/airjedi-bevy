use bevy::prelude::*;
use bevy::asset::AssetLoadFailedEvent;
use bevy::pbr::StandardMaterial;
use bevy_slippy_tiles::*;

use crate::constants;
use crate::map::{MapState, ZoomState};
use crate::tile_cache;
use crate::view3d;
use crate::camera::MapCamera;
use crate::{clamp_latitude, clamp_longitude, ZoomDebugLogger};

// =============================================================================
// Components and Resources
// =============================================================================

/// Component to track tile fade state for smooth zoom transitions.
#[derive(Component)]
pub(crate) struct TileFadeState {
    pub(crate) alpha: f32,
    /// The zoom level this tile was spawned for
    pub(crate) tile_zoom: u8,
}

/// Links a tile entity to its 3D mesh quad companion (used in 3D mode only).
#[derive(Component)]
pub(crate) struct TileMeshQuad(pub Entity);

/// Marker on 3D mesh quad companion entities so orphans can be detected.
#[derive(Component)]
struct TileQuad3d;

/// Shared mesh handle for all 3D tile quads (sized to match DEFAULT_TILE_PIXELS).
#[derive(Resource)]
pub(crate) struct TileQuadMesh(pub Handle<Mesh>);

/// Tracks which tile positions have been spawned to prevent duplicate entities.
/// Key is (transform_x rounded, transform_y rounded, zoom_level).
#[derive(Resource, Default)]
pub(crate) struct SpawnedTiles {
    pub(crate) positions: std::collections::HashSet<(i32, i32, u8)>,
}

/// Timer that triggers periodic tile re-requests in 3D mode so that camera
/// orbit, pan, and altitude changes continuously fill visible areas.
#[derive(Resource)]
pub(crate) struct Tile3DRefreshTimer(Timer);

impl Default for Tile3DRefreshTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.3, TimerMode::Repeating))
    }
}

/// Tracks the previous camera altitude to detect active altitude changes.
/// During rapid altitude changes, tile culling is softened to prevent
/// flashing while new tiles load.
#[derive(Resource)]
struct AltitudeChangeTracker {
    prev_altitude: f32,
    /// Seconds since the last significant altitude change.
    idle_secs: f32,
}

impl Default for AltitudeChangeTracker {
    fn default() -> Self {
        Self {
            prev_altitude: 10000.0,
            idle_secs: f32::MAX,
        }
    }
}

// =============================================================================
// Plugin
// =============================================================================

pub(crate) struct TilesPlugin;

impl Plugin for TilesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnedTiles>()
            .init_resource::<Tile3DRefreshTimer>()
            .init_resource::<AltitudeChangeTracker>()
            .add_systems(Startup, setup_tile_quad_mesh)
            .add_systems(Update, handle_window_resize)
            .add_systems(Update, handle_3d_view_tile_refresh)
            .add_systems(Update, request_3d_tiles_continuous.after(handle_3d_view_tile_refresh))
            .add_systems(Update, track_altitude_changes)
            .add_systems(Update, handle_tile_load_failures)
            .add_systems(Update, display_tiles_filtered.after(bevy::ecs::schedule::ApplyDeferred))
            .add_systems(Update, animate_tile_fades.after(display_tiles_filtered))
            .add_systems(Update, cull_offscreen_tiles.after(display_tiles_filtered))
            .add_systems(Update, sync_tile_mesh_quads.after(animate_tile_fades))
            .add_systems(Update, sync_tile_mesh_alpha.after(sync_tile_mesh_quads))
            .add_systems(Update, sync_tile_mesh_transforms.after(sync_tile_mesh_quads))
            .add_systems(Update, hide_tile_sprites_in_3d.after(sync_tile_mesh_quads))
            .add_systems(Update, cleanup_orphaned_tile_quads.after(sync_tile_mesh_quads));
    }
}

// =============================================================================
// Altitude-Adaptive Zoom
// =============================================================================

/// Map camera altitude (feet) to an appropriate tile zoom level for 3D mode.
/// Uses a logarithmic mapping since each zoom level doubles resolution.
/// Higher altitudes use lower zoom levels (wider view), lower altitudes
/// use higher zoom levels (more detail).
pub(crate) fn altitude_to_zoom_level(altitude_ft: f32) -> u8 {
    // Logarithmic mapping: zoom = base - log2(altitude / reference)
    // Tuned so that ~5,000 ft → zoom 16, ~120,000 ft → zoom 9
    let reference_alt = 5000.0_f32;
    let reference_zoom = 16.0_f32;

    let ratio = (altitude_ft / reference_alt).max(1.0);
    let zoom = reference_zoom - ratio.log2() * 1.5;
    (zoom.round() as u8).clamp(8, 18)
}

// =============================================================================
// Tile Helpers
// =============================================================================

/// Compute the tile download radius needed to cover the viewport.
///
/// In 2D (orthographic): each tile occupies `256 * camera_zoom` screen pixels.
/// In 3D (perspective): the tilted camera sees a larger ground footprint, so we
/// estimate the visible ground extent from the camera distance, pitch, and FOV.
pub(crate) fn compute_tile_radius(
    window_width: f32,
    window_height: f32,
    camera_zoom: f32,
    view3d_state: Option<&view3d::View3DState>,
) -> u8 {
    // Check if we're in 3D perspective mode
    if let Some(state) = view3d_state {
        if state.is_3d_active() {
            let fov = 60.0_f32.to_radians();
            let aspect = window_width / window_height;
            let half_vfov = fov / 2.0;
            let half_hfov = (aspect * half_vfov.tan()).atan();
            let pitch_rad = state.camera_pitch.to_radians();

            // Camera height above the map plane
            let effective_distance = state.altitude_to_distance();
            let camera_height = effective_distance * pitch_rad.sin();

            // The far ground edge angle: pitch - half_vfov from horizontal
            // Ground distance = camera_height / tan(pitch - half_vfov)
            // Clamp the angle so we don't get infinity when looking near the horizon
            let far_angle = (pitch_rad - half_vfov).max(0.05);
            let far_ground_dist = camera_height / far_angle.tan();

            // Horizontal extent at the ground plane center
            let center_ground_dist = effective_distance * pitch_rad.cos();
            let half_width = center_ground_dist * half_hfov.tan();

            // Use whichever axis demands more tiles
            let max_ground_extent = far_ground_dist.max(half_width);
            let tile_world_size = constants::DEFAULT_TILE_PIXELS;
            let tiles_needed = (max_ground_extent / tile_world_size).ceil() as u8;
            return tiles_needed.clamp(3, 12);
        }
    }

    // 2D orthographic mode
    let tile_screen_px = constants::DEFAULT_TILE_PIXELS * camera_zoom;
    let half_tiles_x = (window_width / (2.0 * tile_screen_px)).ceil() as u8;
    let half_tiles_y = (window_height / (2.0 * tile_screen_px)).ceil() as u8;
    half_tiles_x.max(half_tiles_y).clamp(3, 8)
}

/// Send a tile download request for the current map location.
pub(crate) fn request_tiles_at_location(
    download_events: &mut MessageWriter<DownloadSlippyTilesMessage>,
    latitude: f64,
    longitude: f64,
    zoom_level: ZoomLevel,
    use_cache: bool,
) {
    download_events.write(DownloadSlippyTilesMessage {
        tile_size: constants::DEFAULT_TILE_SIZE,
        zoom_level,
        coordinates: Coordinates::from_latitude_longitude(latitude, longitude),
        radius: Radius(constants::TILE_DOWNLOAD_RADIUS),
        use_cache,
    });
}

// =============================================================================
// Tile Systems
// =============================================================================

/// When a tile image fails to load, check if the cached file is corrupt and remove it.
/// The tile will be re-requested automatically by bevy_slippy_tiles on the next frame.
fn handle_tile_load_failures(
    mut failed_events: MessageReader<AssetLoadFailedEvent<Image>>,
) {
    for event in failed_events.read() {
        let asset_path = event.path.path();
        // Only handle tile files
        let path_str = asset_path.to_string_lossy();
        if path_str.contains(".tile.") {
            warn!("Tile asset load failed: {:?} — checking for corrupt cache file", asset_path);
            tile_cache::remove_corrupt_cached_tile(asset_path);
        }
    }
}

/// Request tiles when the window is resized or maximized so newly exposed areas are filled.
fn handle_window_resize(
    mut resize_events: MessageReader<bevy::window::WindowResized>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    map_state: Res<MapState>,
    zoom_state: Res<ZoomState>,
    view3d_state: Res<view3d::View3DState>,
) {
    for event in resize_events.read() {
        let radius = compute_tile_radius(
            event.width,
            event.height,
            zoom_state.camera_zoom,
            Some(&view3d_state),
        );
        download_events.write(DownloadSlippyTilesMessage {
            tile_size: constants::DEFAULT_TILE_SIZE,
            zoom_level: map_state.zoom_level,
            coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
            radius: Radius(radius),
            use_cache: true,
        });
    }
}

/// Re-request tiles when 3D view state changes (entering/exiting 3D, orbit, pitch, distance)
/// so the larger perspective footprint is covered.
/// When returning to 2D, clears spawned tile tracking so tiles are freshly re-displayed.
fn handle_3d_view_tile_refresh(
    view3d_state: Res<view3d::View3DState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    mut map_state: ResMut<MapState>,
    zoom_state: Res<ZoomState>,
    window_query: Query<&Window>,
    mut spawned_tiles: ResMut<SpawnedTiles>,
) {
    if !view3d_state.is_changed() {
        return;
    }

    // When we've just returned to 2D mode, clear the spawned tiles tracker
    // and restore the saved 2D zoom level.
    // 3D mode uses multi-resolution tiles at different zoom levels and scales;
    // without clearing, the dedup check in display_tiles_filtered would skip
    // re-spawning tiles at the current zoom level, leaving a blank map.
    if matches!(view3d_state.mode, view3d::ViewMode::Map2D) && !view3d_state.is_transitioning() {
        spawned_tiles.positions.clear();
        // Restore the 2D zoom level that was saved when entering 3D mode
        if let Some(saved_zoom) = view3d_state.saved_2d_zoom_level {
            if let Ok(zoom) = ZoomLevel::try_from(saved_zoom) {
                map_state.zoom_level = zoom;
                debug!("Restored 2D zoom level: {}", saved_zoom);
            }
        }
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let radius = compute_tile_radius(
        window.width(),
        window.height(),
        zoom_state.camera_zoom,
        Some(&view3d_state),
    );
    download_events.write(DownloadSlippyTilesMessage {
        tile_size: constants::DEFAULT_TILE_SIZE,
        zoom_level: map_state.zoom_level,
        coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
        radius: Radius(radius),
        use_cache: true,
    });
}

/// Continuously request multi-resolution tiles in 3D mode so that camera orbit,
/// pan, and altitude changes fill the visible area without waiting for explicit
/// View3DState change events.
///
/// Three distance bands load tiles at decreasing zoom levels:
/// - Near (0-40% of ground extent): current zoom_level
/// - Mid  (40-70%): zoom_level - 1
/// - Far  (70-100%): zoom_level - 2
///
/// Mid and far bands are offset in the camera look direction so tiles load ahead
/// of where the user is looking. Band radii adapt to pitch: low pitch (looking
/// toward horizon) favours far tiles; high pitch (looking down) favours near tiles.
fn request_3d_tiles_continuous(
    mut timer: ResMut<Tile3DRefreshTimer>,
    time: Res<Time>,
    view3d_state: Res<view3d::View3DState>,
    mut map_state: ResMut<MapState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
) {
    if !view3d_state.is_3d_active() {
        return;
    }

    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    // Compute zoom level from camera altitude so that flying close to the
    // ground automatically loads higher-detail tiles.
    let adaptive_zoom = altitude_to_zoom_level(view3d_state.camera_altitude);
    if let Ok(new_zoom) = ZoomLevel::try_from(adaptive_zoom) {
        if map_state.zoom_level != new_zoom {
            debug!("3D adaptive zoom: altitude {:.0} ft -> zoom {}", view3d_state.camera_altitude, adaptive_zoom);
            map_state.zoom_level = new_zoom;
        }
    }

    let base_zoom = map_state.zoom_level.to_u8();
    let lat = map_state.latitude;
    let lon = map_state.longitude;
    let yaw_rad = view3d_state.camera_yaw.to_radians();
    let pitch = view3d_state.camera_pitch;

    // pitch_factor: 0.0 = low pitch (horizon), 1.0 = high pitch (looking down)
    let pitch_factor = ((pitch - 15.0) / (89.0 - 15.0)).clamp(0.0, 1.0);

    // Adaptive band radii based on pitch
    let near_radius = 3 + (3.0 * pitch_factor) as u8;          // 3-6
    let mid_radius  = 3 + (2.0 * (1.0 - pitch_factor)) as u8;  // 3-5
    let far_radius  = 2 + (3.0 * (1.0 - pitch_factor)) as u8;  // 2-5

    // --- Near band: current zoom level, centered on map position ---
    download_events.write(DownloadSlippyTilesMessage {
        tile_size: constants::DEFAULT_TILE_SIZE,
        zoom_level: map_state.zoom_level,
        coordinates: Coordinates::from_latitude_longitude(lat, lon),
        radius: Radius(near_radius),
        use_cache: true,
    });

    // Helper: send a tile request at a given zoom level, offset from (lat, lon)
    // by `fwd` tiles forward and `side` tiles sideways relative to the camera yaw.
    let mut request_band = |zoom_offset: u8, fwd: f64, side: f64, radius: u8| {
        if base_zoom < zoom_offset {
            return;
        }
        let z = base_zoom - zoom_offset;
        let Ok(zoom) = ZoomLevel::try_from(z) else {
            return;
        };
        let deg_per_tile_lon = 360.0 / (1u64 << z) as f64;
        let deg_per_tile_lat = deg_per_tile_lon * lat.to_radians().cos();
        // Forward = along yaw, sideways = perpendicular (yaw + 90°)
        let offset_lat = fwd * deg_per_tile_lat * yaw_rad.cos() as f64
            - side * deg_per_tile_lat * yaw_rad.sin() as f64;
        let offset_lon = fwd * deg_per_tile_lon * yaw_rad.sin() as f64
            + side * deg_per_tile_lon * yaw_rad.cos() as f64;
        download_events.write(DownloadSlippyTilesMessage {
            tile_size: constants::DEFAULT_TILE_SIZE,
            zoom_level: zoom,
            coordinates: Coordinates::from_latitude_longitude(
                clamp_latitude(lat + offset_lat),
                clamp_longitude(lon + offset_lon),
            ),
            radius: Radius(radius),
            use_cache: true,
        });
    };

    // --- Mid band: zoom_level - 1 ---
    request_band(1, 3.0, 0.0, mid_radius);
    request_band(1, 2.0, -4.0, mid_radius);
    request_band(1, 2.0, 4.0, mid_radius);

    // --- Far band: zoom_level - 2 ---
    request_band(2, 4.0, 0.0, far_radius);
    request_band(2, 3.0, -5.0, far_radius);
    request_band(2, 3.0, 5.0, far_radius);

    // --- Horizon bands: zoom_level - 3 and - 4 ---
    let hr = 4 + (3.0 * (1.0 - pitch_factor)) as u8; // 4-7

    // zoom-3: sweep at multiple forward distances
    for &fwd in &[2.0, 5.0, 8.0] {
        request_band(3, fwd, 0.0, hr);
        let spread = fwd * 1.5 + 4.0;
        request_band(3, fwd, -spread, hr);
        request_band(3, fwd, spread, hr);
    }

    // zoom-4: coarser tiles for the far horizon, even wider sweep
    let ur = 4 + (2.0 * (1.0 - pitch_factor)) as u8; // 4-6
    for &fwd in &[2.0, 5.0, 8.0] {
        request_band(4, fwd, 0.0, ur);
        let spread = fwd * 2.0 + 5.0;
        request_band(4, fwd, -spread, ur);
        request_band(4, fwd, spread, ur);
    }
}

/// Track camera altitude changes to soften tile culling during rapid zoom.
fn track_altitude_changes(
    time: Res<Time>,
    view3d_state: Res<view3d::View3DState>,
    mut tracker: ResMut<AltitudeChangeTracker>,
) {
    let current = view3d_state.camera_altitude;
    let delta = (current - tracker.prev_altitude).abs();
    if delta > 50.0 {
        // Significant altitude change — reset cooldown
        tracker.idle_secs = 0.0;
    } else {
        tracker.idle_secs += time.delta_secs();
    }
    tracker.prev_altitude = current;
}

/// Custom tile display system that filters tiles by current zoom level.
/// When new tiles arrive at the current zoom, old tiles from previous zoom levels
/// are marked for delayed despawn so the screen is never blank.
fn display_tiles_filtered(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    mut tile_events: MessageReader<SlippyTileDownloadedMessage>,
    mut _tile_query: Query<(Entity, &mut TileFadeState), With<MapTile>>,
    mut spawned_tiles: ResMut<SpawnedTiles>,
    logger: Option<Res<ZoomDebugLogger>>,
    view3d_state: Res<view3d::View3DState>,
) {
    let current_zoom = map_state.zoom_level.to_u8();

    for event in tile_events.read() {
        // In 3D mode, accept tiles within 4 zoom levels below current (multi-resolution bands).
        // In 2D mode, only accept tiles at the exact current zoom level.
        let event_zoom = event.zoom_level.to_u8();
        if view3d_state.is_3d_active() {
            if event_zoom > current_zoom || current_zoom - event_zoom > 4 {
                continue;
            }
        } else if event.zoom_level != map_state.zoom_level {
            continue;
        }

        // Calculate tile position (same logic as bevy_slippy_tiles display_tiles)
        let reference_point = LatitudeLongitudeCoordinates {
            latitude: tile_settings.reference_latitude,
            longitude: tile_settings.reference_longitude,
        };
        let (ref_x, ref_y) = world_coords_to_world_pixel(
            &reference_point,
            event.tile_size,
            event.zoom_level
        );

        let current_coords = match &event.coordinates {
            Coordinates::LatitudeLongitude(coords) => *coords,
            Coordinates::SlippyTile(coords) => coords.to_latitude_longitude(event.zoom_level),
        };
        let (tile_x, tile_y) = world_coords_to_world_pixel(
            &current_coords,
            event.tile_size,
            event.zoom_level
        );

        let half_tile = event.tile_size.to_pixels() as f64 / 2.0;
        let tile_center_x = tile_x + half_tile;
        let tile_center_y = tile_y - half_tile;

        let mut transform_x = (tile_center_x - ref_x) as f32;
        let mut transform_y = (tile_center_y - ref_y) as f32;

        // In 3D mode, lower-zoom tiles are in a different pixel coordinate
        // system. Rescale their position and size so they align with the
        // current zoom level's world space.
        let zoom_diff = current_zoom.saturating_sub(event_zoom) as u32;
        let rescale = if view3d_state.is_3d_active() && zoom_diff > 0 {
            let s = (1u32 << zoom_diff) as f32; // 2, 4, 8, 16
            transform_x *= s;
            transform_y *= s;
            s
        } else {
            1.0
        };

        // Skip if a tile entity already exists at this position and zoom level
        let tile_key = (transform_x as i32, transform_y as i32, event_zoom);
        if !spawned_tiles.positions.insert(tile_key) {
            continue; // Already spawned
        }

        let tile_path = event.path.clone();
        let tile_handle = asset_server.load(tile_path.clone());
        asset_server.reload(tile_path);

        // Spawn new tiles translucent and slightly above old tiles so they
        // fade in on top, hiding the old zoom level progressively.
        // In 3D mode, spawn at ground elevation so tiles are coplanar with
        // airports, runways, and other ground-level features.
        // Lower-zoom tiles sit slightly below so higher-zoom tiles win depth.
        let tile_z = if view3d_state.is_3d_active() {
            view3d_state.altitude_to_z(view3d_state.ground_elevation_ft)
                - zoom_diff as f32 * 0.05
        } else {
            tile_settings.z_layer + 0.1
        };
        commands.spawn((
            Name::new(format!("Map Tile z{}", event_zoom)),
            Sprite {
                image: tile_handle,
                color: Color::srgba(1.0, 1.0, 1.0, 0.0),
                ..default()
            },
            Transform::from_xyz(transform_x, transform_y, tile_z)
                .with_scale(Vec3::splat(rescale)),
            MapTile,
            TileFadeState {
                alpha: 0.0,
                tile_zoom: event_zoom,
            },
        ));

        if let Some(ref log) = logger {
            log.log(&format!("TILE DISPLAYED: zoom={} pos=({:.0}, {:.0})",
                current_zoom, transform_x, transform_y));
        }
    }
}

/// Maximum number of tile entities allowed at any time.
/// In 3D mode, scales with camera altitude since higher = wider view = more tiles.
fn max_tile_entities(view3d_state: Option<&view3d::View3DState>) -> usize {
    if let Some(state) = view3d_state {
        if state.is_3d_active() {
            let alt_factor = (state.camera_altitude / 60000.0).clamp(0.0, 1.0);
            return 300 + (500.0 * alt_factor) as usize; // 300-800 range
        }
    }
    400 // Original 2D limit
}

/// Despawn tile entities that are far outside the visible viewport.
/// Without this, tiles accumulate indefinitely as the user pans, causing
/// frame time to grow continuously until the app becomes unresponsive.
fn cull_offscreen_tiles(
    mut commands: Commands,
    camera_query: Query<(&Transform, &Projection), With<MapCamera>>,
    tile_query: Query<(Entity, &Transform, &TileFadeState), With<MapTile>>,
    window_query: Query<&Window>,
    mut spawned_tiles: ResMut<SpawnedTiles>,
    view3d_state: Res<view3d::View3DState>,
    alt_tracker: Res<AltitudeChangeTracker>,
) {
    let Ok((camera_tf, projection)) = camera_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };

    let cam_x = camera_tf.translation.x;
    let cam_y = camera_tf.translation.y;

    // Compute culling extents depending on mode
    let (half_w, half_h, forward_bias_x, forward_bias_y) = if view3d_state.is_3d_active() {
        // 3D mode: compute ground footprint from frustum geometry
        let fov = 60.0_f32.to_radians();
        let aspect = window.width() / window.height();
        let half_vfov = fov / 2.0;
        let half_hfov = (aspect * half_vfov.tan()).atan();
        let pitch_rad = view3d_state.camera_pitch.to_radians();
        let effective_distance = view3d_state.altitude_to_distance();
        let camera_height = effective_distance * pitch_rad.sin();

        let far_angle = (pitch_rad - half_vfov).max(0.05);
        let far_ground_dist = camera_height / far_angle.tan();
        let center_ground_dist = effective_distance * pitch_rad.cos();
        let half_width_at_horizon = far_ground_dist * half_hfov.tan();

        // Widen margin during active altitude changes so tiles survive
        // long enough for replacements to load.  Cooldown of ~0.5s.
        let margin = if alt_tracker.idle_secs < 0.5 { 4.0 } else { 2.5 };
        let hw = half_width_at_horizon * margin;
        let hh = far_ground_dist.max(center_ground_dist) * margin;

        // Directional bias: extend forward culling margin by 1.5x, reduce backward to 1.0x
        let yaw_rad = view3d_state.camera_yaw.to_radians();
        let bias_magnitude = far_ground_dist * 0.25;
        let bias_x = bias_magnitude * yaw_rad.sin();
        let bias_y = bias_magnitude * yaw_rad.cos();

        (hw, hh, bias_x, bias_y)
    } else {
        // 2D mode: orthographic viewport extents
        let ortho_scale = if let Projection::Orthographic(ref ortho) = projection {
            ortho.scale
        } else {
            1.0
        };
        let margin = 1.5;
        let hw = (window.width() / 2.0) * ortho_scale * margin;
        let hh = (window.height() / 2.0) * ortho_scale * margin;
        (hw, hh, 0.0, 0.0)
    };

    // Effective center shifted by forward bias (tiles ahead of camera get extra margin)
    let center_x = cam_x + forward_bias_x;
    let center_y = cam_y + forward_bias_y;

    // Collect all tiles with their distance from camera for sorting
    let mut tiles: Vec<(Entity, f32, i32, i32, u8)> = tile_query
        .iter()
        .map(|(entity, tile_tf, fade_state)| {
            let dx = (tile_tf.translation.x - cam_x).abs();
            let dy = (tile_tf.translation.y - cam_y).abs();
            let dist = dx.max(dy); // Chebyshev distance
            (entity, dist, tile_tf.translation.x as i32, tile_tf.translation.y as i32, fade_state.tile_zoom)
        })
        .collect();

    let mut culled = 0u32;

    // First pass: cull tiles outside the viewport margin (using biased center)
    tiles.retain(|&(entity, _, tx, ty, zoom)| {
        let dx = (tx as f32 - center_x).abs();
        let dy = (ty as f32 - center_y).abs();
        if dx > half_w || dy > half_h {
            spawned_tiles.positions.remove(&(tx, ty, zoom));
            commands.entity(entity).despawn();
            culled += 1;
            false
        } else {
            true
        }
    });

    // Second pass: if still over budget, cull farthest tiles.
    // Raise the limit during active altitude changes to avoid thrashing.
    let base_limit = max_tile_entities(Some(&view3d_state));
    let tile_limit = if view3d_state.is_3d_active() && alt_tracker.idle_secs < 0.5 {
        base_limit + 200
    } else {
        base_limit
    };
    if tiles.len() > tile_limit {
        tiles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for &(entity, _, tx, ty, zoom) in &tiles[..tiles.len() - tile_limit] {
            spawned_tiles.positions.remove(&(tx, ty, zoom));
            commands.entity(entity).despawn();
            culled += 1;
        }
    }

    if culled > 0 {
        debug!("Culled {} tiles (remaining: {})", culled, tiles.len().min(tile_limit));
    }
}

/// Animate tile fade-in and despawn old tiles only when covered by fully-loaded new tiles.
fn animate_tile_fades(
    mut commands: Commands,
    time: Res<Time>,
    map_state: Res<MapState>,
    mut tile_query: Query<(Entity, &mut TileFadeState, &mut Sprite, &Transform), With<MapTile>>,
    mut spawned_tiles: ResMut<SpawnedTiles>,
    view3d_state: Res<view3d::View3DState>,
) {
    let delta = time.delta_secs();
    let current_zoom = map_state.zoom_level.to_u8();

    // In 3D mode, tiles within the multi-resolution band (current_zoom to
    // current_zoom - 4) are intentional and should NOT be treated as "old."
    let is_3d = view3d_state.is_3d_active();

    // Collect grid cells covered by fully-opaque new tiles.
    // Quantize positions to 256px cells so old (rescaled) tiles can be matched.
    let mut loaded_cells: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    let mut old_tiles: Vec<(Entity, i32, i32, u8)> = Vec::new();

    for (entity, mut fade_state, mut sprite, transform) in tile_query.iter_mut() {
        let dominated = if is_3d {
            // In 3D mode, only tiles outside the multi-resolution band are "old"
            fade_state.tile_zoom > current_zoom || current_zoom - fade_state.tile_zoom > 4
        } else {
            fade_state.tile_zoom != current_zoom
        };

        if !dominated {
            // Current / active tile: fade in
            if fade_state.alpha < 1.0 {
                fade_state.alpha += constants::TILE_FADE_SPEED * delta;
                fade_state.alpha = fade_state.alpha.min(1.0);
                sprite.color = Color::srgba(1.0, 1.0, 1.0, fade_state.alpha);
            }
            // Track fully-opaque tiles by grid cell
            if fade_state.alpha >= 1.0 {
                let cell = (
                    (transform.translation.x / constants::DEFAULT_TILE_PIXELS).round() as i32,
                    (transform.translation.y / constants::DEFAULT_TILE_PIXELS).round() as i32,
                );
                loaded_cells.insert(cell);
            }
        } else {
            // Old-zoom tile: record for coverage check
            let cell = (
                (transform.translation.x / constants::DEFAULT_TILE_PIXELS).round() as i32,
                (transform.translation.y / constants::DEFAULT_TILE_PIXELS).round() as i32,
            );
            old_tiles.push((entity, cell.0, cell.1, fade_state.tile_zoom));
        }
    }

    // Despawn old tiles whose grid cell is covered by a fully-loaded new tile.
    // Remove from spawned_tiles so the position can be re-used.
    for (entity, cx, cy, zoom) in old_tiles {
        if loaded_cells.contains(&(cx, cy)) {
            let tx = (cx as f32 * constants::DEFAULT_TILE_PIXELS) as i32;
            let ty = (cy as f32 * constants::DEFAULT_TILE_PIXELS) as i32;
            spawned_tiles.positions.remove(&(tx, ty, zoom));
            commands.entity(entity).despawn();
        }
    }
}

// =============================================================================
// 3D Mesh Quad Systems
// =============================================================================

/// Create the shared mesh used by all tile 3D quads.
fn setup_tile_quad_mesh(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(constants::DEFAULT_TILE_PIXELS / 2.0)));
    commands.insert_resource(TileQuadMesh(mesh));
}

/// Spawn or despawn 3D mesh quad companions for tile entities based on view mode.
///
/// In 3D mode: for each tile lacking a `TileMeshQuad`, spawn a `StandardMaterial`
/// mesh entity using the sprite's texture. In 2D mode: despawn all companions.
fn sync_tile_mesh_quads(
    mut commands: Commands,
    view3d_state: Res<view3d::View3DState>,
    quad_mesh: Option<Res<TileQuadMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tiles_without_quad: Query<(Entity, &Sprite, &Transform, &TileFadeState), (With<MapTile>, Without<TileMeshQuad>)>,
    tiles_with_quad: Query<(Entity, &TileMeshQuad), With<MapTile>>,
    all_quad_entities: Query<Entity, With<TileQuad3d>>,
) {
    let Some(quad_mesh) = quad_mesh else { return };

    if view3d_state.is_3d_active() {
        // Spawn mesh companions for tiles that don't have one yet
        for (tile_entity, sprite, transform, fade_state) in tiles_without_quad.iter() {
            // Always use Opaque so the mesh writes depth. Without depth writes,
            // the atmosphere post-process treats these pixels as sky and
            // overwrites them (same issue as aircraft GLB models).
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(sprite.image.clone()),
                base_color: Color::WHITE,
                unlit: true,
                alpha_mode: AlphaMode::Opaque,
                ..default()
            });

            let pos_yup = view3d::zup_to_yup(transform.translation);
            let mesh_entity = commands.spawn((
                TileQuad3d,
                Mesh3d(quad_mesh.0.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(pos_yup)
                    .with_scale(Vec3::new(transform.scale.x, 1.0, transform.scale.x)),
                Pickable::IGNORE,
            )).id();

            commands.entity(tile_entity).queue_silenced(move |mut entity: EntityWorldMut| {
                entity.insert(TileMeshQuad(mesh_entity));
            });
        }
    } else {
        // 2D mode: remove TileMeshQuad component from tiles
        for (tile_entity, _) in tiles_with_quad.iter() {
            commands.entity(tile_entity).remove::<TileMeshQuad>();
        }
        // Despawn all companion mesh entities (referenced + orphans)
        for quad_entity in all_quad_entities.iter() {
            commands.entity(quad_entity).despawn();
        }
    }
}

/// Sync mesh companion visibility with the tile's fade state.
/// Mesh quads are always AlphaMode::Opaque (required for depth writes so the
/// atmosphere post-process doesn't overwrite them). Tiles that haven't finished
/// loading (alpha near 0) are hidden entirely to avoid showing placeholder white.
fn sync_tile_mesh_alpha(
    view3d_state: Res<view3d::View3DState>,
    tile_query: Query<(&TileFadeState, &TileMeshQuad), With<MapTile>>,
    mut vis_query: Query<&mut Visibility>,
) {
    if !view3d_state.is_3d_active() {
        return;
    }

    for (fade_state, quad) in tile_query.iter() {
        let Ok(mut vis) = vis_query.get_mut(quad.0) else { continue };
        // Show the mesh quad once the tile texture has started loading.
        // A very low threshold (0.01) makes tiles appear within 1 frame of
        // spawning, reducing the dark-flash gap during rapid zoom.  The mesh
        // uses AlphaMode::Opaque so any non-zero alpha is fully visible.
        *vis = if fade_state.alpha > 0.01 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Sync mesh companion transforms with tile sprite transforms.
fn sync_tile_mesh_transforms(
    view3d_state: Res<view3d::View3DState>,
    tile_query: Query<(&Transform, &TileMeshQuad), With<MapTile>>,
    mut mesh_transforms: Query<&mut Transform, Without<MapTile>>,
) {
    if !view3d_state.is_3d_active() {
        return;
    }

    for (tile_tf, quad) in tile_query.iter() {
        let Ok(mut mesh_tf) = mesh_transforms.get_mut(quad.0) else { continue };
        let pos_yup = view3d::zup_to_yup(tile_tf.translation);
        mesh_tf.translation = pos_yup;
        mesh_tf.scale = Vec3::new(tile_tf.scale.x, 1.0, tile_tf.scale.x);
    }
}

/// Hide tile sprites in 3D mode so Camera2d shows nothing for tiles.
/// In 2D mode this is a no-op since sprites already have correct alpha.
fn hide_tile_sprites_in_3d(
    view3d_state: Res<view3d::View3DState>,
    mut tile_query: Query<&mut Sprite, (With<MapTile>, With<TileMeshQuad>)>,
) {
    if !view3d_state.is_3d_active() {
        return;
    }

    for mut sprite in tile_query.iter_mut() {
        sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
    }
}

/// Despawn companion mesh entities that are no longer referenced by any tile.
/// This catches orphans created when tiles are culled in the same frame as
/// companion spawning (deferred commands mean the tile despawn and companion
/// spawn can race).
fn cleanup_orphaned_tile_quads(
    mut commands: Commands,
    view3d_state: Res<view3d::View3DState>,
    quad_entities: Query<Entity, With<TileQuad3d>>,
    tile_quads: Query<&TileMeshQuad, With<MapTile>>,
) {
    // In 2D mode, sync_tile_mesh_quads already despawns all companions
    if !view3d_state.is_3d_active() {
        return;
    }
    let referenced: std::collections::HashSet<Entity> =
        tile_quads.iter().map(|q| q.0).collect();
    for entity in quad_entities.iter() {
        if !referenced.contains(&entity) {
            commands.entity(entity).despawn();
        }
    }
}
