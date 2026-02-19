//! Sky rendering with star field and day/night cycle.
//!
//! The star field is rendered as a large 2D sprite on Camera2d at a low
//! z-depth (behind map tiles). This avoids multi-camera compositing
//! issues while ensuring stars never bleed through opaque tiles.
//! Sun position is computed from real wall-clock time and the map's
//! geographic coordinates.

use bevy::prelude::*;
use bevy::pbr::{Atmosphere, AtmosphereSettings, DistanceFog, FogFalloff, StandardMaterial};

use super::View3DState;
use crate::map::MapState;

/// Z-depth for the star field sprite (behind tiles at z=0.1)
const STAR_Z: f32 = -1.0;

/// Marker component for the star field sprite
#[derive(Component)]
pub struct StarField;

/// Marker component for the ground plane mesh
#[derive(Component)]
pub struct GroundPlane;

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

/// Spawn the star field as a 2D sprite and the ground plane mesh.
pub fn setup_sky(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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

    // Spawn ground plane mesh (hidden until 3D mode activates).
    // Large flat dark surface extends to the horizon beneath tiles.
    let ground_mesh = meshes.add(Plane3d::new(Vec3::Z, Vec2::splat(250_000.0)));
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.1, 0.12),
        unlit: false,
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    commands.spawn((
        Name::new("Ground Plane"),
        GroundPlane,
        Mesh3d(ground_mesh),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden,
    ));
}

/// Generate a procedural star texture as an Image.
/// Black background with scattered white dots.
fn generate_star_texture(size: u32) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    // All pixels start as RGBA(0,0,0,0) — fully transparent.
    // Only star pixels get alpha=255 so the star field composites
    // over the atmosphere sky without an opaque black background.
    let mut data = vec![0u8; (size * size * 4) as usize];

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
    state: Res<View3DState>,
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

    // Scale ambient light
    let ambient_factor = ((elevation + 12.0) / 24.0).clamp(0.05, 1.0);
    if state.is_3d_active() {
        // In 3D mode, atmosphere provides sky irradiance; reduce ambient to avoid double-lighting
        ambient.brightness = 80.0 * ambient_factor;
    } else {
        ambient.brightness = 300.0 * ambient_factor;
    }
}

/// Insert or remove atmosphere components on Camera3d based on 3D mode state.
/// In 3D mode, Camera3d renders first (order=0) with atmosphere painting the sky,
/// and Camera2d renders on top (order=1) with tiles composited over.
pub fn manage_atmosphere_camera(
    mut commands: Commands,
    state: Res<View3DState>,
    medium_handle: Option<Res<crate::AtmosphereMediumHandle>>,
    mut camera_3d: Query<(Entity, &mut Camera, Option<&Atmosphere>), With<Camera3d>>,
    mut camera_2d: Query<&mut Camera, (With<Camera2d>, Without<Camera3d>)>,
    mut ground_query: Query<(&mut Transform, &mut Visibility), With<GroundPlane>>,
) {
    let Some(medium_handle) = medium_handle else {
        return;
    };
    let Ok((cam3d_entity, mut cam3d, has_atmo)) = camera_3d.single_mut() else {
        return;
    };
    let Ok(mut cam2d) = camera_2d.single_mut() else {
        return;
    };

    if state.is_3d_active() {
        if has_atmo.is_none() {
            let scene_units_to_m = 1000.0 / (super::PIXEL_SCALE * state.altitude_scale);
            let mut atmo = Atmosphere::earthlike(medium_handle.0.clone());
            atmo.ground_albedo = Vec3::new(0.05, 0.05, 0.08);
            let fog_density = 3.0 / state.visibility_range;
            commands.entity(cam3d_entity).insert((
                atmo,
                AtmosphereSettings {
                    scene_units_to_m,
                    ..default()
                },
                DistanceFog {
                    color: Color::srgba(0.35, 0.4, 0.5, 1.0),
                    directional_light_color: Color::srgba(1.0, 0.9, 0.7, 0.3),
                    directional_light_exponent: 8.0,
                    falloff: FogFalloff::Exponential { density: fog_density },
                },
            ));
        }
        // Camera3d renders first (order=0), atmosphere paints sky
        cam3d.order = 0;
        cam3d.clear_color = ClearColorConfig::Default;
        // Camera2d renders on top (order=1), tiles composite over atmosphere
        cam2d.order = 1;
        cam2d.clear_color = ClearColorConfig::Custom(Color::NONE);
        // Show ground plane at ground elevation, centered on camera target
        if let Ok((mut gp_transform, mut gp_vis)) = ground_query.single_mut() {
            *gp_vis = Visibility::Inherited;
            gp_transform.translation.z = state.altitude_to_z(state.ground_elevation_ft);
            gp_transform.translation.x = state.saved_2d_center.x;
            gp_transform.translation.y = state.saved_2d_center.y;
        }
    } else {
        if has_atmo.is_some() {
            commands.entity(cam3d_entity)
                .remove::<Atmosphere>()
                .remove::<AtmosphereSettings>()
                .remove::<DistanceFog>();
        }
        // Restore original camera order
        cam2d.order = 0;
        cam2d.clear_color = ClearColorConfig::Default;
        cam3d.order = 1;
        cam3d.clear_color = ClearColorConfig::Custom(Color::NONE);
        // Hide ground plane in 2D
        if let Ok((_, mut gp_vis)) = ground_query.single_mut() {
            *gp_vis = Visibility::Hidden;
        }
    }
}

