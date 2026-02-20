# Atmosphere Realism Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade AirJedi's 3D view with accurate solar positioning, enhanced atmosphere/fog, improved night sky, moonlight, time controls, and subtle 2D mode tinting.

**Architecture:** Five independent phases implemented in git worktrees. Phases 1, 3, and 5 are fully independent and run in parallel. Phases 2 and 4 depend on Phase 1 (TimeState resource). Each phase is a feature branch merged to main after completion.

**Tech Stack:** Bevy 0.18 (Atmosphere, DistanceFog, DirectionalLight), `solar-positioning` crate (SPA algorithm), `chrono` (already a dependency), bevy_egui for time slider UI.

---

## Worktree Setup

Before starting any tasks, create worktrees for the three parallel phases:

```bash
cd /Users/ccustine/development/aviation/airjedi-bevy

# Phase 1 worktree
git worktree add ../.worktrees/airjedi-solar-accuracy -b feat/solar-accuracy

# Phase 3 worktree (independent, starts in parallel)
git worktree add ../.worktrees/airjedi-night-sky -b feat/night-sky

# Phase 5 worktree (independent, starts in parallel)
git worktree add ../.worktrees/airjedi-2d-tinting -b feat/2d-tinting
```

After Phase 1 merges, create worktrees for dependent phases:

```bash
# Phase 2 worktree (depends on Phase 1)
git worktree add ../.worktrees/airjedi-atmosphere-tuning -b feat/atmosphere-tuning

# Phase 4 worktree (depends on Phase 1)
git worktree add ../.worktrees/airjedi-moonlight -b feat/moonlight
```

---

## Task 1: Accurate Solar Positioning (Phase 1)

**Worktree:** `../.worktrees/airjedi-solar-accuracy`

**Files:**
- Modify: `Cargo.toml` (add `solar-positioning` dependency)
- Modify: `src/view3d/sky.rs:46-92` (replace `compute_sun_position`)
- Modify: `src/view3d/sky.rs:254-292` (update `update_sun_position` to use new API)

### Step 1: Add solar-positioning dependency

In `Cargo.toml`, add to `[dependencies]`:

```toml
solar-positioning = "0.3"
```

Run: `cargo check`
Expected: compiles with new dependency available.

### Step 2: Replace compute_sun_position with SPA algorithm

Replace the entire `compute_sun_position` function in `src/view3d/sky.rs:46-92` with:

```rust
/// Compute sun elevation and azimuth using the NREL Solar Position Algorithm.
/// Accuracy: ~0.0003 degrees. Handles polar day/night edge cases.
pub fn compute_sun_position(latitude: f64, longitude: f64) -> (f32, f32) {
    use chrono::Utc;
    use solar_positioning::{spa, DeltaT, RefractionCorrection};

    let now = Utc::now().fixed_offset();
    compute_sun_position_at(latitude, longitude, &now)
}

/// Compute sun position at a specific time (for time slider support).
pub fn compute_sun_position_at(
    latitude: f64,
    longitude: f64,
    datetime: &chrono::DateTime<chrono::FixedOffset>,
) -> (f32, f32) {
    use solar_positioning::{spa, DeltaT, RefractionCorrection};

    let delta_t = DeltaT::estimate_from_date_like(datetime)
        .unwrap_or(DeltaT::new(69.184));

    match spa::solar_position(
        *datetime,
        latitude,
        longitude,
        0.0, // elevation meters (sea level default)
        delta_t,
        Some(RefractionCorrection::standard()),
    ) {
        Ok(position) => {
            let elevation = position.elevation_angle() as f32;
            let azimuth = position.azimuth() as f32;
            (elevation, azimuth)
        }
        Err(_) => {
            // Fallback: sun at noon position
            (45.0, 180.0)
        }
    }
}
```

### Step 3: Update update_sun_position to use compute_sun_position_at

In `src/view3d/sky.rs`, modify `update_sun_position` (line 254) to accept `TimeState`:

```rust
/// Update sun direction from time and map coordinates.
pub fn update_sun_position(
    map_state: Res<MapState>,
    state: Res<View3DState>,
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

    // Use physically-based illuminance: ~128,000 lux raw sunlight pre-scattering,
    // scaled by elevation. The atmosphere component handles scattering reduction.
    if elevation > 0.0 {
        let factor = (elevation / 90.0).clamp(0.0, 1.0);
        light.illuminance = 128_000.0 * factor.sqrt();
    } else {
        light.illuminance = 0.0;
    }

    // Smooth ambient light curve through twilight zones:
    // Civil twilight (-6°), nautical (-12°), astronomical (-18°)
    let ambient_factor = if elevation > 0.0 {
        1.0
    } else if elevation > -6.0 {
        // Civil twilight: rapid falloff
        ((elevation + 6.0) / 6.0).clamp(0.0, 1.0) * 0.8 + 0.2
    } else if elevation > -12.0 {
        // Nautical twilight
        ((elevation + 12.0) / 6.0).clamp(0.0, 1.0) * 0.15 + 0.05
    } else if elevation > -18.0 {
        // Astronomical twilight
        ((elevation + 18.0) / 6.0).clamp(0.0, 1.0) * 0.04 + 0.01
    } else {
        0.01
    };

    if state.is_3d_active() {
        ambient.brightness = 80.0 * ambient_factor;
    } else {
        ambient.brightness = 300.0 * ambient_factor;
    }
}
```

### Step 4: Build and verify

Run: `cargo build`
Expected: compiles without errors. The sun position should now be more accurate (noticeable at high latitudes or near sunrise/sunset).

### Step 5: Commit

```bash
git add Cargo.toml src/view3d/sky.rs
git commit -m "Replace solar algorithm with SPA via solar-positioning crate"
```

---

## Task 2: Time State Resource and Slider (Phase 1)

**Worktree:** `../.worktrees/airjedi-solar-accuracy`

**Files:**
- Modify: `src/view3d/sky.rs` (add `TimeState` resource)
- Modify: `src/view3d/mod.rs:209-300` (add time slider to 3D panel)
- Modify: `src/view3d/mod.rs:742-771` (register `TimeState` resource)

### Step 1: Add TimeState resource to sky.rs

Add after the `SunState` definition (after line 42):

```rust
/// Controls whether the app uses real wall-clock time or a manual override.
#[derive(Resource)]
pub struct TimeState {
    /// When Some, use this fixed time instead of wall clock.
    pub override_time: Option<chrono::DateTime<chrono::FixedOffset>>,
    /// The UTC offset for the map's current longitude (approximate).
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
    /// Get the current datetime (override or wall clock).
    pub fn current_datetime(&self) -> chrono::DateTime<chrono::FixedOffset> {
        self.override_time.unwrap_or_else(|| chrono::Utc::now().fixed_offset())
    }

    /// Whether we're using manual time override.
    pub fn is_manual(&self) -> bool {
        self.override_time.is_some()
    }

    /// Set override to a specific hour (0-24) on today's date at the map's longitude.
    pub fn set_hour(&mut self, hour: f32) {
        use chrono::{Utc, Duration, Datelike, Timelike};
        let now = Utc::now();
        let offset_secs = (self.utc_offset_hours * 3600.0) as i32;
        let offset = chrono::FixedOffset::east_opt(offset_secs)
            .unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
        let local_today = now.with_timezone(&offset);

        let h = hour.floor() as u32;
        let m = ((hour.fract()) * 60.0).floor() as u32;
        if let Some(dt) = local_today.with_hour(h.min(23))
            .and_then(|d| d.with_minute(m.min(59)))
            .and_then(|d| d.with_second(0))
        {
            self.override_time = Some(dt.fixed_offset());
        }
    }

    /// Reset to wall-clock time.
    pub fn reset_to_live(&mut self) {
        self.override_time = None;
    }
}
```

### Step 2: Add system to update UTC offset from map longitude

Add to `sky.rs`:

```rust
/// Keep TimeState's UTC offset in sync with the map center longitude.
pub fn sync_time_offset(
    map_state: Res<MapState>,
    mut time_state: ResMut<TimeState>,
) {
    // Approximate UTC offset: 1 hour per 15 degrees longitude
    time_state.utc_offset_hours = (map_state.longitude / 15.0) as f32;
}
```

### Step 3: Add time slider to the 3D view panel

In `src/view3d/mod.rs`, inside `render_3d_view_panel` (after the Ground Elevation section, before the "Press '3'" label), add:

```rust
            ui.separator();
            ui.heading("Time of Day");

            let mut time_state = state; // We'll need to adjust the function signature
```

