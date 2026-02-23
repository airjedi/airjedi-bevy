//! Sky rendering with star field and day/night cycle.
//!
//! The star field is rendered as a large 2D sprite on Camera2d at a low
//! z-depth (behind map tiles). This avoids multi-camera compositing
//! issues while ensuring stars never bleed through opaque tiles.
//! Sun position is computed using the NREL Solar Position Algorithm (SPA)
//! via the `solar-positioning` crate (~0.0003 degree accuracy).
//! Supports both real-time wall clock and manual time override via TimeState.

use bevy::prelude::*;
use bevy::camera::{CameraOutputMode, Exposure};
use bevy::pbr::{Atmosphere, AtmosphereSettings, DistanceFog, FogFalloff, StandardMaterial};
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::render::render_resource::BlendState;

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

/// Marker for the 2D mode day/night color overlay sprite.
#[derive(Component)]
pub struct DayNightTint;

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

/// Controls whether the app uses real wall-clock time or a manual override.
#[derive(Resource)]
pub struct TimeState {
    pub override_time: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub utc_offset_hours: f32,
}

impl Default for TimeState {
    fn default() -> Self {
        Self {
            override_time: None,
            utc_offset_hours: 0.0,
        }
    }
}

impl TimeState {
    pub fn current_datetime(&self) -> chrono::DateTime<chrono::FixedOffset> {
        self.override_time
            .unwrap_or_else(|| chrono::Utc::now().fixed_offset())
    }

    pub fn is_manual(&self) -> bool {
        self.override_time.is_some()
    }

    pub fn set_hour(&mut self, hour: f32) {
        use chrono::Timelike;
        let now = chrono::Utc::now();
        let offset_secs = (self.utc_offset_hours * 3600.0) as i32;
        let offset = chrono::FixedOffset::east_opt(offset_secs)
            .unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
        let local_today = now.with_timezone(&offset);

        let h = hour.floor() as u32;
        let m = ((hour.fract()) * 60.0).floor() as u32;
        if let Some(dt) = local_today
            .with_hour(h.min(23))
            .and_then(|d| d.with_minute(m.min(59)))
            .and_then(|d| d.with_second(0))
        {
            self.override_time = Some(dt.fixed_offset());
        }
    }

    pub fn reset_to_live(&mut self) {
        self.override_time = None;
    }
}

/// Compute sun elevation and azimuth using the NREL Solar Position Algorithm.
/// Accuracy: ~0.0003 degrees. Handles polar day/night edge cases.
pub fn compute_sun_position(latitude: f64, longitude: f64) -> (f32, f32) {
    let now = chrono::Utc::now().fixed_offset();
    compute_sun_position_at(latitude, longitude, &now)
}

/// Compute sun position at a specific time (for time slider support).
pub fn compute_sun_position_at(
    latitude: f64,
    longitude: f64,
    datetime: &chrono::DateTime<chrono::FixedOffset>,
) -> (f32, f32) {
    use solar_positioning::{spa, time::DeltaT, RefractionCorrection};

    let delta_t = DeltaT::estimate_from_date_like(*datetime).unwrap_or(69.184);

    match spa::solar_position(
        *datetime,
        latitude,
        longitude,
        0.0,
        delta_t,
        Some(RefractionCorrection::standard()),
    ) {
        Ok(position) => {
            let elevation = position.elevation_angle() as f32;
            let azimuth = position.azimuth() as f32;
            (elevation, azimuth)
        }
        Err(_) => (45.0, 180.0),
    }
}

/// Marker for the directional light used as the sun
#[derive(Component)]
pub struct SunLight;

/// Marker for the directional light used as moonlight.
#[derive(Component)]
pub struct MoonLight;

/// Resource tracking current moon position and phase.
#[derive(Resource)]
pub struct MoonState {
    pub elevation: f32,
    pub azimuth: f32,
    /// 0.0 = new moon, 0.5 = full moon, 1.0 = new moon again
    pub phase: f32,
}

