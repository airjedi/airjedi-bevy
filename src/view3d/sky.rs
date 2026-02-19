//! Sky rendering with star field and day/night cycle.
//!
//! The star field is rendered as a large 2D sprite on Camera2d at a low
//! z-depth (behind map tiles). This avoids multi-camera compositing
//! issues while ensuring stars never bleed through opaque tiles.
//! Sun position is computed from real wall-clock time and the map's
//! geographic coordinates.

use bevy::prelude::*;

use super::View3DState;
use crate::map::MapState;

/// Z-depth for the star field sprite (behind tiles at z=0.1)
const STAR_Z: f32 = -1.0;

/// Marker component for the star field sprite
#[derive(Component)]
pub struct StarField;

/// Resource tracking current sun position
#[derive(Resource)]
pub struct SunState {
    /// Sun elevation in degrees (-90 to 90, negative = below horizon)
    pub elevation: f32,
    /// Sun azimuth in degrees (0 = north, 90 = east)
    pub azimuth: f32,
}

impl Default for SunState {
    fn default() -> Self {
        Self {
            elevation: 45.0,
            azimuth: 180.0,
        }
    }
}

/// Compute sun elevation and azimuth from current time and geographic position.
/// Uses a simplified solar position algorithm accurate to ~1 degree.
pub fn compute_sun_position(latitude: f64, longitude: f64) -> (f32, f32) {
    use std::time::SystemTime;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs_f64();

    // Days since J2000.0 epoch (2000-01-01 12:00 UTC)
    let j2000_epoch = 946728000.0; // Unix timestamp of J2000.0
    let days = (total_secs - j2000_epoch) / 86400.0;

    // Solar mean longitude (degrees)
    let mean_lon = (280.460 + 0.9856474 * days) % 360.0;
    // Solar mean anomaly (degrees)
    let mean_anomaly = ((357.528 + 0.9856003 * days) % 360.0).to_radians();
    // Ecliptic longitude (degrees)
    let ecliptic_lon =
        (mean_lon + 1.915 * mean_anomaly.sin() + 0.020 * (2.0 * mean_anomaly).sin()).to_radians();
    // Obliquity of ecliptic
    let obliquity = 23.439_f64.to_radians();

    // Solar declination
    let declination = (obliquity.sin() * ecliptic_lon.sin()).asin();

    // Hour angle
    let utc_hours = (total_secs % 86400.0) / 3600.0;
    let solar_noon_offset = longitude / 15.0;
    let hour_angle = ((utc_hours - 12.0 + solar_noon_offset) * 15.0).to_radians();

    let lat_rad = latitude.to_radians();

    // Solar elevation
    let sin_elevation =
        lat_rad.sin() * declination.sin() + lat_rad.cos() * declination.cos() * hour_angle.cos();
    let elevation = sin_elevation.asin();

    // Solar azimuth
    let cos_azimuth =
        (declination.sin() - lat_rad.sin() * sin_elevation) / (lat_rad.cos() * elevation.cos());
    let mut azimuth = cos_azimuth.clamp(-1.0, 1.0).acos();
    if hour_angle > 0.0 {
        azimuth = std::f64::consts::TAU - azimuth;
    }

    (elevation.to_degrees() as f32, azimuth.to_degrees() as f32)
}

/// Marker for the directional light used as the sun
#[derive(Component)]
pub struct SunLight;

/// No-op: sky visibility is handled entirely through star field sprite visibility.
/// Kept as a system entry point for future atmospheric effects.
pub fn update_sky_visibility(
    _state: Res<View3DState>,
) {
}

