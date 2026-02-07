use bevy::{prelude::*, input::mouse::MouseWheel, ecs::schedule::ApplyDeferred};
use bevy_slippy_tiles::*;
use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};

mod config;
mod data;
mod geo;
mod units;
mod map;
mod aviation;
mod aircraft;
mod adsb;
mod keyboard;
mod weather;
mod bookmarks;
mod recording;
mod tools;
mod coverage;
mod airspace;
mod data_sources;
mod export;
mod view3d;
mod ui_panels;
mod toolbar;
mod tools_window;

// Re-export core types so crate::Aircraft, crate::MapState, crate::ZoomState
// continue to resolve throughout the codebase.
pub(crate) use aircraft::components::{Aircraft, AircraftLabel};
pub(crate) use map::{MapState, ZoomState};
use config::ConfigPlugin;
use keyboard::{HelpOverlayState, handle_keyboard_shortcuts, toggle_overlays_keyboard, update_help_overlay, sync_panel_manager_to_resources, sync_resources_to_panel_manager};
use bevy_egui::EguiContexts;

// ADS-B client types

// =============================================================================
// Constants - All magic numbers centralized here
// =============================================================================

#[allow(dead_code)]  // Some constants defined for future use
pub(crate) mod constants {
    // Mercator projection limits
    pub const MERCATOR_LAT_LIMIT: f64 = 85.0511;

    // Zoom thresholds for tile level transitions (use stdlib constants)
    pub const ZOOM_UPGRADE_THRESHOLD: f32 = std::f32::consts::SQRT_2;
    pub const ZOOM_DOWNGRADE_THRESHOLD: f32 = std::f32::consts::FRAC_1_SQRT_2;

    // Camera zoom bounds
    pub const MIN_CAMERA_ZOOM: f32 = 0.1;
    pub const MAX_CAMERA_ZOOM: f32 = 10.0;

    // Tile download settings
    pub const TILE_DOWNLOAD_RADIUS: u8 = 3;

    // Zoom sensitivity
    pub const ZOOM_SENSITIVITY_LINE: f32 = 0.1;  // Mouse wheel
    pub const ZOOM_SENSITIVITY_PIXEL: f32 = 0.002;  // Trackpad

    // Movement threshold for tile requests (degrees, ~100m at equator)
    pub const PAN_TILE_REQUEST_THRESHOLD: f64 = 0.001;

    // UI and rendering
    pub const BASE_FONT_SIZE: f32 = 14.0;
    pub const AIRCRAFT_MARKER_RADIUS: f32 = 8.0;
    pub const LABEL_SCREEN_OFFSET: f32 = 25.0;
    pub const BUTTON_FONT_SIZE: f32 = 16.0;

    // Tile fade/despawn timing
    pub const TILE_FADE_SPEED: f32 = 4.0;
    pub const OLD_TILE_DESPAWN_DELAY: f32 = 0.4;

    // Z-layers
    pub const TILE_Z_LAYER: f32 = 0.0;
    pub const AIRCRAFT_Z_LAYER: f32 = 10.0;
    pub const LABEL_Z_LAYER: f32 = 11.0;

    // UI colors
    pub const BUTTON_NORMAL: (f32, f32, f32, f32) = (0.2, 0.2, 0.2, 0.9);
    pub const BUTTON_HOVERED: (f32, f32, f32, f32) = (0.3, 0.3, 0.3, 0.9);
    pub const BUTTON_PRESSED: (f32, f32, f32, f32) = (0.4, 0.4, 0.4, 0.9);
    pub const OVERLAY_BG: (f32, f32, f32, f32) = (0.0, 0.0, 0.0, 0.5);

    // Default map center (Wichita, KS)
    pub const DEFAULT_LATITUDE: f64 = 37.6872;
    pub const DEFAULT_LONGITUDE: f64 = -97.3301;

    // ADS-B connection settings
    pub const ADSB_SERVER_ADDRESS: &str = "98.186.33.60:30003";
    pub const ADSB_MAX_DISTANCE_MILES: f64 = 250.0;
    pub const ADSB_AIRCRAFT_TIMEOUT_SECS: i64 = 180;
}

// =============================================================================
// Coordinate Helpers - Centralized coordinate conversion functions
// =============================================================================

/// Clamp latitude to valid Mercator projection range
fn clamp_latitude(lat: f64) -> f64 {
    lat.clamp(-constants::MERCATOR_LAT_LIMIT, constants::MERCATOR_LAT_LIMIT)
}

/// Clamp longitude to valid range
fn clamp_longitude(lon: f64) -> f64 {
    lon.clamp(-180.0, 180.0)
}

