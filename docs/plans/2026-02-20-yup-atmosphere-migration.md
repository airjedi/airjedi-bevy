# Y-up Atmosphere Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate the 3D view from Z-up to Y-up coordinates so Bevy's native Atmosphere component works, replacing the hand-rolled sky system.

**Architecture:** Camera3d becomes the primary 3D camera in Y-up space with Atmosphere. Camera2d derives its transform via a fixed 90-degree rotation so tiles on the XY plane render from the equivalent viewpoint. All 3D-rendered entities (aircraft meshes, ground plane) are positioned in Y-up space. Data model (saved_2d_center, MapState) stays Z-up.

**Tech Stack:** Bevy 0.18, bevy::pbr::Atmosphere / AtmosphereSettings / AtmosphereEnvironmentMapLight / ScatteringMedium

**Design doc:** `docs/plans/2026-02-20-yup-atmosphere-migration-design.md`

---

### Task 1: Add coordinate conversion utilities

**Files:**
- Modify: `src/view3d/mod.rs` (add after line 8, before `use` block)

**Step 1: Add the conversion functions and constant**

Add a `coord` submodule inline or as utility functions in mod.rs. Place after the `pub mod sky;` line:

```rust
use std::f32::consts::FRAC_PI_2;

/// Rotation that converts Z-up coordinates to Y-up (Bevy-native) coordinates.
/// Applies: X stays, Y(north) -> -Z(forward), Z(up) -> Y(up).
pub(crate) const ZUP_TO_YUP_ROTATION: Quat = Quat::from_xyzw(
    -FRAC_PI_2.sin() * 0.5_f32.sqrt(),  // This won't work as const — see step below
    0.0,
    0.0,
    FRAC_PI_2.cos() * 0.5_f32.sqrt(),
);
```

Actually, `Quat::from_rotation_x` is not const in Bevy. Use helper functions instead:

```rust
/// Convert a position from Z-up (X=east, Y=north, Z=up) to
/// Y-up (X=east, Y=up, Z=south) coordinate space.
pub(crate) fn zup_to_yup(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y)
}

/// Convert a position from Y-up back to Z-up coordinate space.
pub(crate) fn yup_to_zup(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y)
}

/// Build the rotation quaternion that transforms Z-up to Y-up.
/// This is a -90 degree rotation around the X axis.
pub(crate) fn zup_to_yup_rotation() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}
```

**Step 2: Verify build**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles with existing warnings, no new errors

**Step 3: Commit**

```bash
git add src/view3d/mod.rs
git commit -m "Add Z-up to Y-up coordinate conversion utilities"
```

---

### Task 2: Add ScatteringMedium resource

**Files:**
- Modify: `src/main.rs:1` (add `pbr::ScatteringMedium` to use statement)
- Modify: `src/main.rs` (add `AtmosphereMediumHandle` resource struct near other resources around line 390)
- Modify: `src/main.rs:setup_map` (add parameter and medium creation)

**Step 1: Add the import**

In `src/main.rs` line 1, add `pbr::ScatteringMedium` to the bevy use:

```rust
use bevy::{prelude::*, camera::visibility::RenderLayers, gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore}, light::SunDisk, pbr::ScatteringMedium};
```

**Step 2: Add the resource struct**

After the `DragState` struct (around line 396), add:

```rust
/// Holds the shared Handle<ScatteringMedium> for Atmosphere components.
#[derive(Resource)]
pub struct AtmosphereMediumHandle(pub Handle<ScatteringMedium>);
```

**Step 3: Create the medium in setup_map**

Add `mut scattering_mediums: ResMut<Assets<ScatteringMedium>>` parameter to `setup_map`.

After the moonlight spawn (after the `SunDisk::EARTH` / moonlight section), add:

```rust
let medium = scattering_mediums.add(ScatteringMedium::earthlike(256, 256));
commands.insert_resource(AtmosphereMediumHandle(medium));
```