Wait — `render_3d_view_panel` currently only takes `View3DState`. We need to add `TimeState` as a parameter. Modify the function signature at line 209:

```rust
pub fn render_3d_view_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<View3DState>,
    mut time_state: ResMut<sky::TimeState>,
    sun_state: Res<sky::SunState>,
) {
```

Then add this UI section inside the `show` closure, after the ground elevation block (before the "Press '3'" separator):

```rust
            ui.separator();
            ui.heading("Time of Day");

            let mut is_manual = time_state.is_manual();
            if ui.checkbox(&mut is_manual, "Manual time override").changed() {
                if is_manual {
                    // Initialize to current wall-clock hour
                    let now = chrono::Utc::now().fixed_offset();
                    let offset_secs = (time_state.utc_offset_hours * 3600.0) as i32;
                    let offset = chrono::FixedOffset::east_opt(offset_secs)
                        .unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
                    let local = now.with_timezone(&offset);
                    let hour = local.hour() as f32 + local.minute() as f32 / 60.0;
                    time_state.set_hour(hour);
                } else {
                    time_state.reset_to_live();
                }
            }

            if time_state.is_manual() {
                let current = time_state.current_datetime();
                let offset_secs = (time_state.utc_offset_hours * 3600.0) as i32;
                let offset = chrono::FixedOffset::east_opt(offset_secs)
                    .unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
                let local = current.with_timezone(&offset);
                let mut hour = local.hour() as f32 + local.minute() as f32 / 60.0;

                ui.horizontal(|ui| {
                    ui.label("Hour:");
                    if ui.add(egui::Slider::new(&mut hour, 0.0..=24.0)
                        .custom_formatter(|v, _| {
                            let h = v as u32;
                            let m = ((v.fract()) * 60.0) as u32;
                            format!("{:02}:{:02}", h % 24, m)
                        })
                    ).changed() {
                        time_state.set_hour(hour);
                    }
                });
            }

            // Show current sun info
            ui.horizontal(|ui| {
                ui.label("Sun:");
                let elev = sun_state.elevation;
                let desc = if elev > 0.0 { "above horizon" }
                    else if elev > -6.0 { "civil twilight" }
                    else if elev > -12.0 { "nautical twilight" }
                    else if elev > -18.0 { "astronomical twilight" }
                    else { "night" };
                ui.label(format!("{:.1}° ({})", elev, desc));
            });
```

### Step 4: Register TimeState in View3DPlugin

In `src/view3d/mod.rs`, inside `View3DPlugin::build` (line 744), add:

```rust
.init_resource::<sky::TimeState>()
```

And add the `sync_time_offset` system:

```rust
.add_systems(Update, sky::sync_time_offset)
```

### Step 5: Build and verify

Run: `cargo build`
Expected: compiles. The 3D panel now shows a "Time of Day" section with a manual override checkbox and hour slider.

### Step 6: Commit

```bash
git add src/view3d/sky.rs src/view3d/mod.rs
git commit -m "Add TimeState resource with manual time slider in 3D panel"
```

---

## Task 3: Enhanced Night Sky (Phase 3)

**Worktree:** `../.worktrees/airjedi-night-sky`

**Files:**
- Modify: `src/view3d/sky.rs:185-251` (star generation + visibility)

### Step 1: Replace generate_star_texture with enhanced version

Replace the `generate_star_texture` function at `sky.rs:185-219` with:

```rust
/// Generate a procedural star texture with magnitude distribution and Milky Way band.
/// - ~3000 stars with realistic brightness distribution (many dim, few bright)
/// - Gaussian Milky Way band of extra-dim stars across a diagonal
/// - 4096x4096 resolution for detail at full screen
fn generate_star_texture(size: u32) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let mut data = vec![0u8; (size * size * 4) as usize];

    // Main star field: ~3000 stars with magnitude-based brightness
    let num_stars = 3000u32;
    for i in 0..num_stars {
        let hash = pseudo_hash(i);
        let x = (hash % size) as usize;
        let y = ((hash / size) % size) as usize;

        // Magnitude distribution: most stars are dim
        // Use a power curve: brightness = base ^ (random factor)
        let mag_hash = pseudo_hash(i + num_stars) % 1000;
        let mag_factor = mag_hash as f32 / 1000.0;
        // Cubic falloff: many dim stars, few bright ones
        let brightness = (40.0 + 215.0 * mag_factor * mag_factor * mag_factor) as u8;

        let idx = (y * size as usize + x) * 4;
        if idx + 3 < data.len() {
            // Slight color variation: hot stars blue-white, cool stars warm
            let color_hash = pseudo_hash(i + num_stars * 2) % 100;
            let (r, g, b) = if color_hash < 15 {
                // Blue-white hot star
                (brightness.saturating_sub(20), brightness.saturating_sub(10), brightness)
            } else if color_hash < 25 {
                // Warm yellow star
                (brightness, brightness.saturating_sub(15), brightness.saturating_sub(40))
            } else {
                // White
                (brightness, brightness, brightness)
            };
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = 255;
        }
    }

    // Milky Way band: a gaussian concentration of very dim stars across a diagonal
    let milky_way_stars = 2000u32;
    for i in 0..milky_way_stars {
        let hash = pseudo_hash(i + num_stars * 3);
        // Position along the band (0..1 across the texture)
        let along = (hash % 10000) as f32 / 10000.0;
        // Perpendicular offset with gaussian-like distribution
        let offset_hash = pseudo_hash(i + num_stars * 4);
        let gauss = ((offset_hash % 100) as f32 / 100.0 - 0.5)
            + ((pseudo_hash(i + num_stars * 5) % 100) as f32 / 100.0 - 0.5);
        let band_width = 0.08; // Narrow band
        let perpendicular = gauss * band_width;

        // Diagonal from bottom-left to top-right
        let x = ((along + perpendicular * 0.7) * size as f32) as usize % size as usize;
        let y = ((along * 0.8 + 0.1 + perpendicular) * size as f32) as usize % size as usize;

        let brightness = 25 + (pseudo_hash(i + num_stars * 6) % 35) as u8; // Very dim

        let idx = (y * size as usize + x) * 4;
        if idx + 3 < data.len() {
            // Additive: don't overwrite brighter stars
            let existing = data[idx];
            if brightness > existing {
                data[idx] = brightness;
                data[idx + 1] = brightness;
                data[idx + 2] = brightness + 5; // Slight blue tint
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
```

### Step 2: Update setup_sky to use 4096 texture size

In `setup_sky` (line 149), change:

```rust
let star_image = generate_star_texture(4096);
```

And update `sync_sky_camera` (lines 134-137) to match:

```rust
let sx = (window.width() * scale_factor * 2.0) / 4096.0;
let sy = (window.height() * scale_factor * 2.0) / 4096.0;
```

### Step 3: Replace binary star visibility with gradual twilight fade

Replace `update_star_visibility` (lines 231-251) with:

```rust
/// Fade star field based on sun elevation with gradual twilight transition.
/// Stars begin appearing at civil twilight (-6°) and reach full brightness
/// at nautical twilight (-12°). Twinkling is applied to brightest stars.
pub fn update_star_visibility(
    state: Res<View3DState>,
    sun_state: Res<SunState>,
    time: Res<Time>,
    mut star_query: Query<(&mut Visibility, &mut Sprite), With<StarField>>,
) {
    let Ok((mut vis, mut sprite)) = star_query.single_mut() else {
        return;
    };

    if !state.is_3d_active() {
        *vis = Visibility::Hidden;
        return;
    }

    let elevation = sun_state.elevation;

    if elevation > 0.0 {
        // Sun above horizon: stars hidden
        *vis = Visibility::Hidden;
    } else if elevation > -6.0 {
        // Civil twilight: stars fade in (0.0 at horizon, ~0.3 at -6°)
        *vis = Visibility::Inherited;
        let alpha = ((0.0 - elevation) / 6.0) * 0.3;
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    } else if elevation > -12.0 {
        // Nautical twilight: stars brightening (0.3 to 0.8)
        *vis = Visibility::Inherited;
        let t = ((elevation + 6.0).abs()) / 6.0;
        let alpha = 0.3 + t * 0.5;
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    } else {
        // Full night: stars at near-full brightness with subtle twinkle
        *vis = Visibility::Inherited;
        // Gentle global twinkle: oscillate alpha between 0.85 and 1.0
        let twinkle = 0.925 + 0.075 * (time.elapsed_secs() * 0.3).sin();
        sprite.color = Color::srgba(1.0, 1.0, 1.0, twinkle);
    }
}
```