// =============================================================================
// Zoom Calculation Helpers
// =============================================================================

/// Convert mouse wheel event to zoom delta factor
/// Returns positive for zoom in, negative for zoom out
fn calculate_zoom_delta(event: &MouseWheel) -> f32 {
    match event.unit {
        bevy::input::mouse::MouseScrollUnit::Line => {
            event.y * constants::ZOOM_SENSITIVITY_LINE
        }
        bevy::input::mouse::MouseScrollUnit::Pixel => {
            event.y * constants::ZOOM_SENSITIVITY_PIXEL
        }
    }
}

/// Calculate new map center to keep the point under cursor stationary during zoom
///
/// Returns the new (latitude, longitude) for the map center
fn calculate_zoom_to_cursor_center(
    cursor_viewport_pos: Vec2,
    window_size: (f32, f32),
    current_center: (f64, f64),
    camera_zoom_before: f32,
    camera_zoom_after: f32,
    old_tile_zoom: ZoomLevel,
    new_tile_zoom: ZoomLevel,
) -> (f64, f64) {
    // Calculate cursor offset from screen center
    let screen_center = (window_size.0 / 2.0, window_size.1 / 2.0);
    let cursor_offset = (
        (cursor_viewport_pos.x - screen_center.0) as f64,
        -(cursor_viewport_pos.y - screen_center.1) as f64, // Y inverted
    );

    // Convert to world pixels before zoom (using old camera zoom)
    let world_offset_before = (
        cursor_offset.0 / camera_zoom_before as f64,
        cursor_offset.1 / camera_zoom_before as f64,
    );

    // Get current center in world pixels at old zoom level
    let center_pixel = world_coords_to_world_pixel(
        &LatitudeLongitudeCoordinates {
            latitude: current_center.0,
            longitude: current_center.1,
        },
        TileSize::Normal,
        old_tile_zoom,
    );

    // Calculate cursor geographic position at old zoom level
    let cursor_geo = world_pixel_to_world_coords(
        center_pixel.0 + world_offset_before.0,
        center_pixel.1 + world_offset_before.1,
        TileSize::Normal,
        old_tile_zoom,
    );

    // Calculate world offset after zoom (using new camera zoom)
    let world_offset_after = (
        cursor_offset.0 / camera_zoom_after as f64,
        cursor_offset.1 / camera_zoom_after as f64,
    );

    // Convert cursor geo back to pixels at new zoom level
    let cursor_pixel_after = world_coords_to_world_pixel(
        &LatitudeLongitudeCoordinates {
            latitude: cursor_geo.latitude,
            longitude: cursor_geo.longitude,
        },
        TileSize::Normal,
        new_tile_zoom,
    );

    // New center = cursor position minus the offset
    let new_center = world_pixel_to_world_coords(
        cursor_pixel_after.0 - world_offset_after.0,
        cursor_pixel_after.1 - world_offset_after.1,
        TileSize::Normal,
        new_tile_zoom,
    );

    (new_center.latitude, new_center.longitude)
}

// =============================================================================
// Tile Download Helper
// =============================================================================

/// Compute the tile download radius needed to cover the viewport.
///
/// In 2D (orthographic): each tile occupies `256 * camera_zoom` screen pixels.
/// In 3D (perspective): the tilted camera sees a larger ground footprint, so we
/// estimate the visible ground extent from the camera distance, pitch, and FOV.
fn compute_tile_radius(
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
            let effective_distance = state.camera_distance * 20.0; // PIXEL_SCALE
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
            let tiles_needed = (max_ground_extent / 256.0).ceil() as u8;
            return tiles_needed.clamp(3, 12);
        }
    }

    // 2D orthographic mode
    let tile_screen_px = 256.0 * camera_zoom;
    let half_tiles_x = (window_width / (2.0 * tile_screen_px)).ceil() as u8;
    let half_tiles_y = (window_height / (2.0 * tile_screen_px)).ceil() as u8;
    half_tiles_x.max(half_tiles_y).clamp(3, 8)
}

