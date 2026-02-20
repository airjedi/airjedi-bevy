use bevy::{prelude::*, camera::visibility::RenderLayers, gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore}, input::mouse::MouseWheel, input::gestures::PinchGesture, ecs::schedule::ApplyDeferred, light::SunDisk, pbr::ScatteringMedium};
use bevy_slippy_tiles::*;
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
mod debug_panel;
mod dock;
mod statusbar;
mod tile_cache;
pub(crate) mod theme;

// Re-export core types so crate::Aircraft, crate::MapState, crate::ZoomState
// continue to resolve throughout the codebase.
pub(crate) use aircraft::components::{Aircraft, AircraftLabel};
pub(crate) use map::{MapState, ZoomState};
use config::ConfigPlugin;
use keyboard::{HelpOverlayState, handle_keyboard_shortcuts, toggle_overlays_keyboard, update_help_overlay, sync_panel_manager_to_resources, sync_resources_to_panel_manager};
use bevy_egui::{EguiContexts, EguiGlobalSettings, PrimaryEguiContext};

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
    pub const TILE_FADE_SPEED: f32 = 3.0;

    // Z-layers
    pub const TILE_Z_LAYER: f32 = 0.0;
    pub const AIRCRAFT_Z_LAYER: f32 = 10.0;
    pub const LABEL_Z_LAYER: f32 = 11.0;

    // 3D model scale: model is ~4 units across, target is 32 world units (AIRCRAFT_MARKER_RADIUS * 4)
    pub const AIRCRAFT_MODEL_SCALE: f32 = 8.0;

    // Default map center (Wichita, KS)
    pub const DEFAULT_LATITUDE: f64 = 37.6872;
    pub const DEFAULT_LONGITUDE: f64 = -97.3301;

    // Aviation feature visibility radius (nautical miles)
    pub const AVIATION_FEATURE_RADIUS_NM: f64 = 120.0;
    // Bounding box pre-filter margin in degrees (~3° covers 120 NM at all latitudes)
    pub const AVIATION_FEATURE_BBOX_DEG: f64 = 3.0;

    // ADS-B connection settings
    pub const ADSB_SERVER_ADDRESS: &str = "98.186.33.60:30003";
    pub const ADSB_MAX_DISTANCE_MILES: f64 = 250.0;
    pub const ADSB_AIRCRAFT_TIMEOUT_SECS: i64 = 180;
}

// =============================================================================
// Coordinate Helpers - Centralized coordinate conversion functions
// =============================================================================

/// Clamp latitude to valid Mercator projection range
pub(crate) fn clamp_latitude(lat: f64) -> f64 {
    lat.clamp(-constants::MERCATOR_LAT_LIMIT, constants::MERCATOR_LAT_LIMIT)
}