/// Keep star field sprite centered on Camera2d and scaled to fill the viewport.
pub fn sync_sky_camera(
    state: Res<View3DState>,
    main_camera: Query<(&Transform, &Projection), (With<Camera2d>, Without<StarField>)>,
    window_query: Query<&Window>,
    zoom_state: Res<crate::ZoomState>,
    mut star_query: Query<&mut Transform, (With<StarField>, Without<Camera2d>)>,
) {
    if !state.is_3d_active() {
        return;
    }

    let Ok((main_tf, _main_proj)) = main_camera.single() else {
        return;
    };

    let Ok(mut star_tf) = star_query.single_mut() else {
        return;
    };

    // Position star field at camera XY but behind tiles
    star_tf.translation.x = main_tf.translation.x;
    star_tf.translation.y = main_tf.translation.y;
    star_tf.translation.z = STAR_Z;

    // Scale to fill viewport (account for camera zoom)
    if let Ok(window) = window_query.single() {
        let scale_factor = 1.0 / zoom_state.camera_zoom;
        // Scale sprite to cover viewport with some margin for panning
        let sx = (window.width() * scale_factor * 2.0) / 2048.0;
        let sy = (window.height() * scale_factor * 2.0) / 2048.0;
        let s = sx.max(sy);
        star_tf.scale = Vec3::new(s, s, 1.0);
    }
}

/// Spawn the star field as a 2D sprite.
pub fn setup_sky(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    // Generate procedural star texture
    let star_image = generate_star_texture(2048);
    let star_texture = images.add(star_image);

    commands.spawn((
        Name::new("Star Field"),
        StarField,
        Sprite {
            image: star_texture,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, STAR_Z),
        Visibility::Hidden,
    ));
}

/// Generate a procedural star texture as an Image.
/// Black background with scattered white dots.
fn generate_star_texture(size: u32) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let mut data = vec![0u8; (size * size * 4) as usize];

    // Set alpha to 255 for all pixels (opaque black background)
    for pixel in 0..(size * size) as usize {
        data[pixel * 4 + 3] = 255;
    }

    let num_stars = 800;
    for i in 0..num_stars {
        let hash = pseudo_hash(i);
        let x = (hash % size) as usize;
        let y = ((hash / size) % size) as usize;
        let brightness = 128 + (pseudo_hash(i + num_stars) % 128) as u8;
        let idx = (y * size as usize + x) * 4;
        if idx + 3 < data.len() {
            data[idx] = brightness;
            data[idx + 1] = brightness;
            data[idx + 2] = brightness;
            data[idx + 3] = 255;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    )
}

/// Simple deterministic hash for star placement.
fn pseudo_hash(seed: u32) -> u32 {
    let mut h = seed.wrapping_mul(2654435761);
    h ^= h >> 16;
    h = h.wrapping_mul(2246822507);
    h ^= h >> 13;
    h
}

/// Fade star field visibility based on sun elevation.
pub fn update_star_visibility(
    state: Res<View3DState>,
    sun_state: Res<SunState>,
    mut star_query: Query<&mut Visibility, With<StarField>>,
) {
    let Ok(mut vis) = star_query.single_mut() else {
        return;
    };

    if !state.is_3d_active() {
        *vis = Visibility::Hidden;
        return;
    }

    // Stars visible when sun is below horizon
    *vis = if sun_state.elevation < 0.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

/// Update sun direction from real wall-clock time and map coordinates.
pub fn update_sun_position(
    map_state: Res<MapState>,
    mut sun_state: ResMut<SunState>,
    mut sun_query: Query<(&mut DirectionalLight, &mut Transform), With<SunLight>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    let (elevation, azimuth) = compute_sun_position(map_state.latitude, map_state.longitude);
    sun_state.elevation = elevation;
    sun_state.azimuth = azimuth;

    let Ok((mut light, mut transform)) = sun_query.single_mut() else {
        return;
    };

    // Convert sun elevation and azimuth to directional light rotation.
    let elev_rad = elevation.to_radians();
    let azim_rad = azimuth.to_radians();
    *transform = Transform::from_rotation(
        Quat::from_euler(EulerRot::YXZ, -azim_rad, -elev_rad, 0.0),
    );

    // Scale illuminance with sun elevation
    if elevation > 0.0 {
        let factor = (elevation / 90.0).clamp(0.0, 1.0);
        light.illuminance = 5000.0 * factor.sqrt();
    } else {
        light.illuminance = 0.0;
    }

    // Scale ambient light: bright during day, dim at night
    let ambient_factor = ((elevation + 12.0) / 24.0).clamp(0.05, 1.0);
    ambient.brightness = 300.0 * ambient_factor;
}