/// Send a tile download request for the current map location
pub fn request_tiles_at_location(
    download_events: &mut MessageWriter<DownloadSlippyTilesMessage>,
    latitude: f64,
    longitude: f64,
    zoom_level: ZoomLevel,
    use_cache: bool,
) {
    download_events.write(DownloadSlippyTilesMessage {
        tile_size: TileSize::Normal,
        zoom_level,
        coordinates: Coordinates::from_latitude_longitude(latitude, longitude),
        radius: Radius(constants::TILE_DOWNLOAD_RADIUS),
        use_cache,
    });
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "AirJedi - Aircraft Map Tracker".to_string(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            }),
            SlippyTilesPlugin,
            ConfigPlugin,
            aviation::AviationPlugin,
            aircraft::AircraftPlugin,
            weather::WeatherPlugin,
            bookmarks::BookmarksPlugin,
            recording::RecordingPlugin,
            tools::ToolsPlugin,
            coverage::CoveragePlugin,
            airspace::AirspacePlugin,
            data_sources::DataSourcesPlugin,
            export::ExportPlugin,
            view3d::View3DPlugin,
            adsb::AdsbPlugin,
        ))
        .init_resource::<DragState>()
        .init_resource::<HelpOverlayState>()
        .init_resource::<ui_panels::UiPanelManager>()
        .init_resource::<tools_window::ToolsWindowState>()
        .insert_resource(ZoomState::new())
        // SlippyTilesSettings will be updated by setup_slippy_tiles_from_config after config is loaded
        .insert_resource(SlippyTilesSettings {
            endpoint: config::BasemapStyle::default().endpoint_url().to_string(),
            tiles_directory: std::path::PathBuf::from(""),  // Root assets directory
            reference_latitude: constants::DEFAULT_LATITUDE,   // Wichita, KS (matches MapState default)
            reference_longitude: constants::DEFAULT_LONGITUDE,  // Wichita, KS (matches MapState default)
            z_layer: 0.0,                  // Render tiles at z=0 (behind aircraft at z=10)
            auto_render: false,            // Disable auto-render, we handle tile display ourselves
            ..default()
        })
        .add_systems(Startup, (setup_debug_logger, setup_map))
        .add_systems(Update, check_egui_wants_input.before(handle_pan_drag).before(handle_zoom))
        .add_systems(Update, handle_pan_drag)
        .add_systems(Update, handle_zoom)
        .add_systems(Update, handle_window_resize)
        .add_systems(Update, handle_3d_view_tile_refresh)
        // Apply deferred commands (like despawns) before updating camera/tiles
        // This ensures old tiles are gone before new camera position is applied
        .add_systems(Update, ApplyDeferred.after(handle_zoom))
        .add_systems(Update, apply_camera_zoom.after(ApplyDeferred))
        .add_systems(Update, follow_aircraft.after(adsb::sync_aircraft_from_adsb))
        .add_systems(Update, update_camera_position.after(handle_pan_drag).after(apply_camera_zoom).after(follow_aircraft))
        .add_systems(Update, update_aircraft_positions.after(update_camera_position).after(adsb::sync_aircraft_from_adsb))
        .add_systems(Update, scale_aircraft_and_labels.after(apply_camera_zoom))
        .add_systems(Update, update_aircraft_labels.after(update_aircraft_positions))
        .add_systems(bevy_egui::EguiPrimaryContextPass, (
            toolbar::render_toolbar,
            toolbar::render_map_attribution,
            tools_window::render_tools_window,
        ))
        .add_systems(Update, display_tiles_filtered.after(ApplyDeferred))
        .add_systems(Update, animate_tile_fades.after(display_tiles_filtered))
        .add_systems(Update, handle_keyboard_shortcuts)
        .add_systems(Update, toggle_overlays_keyboard)
        .add_systems(Update, sync_panel_manager_to_resources.after(handle_keyboard_shortcuts))
        .add_systems(Update, sync_resources_to_panel_manager.after(sync_panel_manager_to_resources))
        .add_systems(Update, update_help_overlay)
        .run();
}


// Component to track tile fade state for smooth zoom transitions
#[derive(Component)]
struct TileFadeState {
    alpha: f32,
    /// If Some, this tile is from an old zoom level and will despawn after the timer expires
    despawn_delay: Option<f32>,
}

// Resource to track pan/drag state
#[derive(Resource, Default)]
struct DragState {
    is_dragging: bool,
    last_position: Option<Vec2>,
    last_tile_request_coords: Option<(f64, f64)>,
}

// Resource to hold debug log file handle
#[derive(Resource, Clone)]
struct ZoomDebugLogger {
    file: Arc<Mutex<std::fs::File>>,
}

impl ZoomDebugLogger {
    fn log(&self, msg: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{}", msg);
            let _ = file.flush();
        }
    }
}

// ADS-B integration is in src/adsb/ module