/// Clamp longitude to valid range
pub(crate) fn clamp_longitude(lon: f64) -> f64 {
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
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "AirJedi - Aircraft Map Tracker".to_string(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    filter: "info,wgpu=warn,naga=warn,bevy_render=info".to_string(),
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
        // Full speed when focused; ~4 FPS when unfocused to keep ADS-B data
        // flowing without overwhelming the GPU or triggering macOS throttling.
        .insert_resource(bevy::winit::WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::reactive(
                std::time::Duration::from_millis(250),
            ),
        })
        .init_resource::<SpawnedTiles>()
        .init_resource::<DragState>()
        .init_resource::<Tile3DRefreshTimer>()
        .init_resource::<EguiWantsPointer>()
        .init_resource::<HelpOverlayState>()
        .init_resource::<ui_panels::UiPanelManager>()
        .init_resource::<tools_window::ToolsWindowState>()
        .init_resource::<debug_panel::DebugPanelState>()
        .init_resource::<dock::DockTreeState>()
        .init_resource::<statusbar::StatusBarState>()
        .insert_resource(ZoomState::new())
        // SlippyTilesSettings will be updated by setup_slippy_tiles_from_config after config is loaded
        .insert_resource(SlippyTilesSettings {
            endpoint: config::BasemapStyle::default().endpoint_url().to_string(),
            tiles_directory: std::path::PathBuf::from("tiles/"),  // Symlinked to centralized cache
            reference_latitude: constants::DEFAULT_LATITUDE,   // Wichita, KS (matches MapState default)
            reference_longitude: constants::DEFAULT_LONGITUDE,  // Wichita, KS (matches MapState default)
            z_layer: 0.0,                  // Render tiles at z=0 (behind aircraft at z=10)
            auto_render: false,            // Disable auto-render, we handle tile display ourselves
            ..default()
        })
        .add_systems(Startup, (setup_debug_logger, setup_map, configure_gizmo_layers))
        .add_systems(Update, check_egui_wants_input.before(handle_pan_drag).before(handle_zoom))
        .add_systems(Update, handle_pan_drag)
        .add_systems(Update, handle_zoom)
        .add_systems(Update, handle_pinch_zoom)
        .add_systems(Update, handle_window_resize)
        .add_systems(Update, handle_3d_view_tile_refresh)
        .add_systems(Update, request_3d_tiles_continuous.after(handle_3d_view_tile_refresh))
        // Apply deferred commands (like despawns) before updating camera/tiles
        // This ensures old tiles are gone before new camera position is applied
        .add_systems(Update, ApplyDeferred.after(handle_zoom))
        .add_systems(Update, apply_camera_zoom.after(ApplyDeferred))
        .add_systems(Update, follow_aircraft.after(adsb::sync_aircraft_from_adsb))
        .add_systems(Update, update_camera_position.after(handle_pan_drag).after(apply_camera_zoom).after(follow_aircraft))
        .add_systems(Update, sync_aircraft_camera
            .after(update_camera_position)
            .after(apply_camera_zoom)
            .after(view3d::update_3d_camera))
        .add_systems(Update, update_aircraft_positions.after(update_camera_position).after(adsb::sync_aircraft_from_adsb))
        .add_systems(Update, scale_aircraft_and_labels.after(apply_camera_zoom))
        .add_systems(Update, update_aircraft_labels.after(update_aircraft_positions))
        .add_systems(bevy_egui::EguiPrimaryContextPass, (
            theme::apply_egui_theme,
            toolbar::render_toolbar,
            statusbar::render_statusbar.after(toolbar::render_toolbar),
            dock::render_dock_tree.after(statusbar::render_statusbar),
        ))
        .add_systems(Update, display_tiles_filtered.after(ApplyDeferred))
        .add_systems(Update, animate_tile_fades.after(display_tiles_filtered))
        .add_systems(Update, cull_offscreen_tiles.after(display_tiles_filtered))
        .add_systems(Update, handle_keyboard_shortcuts)
        .add_systems(Update, toggle_overlays_keyboard)
        .add_systems(Update, sync_resources_to_panel_manager.after(handle_keyboard_shortcuts))
        .add_systems(Update, sync_panel_manager_to_resources.after(sync_resources_to_panel_manager))
        .add_systems(Update, update_help_overlay)
        .add_systems(Update, debug_panel::update_debug_metrics)
        .add_systems(Update, heartbeat_diagnostic)
        .run();
}


/// Marker for the 3D camera that renders aircraft models
#[derive(Component)]
struct AircraftCamera;

/// Marker for the primary 2D map camera (distinguishes it from the egui UI camera)
#[derive(Component)]
pub(crate) struct MapCamera;

// Component to track tile fade state for smooth zoom transitions
#[derive(Component)]
pub(crate) struct TileFadeState {
    pub(crate) alpha: f32,
    /// The zoom level this tile was spawned for
    pub(crate) tile_zoom: u8,
}

/// Tracks which tile positions have been spawned to prevent duplicate entities.
/// Key is (transform_x rounded, transform_y rounded, zoom_level).
#[derive(Resource, Default)]
struct SpawnedTiles {
    positions: std::collections::HashSet<(i32, i32, u8)>,
}

// Resource to track pan/drag state
#[derive(Resource, Default)]
struct DragState {
    is_dragging: bool,
    last_position: Option<Vec2>,
    last_tile_request_coords: Option<(f64, f64)>,
}

/// Holds the shared Handle<ScatteringMedium> used by Atmosphere components.
/// Created at startup with ScatteringMedium::earthlike() defaults.
#[derive(Resource)]
pub struct AtmosphereMediumHandle(pub Handle<ScatteringMedium>);

/// Timer that triggers periodic tile re-requests in 3D mode so that camera
/// orbit, pan, and altitude changes continuously fill visible areas.
#[derive(Resource)]
struct Tile3DRefreshTimer(Timer);