impl Default for MoonState {
    fn default() -> Self {
        Self { elevation: -10.0, azimuth: 0.0, phase: 0.5 }
    }
}

/// Simplified moon position using J2000.0 epoch (~2-5 degree accuracy).
fn compute_moon_position(
    latitude: f64,
    longitude: f64,
    datetime: &chrono::DateTime<chrono::FixedOffset>,
) -> (f32, f32, f32) {
    let timestamp = datetime.timestamp() as f64;
    let j2000_epoch = 946728000.0_f64;
    let days = (timestamp - j2000_epoch) / 86400.0;

    let l = (218.316 + 13.176396 * days) % 360.0;
    let m = (134.963 + 13.064993 * days) % 360.0;
    let f = (93.272 + 13.229350 * days) % 360.0;

    let m_rad = m.to_radians();
    let f_rad = f.to_radians();

    let ecl_lon = (l + 6.289 * m_rad.sin()).to_radians();
    let ecl_lat = (5.128 * f_rad.sin()).to_radians();

    let obliquity = 23.439_f64.to_radians();

    let sin_ra = ecl_lon.sin() * obliquity.cos() - ecl_lat.tan() * obliquity.sin();
    let cos_ra = ecl_lon.cos();
    let declination = (ecl_lat.cos() * obliquity.sin() * ecl_lon.sin()
        + ecl_lat.sin() * obliquity.cos()).asin();

    let gmst = (280.46061837 + 360.98564736629 * days) % 360.0;
    let ra = sin_ra.atan2(cos_ra).to_degrees();
    let local_sidereal = (gmst + longitude) % 360.0;
    let hour_angle = (local_sidereal - ra).to_radians();

    let lat_rad = latitude.to_radians();

    let sin_alt = lat_rad.sin() * declination.sin()
        + lat_rad.cos() * declination.cos() * hour_angle.cos();
    let elevation = sin_alt.asin();

    let cos_az = (declination.sin() - lat_rad.sin() * sin_alt)
        / (lat_rad.cos() * elevation.cos());
    let mut azimuth = cos_az.clamp(-1.0, 1.0).acos();
    if hour_angle.sin() > 0.0 {
        azimuth = std::f64::consts::TAU - azimuth;
    }

    // Lunar phase: synodic month = 29.530588853 days
    // Known new moon: 2000-01-06 18:14 UTC (J2000 + 5.76 days)
    let synodic_month = 29.530588853;
    let phase = ((days - 5.76) % synodic_month) / synodic_month;
    let phase = if phase < 0.0 { phase + 1.0 } else { phase };

    (
        elevation.to_degrees() as f32,
        azimuth.to_degrees() as f32,
        phase as f32,
    )
}

/// No-op: sky visibility is handled entirely through star field sprite visibility.
/// Kept as a system entry point for future atmospheric effects.
pub fn update_sky_visibility(
    _state: Res<View3DState>,
) {
}