fn setup_debug_logger(mut commands: Commands) {
    use std::fs::OpenOptions;

    // Create or truncate the debug log file in tmp directory (per project conventions)
    let log_path = std::env::current_dir()
        .ok()
        .map(|path| {
            let tmp_dir = path.join("tmp");
            // Ensure tmp directory exists
            let _ = std::fs::create_dir_all(&tmp_dir);
            tmp_dir.join("zoom_debug.log")
        })
        .unwrap_or_else(|| std::path::PathBuf::from("tmp/zoom_debug.log"));

    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
    {
        Ok(file) => {
            let logger = ZoomDebugLogger {
                file: Arc::new(Mutex::new(file)),
            };
            logger.log("=== ZOOM DEBUG LOG INITIALIZED ===");
            commands.insert_resource(logger);
            info!("Debug logging enabled to {:?}", log_path);
        }
        Err(e) => {
            warn!("Failed to create debug log file: {}", e);
        }
    }
}

pub(crate) fn setup_map(
    mut commands: Commands,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    mut tile_settings: ResMut<SlippyTilesSettings>,
    app_config: Res<config::AppConfig>,
) {
    // Set up camera
    commands.spawn(Camera2d);

    // Update SlippyTilesSettings from config
    tile_settings.endpoint = app_config.map.basemap_style.endpoint_url().to_string();
    tile_settings.reference_latitude = app_config.map.default_latitude;
    tile_settings.reference_longitude = app_config.map.default_longitude;

    // Initialize map state resource from config
    let map_state = MapState {
        latitude: app_config.map.default_latitude,
        longitude: app_config.map.default_longitude,
        zoom_level: ZoomLevel::try_from(app_config.map.default_zoom).unwrap_or(ZoomLevel::L10),
    };

    // Send initial tile download request
    request_tiles_at_location(
        &mut download_events,
        map_state.latitude,
        map_state.longitude,
        map_state.zoom_level,
        true,
    );

    commands.insert_resource(map_state);
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
            tile_size: TileSize::Normal,
            zoom_level: map_state.zoom_level,
            coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
            radius: Radius(radius),
            use_cache: true,
        });
    }
}

/// Re-request tiles when 3D view state changes (entering/exiting 3D, orbit, pitch, distance)
/// so the larger perspective footprint is covered.
fn handle_3d_view_tile_refresh(
    view3d_state: Res<view3d::View3DState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    map_state: Res<MapState>,
    zoom_state: Res<ZoomState>,
    window_query: Query<&Window>,
) {
    if !view3d_state.is_changed() {
        return;
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
        tile_size: TileSize::Normal,
        zoom_level: map_state.zoom_level,
        coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
        radius: Radius(radius),
        use_cache: true,
    });
}

// Aircraft texture setup, sync, label update, and connection status
// are now in the adsb module.

fn check_egui_wants_input(
    mut contexts: EguiContexts,
    mut drag_state: ResMut<DragState>,
) {
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() || ctx.wants_pointer_input() {
            // Reset drag state to prevent map panning while in settings
            drag_state.is_dragging = false;
            drag_state.last_position = None;
        }
    }
}

fn handle_pan_drag(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut cursor_moved: MessageReader<CursorMoved>,
    mut map_state: ResMut<MapState>,
    mut drag_state: ResMut<DragState>,
    zoom_state: Res<ZoomState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    mut follow_state: ResMut<aircraft::CameraFollowState>,
    window_query: Query<&Window>,
) {
    let Ok(_window) = window_query.single() else {
        return;
    };

    // Check if left mouse button is pressed
    if mouse_button.just_pressed(MouseButton::Left) {
        drag_state.is_dragging = true;
        drag_state.last_position = None;
    }

    if mouse_button.just_released(MouseButton::Left) {
        drag_state.is_dragging = false;
        drag_state.last_position = None;

        // Request new tiles after drag completes
        if let Some((last_lat, last_lon)) = drag_state.last_tile_request_coords {
            // Only request if moved significantly (more than ~100m at equator)
            let lat_diff = (map_state.latitude - last_lat).abs();
            let lon_diff = (map_state.longitude - last_lon).abs();
            if lat_diff > constants::PAN_TILE_REQUEST_THRESHOLD
                || lon_diff > constants::PAN_TILE_REQUEST_THRESHOLD
            {
                request_tiles_at_location(
                    &mut download_events,
                    map_state.latitude,
                    map_state.longitude,
                    map_state.zoom_level,
                    true,
                );
                drag_state.last_tile_request_coords = Some((map_state.latitude, map_state.longitude));
            }
        } else {
            drag_state.last_tile_request_coords = Some((map_state.latitude, map_state.longitude));
        }
    }

    // Handle dragging
    if drag_state.is_dragging {
        for event in cursor_moved.read() {
            if let Some(last_pos) = drag_state.last_position {
                let delta = event.position - last_pos;

                // Break follow mode when user manually pans
                if delta.length() > 2.0 && follow_state.following_icao.is_some() {
                    follow_state.following_icao = None;
                }

                // Convert screen delta to world delta (account for ortho projection)
                // When ortho.scale = 1/camera_zoom, world_delta = screen_delta / camera_zoom
                let delta_world_x = -(delta.x as f64) / zoom_state.camera_zoom as f64; // Negative for natural pan direction
                let delta_world_y = (delta.y as f64) / zoom_state.camera_zoom as f64;

                // Get current center in world pixels
                let center_ll = LatitudeLongitudeCoordinates {
                    latitude: map_state.latitude,
                    longitude: map_state.longitude,
                };
                let center_pixel = world_coords_to_world_pixel(
                    &center_ll,
                    TileSize::Normal,
                    map_state.zoom_level
                );

                // Calculate new center in world pixels
                let new_center_x = center_pixel.0 + delta_world_x;
                let new_center_y = center_pixel.1 + delta_world_y;

                // Convert back to geographic coordinates
                let new_center_geo = world_pixel_to_world_coords(
                    new_center_x,
                    new_center_y,
                    TileSize::Normal,
                    map_state.zoom_level
                );

                // Update map coordinates
                map_state.latitude = clamp_latitude(new_center_geo.latitude);
                map_state.longitude = clamp_longitude(new_center_geo.longitude);
            }
            drag_state.last_position = Some(event.position);
        }
    }
}

