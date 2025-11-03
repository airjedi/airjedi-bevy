use bevy::{prelude::*, input::mouse::MouseWheel};
use bevy_slippy_tiles::*;
use std::fs;

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
        ))
        .init_resource::<DragState>()
        .insert_resource(ZoomState::new())
        .insert_resource(SlippyTilesSettings {
            endpoint: "https://cartodb-basemaps-a.global.ssl.fastly.net/dark_all".to_string(),
            tiles_directory: std::path::PathBuf::from(""),  // Root assets directory
            reference_latitude: 51.5074,   // London latitude (matches MapState default)
            reference_longitude: -0.1278,  // London longitude (matches MapState default)
            z_layer: 0.0,                  // Render tiles at z=0 (behind aircraft at z=10)
            ..default()
        })
        .add_systems(Startup, (setup_map, setup_ui, spawn_sample_aircraft))
        .add_systems(Update, handle_pan_drag)
        .add_systems(Update, handle_zoom)
        .add_systems(Update, update_camera_position.after(handle_pan_drag).after(handle_zoom))
        .add_systems(Update, apply_camera_zoom.after(handle_zoom))
        .add_systems(Update, update_aircraft_positions)
        .add_systems(Update, scale_aircraft_and_labels.after(apply_camera_zoom))
        .add_systems(Update, update_aircraft_labels.after(update_aircraft_positions))
        .add_systems(Update, handle_clear_cache_button)
        .run();
}

// Component for aircraft entities
#[derive(Component)]
struct Aircraft {
    #[allow(dead_code)]
    id: String,
    latitude: f64,
    longitude: f64,
    #[allow(dead_code)]
    altitude: f32,
    heading: f32,
}

// Component to link aircraft labels to their aircraft
#[derive(Component)]
struct AircraftLabel {
    aircraft_entity: Entity,
}

// Component to mark the clear cache button
#[derive(Component)]
struct ClearCacheButton;

// Resource to track map state
#[derive(Resource, Clone)]
struct MapState {
    // Current map center (where camera is looking)
    latitude: f64,
    longitude: f64,
    zoom_level: ZoomLevel,
    // Reference point (where tiles are anchored in world space)
    reference_latitude: f64,
    reference_longitude: f64,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            latitude: 51.5074,
            longitude: -0.1278,
            zoom_level: ZoomLevel::L10,
            reference_latitude: 51.5074,
            reference_longitude: -0.1278,
        }
    }
}

// Resource to track pan/drag state
#[derive(Resource, Default)]
struct DragState {
    is_dragging: bool,
    last_position: Option<Vec2>,
    last_tile_request_coords: Option<(f64, f64)>,
}

// Resource to track zoom scroll accumulation for smooth trackpad zooming
#[derive(Resource)]
struct ZoomState {
    // Continuous camera zoom level (1.0 = normal, 2.0 = 2x zoomed in, 0.5 = 2x zoomed out)
    camera_zoom: f32,
    // Minimum and maximum zoom levels
    min_zoom: f32,
    max_zoom: f32,
}

impl ZoomState {
    fn new() -> Self {
        Self {
            camera_zoom: 1.0,
            min_zoom: 0.1,   // Can zoom out to 10% (10x out)
            max_zoom: 10.0,  // Can zoom in to 1000% (10x in)
        }
    }
}

impl Default for ZoomState {
    fn default() -> Self {
        Self::new()
    }
}

fn setup_map(mut commands: Commands, mut download_events: MessageWriter<DownloadSlippyTilesEvent>) {
    // Set up camera
    commands.spawn(Camera2d);

    // Initialize map state resource
    let map_state = MapState::default();

    // Send initial tile download request
    download_events.write(DownloadSlippyTilesEvent {
        tile_size: TileSize::Normal,
        zoom_level: map_state.zoom_level,
        coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
        radius: Radius(3), // Download a 7x7 grid of tiles (covers 1792x1792 pixels)
        use_cache: true,
    });

    commands.insert_resource(map_state);
}