/// Update atmosphere scale when View3DState changes (altitude_scale).
pub fn update_atmosphere_scale(
    state: Res<View3DState>,
    mut settings_query: Query<&mut AtmosphereSettings, With<Camera3d>>,
) {
    if !state.is_changed() {
        return;
    }
    let Ok(mut settings) = settings_query.single_mut() else {
        return;
    };
    settings.scene_units_to_m = 1000.0 / (super::PIXEL_SCALE * state.altitude_scale);
}

/// Keep the ground plane centered on the camera target so it appears infinite.
pub fn sync_ground_plane(
    state: Res<View3DState>,
    mut ground_query: Query<(&mut Transform, &mut Visibility), With<GroundPlane>>,
) {
    let Ok((mut gp_transform, mut gp_vis)) = ground_query.single_mut() else {
        return;
    };

    if state.is_3d_active() {
        *gp_vis = Visibility::Inherited;
        gp_transform.translation.x = state.saved_2d_center.x;
        gp_transform.translation.y = state.saved_2d_center.y;
        gp_transform.translation.z = state.altitude_to_z(state.ground_elevation_ft);
    } else {
        *gp_vis = Visibility::Hidden;
    }
}

/// Update fog color and density based on sun position and visibility range.
pub fn update_fog_parameters(
    state: Res<View3DState>,
    sun_state: Res<SunState>,
    mut fog_query: Query<&mut DistanceFog, With<Camera3d>>,
) {
    let Ok(mut fog) = fog_query.single_mut() else {
        return;
    };

    // Fog density from visibility range
    fog.falloff = FogFalloff::Exponential {
        density: 3.0 / state.visibility_range,
    };

    // Fog color transitions with sun elevation:
    // - High sun (>30°): blue-gray haze
    // - Low sun (0-30°): warm amber horizon
    // - Below horizon (<0°): dark blue-black
    let elevation = sun_state.elevation;

    let (r, g, b) = if elevation > 30.0 {
        // Midday: muted blue-gray
        (0.55, 0.62, 0.72)
    } else if elevation > 0.0 {
        // Golden hour: interpolate from warm to blue-gray
        let t = elevation / 30.0;
        let warm = (0.7, 0.5, 0.3);
        let cool = (0.55, 0.62, 0.72);
        (
            warm.0 + (cool.0 - warm.0) * t,
            warm.1 + (cool.1 - warm.1) * t,
            warm.2 + (cool.2 - warm.2) * t,
        )
    } else if elevation > -12.0 {
        // Twilight: fade to dark
        let t = (elevation + 12.0) / 12.0; // 1.0 at horizon, 0.0 at -12°
        (0.7 * t * 0.15, 0.5 * t * 0.15, 0.3 * t * 0.2)
    } else {
        // Night: near black
        (0.02, 0.02, 0.04)
    };

    fog.color = Color::srgb(r, g, b);

    // Sun glow through fog (warm directional light effect)
    if elevation > 0.0 {
        let glow_intensity = (elevation / 90.0).sqrt() * 0.5;
        fog.directional_light_color = Color::srgba(1.0, 0.85, 0.6, glow_intensity);
    } else {
        fog.directional_light_color = Color::srgba(0.0, 0.0, 0.0, 0.0);
    }
}