### Step 4: Build and verify

Run: `cargo build`
Expected: compiles. At night, stars should show a Milky Way band, color variation, and gradual twilight fading.

### Step 5: Commit

```bash
git add src/view3d/sky.rs
git commit -m "Enhance night sky with 3000 stars, Milky Way band, and twilight fade"
```

---

## Task 4: 2D Mode Time-of-Day Tinting (Phase 5)

**Worktree:** `../.worktrees/airjedi-2d-tinting`

**Files:**
- Modify: `src/view3d/sky.rs` (add tint overlay entity and system)
- Modify: `src/view3d/mod.rs` (register new system)

### Step 1: Add DayNightTint marker and spawn overlay in setup_sky

In `sky.rs`, add after the `GroundPlane` component definition (line 24):

```rust
/// Marker for the 2D mode day/night color overlay sprite.
#[derive(Component)]
pub struct DayNightTint;
```

In `setup_sky`, after spawning the ground plane, add:

```rust
    // Spawn a full-screen tint overlay for 2D mode day/night effect.
    // Positioned between tiles (z=0) and aircraft (z=10) at z=5.
    // Starts fully transparent; updated each frame by update_2d_tint.
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
```

### Step 2: Add update_2d_tint system

Add to `sky.rs`:

```rust
/// Apply subtle time-of-day color tinting in 2D map mode.
/// Golden hour: warm amber. Night: cool blue-black. Midday: transparent.
pub fn update_2d_tint(
    state: Res<View3DState>,
    sun_state: Res<SunState>,
    main_camera: Query<&Transform, (With<crate::MapCamera>, Without<DayNightTint>)>,
    mut tint_query: Query<(&mut Transform, &mut Sprite, &mut Visibility), With<DayNightTint>>,
) {
    let Ok((mut tint_tf, mut sprite, mut vis)) = tint_query.single_mut() else {
        return;
    };

    // Only active in 2D mode
    if state.is_3d_active() {
        *vis = Visibility::Hidden;
        return;
    }

    let elevation = sun_state.elevation;

    // Compute tint color and opacity based on sun elevation
    let (r, g, b, a) = if elevation > 10.0 {
        // Full daylight: no tint
        (0.0, 0.0, 0.0, 0.0)
    } else if elevation > 0.0 {
        // Golden hour (0-10°): warm amber tint, increasing as sun approaches horizon
        let t = 1.0 - (elevation / 10.0);
        (0.9, 0.6, 0.2, t * 0.08)
    } else if elevation > -6.0 {
        // Civil twilight: transition from amber to blue
        let t = (-elevation) / 6.0;
        let r = 0.9 * (1.0 - t) + 0.1 * t;
        let g = 0.6 * (1.0 - t) + 0.1 * t;
        let b = 0.2 * (1.0 - t) + 0.3 * t;
        (r, g, b, 0.08 + t * 0.12)
    } else if elevation > -18.0 {
        // Twilight to night: deepening blue-black
        let t = ((-elevation) - 6.0) / 12.0;
        (0.05, 0.05, 0.15 + 0.1 * (1.0 - t), 0.2 + t * 0.15)
    } else {
        // Full night: dark blue-black overlay
        (0.02, 0.02, 0.08, 0.3)
    };

    if a < 0.001 {
        *vis = Visibility::Hidden;
    } else {
        *vis = Visibility::Inherited;
        sprite.color = Color::srgba(r, g, b, a);
    }

    // Keep tint centered on the map camera
    if let Ok(cam_tf) = main_camera.single() {
        tint_tf.translation.x = cam_tf.translation.x;
        tint_tf.translation.y = cam_tf.translation.y;
    }
}
```

### Step 3: Register update_2d_tint in View3DPlugin

In `src/view3d/mod.rs`, inside `View3DPlugin::build`, add:

```rust
.add_systems(Update, sky::update_2d_tint.after(sky::update_sun_position))
```

### Step 4: Build and verify

Run: `cargo build`
Expected: compiles. In 2D mode at night (or using the time slider from Phase 1 once merged), a subtle blue-black overlay appears. At golden hour, a warm amber tint.

### Step 5: Commit

```bash
git add src/view3d/sky.rs src/view3d/mod.rs
git commit -m "Add subtle day/night tinting to 2D map mode"
```