fn setup_ui(mut commands: Commands) {
    // Map Attribution
    commands.spawn((
        Text::new("© OpenStreetMap contributors, © CartoDB"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(5.0),
            right: Val::Px(5.0),
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
    ));

    // Instructions
    commands.spawn((
        Text::new("Controls: Drag to pan | Scroll/Two-finger to zoom"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
    ));

    // Menu button - Clear Cache
    commands.spawn((
        Button,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(50.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.9)),
        ClearCacheButton,
    )).with_child((
        Text::new("Clear Cache"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn spawn_sample_aircraft(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Spawn some sample aircraft around London
    let sample_aircraft = vec![
        ("BA123", 51.5074, -0.1278, 35000.0, 90.0),
        ("AA456", 51.4774, -0.0878, 38000.0, 180.0),
        ("LH789", 51.5374, -0.1678, 32000.0, 270.0),
    ];

    for (id, lat, lon, alt, heading) in sample_aircraft {
        // Spawn aircraft marker
        let aircraft_entity = commands.spawn((
            Mesh2d(meshes.add(Circle::new(8.0))),
            MeshMaterial2d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
            Transform::from_xyz(0.0, 0.0, 10.0),
            Aircraft {
                id: id.to_string(),
                latitude: lat,
                longitude: lon,
                altitude: alt,
                heading,
            },
        )).id();

        // Spawn label for this aircraft
        let label_text = format!("{}\n{:.0} ft", id, alt);
        commands.spawn((
            Text2d::new(label_text),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, 0.0, 11.0), // Slightly higher z-index than aircraft
            AircraftLabel {
                aircraft_entity,
            },
        ));
    }
}

fn handle_pan_drag(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut cursor_moved: MessageReader<CursorMoved>,
    mut map_state: ResMut<MapState>,
    mut drag_state: ResMut<DragState>,
    mut download_events: MessageWriter<DownloadSlippyTilesEvent>,
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
            if lat_diff > 0.001 || lon_diff > 0.001 {
                download_events.write(DownloadSlippyTilesEvent {
                    tile_size: TileSize::Normal,
                    zoom_level: map_state.zoom_level,
                    coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
                    radius: Radius(3),
                    use_cache: true,
                });
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

                // Calculate the degrees per pixel based on zoom level
                // At zoom level 10, the world is 2^10 * 256 = 262,144 pixels wide
                // 360 degrees / 262,144 pixels = ~0.00137 degrees per pixel
                let zoom_factor = 2u32.pow(map_state.zoom_level.to_u8() as u32) as f64;
                let pixels_per_degree_lon = (zoom_factor * 256.0) / 360.0;

                // Latitude scaling is more complex due to Mercator projection
                // Use a simplified approximation based on current latitude
                let lat_rad = map_state.latitude.to_radians();
                let pixels_per_degree_lat = (zoom_factor * 256.0) / 360.0 * lat_rad.cos();

                // Convert pixel delta to degree delta (inverted Y for screen coordinates)
                let lon_delta = -(delta.x as f64) / pixels_per_degree_lon;
                let lat_delta = (delta.y as f64) / pixels_per_degree_lat;

                // Update map coordinates
                map_state.latitude = (map_state.latitude + lat_delta).clamp(-85.0511, 85.0511);
                map_state.longitude = (map_state.longitude + lon_delta).clamp(-180.0, 180.0);
            }
            drag_state.last_position = Some(event.position);
        }
    }
}

fn update_camera_position(
    map_state: Res<MapState>,
    zoom_state: Res<ZoomState>,
    mut camera_query: Query<&mut Transform, With<Camera2d>>,
) {
    if let Ok(mut camera_transform) = camera_query.single_mut() {
        // Calculate the offset from reference point to current map center
        let lat_delta = map_state.latitude - map_state.reference_latitude;
        let lon_delta = map_state.longitude - map_state.reference_longitude;

        // Calculate pixel offset based on tile zoom level
        let zoom_factor = 2u32.pow(map_state.zoom_level.to_u8() as u32) as f64;
        let pixels_per_degree_lon = (zoom_factor * 256.0) / 360.0;

        // Use Mercator projection compensation for latitude
        // IMPORTANT: Use reference_latitude to match tile coordinate system
        let lat_rad = map_state.reference_latitude.to_radians();
        let pixels_per_degree_lat = pixels_per_degree_lon * lat_rad.cos();

        // Position camera to show the current map center
        // Scale by camera_zoom to keep position consistent when tile zoom changes
        let camera_zoom_scale = zoom_state.camera_zoom as f64;
        camera_transform.translation.x = (lon_delta * pixels_per_degree_lon * camera_zoom_scale) as f32;
        camera_transform.translation.y = (lat_delta * pixels_per_degree_lat * camera_zoom_scale) as f32;
    }
}

fn handle_zoom(
    mut scroll_events: MessageReader<MouseWheel>,
    mut map_state: ResMut<MapState>,
    mut zoom_state: ResMut<ZoomState>,
    mut download_events: MessageWriter<DownloadSlippyTilesEvent>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for event in scroll_events.read() {
        // Get cursor position in viewport coordinates (None if cursor not in window)
        let Some(cursor_viewport_pos) = window.cursor_position() else {
            // No cursor, just zoom at center
            let zoom_delta = match event.unit {
                bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.1,
                bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.002,
            };
            let zoom_factor = 1.0 - zoom_delta;
            zoom_state.camera_zoom = (zoom_state.camera_zoom * zoom_factor)
                .clamp(zoom_state.min_zoom, zoom_state.max_zoom);
            continue;
        };

        // Convert cursor viewport position to world position
        let Ok(cursor_world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_viewport_pos) else {
            continue;
        };

        // Calculate what geographic location is under the cursor BEFORE zoom
        let zoom_factor_tiles = 2u32.pow(map_state.zoom_level.to_u8() as u32) as f64;
        let pixels_per_degree_lon = (zoom_factor_tiles * 256.0) / 360.0;

        // IMPORTANT: Use reference_latitude to match tile coordinate system
        let lat_rad = map_state.reference_latitude.to_radians();
        let pixels_per_degree_lat = pixels_per_degree_lon * lat_rad.cos();
        let camera_zoom_scale = zoom_state.camera_zoom as f64;

        // Calculate camera position in world space (where camera is currently looking)
        let lat_delta_cam = map_state.latitude - map_state.reference_latitude;
        let lon_delta_cam = map_state.longitude - map_state.reference_longitude;
        let camera_world_x = lon_delta_cam * pixels_per_degree_lon * camera_zoom_scale;
        let camera_world_y = lat_delta_cam * pixels_per_degree_lat * camera_zoom_scale;

        // Cursor world position relative to camera (world offset from camera)
        let cursor_world_offset_x = (cursor_world_pos.x as f64) - camera_world_x;
        let cursor_world_offset_y = (cursor_world_pos.y as f64) - camera_world_y;

        // Convert world offset to screen offset (in screen pixels)
        // screen_offset = world_offset * camera_zoom (since ortho_scale = 1/camera_zoom)
        let cursor_screen_offset_x = cursor_world_offset_x * camera_zoom_scale;
        let cursor_screen_offset_y = cursor_world_offset_y * camera_zoom_scale;

        // Geographic offset from map center using formula: geo_offset = screen_offset / (pixels_per_degree * camera_zoom²)
        let lon_offset_before = cursor_screen_offset_x / (pixels_per_degree_lon * camera_zoom_scale * camera_zoom_scale);
        let lat_offset_before = cursor_screen_offset_y / (pixels_per_degree_lat * camera_zoom_scale * camera_zoom_scale);

        // Geographic location under cursor
        let cursor_lat_before = map_state.latitude + lat_offset_before;
        let cursor_lon_before = map_state.longitude + lon_offset_before;

        // Zoom factor per scroll unit (adjust for sensitivity)
        let zoom_delta = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => {
                // Mouse wheel - larger steps
                event.y * 0.1
            }
            bevy::input::mouse::MouseScrollUnit::Pixel => {
                // Trackpad - smooth, smaller steps
                event.y * 0.002
            }
        };

        // Update camera zoom (multiplicative for smooth feel)
        // Positive scroll = zoom in (smaller scale), negative = zoom out (larger scale)
        let zoom_factor = 1.0 - zoom_delta;
        zoom_state.camera_zoom = (zoom_state.camera_zoom * zoom_factor)
            .clamp(zoom_state.min_zoom, zoom_state.max_zoom);

        // After zoom, we want cursor to still point at the same geographic location
        // The cursor screen position (cursor_screen_offset_x/y) doesn't change

        let new_camera_zoom_scale = zoom_state.camera_zoom as f64;

        // Calculate new geographic offset using formula: geo_offset = screen_offset / (pixels_per_degree * camera_zoom²)
        let new_lon_offset = cursor_screen_offset_x / (pixels_per_degree_lon * new_camera_zoom_scale * new_camera_zoom_scale);
        let new_lat_offset = cursor_screen_offset_y / (pixels_per_degree_lat * new_camera_zoom_scale * new_camera_zoom_scale);

        // Set map center so cursor points to the same geographic location:
        // cursor_geo = new_map_geo + new_geo_offset
        // Therefore: new_map_geo = cursor_geo - new_geo_offset
        map_state.latitude = cursor_lat_before - new_lat_offset;
        map_state.longitude = cursor_lon_before - new_lon_offset;

        // Clamp to valid ranges
        map_state.latitude = map_state.latitude.clamp(-85.0511, 85.0511);
        map_state.longitude = map_state.longitude.clamp(-180.0, 180.0);
    }

    // Determine what tile zoom level we should be at based on camera zoom
    // Use camera_zoom directly since it represents the zoom relative to current tile level
    let current_tile_zoom = map_state.zoom_level.to_u8();

    let ideal_tile_zoom = if zoom_state.camera_zoom >= 1.5 {
        // Zoomed in enough, upgrade tiles and adjust camera zoom to compensate for world scale change
        if current_tile_zoom < 19 {
            // Tiles will be 2x more detailed (world becomes 2x bigger in pixels)
            // Divide camera_zoom by 2 to maintain visual continuity
            zoom_state.camera_zoom /= 2.0;
            current_tile_zoom + 1
        } else {
            current_tile_zoom
        }
    } else if zoom_state.camera_zoom <= 0.75 {
        // Zoomed out enough, downgrade tiles and adjust camera zoom to compensate for world scale change
        if current_tile_zoom > 0 {
            // Tiles will be 2x less detailed (world becomes 2x smaller in pixels)
            // Multiply camera_zoom by 2 to maintain visual continuity
            zoom_state.camera_zoom *= 2.0;
            current_tile_zoom - 1
        } else {
            current_tile_zoom
        }
    } else {
        current_tile_zoom
    };

    // Request new tiles if zoom level changed
    if ideal_tile_zoom != current_tile_zoom {
        if let Ok(new_zoom_level) = ZoomLevel::try_from(ideal_tile_zoom) {
            map_state.zoom_level = new_zoom_level;

            download_events.write(DownloadSlippyTilesEvent {
                tile_size: TileSize::Normal,
                zoom_level: map_state.zoom_level,
                coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
                radius: Radius(3),
                use_cache: true,
            });
        }
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

// Scale aircraft markers and labels based on camera zoom
fn scale_aircraft_and_labels(
    zoom_state: Res<ZoomState>,
    mut aircraft_query: Query<&mut Transform, With<Aircraft>>,
    mut label_query: Query<&mut TextFont, With<AircraftLabel>>,
) {
    // Aircraft are positioned in world-space (scaled with tiles), so we scale inversely
    // to camera zoom to keep them constant screen size
    let inverse_scale = 1.0 / zoom_state.camera_zoom;

    for mut transform in aircraft_query.iter_mut() {
        transform.scale = Vec3::splat(inverse_scale);
    }

    // Keep label font size constant for readability
    let base_font_size = 14.0;

    for mut text_font in label_query.iter_mut() {
        text_font.font_size = base_font_size;
    }
}

fn update_aircraft_positions(
    map_state: Res<MapState>,
    zoom_state: Res<ZoomState>,
    mut aircraft_query: Query<(&Aircraft, &mut Transform)>,
) {
    // Calculate zoom-aware scaling factor based on tile zoom level
    let zoom_factor = 2u32.pow(map_state.zoom_level.to_u8() as u32) as f64;
    let pixels_per_degree_lon = (zoom_factor * 256.0) / 360.0;

    // Scale by camera_zoom to keep positions consistent when tile zoom changes
    let camera_zoom_scale = zoom_state.camera_zoom as f64;

    for (aircraft, mut transform) in aircraft_query.iter_mut() {
        // Position aircraft relative to the reference point (same as tiles)
        let lat_diff = aircraft.latitude - map_state.reference_latitude;
        let lon_diff = aircraft.longitude - map_state.reference_longitude;

        // Use Mercator projection compensation for latitude
        let lat_rad = map_state.reference_latitude.to_radians();
        let pixels_per_degree_lat = pixels_per_degree_lon * lat_rad.cos();

        // Set world position (camera handles viewing the correct area)
        // Scale by camera_zoom to match camera position scaling
        transform.translation.x = (lon_diff * pixels_per_degree_lon * camera_zoom_scale) as f32;
        transform.translation.y = (lat_diff * pixels_per_degree_lat * camera_zoom_scale) as f32;
        transform.rotation = Quat::from_rotation_z(aircraft.heading.to_radians());
    }
}

fn update_aircraft_labels(
    aircraft_query: Query<&Transform, With<Aircraft>>,
    mut label_query: Query<(&AircraftLabel, &mut Transform), Without<Aircraft>>,
) {
    // Use constant world-space offset
    let offset = 15.0;

    for (label, mut label_transform) in label_query.iter_mut() {
        if let Ok(aircraft_transform) = aircraft_query.get(label.aircraft_entity) {
            // Position label above and slightly to the right of the aircraft
            label_transform.translation.x = aircraft_transform.translation.x + offset;
            label_transform.translation.y = aircraft_transform.translation.y + offset;
        }
    }
}

fn handle_clear_cache_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ClearCacheButton>),
    >,
    map_state: Res<MapState>,
    mut download_events: MessageWriter<DownloadSlippyTilesEvent>,
    mut commands: Commands,
    tile_query: Query<Entity, With<MapTile>>,
) {
    for (interaction, mut background_color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                // Change button color when pressed
                *background_color = BackgroundColor(Color::srgba(0.4, 0.4, 0.4, 0.9));

                // Despawn all existing tile entities to refresh the display
                let mut despawned_count = 0;
                for entity in tile_query.iter() {
                    commands.entity(entity).despawn();
                    despawned_count += 1;
                }
                info!("Despawned {} tile entities", despawned_count);

                // Clear the tile cache from disk
                clear_tile_cache();

                // Request fresh tiles after clearing cache
                download_events.write(DownloadSlippyTilesEvent {
                    tile_size: TileSize::Normal,
                    zoom_level: map_state.zoom_level,
                    coordinates: Coordinates::from_latitude_longitude(map_state.latitude, map_state.longitude),
                    radius: Radius(3),
                    use_cache: false,  // Force fresh download
                });

                info!("Tile cache cleared and requesting fresh tiles");
            }
            Interaction::Hovered => {
                // Highlight on hover
                *background_color = BackgroundColor(Color::srgba(0.3, 0.3, 0.3, 0.9));
            }
            Interaction::None => {
                // Default color
                *background_color = BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.9));
            }
        }
    }
}

fn clear_tile_cache() {
    // Get the assets directory path
    let assets_path = std::env::current_dir()
        .ok()
        .and_then(|path| Some(path.join("assets")))
        .unwrap_or_else(|| std::path::PathBuf::from("assets"));

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