/// Keep star field sprite centered on Camera2d and scaled to fill the viewport.
pub fn sync_sky_camera(
    state: Res<View3DState>,
    main_camera: Query<(&Transform, &Projection), (With<crate::MapCamera>, Without<StarField>)>,
    window_query: Query<&Window>,
    zoom_state: Res<crate::ZoomState>,
    mut star_query: Query<&mut Transform, (With<StarField>, Without<crate::MapCamera>)>,
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
        let sx = (window.width() * scale_factor * 2.0) / 4096.0;
        let sy = (window.height() * scale_factor * 2.0) / 4096.0;
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
    let star_image = generate_star_texture(4096);
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
    // Color matches dark CartoDB basemap tiles so the transition
    // from tiles to ground plane is seamless at distance.
    let ground_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(250_000.0)));
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.15, 0.18),
        unlit: true,
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

    // Full-screen tint overlay for 2D mode day/night effect.
    // Between tiles (z=0) and aircraft (z=10) at z=5.
    commands.spawn((
        Name::new("Day Night Tint"),
        DayNightTint,
        Sprite {
            color: Color::srgba(0.0, 0.0, 0.0, 0.0),
            custom_size: Some(Vec2::new(100_000.0, 100_000.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 5.0),
        Visibility::Hidden,
    ));
}

/// Generate a procedural star texture as an Image.
/// ~3000 main stars with magnitude-based brightness and color variation,
/// plus a ~2000-star Milky Way band along a diagonal gaussian belt.
fn generate_star_texture(size: u32) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let mut data = vec![0u8; (size * size * 4) as usize];

    // Main star field: ~3000 stars with magnitude-based brightness
    let num_stars = 3000u32;
    for i in 0..num_stars {
        let hash = pseudo_hash(i);
        let x = (hash % size) as usize;
        let y = ((hash / size) % size) as usize;

        let mag_hash = pseudo_hash(i + num_stars) % 1000;
        let mag_factor = mag_hash as f32 / 1000.0;
        let brightness = (40.0 + 215.0 * mag_factor * mag_factor * mag_factor) as u8;

        let idx = (y * size as usize + x) * 4;
        if idx + 3 < data.len() {
            let color_hash = pseudo_hash(i + num_stars * 2) % 100;
            let (r, g, b) = if color_hash < 15 {
                // Blue-white hot stars
                (brightness.saturating_sub(20), brightness.saturating_sub(10), brightness)
            } else if color_hash < 25 {
                // Warm yellow stars
                (brightness, brightness.saturating_sub(15), brightness.saturating_sub(40))
            } else {
                (brightness, brightness, brightness)
            };
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = 255;
        }
    }

    // Milky Way band: extra-dim stars concentrated along a diagonal gaussian belt
    let milky_way_stars = 2000u32;
    for i in 0..milky_way_stars {
        let hash = pseudo_hash(i + num_stars * 3);
        let along = (hash % 10000) as f32 / 10000.0;
        let offset_hash = pseudo_hash(i + num_stars * 4);
        let gauss = ((offset_hash % 100) as f32 / 100.0 - 0.5)
            + ((pseudo_hash(i + num_stars * 5) % 100) as f32 / 100.0 - 0.5);
        let band_width = 0.08;
        let perpendicular = gauss * band_width;

        let x = ((along + perpendicular * 0.7) * size as f32) as usize % size as usize;
        let y = ((along * 0.8 + 0.1 + perpendicular) * size as f32) as usize % size as usize;

        let brightness = 25 + (pseudo_hash(i + num_stars * 6) % 35) as u8;

        let idx = (y * size as usize + x) * 4;
        if idx + 3 < data.len() {
            // Additive blending: don't overwrite brighter stars with dimmer Milky Way pixels
            let existing = data[idx];
            if brightness > existing {
                data[idx] = brightness;
                data[idx + 1] = brightness;
                data[idx + 2] = brightness + 5;
                data[idx + 3] = 255;
            }
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

/// Hide star field sprite. The star field is a Camera2d sprite; in 3D mode,
/// Camera2d composites on top of Camera3d via alpha blending, so visible stars
/// bleed through onto the mesh quad tiles. The Atmosphere component handles
/// sky rendering. Night stars would require a Camera3d skybox mesh.
pub fn update_star_visibility(
    _state: Res<View3DState>,
    _sun_state: Res<SunState>,
    _time: Res<Time>,
    mut star_query: Query<&mut Visibility, With<StarField>>,
) {
    let Ok(mut vis) = star_query.single_mut() else {
        return;
    };
    *vis = Visibility::Hidden;
}

/// Update sun direction from time state and map coordinates.
pub fn update_sun_position(
    map_state: Res<MapState>,
    time_state: Res<TimeState>,
    mut sun_state: ResMut<SunState>,
    mut sun_query: Query<(&mut DirectionalLight, &mut Transform), With<SunLight>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    let datetime = time_state.current_datetime();
    let (elevation, azimuth) = compute_sun_position_at(
        map_state.latitude,
        map_state.longitude,
        &datetime,
    );

    // Only update when position changes meaningfully (0.05 degrees ≈ 12 seconds of time).
    // Avoiding per-frame writes prevents Bevy change detection from triggering
    // atmosphere/fog recalculation every frame, which causes tile flashing.
    let elev_changed = (sun_state.elevation - elevation).abs() > 0.05;
    let azim_changed = (sun_state.azimuth - azimuth).abs() > 0.05;
    if !elev_changed && !azim_changed {
        return;
    }

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

    // Scale illuminance with sun elevation (128,000 lux = raw sunlight pre-scattering)
    if elevation > 0.0 {
        let factor = (elevation / 90.0).clamp(0.0, 1.0);
        light.illuminance = 128_000.0 * factor.sqrt();
    } else {
        light.illuminance = 0.0;
    }

    // Ambient light with civil/nautical/astronomical twilight zones
    let ambient_factor = if elevation > 0.0 {
        1.0
    } else if elevation > -6.0 {
        // Civil twilight: -6 to 0 degrees
        ((elevation + 6.0) / 6.0).clamp(0.0, 1.0) * 0.8 + 0.2
    } else if elevation > -12.0 {
        // Nautical twilight: -12 to -6 degrees
        ((elevation + 12.0) / 6.0).clamp(0.0, 1.0) * 0.15 + 0.05
    } else if elevation > -18.0 {
        // Astronomical twilight: -18 to -12 degrees
        ((elevation + 18.0) / 6.0).clamp(0.0, 1.0) * 0.04 + 0.01
    } else {
        // Full night
        0.01
    };

    ambient.brightness = 300.0 * ambient_factor;
}

/// Update moon position and moonlight from time and map coordinates.
pub fn update_moon_position(
    map_state: Res<MapState>,
    time_state: Res<TimeState>,
    mut moon_state: ResMut<MoonState>,
    mut moon_query: Query<(&mut DirectionalLight, &mut Transform), With<MoonLight>>,
) {
    let datetime = time_state.current_datetime();
    let (elevation, azimuth, phase) = compute_moon_position(
        map_state.latitude,
        map_state.longitude,
        &datetime,
    );

    // Only update when position changes meaningfully.
    let elev_changed = (moon_state.elevation - elevation).abs() > 0.05;
    let azim_changed = (moon_state.azimuth - azimuth).abs() > 0.05;
    if !elev_changed && !azim_changed {
        return;
    }

    moon_state.elevation = elevation;
    moon_state.azimuth = azimuth;
    moon_state.phase = phase;

    let Ok((mut light, mut transform)) = moon_query.single_mut() else {
        return;
    };

    let elev_rad = elevation.to_radians();
    let azim_rad = azimuth.to_radians();
    *transform = Transform::from_rotation(
        Quat::from_euler(EulerRot::YXZ, -azim_rad, -elev_rad, 0.0),
    );

    // Full moon ~0.25 lux, scaled by phase (sine curve peaks at phase=0.5)
    let phase_illuminance = (std::f32::consts::PI * phase).sin();
    if elevation > 0.0 {
        let elev_factor = (elevation / 90.0).clamp(0.0, 1.0).sqrt();
        light.illuminance = 0.25 * phase_illuminance * elev_factor;
    } else {
        light.illuminance = 0.0;
    }
}

/// Keep time offset in sync with map longitude.
pub fn sync_time_offset(
    map_state: Res<MapState>,
    mut time_state: ResMut<TimeState>,
) {
    let new_offset = (map_state.longitude / 15.0) as f32;
    if (time_state.utc_offset_hours - new_offset).abs() > 0.01 {
        time_state.utc_offset_hours = new_offset;
    }
}

/// Manage Atmosphere component on Camera3d based on 3D mode state.
/// In 3D mode, Camera3d renders first (order=0) with atmosphere painting the sky.
/// Camera2d renders on top (order=1) with tiles composited over.
pub fn manage_atmosphere_camera(
    mut commands: Commands,
    state: Res<View3DState>,
    sun_state: Res<SunState>,
    medium_handle: Option<Res<crate::AtmosphereMediumHandle>>,
    mut camera_3d: Query<(Entity, &mut Camera, Option<&Atmosphere>), With<Camera3d>>,
    mut camera_2d: Query<&mut Camera, (With<crate::MapCamera>, Without<Camera3d>)>,
    mut ground_query: Query<(&mut Transform, &mut Visibility), With<GroundPlane>>,
) {
    let Ok((cam3d_entity, mut cam3d, has_atmo)) = camera_3d.single_mut() else {
        return;
    };
    let Ok(mut cam2d) = camera_2d.single_mut() else {
        return;
    };

    if state.is_3d_active() {
        // Atmosphere is kept always-present while in 3D mode. Adding/removing
        // the Atmosphere component across the day/night threshold disrupts
        // Bevy's HDR rendering pipeline, causing tiles to go black.
        // With the thin atmosphere (15km, 0.3x scale), nighttime scattering
        // is naturally minimal so the sky stays dark without removal.
        let show_atmo = state.atmosphere_enabled;

        if show_atmo {
            cam3d.clear_color = ClearColorConfig::Default;
        } else {
            cam3d.clear_color = ClearColorConfig::Custom(Color::BLACK);
        }

        // Add atmosphere components once when entering 3D (or re-enabling)
        if show_atmo && has_atmo.is_none() {
            // Scale factor reduced (0.3x) to shorten effective atmospheric path
            // lengths, suppressing the bright Mie scattering band at the horizon.
            let scene_units_to_m = 0.3 * 1000.0 / (super::PIXEL_SCALE * state.altitude_scale);
            if let Some(ref medium_handle) = medium_handle {
                let mut atmo = Atmosphere::earthlike(medium_handle.0.clone());
                // Zero ground albedo hides the below-horizon atmosphere
                // rendering so only the sky dome is visible.
                atmo.ground_albedo = Vec3::ZERO;
                // Use default earthlike top_radius (100km). The 0.3x
                // scene_units_to_m already reduces effective scattering.
                commands.entity(cam3d_entity).insert((
                    atmo,
                    AtmosphereSettings {
                        scene_units_to_m,
                        // Limit aerial perspective range to prevent the
                        // atmosphere post-process from hazing the ground
                        // plane into a bright band at the horizon.
                        aerial_view_lut_max_distance: 500.0,
                        ..default()
                    },
                    AtmosphereEnvironmentMapLight::default(),
                    Exposure { ev100: 9.0 },
                    DistanceFog {
                        // Dark fog color matching CartoDB dark basemap tiles
                        // to avoid a bright band at the horizon.
                        color: Color::srgba(0.10, 0.10, 0.12, 1.0),
                        directional_light_color: Color::NONE,
                        directional_light_exponent: 30.0,
                        falloff: FogFalloff::Linear {
                            start: state.visibility_range * 0.4,
                            end: state.visibility_range,
                        },
                    },
                ));
            }
        } else if !show_atmo && has_atmo.is_some() {
            // User disabled atmosphere via checkbox — remove all components
            commands.entity(cam3d_entity)
                .remove::<Atmosphere>()
                .remove::<AtmosphereSettings>()
                .remove::<AtmosphereEnvironmentMapLight>()
                .remove::<Exposure>()
                .remove::<DistanceFog>();
        }
        // Camera3d renders first (order=0), atmosphere paints sky
        cam3d.order = 0;
        // Camera2d renders on top (order=1) with alpha blending so transparent
        // areas show Camera3d's atmosphere sky through.
        cam2d.order = 1;
        cam2d.clear_color = ClearColorConfig::Custom(Color::NONE);
        cam2d.output_mode = CameraOutputMode::Write {
            blend_state: Some(BlendState::ALPHA_BLENDING),
            clear_color: ClearColorConfig::None,
        };
        // Ground plane visible as a depth backstop — it writes depth everywhere
        // below the horizon so the atmosphere post-process never treats those
        // pixels as sky. Tile mesh quads render on top of it.
        if let Ok((_, mut gp_vis)) = ground_query.single_mut() {
            *gp_vis = Visibility::Inherited;
        }
    } else {
        if has_atmo.is_some() {
            commands.entity(cam3d_entity)
                .remove::<Atmosphere>()
                .remove::<AtmosphereSettings>()
                .remove::<AtmosphereEnvironmentMapLight>()
                .remove::<Exposure>()
                .remove::<DistanceFog>();
        }
        cam2d.order = 0;
        cam2d.clear_color = ClearColorConfig::Default;
        cam2d.output_mode = CameraOutputMode::default();
        cam3d.order = 1;
        cam3d.clear_color = ClearColorConfig::Custom(Color::NONE);
        if let Ok((_, mut gp_vis)) = ground_query.single_mut() {
            *gp_vis = Visibility::Hidden;
        }
    }
}

/// Update atmosphere scale when altitude_scale changes.
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
    settings.scene_units_to_m = 0.3 * 1000.0 / (super::PIXEL_SCALE * state.altitude_scale);
}

/// Blend DistanceFog color between daytime blue-gray and nighttime dark
/// based on sun elevation, so the fog matches the scene lighting.
pub fn update_fog_color_for_time(
    sun_state: Res<SunState>,
    state: Res<View3DState>,
    mut fog_query: Query<&mut DistanceFog, With<Camera3d>>,
) {
    if !state.is_3d_active() || !state.atmosphere_enabled {
        return;
    }
    let Ok(mut fog) = fog_query.single_mut() else {
        return;
    };

    let elev = sun_state.elevation;

    // Blend factor: 1.0 at full day (sun > 10°), 0.0 at night (sun < -6°)
    let t = if elev > 10.0 {
        1.0
    } else if elev > -6.0 {
        (elev + 6.0) / 16.0
    } else {
        0.0
    };

    // Day: subtle blue-gray, low opacity. Night: dark, high opacity.
    // Both color and alpha animate so daytime fog is barely visible
    // while nighttime fog blends smoothly into the dark scene.
    let r = 0.10 + 0.30 * t;
    let g = 0.10 + 0.35 * t;
    let b = 0.12 + 0.38 * t;
    let a = 0.8 - 0.5 * t; // Night: 0.8, Day: 0.3
    fog.color = Color::srgba(r, g, b, a);
}

/// Adapt camera exposure to time of day.
/// At night, lower the EV100 so faint atmospheric scattering isn't amplified
/// into a bright horizon glow.
///
/// Uses a large dead zone (0.5 EV) to avoid frequent updates that can cause
/// the atmosphere post-process to re-render and flash tiles near the horizon.
pub fn update_exposure_for_time(
    sun_state: Res<SunState>,
    state: Res<View3DState>,
    mut camera_query: Query<&mut Exposure, With<Camera3d>>,
) {
    if !state.is_3d_active() || !state.atmosphere_enabled {
        return;
    }
    let Ok(mut exposure) = camera_query.single_mut() else {
        return;
    };

    let elev = sun_state.elevation;

    // EV100: 13.0 at full day, ramp down through twilight to 2.0 at night.
    // The low night value crushes residual atmosphere scattering at the
    // horizon to near-invisible (atmosphere is kept always-present in 3D).
    let ev = if elev > 10.0 {
        13.0
    } else if elev > 0.0 {
        // Low sun: 11..13
        11.0 + 2.0 * (elev / 10.0)
    } else if elev > -6.0 {
        // Civil twilight: 5..11
        5.0 + 6.0 * ((elev + 6.0) / 6.0)
    } else if elev > -12.0 {
        // Nautical twilight: 3..5
        3.0 + 2.0 * ((elev + 12.0) / 6.0)
    } else if elev > -18.0 {
        // Astronomical twilight: 2..3
        2.0 + 1.0 * ((elev + 18.0) / 6.0)
    } else {
        2.0
    };

    // Only write when crossing a significant threshold to minimize
    // atmosphere post-process recalculations that can flash tiles.
    if (exposure.ev100 - ev).abs() > 0.5 {
        exposure.ev100 = ev;
    }
}

/// Keep the ground plane centered on the camera target in Y-up space.
pub fn sync_ground_plane(
    state: Res<View3DState>,
    mut ground_query: Query<(&mut Transform, &mut Visibility), With<GroundPlane>>,
) {
    let Ok((mut gp_transform, mut gp_vis)) = ground_query.single_mut() else {
        return;
    };

    if state.is_3d_active() {
        *gp_vis = Visibility::Inherited;
        // Place the ground plane slightly below the tile mesh quads so tiles
        // always render on top. Lower-zoom tiles can be up to 0.2 units below
        // the base ground level (zoom_diff * 0.05), so offset by 1.0 to clear all.
        let ground_alt = state.altitude_to_z(state.ground_elevation_ft) - 1.0;
        let pos_yup = super::zup_to_yup(Vec3::new(
            state.saved_2d_center.x,
            state.saved_2d_center.y,
            ground_alt,
        ));
        gp_transform.translation = pos_yup;
    } else {
        *gp_vis = Visibility::Hidden;
    }
}

/// Darken the ground plane at night so it doesn't create a gray band
/// against the black sky at the horizon.
pub fn update_ground_plane_color(
    sun_state: Res<SunState>,
    state: Res<View3DState>,
    ground_query: Query<&MeshMaterial3d<StandardMaterial>, With<GroundPlane>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !state.is_3d_active() {
        return;
    }
    let Ok(mat_handle) = ground_query.single() else {
        return;
    };
    let Some(material) = materials.get_mut(mat_handle.id()) else {
        return;
    };

    // Daytime base color matches dark CartoDB tiles (0.15, 0.15, 0.18).
    // Below the horizon, fade to black.
    let factor = if sun_state.elevation > 6.0 {
        1.0
    } else if sun_state.elevation > 0.0 {
        sun_state.elevation / 6.0
    } else {
        0.0
    };

    material.base_color = Color::srgb(0.15 * factor, 0.15 * factor, 0.18 * factor);
}

/// Apply subtle time-of-day color tinting in 2D map mode.
pub fn update_2d_tint(
    state: Res<View3DState>,
    sun_state: Res<SunState>,
    main_camera: Query<&Transform, (With<crate::MapCamera>, Without<DayNightTint>)>,
    mut tint_query: Query<(&mut Transform, &mut Sprite, &mut Visibility), With<DayNightTint>>,
) {
    let Ok((mut tint_tf, mut sprite, mut vis)) = tint_query.single_mut() else {
        return;
    };

    if state.is_3d_active() {
        *vis = Visibility::Hidden;
        return;
    }

    let elevation = sun_state.elevation;

    let (r, g, b, a) = if elevation > 10.0 {
        // Full daylight: no tint
        (0.0, 0.0, 0.0, 0.0)
    } else if elevation > 0.0 {
        // Low sun / golden hour: warm orange tint
        let t = 1.0 - (elevation / 10.0);
        (0.9, 0.6, 0.2, t * 0.08)
    } else if elevation > -6.0 {
        // Civil twilight: transition from warm to cool blue
        let t = (-elevation) / 6.0;
        let r = 0.9 * (1.0 - t) + 0.1 * t;
        let g = 0.6 * (1.0 - t) + 0.1 * t;
        let b = 0.2 * (1.0 - t) + 0.3 * t;
        (r, g, b, 0.08 + t * 0.12)
    } else if elevation > -18.0 {
        // Nautical/astronomical twilight: deepening blue
        let t = ((-elevation) - 6.0) / 12.0;
        (0.05, 0.05, 0.15 + 0.1 * (1.0 - t), 0.2 + t * 0.15)
    } else {
        // Full night: dark blue overlay
        (0.02, 0.02, 0.08, 0.3)
    };

    if a < 0.001 {
        *vis = Visibility::Hidden;
    } else {
        *vis = Visibility::Inherited;
        sprite.color = Color::srgba(r, g, b, a);
    }

    // Keep tint centered on camera so it covers the viewport during panning
    if let Ok(cam_tf) = main_camera.single() {
        tint_tf.translation.x = cam_tf.translation.x;
        tint_tf.translation.y = cam_tf.translation.y;
    }
}