/// System to follow a selected aircraft (moves map center to aircraft position)
fn follow_aircraft(
    mut map_state: ResMut<MapState>,
    follow_state: Res<aircraft::CameraFollowState>,
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
    mut camera_query: Query<&mut Transform, With<Camera2d>>,
    logger: Option<Res<ZoomDebugLogger>>,
) {
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

fn handle_zoom(
    mut scroll_events: MessageReader<MouseWheel>,
    mut map_state: ResMut<MapState>,
    mut zoom_state: ResMut<ZoomState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    window_query: Query<&Window>,
    mut tile_query: Query<(&mut TileFadeState, &mut Transform), With<MapTile>>,
    logger: Option<Res<ZoomDebugLogger>>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    // Macro to log to both console and file
    macro_rules! log_info {
        ($($arg:tt)*) => {
            {
                let msg = format!($($arg)*);
                info!("{}", msg);
                if let Some(ref log) = logger {
                    log.log(&msg);
                }
            }
        };
    }

    // Use constants for zoom level transition thresholds
    use constants::{ZOOM_UPGRADE_THRESHOLD, ZOOM_DOWNGRADE_THRESHOLD};

    for event in scroll_events.read() {
        log_info!("=== SCROLL EVENT START ===");
        log_info!("Event: unit={:?}, y={}", event.unit, event.y);
        log_info!("Before: camera_zoom={}, zoom_level={}", zoom_state.camera_zoom, map_state.zoom_level.to_u8());
        log_info!("Before: map center=({:.6}, {:.6})", map_state.latitude, map_state.longitude);

        // === Calculate zoom delta from scroll event ===
        let zoom_delta = calculate_zoom_delta(event);
        log_info!("Zoom delta: {}", zoom_delta);

        // Get cursor position in viewport coordinates (None if cursor not in window)
        let Some(cursor_viewport_pos) = window.cursor_position() else {
            // No cursor, just zoom at center
            let zoom_factor = 1.0 - zoom_delta;
            zoom_state.camera_zoom = (zoom_state.camera_zoom * zoom_factor)
                .clamp(zoom_state.min_zoom, zoom_state.max_zoom);
            log_info!("No cursor - new camera_zoom={}", zoom_state.camera_zoom);
            continue;
        };

        log_info!("Cursor position: ({:.2}, {:.2})", cursor_viewport_pos.x, cursor_viewport_pos.y);

        // Save old camera zoom BEFORE applying scroll zoom (needed for zoom-to-cursor)
        let camera_zoom_before_scroll = zoom_state.camera_zoom;

        // Update camera zoom (multiplicative for smooth feel)
        // Positive scroll = zoom in (smaller scale), negative = zoom out (larger scale)
        let zoom_factor = 1.0 - zoom_delta;
        let new_camera_zoom = (zoom_state.camera_zoom * zoom_factor)
            .clamp(zoom_state.min_zoom, zoom_state.max_zoom);

        log_info!("Camera zoom: {} -> {}", zoom_state.camera_zoom, new_camera_zoom);
        zoom_state.camera_zoom = new_camera_zoom;

        // === Check for zoom level transitions ===
        let old_tile_zoom = map_state.zoom_level;
        let current_tile_zoom = old_tile_zoom.to_u8();
        let mut zoom_level_changed = false;
        if zoom_state.camera_zoom >= ZOOM_UPGRADE_THRESHOLD && current_tile_zoom < 19 {
            // Upgrade zoom level
            log_info!("*** ZOOM LEVEL TRANSITION: UPGRADE ***");
            log_info!("  Threshold check: camera_zoom={} >= threshold={}", zoom_state.camera_zoom, ZOOM_UPGRADE_THRESHOLD);
            let old_cam = zoom_state.camera_zoom;
            zoom_state.camera_zoom /= 2.0;
            log_info!("  Camera zoom adjusted: {} -> {}", old_cam, zoom_state.camera_zoom);
            if let Ok(new_zoom_level) = ZoomLevel::try_from(current_tile_zoom + 1) {
                log_info!("  Zoom level: {} -> {}", current_tile_zoom, current_tile_zoom + 1);
                map_state.zoom_level = new_zoom_level;
                zoom_level_changed = true;
            }
        } else if zoom_state.camera_zoom <= ZOOM_DOWNGRADE_THRESHOLD && current_tile_zoom > 0 {
            // Downgrade zoom level
            log_info!("*** ZOOM LEVEL TRANSITION: DOWNGRADE ***");
            log_info!("  Threshold check: camera_zoom={} <= threshold={}", zoom_state.camera_zoom, ZOOM_DOWNGRADE_THRESHOLD);
            let old_cam = zoom_state.camera_zoom;
            zoom_state.camera_zoom *= 2.0;
            log_info!("  Camera zoom adjusted: {} -> {}", old_cam, zoom_state.camera_zoom);
            if let Ok(new_zoom_level) = ZoomLevel::try_from(current_tile_zoom - 1) {
                log_info!("  Zoom level: {} -> {}", current_tile_zoom, current_tile_zoom - 1);
                map_state.zoom_level = new_zoom_level;
                zoom_level_changed = true;
            }
        }

        // === Calculate new center (zoom-to-cursor) ===
        log_info!("--- Zoom-to-cursor calculation ---");
        log_info!("  old_zoom_level={}, new_zoom_level={}, zoom_level_changed={}",
            old_tile_zoom.to_u8(), map_state.zoom_level.to_u8(), zoom_level_changed);

        let old_lat = map_state.latitude;
        let old_lon = map_state.longitude;
        let (new_lat, new_lon) = calculate_zoom_to_cursor_center(
            cursor_viewport_pos,
            (window.width(), window.height()),
            (map_state.latitude, map_state.longitude),
            camera_zoom_before_scroll,
            zoom_state.camera_zoom,
            old_tile_zoom,
            map_state.zoom_level,
        );
        map_state.latitude = clamp_latitude(new_lat);
        map_state.longitude = clamp_longitude(new_lon);
        log_info!("  Map center updated: ({:.6}, {:.6}) -> ({:.6}, {:.6})", old_lat, old_lon, map_state.latitude, map_state.longitude);

        // === Handle zoom level transition (scale old tiles, request new) ===
        if zoom_level_changed {
            // Calculate scale factor: when zooming IN, positions double; when zooming OUT, positions halve
            let scale_factor = if map_state.zoom_level.to_u8() > old_tile_zoom.to_u8() {
                2.0_f32 // Zooming in: scale up
            } else {
                0.5_f32 // Zooming out: scale down
            };

            // Scale and reposition old tiles to match new coordinate system, then mark for despawn
            let mut marked = 0;
            for (mut fade_state, mut transform) in tile_query.iter_mut() {
                // Scale the tile position to match the new zoom level's coordinate system
                transform.translation.x *= scale_factor;
                transform.translation.y *= scale_factor;
                // Also scale the tile itself so it visually matches
                transform.scale *= scale_factor;

                fade_state.despawn_delay = Some(constants::OLD_TILE_DESPAWN_DELAY);
                marked += 1;
            }
            log_info!("  Scaled {} tiles by {} and marked for delayed despawn", marked, scale_factor);

            request_tiles_at_location(
                &mut download_events,
                map_state.latitude,
                map_state.longitude,
                map_state.zoom_level,
                true,
            );
            log_info!("  Requested new tiles at zoom level {}", map_state.zoom_level.to_u8());
        }

        log_info!("=== SCROLL EVENT END ===
");
    }
}

// Apply the camera zoom to the actual camera projection
fn apply_camera_zoom(
    zoom_state: Res<ZoomState>,
    mut camera_query: Query<&mut Projection, With<Camera2d>>,
) {
    if let Ok(mut projection) = camera_query.single_mut() {
        // Access the OrthographicProjection within Projection
        if let Projection::Orthographic(ref mut ortho) = projection.as_mut() {
            // Use camera_zoom directly - tiles are already at correct world-space scale
            // Smaller scale = more zoomed in, larger scale = more zoomed out
            ortho.scale = 1.0 / zoom_state.camera_zoom;
        }
    }
}

// Keep aircraft and labels at constant screen size despite zoom changes
fn scale_aircraft_and_labels(
    zoom_state: Res<ZoomState>,
    mut aircraft_query: Query<&mut Transform, (With<Aircraft>, Without<AircraftLabel>)>,
    mut label_query: Query<(&mut Transform, &mut TextFont), With<AircraftLabel>>,
) {
    // ONLY update scales when zoom actually changes
    // This prevents triggering Bevy's change detection every frame
    if !zoom_state.is_changed() {
        return;
    }

    // To maintain constant SCREEN size:
    // - Orthographic projection: screen_size = world_size / ortho.scale
    // - ortho.scale = 1 / camera_zoom
    // - So: screen_size = world_size * camera_zoom
    // - For constant screen size: world_size = constant / camera_zoom
    // - Therefore: transform.scale = 1 / camera_zoom (which equals ortho.scale)
    let scale = 1.0 / zoom_state.camera_zoom;

    // Scale aircraft markers to maintain constant screen size
    for mut transform in aircraft_query.iter_mut() {
        transform.scale = Vec3::splat(scale);
    }

    // Scale label transforms to maintain constant screen size
    for (mut transform, mut text_font) in label_query.iter_mut() {
        transform.scale = Vec3::splat(scale);
        text_font.font_size = constants::BASE_FONT_SIZE;
    }
}

fn update_aircraft_positions(
    map_state: Res<MapState>,
    tile_settings: Res<SlippyTilesSettings>,
    mut aircraft_query: Query<(&Aircraft, &mut Transform)>,
) {
    // Position aircraft RELATIVE to SlippyTilesSettings.reference
    // This matches how bevy_slippy_tiles positions tiles
    let converter = geo::CoordinateConverter::new(&tile_settings, map_state.zoom_level);

    for (aircraft, mut transform) in aircraft_query.iter_mut() {
        let pos = converter.latlon_to_world(aircraft.latitude, aircraft.longitude);

        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        // Apply rotation based on heading (track angle), defaulting to north-facing if no heading data
        // Heading is clockwise from north, Bevy rotation is counter-clockwise, so negate heading
        if let Some(heading) = aircraft.heading {
            transform.rotation = Quat::from_rotation_z((-heading).to_radians());
        } else {
            // Default to north-facing (no rotation needed)
            transform.rotation = Quat::IDENTITY;
        }
    }
}

fn update_aircraft_labels(
    zoom_state: Res<ZoomState>,
    aircraft_query: Query<&Transform, With<Aircraft>>,
    mut label_query: Query<(&AircraftLabel, &mut Transform), Without<Aircraft>>,
) {
    // Use screen-space offset that adapts to camera zoom
    // This keeps labels at a constant visual distance from aircraft markers
    // World offset = screen_offset / camera_zoom (since ortho.scale = 1/camera_zoom)
    // When zoomed in (camera_zoom > 1), world offset is smaller
    // When zoomed out (camera_zoom < 1), world offset is larger
    let world_space_offset = constants::LABEL_SCREEN_OFFSET / zoom_state.camera_zoom;

    for (label, mut label_transform) in label_query.iter_mut() {
        if let Ok(aircraft_transform) = aircraft_query.get(label.aircraft_entity) {
            // Position label above and slightly to the right of the aircraft
            label_transform.translation.x = aircraft_transform.translation.x + world_space_offset;
            label_transform.translation.y = aircraft_transform.translation.y + world_space_offset;
        }
    }
}

pub fn clear_tile_cache() {
    // Get the assets directory path
    let assets_path = std::env::current_dir()
        .map(|path| path.join("assets"))
        .unwrap_or_else(|_| std::path::PathBuf::from("assets"));

    if !assets_path.exists() {
        warn!("Assets directory not found at {:?}", assets_path);
        return;
    }

    // Count tiles deleted
    let mut deleted_count = 0;

    // Read the assets directory
    if let Ok(entries) = fs::read_dir(&assets_path) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Check if it's a tile file (ends with .tile.png)
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.ends_with(".tile.png") {
                        // Delete the tile file
                        if let Err(e) = fs::remove_file(&path) {
                            warn!("Failed to delete tile {:?}: {}", path, e);
                        } else {
                            deleted_count += 1;
                        }
                    }
                }
            }
        }
    }

    info!("Cleared {} tile(s) from cache", deleted_count);
}