impl Default for Tile3DRefreshTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.3, TimerMode::Repeating))
    }
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

/// Diagnostic heartbeat: logs wall-clock time, frame delta, and focus state to
/// tmp/heartbeat.log every ~5 seconds.  If the app freezes, gaps in the log reveal
/// whether the process was suspended (App Nap) or the main thread was blocked.
fn heartbeat_diagnostic(
    time: Res<Time>,
    mut last_wall: Local<Option<std::time::Instant>>,
    windows: Query<&Window>,
    tile_query: Query<(), With<MapTile>>,
) {
    // Use wall clock for interval tracking (immune to Bevy virtual time capping)
    let now = std::time::Instant::now();
    if let Some(prev) = *last_wall {
        if now.duration_since(prev).as_secs() < 5 {
            return;
        }
    }
    *last_wall = Some(now);

    let focused = windows.iter().next().is_some_and(|w| w.focused);
    let delta_ms = time.delta_secs() * 1000.0;
    let tile_count = tile_query.iter().count();
    let elapsed = time.elapsed_secs_f64();

    let msg = format!(
        "HEARTBEAT: elapsed={:.1}s delta={:.1}ms focused={} tiles={}",
        elapsed, delta_ms, focused, tile_count
    );
    debug!("{}", msg);

    // Also write to file with explicit flush
    let log_path = std::env::current_dir()
        .ok()
        .map(|p| p.join("tmp/heartbeat.log"))
        .unwrap_or_else(|| std::path::PathBuf::from("tmp/heartbeat.log"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(f, "{}", msg);
        let _ = f.flush();
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

/// Configure gizmos to render on layer 2 so they are only drawn by Camera2d
/// (which includes layer 2) and not by Camera3d (default layer 0 only).
/// This prevents trails, navaids, and runways from being double-rendered
/// when both cameras share the same transform in 3D mode.
fn configure_gizmo_layers(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.render_layers = RenderLayers::layer(2);
}

pub(crate) fn setup_map(
    mut commands: Commands,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    mut tile_settings: ResMut<SlippyTilesSettings>,
    app_config: Res<config::AppConfig>,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
    mut egui_settings: ResMut<EguiGlobalSettings>,
) {
    // Prevent bevy_egui from auto-attaching to Camera2d. We use a dedicated UI
    // camera so egui stays visible when Camera2d switches to perspective in 3D mode.
    egui_settings.auto_create_primary_context = false;

    // Set up 2D camera for map tiles and labels.
    // Layer 0 = default content (tiles, sprites, text).
    // Layer 2 = gizmos (trails, navaids, runways) — kept off Camera3d to prevent
    //           double-rendering during 2D↔3D transitions.
    commands.spawn((Camera2d, MapCamera, RenderLayers::from_layers(&[0, 2])));

    // Set up 3D camera for aircraft models (renders on top of 2D, with transparent clear).
    // Stays on default layer 0 so it sees 3D meshes (SceneRoot children inherit layer 0)
    // but NOT gizmos (layer 2).
    commands.spawn((
        Camera3d::default(),
        AircraftCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_2d()),
        Transform::default(),
    ));

    // Dedicated UI camera for egui. Renders last (order 100) with no clear so it
    // composites the UI on top of everything. Uses an empty render layer (11) to
    // avoid re-rendering any scene content. PrimaryEguiContext tells bevy_egui to
    // attach the egui context here instead of on Camera2d.
    commands.spawn((
        Camera2d,
        PrimaryEguiContext,
        Camera {
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(11),
    ));

    // Lighting for 3D aircraft models
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        affects_lightmapped_meshes: true,
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: false,
            ..default()
        },
        SunDisk::EARTH,
        view3d::sky::SunLight,
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.3, 0.0)),
    ));

    // Create an earthlike scattering medium for atmospheric rendering.
    // The handle is stored as a resource so sky.rs can attach Atmosphere
    // components to cameras when 3D mode is activated.
    let medium = scattering_mediums.add(ScatteringMedium::earthlike(256, 256));
    commands.insert_resource(AtmosphereMediumHandle(medium));

    // Set up centralized tile cache (creates cache dir + symlink into assets/)
    tile_cache::setup_tile_cache();
    tile_cache::clear_legacy_tiles();
    tile_cache::remove_invalid_tiles();

    // Update SlippyTilesSettings from config
    tile_settings.endpoint = app_config.map.basemap_style.endpoint_url().to_string();
    tile_settings.tile_format = app_config.map.basemap_style.tile_format();
    tile_settings.reverse_axes = app_config.map.basemap_style.reverse_axes();
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
    map_state: Res<MapState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
) {
    if !view3d_state.is_3d_active() {
        return;
    }

    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
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
        tile_size: TileSize::Normal,
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
            tile_size: TileSize::Normal,
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
    // At the horizon, perspective projection makes the ground span
    // very far laterally.  Use a fan pattern: for each forward
    // distance, sweep across the full lateral range so coverage
    // forms a wide arc matching the frustum shape.
    let hr = 4 + (3.0 * (1.0 - pitch_factor)) as u8; // 4-7

    // zoom-3: sweep at multiple forward distances
    for &fwd in &[2.0, 5.0, 8.0] {
        request_band(3, fwd, 0.0, hr);
        // Lateral spread increases with forward distance
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

// Aircraft texture setup, sync, label update, and connection status
// are now in the adsb module.

/// Tracks whether egui wants pointer input this frame, used to prevent
/// map interactions when clicking/scrolling over UI panels.
#[derive(Resource, Default)]
struct EguiWantsPointer(bool);

fn check_egui_wants_input(
    mut contexts: EguiContexts,
    mut drag_state: ResMut<DragState>,
    mut egui_wants: ResMut<EguiWantsPointer>,
    dock_state: Res<dock::DockTreeState>,
) {
    egui_wants.0 = false;
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() || ctx.wants_pointer_input() {
            // Check if pointer is over the map viewport (allow interaction there)
            let over_map = if let Some(map_rect) = dock_state.map_viewport_rect {
                ctx.pointer_latest_pos().is_some_and(|pos| map_rect.contains(pos))
            } else {
                false
            };
            if !over_map {
                egui_wants.0 = true;
                drag_state.is_dragging = false;
                drag_state.last_position = None;
            }
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
    egui_wants: Res<EguiWantsPointer>,
    view3d_state: Res<view3d::View3DState>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    // In 3D mode, panning is handled by handle_3d_camera_controls
    if view3d_state.is_3d_active() || view3d_state.is_transitioning() {
        drag_state.is_dragging = false;
        drag_state.last_position = None;
        return;
    }

    // Only start a new drag if pointer is not over a UI panel
    if mouse_button.just_pressed(MouseButton::Left) && !egui_wants.0 {
        drag_state.is_dragging = true;
        drag_state.last_position = None;
    }

    if mouse_button.just_released(MouseButton::Left) {
        if drag_state.is_dragging {
            // Final tile request at drag end position
            request_tiles_at_location(
                &mut download_events,
                map_state.latitude,
                map_state.longitude,
                map_state.zoom_level,
                true,
            );
            drag_state.last_tile_request_coords = Some((map_state.latitude, map_state.longitude));
        }
        drag_state.is_dragging = false;
        drag_state.last_position = None;
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
                let delta_world_x = -(delta.x as f64) / zoom_state.camera_zoom as f64;
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

                // Request tiles periodically during drag to fill visible area
                let should_request = match drag_state.last_tile_request_coords {
                    Some((last_lat, last_lon)) => {
                        let lat_diff = (map_state.latitude - last_lat).abs();
                        let lon_diff = (map_state.longitude - last_lon).abs();
                        lat_diff > constants::PAN_TILE_REQUEST_THRESHOLD
                            || lon_diff > constants::PAN_TILE_REQUEST_THRESHOLD
                    }
                    None => true,
                };
                if should_request {
                    let radius = compute_tile_radius(
                        window.width(),
                        window.height(),
                        zoom_state.camera_zoom,
                        Some(&view3d_state),
                    );
                    download_events.write(DownloadSlippyTilesMessage {
                        tile_size: TileSize::Normal,
                        zoom_level: map_state.zoom_level,
                        coordinates: Coordinates::from_latitude_longitude(
                            map_state.latitude,
                            map_state.longitude,
                        ),
                        radius: Radius(radius),
                        use_cache: true,
                    });
                    drag_state.last_tile_request_coords =
                        Some((map_state.latitude, map_state.longitude));
                }
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

fn handle_zoom(
    mut scroll_events: MessageReader<MouseWheel>,
    mut map_state: ResMut<MapState>,
    mut zoom_state: ResMut<ZoomState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    window_query: Query<&Window>,
    mut tile_query: Query<(&mut TileFadeState, &mut Transform), With<MapTile>>,
    logger: Option<Res<ZoomDebugLogger>>,
    mut contexts: EguiContexts,
    dock_state: Res<dock::DockTreeState>,
    mut spawned_tiles: ResMut<SpawnedTiles>,
    view3d_state: Res<view3d::View3DState>,
) {
    // In 3D mode, scroll is handled by handle_3d_camera_controls
    if view3d_state.is_3d_active() || view3d_state.is_transitioning() {
        return;
    }

    // Shift+scroll in 2D mode: do nothing (pitch control is only in 3D mode).
    // Read shift from egui since bevy_egui absorbs modifier keys from ButtonInput.
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.input(|i| i.modifiers.shift) {
            return;
        }
    }
    // Don't zoom the map when pointer is over a dock panel (but allow zoom over the map viewport)
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.is_pointer_over_area() {
            // The egui CentralPanel covers the entire window, so is_pointer_over_area() is
            // always true. Check if the pointer is inside the map viewport pane -- if so,
            // allow zoom through to Bevy.
            if let Some(map_rect) = dock_state.map_viewport_rect {
                if let Some(pos) = ctx.pointer_latest_pos() {
                    if !map_rect.contains(pos) {
                        return;
                    }
                } else {
                    return;
                }
            }
        }
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    // Macro to log to both console and file
    macro_rules! log_info {
        ($($arg:tt)*) => {
            {
                let msg = format!($($arg)*);
                debug!("{}", msg);
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
            log_info!("No cursor - new camera_zoom={}", zoom_state.camera_zoom);
            continue;
        };

        log_info!("Cursor position: ({:.2}, {:.2})", cursor_viewport_pos.x, cursor_viewport_pos.y);

        // Save old camera zoom BEFORE applying scroll zoom (needed for zoom-to-cursor)
        let camera_zoom_before_scroll = zoom_state.camera_zoom;

        // Update camera zoom (multiplicative for smooth feel)
        // Positive scroll (up/forward) = zoom in, negative = zoom out
        let zoom_factor = 1.0 + zoom_delta;
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
            // Clear spawned tile tracker since positions change with zoom level
            spawned_tiles.positions.clear();
            // Calculate scale factor: when zooming IN, positions double; when zooming OUT, positions halve
            let scale_factor = if map_state.zoom_level.to_u8() > old_tile_zoom.to_u8() {
                2.0_f32 // Zooming in: scale up
            } else {
                0.5_f32 // Zooming out: scale down
            };

            // Scale and reposition old tiles to match new coordinate system
            // Old tiles are kept visible until new tiles at the current zoom arrive
            let mut marked = 0;
            for (_fade_state, mut transform) in tile_query.iter_mut() {
                // Scale the tile position to match the new zoom level's coordinate system
                transform.translation.x *= scale_factor;
                transform.translation.y *= scale_factor;
                // Also scale the tile itself so it visually matches
                transform.scale *= scale_factor;

                marked += 1;
            }
            log_info!("  Scaled {} tiles by {} (kept visible until new tiles arrive)", marked, scale_factor);

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

/// Handle trackpad pinch-to-zoom gestures (macOS).
/// PinchGesture.0 is positive for zoom in, negative for zoom out.
fn handle_pinch_zoom(
    mut pinch_events: MessageReader<PinchGesture>,
    mut map_state: ResMut<MapState>,
    mut zoom_state: ResMut<ZoomState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    window_query: Query<&Window>,
    mut tile_query: Query<(&mut TileFadeState, &mut Transform), With<MapTile>>,
    mut contexts: EguiContexts,
    dock_state: Res<dock::DockTreeState>,
    mut spawned_tiles: ResMut<SpawnedTiles>,
    view3d_state: Res<view3d::View3DState>,
) {
    // In 3D mode, zoom is handled by handle_3d_camera_controls
    if view3d_state.is_3d_active() || view3d_state.is_transitioning() {
        return;
    }

    // Don't zoom when pointer is over a dock panel (same logic as handle_zoom)
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.is_pointer_over_area() {
            if let Some(map_rect) = dock_state.map_viewport_rect {
                if let Some(pos) = ctx.pointer_latest_pos() {
                    if !map_rect.contains(pos) {
                        return;
                    }
                } else {
                    return;
                }
            }
        }
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    use constants::{ZOOM_UPGRADE_THRESHOLD, ZOOM_DOWNGRADE_THRESHOLD};

    for event in pinch_events.read() {
        let pinch_delta = event.0;
        let camera_zoom_before = zoom_state.camera_zoom;

        // Apply pinch directly as a multiplicative factor
        let zoom_factor = 1.0 + pinch_delta;
        zoom_state.camera_zoom = (zoom_state.camera_zoom * zoom_factor)
            .clamp(zoom_state.min_zoom, zoom_state.max_zoom);

        // Zoom-to-cursor: keep the point under cursor stationary
        if let Some(cursor_viewport_pos) = window.cursor_position() {
            let old_tile_zoom = map_state.zoom_level;

            // Check for zoom level transitions
            let current_tile_zoom = old_tile_zoom.to_u8();
            let mut zoom_level_changed = false;
            if zoom_state.camera_zoom >= ZOOM_UPGRADE_THRESHOLD && current_tile_zoom < 19 {
                zoom_state.camera_zoom /= 2.0;
                if let Ok(new_zoom) = ZoomLevel::try_from(current_tile_zoom + 1) {
                    map_state.zoom_level = new_zoom;
                    zoom_level_changed = true;
                }
            } else if zoom_state.camera_zoom <= ZOOM_DOWNGRADE_THRESHOLD && current_tile_zoom > 0 {
                zoom_state.camera_zoom *= 2.0;
                if let Ok(new_zoom) = ZoomLevel::try_from(current_tile_zoom - 1) {
                    map_state.zoom_level = new_zoom;
                    zoom_level_changed = true;
                }
            }

            let (new_lat, new_lon) = calculate_zoom_to_cursor_center(
                cursor_viewport_pos,
                (window.width(), window.height()),
                (map_state.latitude, map_state.longitude),
                camera_zoom_before,
                zoom_state.camera_zoom,
                old_tile_zoom,
                map_state.zoom_level,
            );
            map_state.latitude = clamp_latitude(new_lat);
            map_state.longitude = clamp_longitude(new_lon);

            if zoom_level_changed {
                spawned_tiles.positions.clear();
                let scale_factor = if map_state.zoom_level.to_u8() > old_tile_zoom.to_u8() {
                    2.0_f32
                } else {
                    0.5_f32
                };
                for (_fade_state, mut transform) in tile_query.iter_mut() {
                    transform.translation.x *= scale_factor;
                    transform.translation.y *= scale_factor;
                    transform.scale *= scale_factor;
                }
                request_tiles_at_location(
                    &mut download_events,
                    map_state.latitude,
                    map_state.longitude,
                    map_state.zoom_level,
                    true,
                );
            }
        }
    }
}

// Apply the camera zoom to the actual camera projection
fn apply_camera_zoom(
    zoom_state: Res<ZoomState>,
    mut camera_query: Query<&mut Projection, With<MapCamera>>,
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

// Keep aircraft and labels at constant screen size despite zoom changes
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

    // To maintain constant SCREEN size:
    // - Orthographic projection: screen_size = world_size / ortho.scale
    // - ortho.scale = 1 / camera_zoom
    // - So: screen_size = world_size * camera_zoom
    // - For constant screen size: world_size = constant / camera_zoom
    // - Therefore: transform.scale = 1 / camera_zoom (which equals ortho.scale)
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
    // This matches how bevy_slippy_tiles positions tiles
    let converter = geo::CoordinateConverter::new(&tile_settings, map_state.zoom_level);

    for (aircraft, mut transform) in aircraft_query.iter_mut() {
        let pos = converter.latlon_to_world(aircraft.latitude, aircraft.longitude);

        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        // Apply rotation for 3D model orientation:
        // GLB model has nose along +Z, wings along X, height along Y.
        // First rotate 180° around Z to flip the model right-side up (top faces camera).
        // Then rotate -90° around X to tilt nose from +Z to +Y (north on screen).
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
    tile_cache::clear_tile_cache();
}

// Custom tile display system that filters tiles by current zoom level.
// When new tiles arrive at the current zoom, old tiles from previous zoom levels
// are marked for delayed despawn so the screen is never blank.
fn display_tiles_filtered(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tile_settings: Res<SlippyTilesSettings>,
    map_state: Res<MapState>,
    mut tile_events: MessageReader<SlippyTileDownloadedMessage>,
    mut tile_query: Query<(Entity, &mut TileFadeState), With<MapTile>>,
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

        let transform_x = (tile_center_x - ref_x) as f32;
        let transform_y = (tile_center_y - ref_y) as f32;

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
        let tile_z = if view3d_state.is_3d_active() {
            view3d_state.altitude_to_z(view3d_state.ground_elevation_ft)
        } else {
            tile_settings.z_layer + 0.1
        };
        commands.spawn((
            Sprite {
                image: tile_handle,
                color: Color::srgba(1.0, 1.0, 1.0, 0.0),
                ..default()
            },
            Transform::from_xyz(transform_x, transform_y, tile_z),
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

/// Despawn tile entities that are far outside the visible viewport.
/// Without this, tiles accumulate indefinitely as the user pans, causing
/// frame time to grow continuously until the app becomes unresponsive.

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

fn cull_offscreen_tiles(
    mut commands: Commands,
    camera_query: Query<(&Transform, &Projection), With<MapCamera>>,
    tile_query: Query<(Entity, &Transform, &TileFadeState), With<MapTile>>,
    window_query: Query<&Window>,
    mut spawned_tiles: ResMut<SpawnedTiles>,
    view3d_state: Res<view3d::View3DState>,
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
        // Use the far ground distance for width so the culling box covers
        // the full trapezoid of the perspective frustum on the ground.
        let half_width_at_horizon = far_ground_dist * half_hfov.tan();

        // 2.5x margin to keep multi-resolution horizon tiles alive
        let margin = 2.5;
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

    // Second pass: if still over budget, cull farthest tiles
    let tile_limit = max_tile_entities(Some(&view3d_state));
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

// Animate tile fade-in and despawn old tiles only when covered by fully-loaded new tiles.
// New tiles fade in ON TOP of old tiles (higher z-layer). Old tiles are only removed
// once a new tile at the same grid position has reached full opacity, guaranteeing
// the user never sees empty gray gaps.
fn animate_tile_fades(
    mut commands: Commands,
    time: Res<Time>,
    map_state: Res<MapState>,
    mut tile_query: Query<(Entity, &mut TileFadeState, &mut Sprite, &Transform), With<MapTile>>,
) {
    let delta = time.delta_secs();
    let current_zoom = map_state.zoom_level.to_u8();

    // Collect grid cells covered by fully-opaque new tiles.
    // Quantize positions to 256px cells so old (rescaled) tiles can be matched.
    let mut loaded_cells: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    let mut old_tiles: Vec<(Entity, i32, i32)> = Vec::new();

    for (entity, mut fade_state, mut sprite, transform) in tile_query.iter_mut() {
        if fade_state.tile_zoom == current_zoom {
            // Fade in new tiles
            if fade_state.alpha < 1.0 {
                fade_state.alpha += constants::TILE_FADE_SPEED * delta;
                fade_state.alpha = fade_state.alpha.min(1.0);
                sprite.color = Color::srgba(1.0, 1.0, 1.0, fade_state.alpha);
            }
            // Track fully-opaque new tiles by grid cell
            if fade_state.alpha >= 1.0 {
                let cell = (
                    (transform.translation.x / 256.0).round() as i32,
                    (transform.translation.y / 256.0).round() as i32,
                );
                loaded_cells.insert(cell);
            }
        } else {
            // Old-zoom tile: record for coverage check
            let cell = (
                (transform.translation.x / 256.0).round() as i32,
                (transform.translation.y / 256.0).round() as i32,
            );
            old_tiles.push((entity, cell.0, cell.1));
        }
    }

    // Despawn old tiles whose grid cell is covered by a fully-loaded new tile
    for (entity, cx, cy) in old_tiles {
        if loaded_cells.contains(&(cx, cy)) {
            commands.entity(entity).despawn();
        }
    }
}