---

## Task 5: Atmosphere and Fog Tuning (Phase 2)

**Worktree:** `../.worktrees/airjedi-atmosphere-tuning`
**Depends on:** Phase 1 merged to main. Create branch from main after merge.

**Files:**
- Modify: `src/view3d/sky.rs:297-365` (atmosphere setup)
- Modify: `src/view3d/sky.rs:401-452` (fog parameters)
- Modify: `src/main.rs:350-359` (DirectionalLight setup)

### Step 1: Switch fog to from_visibility_colors

Replace the `DistanceFog` setup in `manage_atmosphere_camera` (sky.rs, inside the `if state.is_3d_active()` block) — replace the DistanceFog insertion with:

```rust
DistanceFog {
    color: Color::srgba(0.55, 0.62, 0.72, 1.0),
    directional_light_color: Color::srgba(1.0, 0.9, 0.7, 0.3),
    directional_light_exponent: 20.0,
    falloff: FogFalloff::from_visibility_colors(
        state.visibility_range,
        // Extinction color: what the atmosphere absorbs (warm reddish at distance)
        Color::srgb(0.35, 0.5, 0.66),
        // Inscattering color: what scattered sunlight adds (blue-white haze)
        Color::srgb(0.55, 0.62, 0.72),
    ),
},
```

### Step 2: Add AtmosphereEnvironmentMapLight to camera

In `manage_atmosphere_camera`, after inserting `AtmosphereSettings`, also insert:

```rust
bevy::pbr::AtmosphereEnvironmentMapLight::default(),
```

And in the `else` (2D mode) removal block, also remove it:

```rust
.remove::<bevy::pbr::AtmosphereEnvironmentMapLight>()
```

### Step 3: Improve fog color transitions in update_fog_parameters

Replace `update_fog_parameters` entirely with a version that uses visibility_colors and has better twilight zones:

```rust
/// Update fog color, density, and directional light based on sun position.
/// Uses civil (-6°), nautical (-12°), and astronomical (-18°) twilight zones.
pub fn update_fog_parameters(
    state: Res<View3DState>,
    sun_state: Res<SunState>,
    mut fog_query: Query<&mut DistanceFog, With<Camera3d>>,
) {
    let Ok(mut fog) = fog_query.single_mut() else {
        return;
    };

    let elevation = sun_state.elevation;

    // Extinction and inscattering colors shift with sun elevation
    let (extinction, inscattering) = if elevation > 30.0 {
        // High sun: clear blue sky scattering
        (Color::srgb(0.35, 0.5, 0.66), Color::srgb(0.55, 0.62, 0.72))
    } else if elevation > 5.0 {
        // Approaching golden hour: warming
        let t = (elevation - 5.0) / 25.0;
        (
            Color::srgb(0.4 - 0.1 * t, 0.4 + 0.1 * t, 0.5 + 0.16 * t),
            Color::srgb(0.6 - 0.1 * t, 0.55 + 0.07 * t, 0.5 + 0.22 * t),
        )
    } else if elevation > 0.0 {
        // Golden hour: warm amber scattering
        let t = elevation / 5.0;
        (
            Color::srgb(0.5 - 0.1 * t, 0.3 + 0.1 * t, 0.2 + 0.3 * t),
            Color::srgb(0.7 - 0.1 * t, 0.45 + 0.1 * t, 0.25 + 0.25 * t),
        )
    } else if elevation > -6.0 {
        // Civil twilight: deep orange fading to purple
        let t = (-elevation) / 6.0;
        (
            Color::srgb(0.3 * (1.0 - t), 0.15 * (1.0 - t), 0.2),
            Color::srgb(0.5 * (1.0 - t) + 0.1 * t, 0.3 * (1.0 - t), 0.3 * (1.0 - t) + 0.15 * t),
        )
    } else {
        // Night: near-black with slight blue
        (Color::srgb(0.02, 0.02, 0.04), Color::srgb(0.03, 0.03, 0.06))
    };

    fog.falloff = FogFalloff::from_visibility_colors(
        state.visibility_range,
        extinction,
        inscattering,
    );

    // Sun glow through fog
    if elevation > -2.0 {
        let glow_t = ((elevation + 2.0) / 32.0).clamp(0.0, 1.0);
        // Warm golden glow near horizon, white at high elevation
        let warmth = (1.0 - (elevation / 30.0).clamp(0.0, 1.0));
        fog.directional_light_color = Color::srgba(
            1.0,
            0.85 + 0.15 * (1.0 - warmth),
            0.6 + 0.4 * (1.0 - warmth),
            glow_t * 0.5,
        );
        fog.directional_light_exponent = 15.0 + 15.0 * warmth; // Tighter glow at horizon
    } else {
        fog.directional_light_color = Color::srgba(0.0, 0.0, 0.0, 0.0);
    }
}
```