// Custom tile display system that filters tiles by current zoom level
// This prevents stale tiles from wrong zoom levels from being displayed
fn display_tiles_filtered(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    mut tile_events: MessageReader<SlippyTileDownloadedMessage>,
    logger: Option<Res<ZoomDebugLogger>>,
) {
    for event in tile_events.read() {
        info!("Received tile download event: zoom={}, path={:?}", event.zoom_level.to_u8(), event.path);

        // CRITICAL: Only display tiles that match the current zoom level
        // This prevents stale async downloads from wrong zoom levels from appearing
        if event.zoom_level != map_state.zoom_level {
            info!("TILE IGNORED: tile zoom {} != current zoom {}", event.zoom_level.to_u8(), map_state.zoom_level.to_u8());
            if let Some(ref log) = logger {
                log.log("=== TILE IGNORED (wrong zoom) ===");
                log.log(&format!("  tile zoom_level: {} (current map zoom: {})",
                    event.zoom_level.to_u8(), map_state.zoom_level.to_u8()));
            }
            continue;
        }

        info!("Spawning tile at zoom level {}", event.zoom_level.to_u8());

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

        // SlippyTile.to_latitude_longitude returns the NW corner of the tile.
        // Since Bevy sprites are centered on their Transform, we need to offset
        // by half a tile to position the sprite's center at the tile's center.
        // In world coords: +X is east, +Y is north
        // Tile center = NW corner + (half_tile_east, half_tile_south)
        //             = NW corner + (128, -128) for 256-pixel tiles
        let half_tile = event.tile_size.to_pixels() as f64 / 2.0;
        let tile_center_x = tile_x + half_tile;  // East of NW corner
        let tile_center_y = tile_y - half_tile;  // South of NW corner (in Bevy coords where +Y is north)

        let transform_x = (tile_center_x - ref_x) as f32;
        let transform_y = (tile_center_y - ref_y) as f32;

        // Load the tile image and force a reload to ensure fresh data from disk
        // This is necessary because AssetServer caches handles by path, and after
        // clearing the cache, we need to re-read the file from disk
        let tile_path = event.path.clone();
        let tile_handle = asset_server.load(tile_path.clone());
        asset_server.reload(tile_path);

        // Spawn the tile sprite at full opacity for immediate visibility
        commands.spawn((
            Sprite {
                image: tile_handle,
                color: Color::WHITE, // Full opacity
                ..default()
            },
            Transform::from_xyz(transform_x, transform_y, tile_settings.z_layer),
            MapTile,
            TileFadeState {
                alpha: 1.0,
                despawn_delay: None, // Not scheduled for despawn
            },
        ));

        if let Some(ref log) = logger {
            log.log("=== TILE DISPLAYED ===");
            log.log(&format!("  tile zoom_level: {} (current map zoom: {})",
                event.zoom_level.to_u8(), map_state.zoom_level.to_u8()));
            log.log(&format!("  tile coords: ({:.6}, {:.6})", current_coords.latitude, current_coords.longitude));
            log.log(&format!("  tile transform: ({:.2}, {:.2})", transform_x, transform_y));
        }
    }
}