**Step 4: Verify build**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles (AtmosphereMediumHandle unused warning is fine)

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "Add ScatteringMedium resource for atmosphere rendering"
```

---

### Task 3: Migrate camera system to Y-up

This is the core change. Camera3d becomes primary in 3D mode, Camera2d derives from it. All three pieces must change together for the build to succeed.

**Files:**
- Modify: `src/view3d/mod.rs:107-124` (`calculate_camera_transform`)
- Modify: `src/view3d/mod.rs:420-506` (`update_3d_camera`)
- Modify: `src/camera.rs:148-163` (`sync_aircraft_camera`)

**Step 1: Change `calculate_camera_transform` to Y-up**

Replace `src/view3d/mod.rs:106-124`:

```rust
/// Calculate the 3D camera transform in Y-up space.
/// The orbit center is provided in Y-up coordinates.
fn calculate_camera_transform_yup(&self, center: Vec3) -> Transform {
    let pitch_rad = self.camera_pitch.to_radians();
    let yaw_rad = self.camera_yaw.to_radians();

    let effective_distance = self.altitude_to_distance();
    let horizontal_dist = effective_distance * pitch_rad.cos();
    let vertical_dist = effective_distance * pitch_rad.sin();

    // Y is "up" (altitude), orbit in XZ plane.
    // At yaw=0, camera is south of center (+Z direction in Y-up)
    // looking north (-Z), so north stays up on screen.
    let camera_pos = Vec3::new(
        center.x - horizontal_dist * yaw_rad.sin(),
        center.y + vertical_dist,
        center.z + horizontal_dist * yaw_rad.cos(),
    );

    Transform::from_translation(camera_pos).looking_at(center, Vec3::Y)
}
```

Note: `center.z + horizontal_dist * yaw_rad.cos()` — at yaw=0, camera offset is +Z (south in Y-up, since north=-Z). This matches the original behavior where at yaw=0 the camera was at -Y (south in Z-up).

**Step 2: Rewrite `update_3d_camera` to drive both cameras**

Replace `src/view3d/mod.rs:420-506`. The new version queries both Camera2d and Camera3d. In 3D mode, it computes Camera3d's transform in Y-up and derives Camera2d via rotation:

```rust
/// System to update cameras for 3D perspective view.
/// Camera3d is primary in Y-up space; Camera2d derives via rotation for tile rendering.
pub fn update_3d_camera(
    mut state: ResMut<View3DState>,
    mut camera_2d: Query<
        (&mut Transform, &mut Projection),
        (With<crate::MapCamera>, Without<crate::AircraftCamera>),
    >,
    mut camera_3d: Query<
        (&mut Transform, &mut Projection),
        (With<crate::AircraftCamera>, Without<crate::MapCamera>),
    >,
    window_query: Query<&Window>,
    zoom_state: Res<crate::ZoomState>,
) {
    if matches!(state.mode, ViewMode::Map2D) && !state.is_transitioning() {
        return;
    }

    let Ok((mut tf_2d, mut proj_2d)) = camera_2d.single_mut() else {
        return;
    };
    let Ok((mut tf_3d, mut proj_3d)) = camera_3d.single_mut() else {
        return;
    };

    let t = match state.transition {
        TransitionState::Idle => match state.mode {
            ViewMode::Map2D => 0.0,
            ViewMode::Perspective3D => 1.0,
        },
        TransitionState::TransitioningTo3D { progress } => smooth_step(progress),
        TransitionState::TransitioningTo2D { progress } => smooth_step(1.0 - progress),
    };

    // Y-up orbit center: convert saved_2d_center from Z-up pixel space
    let ground_alt = state.altitude_to_z(state.ground_elevation_ft);
    let center_yup = zup_to_yup(Vec3::new(
        state.saved_2d_center.x,
        state.saved_2d_center.y,
        ground_alt,
    ));
    let orbit_yup = state.calculate_camera_transform_yup(center_yup);

    // Matching height: perspective altitude that shows the same area as orthographic
    let base_fov = 60.0_f32.to_radians();
    let matching_height = if let Ok(window) = window_query.single() {
        window.height() / (2.0 * zoom_state.camera_zoom * (base_fov / 2.0).tan())
    } else {
        orbit_yup.translation.y * 0.5
    };

    if t < 0.001 {
        // Pure 2D — restore orthographic, flat position, identity rotation
        let pos_2d = Vec3::new(state.saved_2d_center.x, state.saved_2d_center.y, 0.0);
        *proj_2d = Projection::Orthographic(OrthographicProjection::default_2d());
        tf_2d.translation = pos_2d;
        tf_2d.rotation = Quat::IDENTITY;

        // Camera3d mirrors Camera2d in 2D mode
        *tf_3d = *tf_2d;
        *proj_3d = proj_2d.clone();

        if matches!(state.transition, TransitionState::TransitioningTo2D { .. }) {
            state.mode = ViewMode::Map2D;
            state.transition = TransitionState::Idle;
            info!("Transition to 2D complete");
        }
        return;
    }

    let perspective = PerspectiveProjection {
        fov: base_fov,
        far: 100_000.0,
        ..default()
    };

    if t > 0.999 {
        // Pure 3D — Camera3d at Y-up orbit, Camera2d derived via rotation
        *tf_3d = orbit_yup;
        *proj_3d = Projection::Perspective(perspective.clone());

        // Derive Camera2d: rotate Y-up transform to Z-up for tile rendering
        let rotation = zup_to_yup_rotation().inverse(); // Y-up -> Z-up
        tf_2d.translation = yup_to_zup(tf_3d.translation);
        tf_2d.rotation = rotation * tf_3d.rotation;
        *proj_2d = Projection::Perspective(perspective);
    } else {
        // Transition: interpolate Camera3d in Y-up, derive Camera2d
        let overhead_yup = Vec3::new(
            center_yup.x,
            center_yup.y + matching_height,
            center_yup.z,
        );

        tf_3d.translation = overhead_yup.lerp(orbit_yup.translation, t);
        tf_3d.rotation = Quat::IDENTITY
            .slerp(orbit_yup.rotation, t);
        *proj_3d = Projection::Perspective(perspective.clone());

        // Derive Camera2d from Camera3d
        let rotation = zup_to_yup_rotation().inverse();
        tf_2d.translation = yup_to_zup(tf_3d.translation);
        tf_2d.rotation = rotation * tf_3d.rotation;
        *proj_2d = Projection::Perspective(perspective);
    }
}
```

**Step 3: Update `sync_aircraft_camera` to skip in 3D mode**

Replace `src/camera.rs:148-163`:

```rust
/// Sync Camera3d transform and projection to match Camera2d in 2D mode.
/// In 3D mode, update_3d_camera handles both cameras directly.
fn sync_aircraft_camera(
    view3d_state: Res<view3d::View3DState>,
    camera_2d: Query<(&Transform, &Projection), (With<MapCamera>, Without<AircraftCamera>)>,
    mut camera_3d: Query<(&mut Transform, &mut Projection), (With<AircraftCamera>, Without<Camera2d>)>,
) {
    // In 3D mode or during transitions, update_3d_camera owns both cameras
    if view3d_state.is_3d_active() || view3d_state.is_transitioning() {
        return;
    }

    let (Ok((t2, p2)), Ok((mut t3, mut p3))) = (camera_2d.single(), camera_3d.single_mut()) else {
        return;
    };
    *t3 = *t2;
    *p3 = p2.clone();
}
```

**Step 4: Verify build**

Run: `cargo build 2>&1 | tail -10`
Expected: compiles. The old `calculate_camera_transform` name is no longer called anywhere.

**Step 5: Visual verification**

Run: `cargo run`

- 2D mode should look identical to before
- Press '3' to enter 3D mode — tiles should still render in perspective from the correct viewpoint
- Pan, zoom, orbit should all work
- Camera3d may show a blank sky (no atmosphere yet) — this is expected

**Step 6: Commit**

```bash
git add src/view3d/mod.rs src/camera.rs
git commit -m "Migrate 3D camera system to Y-up with Camera3d as primary"
```

---

### Task 4: Migrate aircraft positioning to Y-up

**Files:**
- Modify: `src/view3d/mod.rs:696-725` (rename and rewrite `update_aircraft_altitude_z`)
- Modify: `src/view3d/mod.rs:846` (update system registration name)
- Modify: `src/camera.rs:196-222` (add Y-up base rotation constant, condition rotation)

**Step 1: Add Y-up base rotation constant to camera.rs**

Near the top of the aircraft rendering section in `src/camera.rs`, add:

```rust
/// Base rotation for aircraft GLB models in Y-up 3D space.
/// GLB model: nose=+Z, top=+Y, right-wing=+X.
/// Y-up world: north=-Z, up=+Y.
/// Rotate 180 deg around Y so nose points -Z (north).
/// Then heading rotation is applied around Y axis.
const BASE_ROT_YUP: Quat = Quat::from_xyzw(0.0, 1.0, 0.0, 0.0); // 180 deg around Y
```

Note: `Quat::from_xyzw(0.0, 1.0, 0.0, 0.0)` is a 180-degree rotation around Y. Verify this produces correct visual orientation at runtime; if the model appears mirrored, use `Quat::from_xyzw(0.0, -1.0, 0.0, 0.0)` or adjust empirically.

**Step 2: Rename and rewrite `update_aircraft_altitude_z`**

Replace `src/view3d/mod.rs:696-725`:

```rust
/// Remap aircraft transforms to Y-up space in 3D mode.
/// In 2D mode, aircraft Z is the fixed layer constant.
/// In 3D mode, positions are converted from Z-up pixel space (set by
/// update_aircraft_positions) to Y-up for Camera3d rendering.
pub fn update_aircraft_3d_transform(
    state: Res<View3DState>,
    mut aircraft_query: Query<(&crate::Aircraft, &mut Transform), Without<crate::AircraftLabel>>,
    mut label_query: Query<(&crate::AircraftLabel, &mut Visibility)>,
) {
    if state.is_3d_active() {
        for (aircraft, mut transform) in aircraft_query.iter_mut() {
            // Read pixel positions set by update_aircraft_positions (Z-up)
            let px = transform.translation.x;
            let py = transform.translation.y;
            let alt = aircraft.altitude.unwrap_or(0);
            let alt_y = state.altitude_to_z(alt); // same scale, now used as Y

            // Remap to Y-up: (px, py, alt_z) -> (px, alt_y, -py)
            transform.translation = Vec3::new(px, alt_y, -py);

            // Heading rotation around Y axis for Y-up space
            let base_rot = crate::camera::BASE_ROT_YUP;
            if let Some(heading) = aircraft.heading {
                transform.rotation =
                    Quat::from_rotation_y((-heading).to_radians()) * base_rot;
            } else {
                transform.rotation = base_rot;
            }
        }
        // Hide text labels in 3D mode (they don't position well in perspective)
        for (_label, mut vis) in label_query.iter_mut() {
            *vis = Visibility::Hidden;
        }
    } else if !state.is_transitioning() {
        for (_aircraft, mut transform) in aircraft_query.iter_mut() {
            transform.translation.z = crate::constants::AIRCRAFT_Z_LAYER;
        }
        for (_label, mut vis) in label_query.iter_mut() {
            if *vis == Visibility::Hidden {
                *vis = Visibility::Inherited;
            }
        }
    }
}
```

Make `BASE_ROT_YUP` `pub(crate)` in camera.rs so view3d can access it.

**Step 3: Update system registration**

In `src/view3d/mod.rs` View3DPlugin::build, change line 846:

```rust
// Old:
.add_systems(Update, update_aircraft_altitude_z)
// New:
.add_systems(Update, update_aircraft_3d_transform)
```

**Step 4: Stop Z-up rotation in update_aircraft_positions during 3D mode**

In `src/camera.rs:196-222`, the rotation is always set in Z-up convention. Since `update_aircraft_3d_transform` overwrites rotation in 3D mode (runs after), no change is strictly needed. But for clarity, you may leave it as-is — the 3D transform system overwrites both position and rotation.

**Step 5: Verify build**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles

**Step 6: Visual verification**

Run: `cargo run`

- Press '3' — aircraft should appear at correct positions above the ground plane
- Aircraft heading should point the correct direction
- If models appear mirrored or upside down, adjust `BASE_ROT_YUP` empirically

**Step 7: Commit**

```bash
git add src/view3d/mod.rs src/camera.rs
git commit -m "Remap aircraft transforms to Y-up in 3D mode"
```

---

### Task 5: Migrate ground plane to Y-up

**Files:**
- Modify: `src/view3d/sky.rs:281` (ground plane mesh normal)
- Modify: `src/view3d/sky.rs:672-689` (`sync_ground_plane`)

**Step 1: Change ground plane mesh normal**

In `src/view3d/sky.rs:281`, change:

```rust
// Old:
let ground_mesh = meshes.add(Plane3d::new(Vec3::Z, Vec2::splat(250_000.0)));
// New:
let ground_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(250_000.0)));
```

**Step 2: Update sync_ground_plane to Y-up positioning**

Replace `src/view3d/sky.rs:672-689`:

```rust
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
        let ground_alt = state.altitude_to_z(state.ground_elevation_ft);
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
```

**Step 3: Verify build**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles

**Step 4: Commit**

```bash
git add src/view3d/sky.rs
git commit -m "Migrate ground plane to Y-up coordinate space"
```

---

### Task 6: Replace sky system with native Atmosphere

This is the payoff. Remove the hand-rolled sky and replace with Bevy's Atmosphere.

**Files:**
- Modify: `src/view3d/sky.rs:10-11` (imports)
- Modify: `src/view3d/sky.rs:559-617` (rewrite `manage_atmosphere_camera`)
- Delete: `src/view3d/sky.rs:619-670` (`compute_sky_color`)
- Delete: `src/view3d/sky.rs:691-758` (`update_fog_parameters`)
- Modify: `src/view3d/sky.rs:496-503` (remove ambient brightness hack in `update_sun_position`)
- Modify: `src/view3d/mod.rs` View3DPlugin (remove `update_fog_parameters` registration)

**Step 1: Update imports in sky.rs**

Replace `src/view3d/sky.rs:10-11`:

```rust
// Old:
use bevy::pbr::{DistanceFog, FogFalloff, StandardMaterial};
// New:
use bevy::pbr::{Atmosphere, AtmosphereSettings, AtmosphereEnvironmentMapLight, StandardMaterial};
```

**Step 2: Rewrite manage_atmosphere_camera**

Replace `src/view3d/sky.rs:551-617`:

```rust
/// Manage Atmosphere component on Camera3d based on 3D mode state.
/// In 3D mode, Camera3d renders first (order=0) with atmosphere painting the sky.
/// Camera2d renders on top (order=1) with tiles composited over.
pub fn manage_atmosphere_camera(
    mut commands: Commands,
    state: Res<View3DState>,
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
        if has_atmo.is_none() {
            let scene_units_to_m = 1000.0 / (super::PIXEL_SCALE * state.altitude_scale);
            if let Some(ref medium_handle) = medium_handle {
                let mut atmo = Atmosphere::earthlike(medium_handle.0.clone());
                atmo.ground_albedo = Vec3::new(0.05, 0.05, 0.08);
                commands.entity(cam3d_entity).insert((
                    atmo,
                    AtmosphereSettings {
                        scene_units_to_m,
                        ..default()
                    },
                    AtmosphereEnvironmentMapLight::default(),
                ));
            }
        }
        // Camera3d renders first (order=0), atmosphere paints sky
        cam3d.order = 0;
        cam3d.clear_color = ClearColorConfig::Default;
        // Camera2d renders on top (order=1), tiles composite over atmosphere
        cam2d.order = 1;
        cam2d.clear_color = ClearColorConfig::Custom(Color::NONE);
        // Show ground plane
        if let Ok((_, mut gp_vis)) = ground_query.single_mut() {
            *gp_vis = Visibility::Inherited;
        }
    } else {
        if has_atmo.is_some() {
            commands.entity(cam3d_entity)
                .remove::<Atmosphere>()
                .remove::<AtmosphereSettings>()
                .remove::<AtmosphereEnvironmentMapLight>();
        }
        cam2d.order = 0;
        cam2d.clear_color = ClearColorConfig::Default;
        cam3d.order = 1;
        cam3d.clear_color = ClearColorConfig::Custom(Color::NONE);
        if let Ok((_, mut gp_vis)) = ground_query.single_mut() {
            *gp_vis = Visibility::Hidden;
        }
    }
}
```

**Step 3: Delete `compute_sky_color`**

Delete the entire `compute_sky_color` function (lines 619-670 in the current file). It is no longer called.

**Step 4: Delete `update_fog_parameters`**

Delete the entire `update_fog_parameters` function (lines 691-758 in the current file). It is no longer called.

**Step 5: Simplify ambient light in `update_sun_position`**

In `src/view3d/sky.rs`, the `update_sun_position` function has a conditional that reduces ambient brightness in 3D mode (lines 498-503). With AtmosphereEnvironmentMapLight handling ambient, remove the 3D-specific branch:

```rust
// Old:
if state.is_3d_active() {
    ambient.brightness = 80.0 * ambient_factor;
} else {
    ambient.brightness = 300.0 * ambient_factor;
}