### Step 4: Build and verify

Run: `cargo build`
Expected: compiles. In 3D mode, fog should look more realistic with atmospheric extinction/inscattering shifting through the day cycle.

### Step 5: Commit

```bash
git add src/view3d/sky.rs src/main.rs
git commit -m "Tune atmosphere fog with visibility colors and twilight zones"
```

---

## Task 6: Moonlight (Phase 4)

**Worktree:** `../.worktrees/airjedi-moonlight`
**Depends on:** Phase 1 merged to main. Create branch from main after merge.

**Files:**
- Modify: `src/view3d/sky.rs` (add MoonState, moon position, moonlight system)
- Modify: `src/view3d/mod.rs` (register MoonState, add moonlight system, spawn MoonLight entity)
- Modify: `src/main.rs` (spawn MoonLight DirectionalLight)

### Step 1: Add moon position calculation and MoonState resource

Add to `sky.rs`:

```rust
/// Marker for the directional light used as moonlight.
#[derive(Component)]
pub struct MoonLight;

/// Resource tracking current moon position and phase.
#[derive(Resource)]
pub struct MoonState {
    /// Moon elevation in degrees (-90 to 90)
    pub elevation: f32,
    /// Moon azimuth in degrees (0 = north)
    pub azimuth: f32,
    /// Lunar phase (0.0 = new moon, 0.5 = full moon, 1.0 = new moon again)
    pub phase: f32,
}

impl Default for MoonState {
    fn default() -> Self {
        Self {
            elevation: -10.0,
            azimuth: 0.0,
            phase: 0.5,
        }
    }
}

/// Simplified moon position using J2000.0 epoch.
/// Accuracy: ~2-5 degrees (sufficient for lighting purposes).
fn compute_moon_position(
    latitude: f64,
    longitude: f64,
    datetime: &chrono::DateTime<chrono::FixedOffset>,
) -> (f32, f32, f32) {
    let timestamp = datetime.timestamp() as f64;
    let j2000_epoch = 946728000.0_f64;
    let days = (timestamp - j2000_epoch) / 86400.0;

    // Moon's mean elements (simplified)
    let l = (218.316 + 13.176396 * days) % 360.0; // Mean longitude
    let m = (134.963 + 13.064993 * days) % 360.0; // Mean anomaly
    let f = (93.272 + 13.229350 * days) % 360.0;  // Argument of latitude

    let m_rad = m.to_radians();
    let f_rad = f.to_radians();

    // Ecliptic longitude and latitude (simplified)
    let ecl_lon = (l + 6.289 * m_rad.sin()).to_radians();
    let ecl_lat = (5.128 * f_rad.sin()).to_radians();

    // Obliquity of ecliptic
    let obliquity = 23.439_f64.to_radians();

    // Equatorial coordinates
    let sin_ra = ecl_lon.sin() * obliquity.cos() - ecl_lat.tan() * obliquity.sin();
    let cos_ra = ecl_lon.cos();
    let declination = (ecl_lat.cos() * obliquity.sin() * ecl_lon.sin()
        + ecl_lat.sin() * obliquity.cos()).asin();

    // Hour angle
    let utc_hours = (timestamp % 86400.0) / 3600.0;
    let gmst = (280.46061837 + 360.98564736629 * days) % 360.0;
    let ra = sin_ra.atan2(cos_ra).to_degrees();
    let local_sidereal = (gmst + longitude) % 360.0;
    let hour_angle = (local_sidereal - ra).to_radians();

    let lat_rad = latitude.to_radians();

    // Altitude (elevation)
    let sin_alt = lat_rad.sin() * declination.sin()
        + lat_rad.cos() * declination.cos() * hour_angle.cos();
    let elevation = sin_alt.asin();

    // Azimuth
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
```