// Animate tile fade-in and handle delayed despawn for smooth zoom transitions
// Uses crossfade technique: new tiles fade in ON TOP of old tiles, old tiles stay
// fully visible until they're covered, then get despawned after a delay.
fn animate_tile_fades(
    mut commands: Commands,
    time: Res<Time>,
    mut tile_query: Query<(Entity, &mut TileFadeState, &mut Sprite), With<MapTile>>,
) {
    let delta = time.delta_secs();

    for (entity, mut fade_state, mut sprite) in tile_query.iter_mut() {
        // Handle tiles scheduled for despawn (old tiles from previous zoom level)
        if let Some(ref mut delay) = fade_state.despawn_delay {
            *delay -= delta;
            if *delay <= 0.0 {
                // Timer expired - despawn the old tile (it's hidden under new tiles anyway)
                commands.entity(entity).despawn();
            }
            // Old tiles stay at their current alpha (fully visible) - don't change anything
            continue;
        }

        // Handle tiles fading in (new tiles)
        if fade_state.alpha < 1.0 {
            fade_state.alpha += constants::TILE_FADE_SPEED * delta;
            fade_state.alpha = fade_state.alpha.min(1.0);

            // Update sprite color alpha
            sprite.color = Color::srgba(1.0, 1.0, 1.0, fade_state.alpha);
        }
    }
}