// New:
ambient.brightness = 300.0 * ambient_factor;
```

Also remove the `state: Res<View3DState>` parameter from `update_sun_position` since it's no longer needed.

**Step 6: Update View3DPlugin system registrations**

In `src/view3d/mod.rs` View3DPlugin::build, remove the `update_fog_parameters` registration:

```rust
// Delete this line:
.add_systems(Update, sky::update_fog_parameters.after(sky::update_sun_position))
```

**Step 7: Add atmosphere scale update system**

Add to sky.rs (after manage_atmosphere_camera):

```rust
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
    settings.scene_units_to_m = 1000.0 / (super::PIXEL_SCALE * state.altitude_scale);
}
```

Register in View3DPlugin:

```rust
.add_systems(Update, sky::update_atmosphere_scale)
```

**Step 8: Verify build**

Run: `cargo build 2>&1 | tail -10`
Expected: compiles. Check for unused import warnings on DistanceFog/FogFalloff (should be gone since we removed them).

**Step 9: Visual verification**

Run: `cargo run`

- 2D mode: should look identical
- Press '3': should see physically-based atmosphere sky — blue during day, sunset colors at dusk, dark at night
- The atmosphere scattering should respond to sun position (pan east/west to see different times of day)
- Stars should still appear at night (star field sprite on Camera2d)

**Step 10: Commit**

```bash
git add src/view3d/sky.rs src/view3d/mod.rs
git commit -m "Replace hand-rolled sky with native Bevy Atmosphere component"
```

---

### Task 7: Cleanup and final verification

**Files:**
- Modify: `src/view3d/sky.rs` (remove any dead imports)
- Modify: `src/view3d/mod.rs` (remove old `calculate_camera_transform` if still present)

**Step 1: Remove dead code**

Check for and remove:
- Any remaining references to `DistanceFog` or `FogFalloff`
- The old `calculate_camera_transform` method (replaced by `calculate_camera_transform_yup`)
- Unused `visibility_range` field in `View3DState` if DistanceFog was the only consumer (check all references first)
- The comment on lines 556-558 about Y-up being the reason Atmosphere isn't used

**Step 2: Verify build with no warnings from our code**

Run: `cargo build 2>&1 | grep "view3d\|camera\|sky" | grep -i "warning\|error"`
Expected: no warnings from our changed files

**Step 3: Full visual verification checklist**

Run: `cargo run`

Test each scenario:
- [ ] 2D mode looks identical to before (pan, zoom, tiles, aircraft icons)
- [ ] 2D day/night tint overlay works
- [ ] Press '3' — smooth transition into 3D perspective
- [ ] Atmosphere sky renders (blue overhead, haze at horizon)
- [ ] Sun position matches time of day (pan east/west for different solar angles)
- [ ] Sunset/sunrise colors appear at appropriate sun elevations
- [ ] Stars visible at night, hidden during day
- [ ] Aircraft models visible at correct positions and altitudes
- [ ] Aircraft heading orientation is correct (nose points along track)
- [ ] Ground plane visible beneath tiles
- [ ] Pan (click-drag) works in 3D mode
- [ ] Orbit (shift-drag) works
- [ ] Scroll zoom works
- [ ] Shift+scroll pitch works
- [ ] Press '3' again — smooth transition back to 2D
- [ ] 2D mode fully restored after returning from 3D
- [ ] Altitude scale slider changes atmosphere perspective

**Step 4: Commit**

```bash
git add -A
git commit -m "Clean up dead code from sky system replacement"
```

---

### Notes for implementer

- **Aircraft rotation empirical tuning:** `BASE_ROT_YUP` may need adjustment. If the model appears upside down or mirrored, try different 180-degree axis rotations. The goal: nose points north (-Z), top faces up (+Y), right wing faces east (+X).

- **Transition smoothness:** The slerp at t=0 starts from `Quat::IDENTITY` which is a straight-down view in Y-up. If the transition looks wrong, the overhead rotation may need to be `Quat::from_rotation_x(-FRAC_PI_2)` (looking down from +Y toward origin) rather than IDENTITY.

- **Camera2d derivation math:** `tf_2d.rotation = rotation * tf_3d.rotation` applies the Y-up-to-Z-up rotation to Camera3d's rotation to get Camera2d's. If tiles appear at wrong angles, verify the rotation multiplication order — it may need to be `tf_3d.rotation * rotation` depending on Bevy's convention (left-multiply = rotate in world space, right-multiply = rotate in local space).

- **fade_distant_sprites:** This system still runs and reads camera distance. It should still work since Camera2d (MapCamera) position is derived correctly in Z-up space. If sprites don't fade correctly, check that the distance calculation uses the correct camera position.