### Step 2: Add update_moon_position system

Add to `sky.rs`:

```rust
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
    moon_state.elevation = elevation;
    moon_state.azimuth = azimuth;
    moon_state.phase = phase;

    let Ok((mut light, mut transform)) = moon_query.single_mut() else {
        return;
    };

    // Convert moon position to directional light rotation
    let elev_rad = elevation.to_radians();
    let azim_rad = azimuth.to_radians();
    *transform = Transform::from_rotation(
        Quat::from_euler(EulerRot::YXZ, -azim_rad, -elev_rad, 0.0),
    );

    // Moonlight illuminance: full moon ~0.25 lux, scaled by phase
    // Phase 0.0 = new (0 lux), 0.5 = full (0.25 lux)
    let phase_illuminance = (std::f32::consts::PI * phase).sin(); // 0 at new/full edges, 1 at full
    if elevation > 0.0 {
        let elev_factor = (elevation / 90.0).clamp(0.0, 1.0).sqrt();
        light.illuminance = 0.25 * phase_illuminance * elev_factor;
    } else {
        light.illuminance = 0.0;
    }
}
```

### Step 3: Spawn MoonLight entity in main.rs

In `src/main.rs`, after the DirectionalLight spawn for the sun (line 350-359), add:

```rust
    // Moonlight: secondary directional light with cool blue-white color
    commands.spawn((
        DirectionalLight {
            illuminance: 0.0,
            shadows_enabled: false,
            color: Color::srgb(0.7, 0.75, 0.9), // Cool blue-white
            ..default()
        },
        view3d::sky::MoonLight,
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 2.0, 0.0)),
    ));
```

### Step 4: Register MoonState and system in View3DPlugin

In `src/view3d/mod.rs`, inside `View3DPlugin::build`, add:

```rust
.init_resource::<sky::MoonState>()
.add_systems(Update, sky::update_moon_position.after(sky::sync_time_offset))
```

### Step 5: Build and verify

Run: `cargo build`
Expected: compiles. At night, a subtle cool blue-white directional light illuminates the scene from the moon's position. Brightness varies with lunar phase.

### Step 6: Commit

```bash
git add src/view3d/sky.rs src/view3d/mod.rs src/main.rs
git commit -m "Add moonlight with lunar phase and position calculation"
```

---

## Task 7: Merge Phases

After all phases are complete and verified:

### Step 1: Merge independent phases (1, 3, 5)

```bash
cd /Users/ccustine/development/aviation/airjedi-bevy
git merge feat/solar-accuracy
git merge feat/night-sky
git merge feat/2d-tinting
```

Resolve any conflicts in `sky.rs` (likely in imports and system registration). The changes touch largely different functions so conflicts should be minimal.

### Step 2: Create dependent phase worktrees and implement

```bash
git worktree add ../.worktrees/airjedi-atmosphere-tuning -b feat/atmosphere-tuning
git worktree add ../.worktrees/airjedi-moonlight -b feat/moonlight
```

Implement Tasks 5 and 6 in their respective worktrees.

### Step 3: Merge dependent phases (2, 4)

```bash
git merge feat/atmosphere-tuning
git merge feat/moonlight
```

### Step 4: Clean up worktrees

```bash
git worktree remove ../.worktrees/airjedi-solar-accuracy
git worktree remove ../.worktrees/airjedi-night-sky
git worktree remove ../.worktrees/airjedi-2d-tinting
git worktree remove ../.worktrees/airjedi-atmosphere-tuning
git worktree remove ../.worktrees/airjedi-moonlight
git branch -d feat/solar-accuracy feat/night-sky feat/2d-tinting feat/atmosphere-tuning feat/moonlight
```

---

## Team Assignment for Parallel Execution

When using teams/teammates, assign as follows:

| Teammate | Tasks | Worktree |
|---|---|---|
| solar-worker | Task 1, Task 2 | `airjedi-solar-accuracy` |
| stars-worker | Task 3 | `airjedi-night-sky` |
| tint-worker | Task 4 | `airjedi-2d-tinting` |

After Phase 1 merges:

| Teammate | Tasks | Worktree |
|---|---|---|
| atmosphere-worker | Task 5 | `airjedi-atmosphere-tuning` |
| moon-worker | Task 6 | `airjedi-moonlight` |

The team lead handles Task 7 (merging).
