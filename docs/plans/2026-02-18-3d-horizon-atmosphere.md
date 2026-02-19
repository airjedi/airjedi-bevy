# 3D Horizon & Atmospheric Haze Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a ground plane, distance fog, and sprite fading to make the 3D view look like a continuous world with atmospheric perspective.

**Architecture:** Three components layered within the existing dual-camera system. Camera3d renders atmosphere sky + fogged ground plane. Camera2d composites tiles on top, with distance-based alpha fading to blend into the fogged ground at distance. No custom shaders.

**Tech Stack:** Bevy 0.18 built-in `DistanceFog`, `FogFalloff::Exponential`, `StandardMaterial`, `Plane3d`

---

### Task 1: Add `visibility_range` field to View3DState

**Files:**
- Modify: `src/view3d/mod.rs:46-78`

**Step 1: Add the field**

In `View3DState` struct (line 47), add after `ground_elevation_ft`:

```rust
/// Distance (world units) before fog reaches full opacity
pub visibility_range: f32,
```

In `Default` impl (line 63), add after `ground_elevation_ft: 0,`:

```rust
visibility_range: 5000.0,
```

**Step 2: Build to verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles with no errors.

**Step 3: Commit**

```
git add src/view3d/mod.rs
git commit -m "Add visibility_range field to View3DState"
```

---

### Task 2: Spawn ground plane mesh

**Files:**
- Modify: `src/view3d/sky.rs:1-20` (imports, marker component)
- Modify: `src/view3d/sky.rs:138-156` (setup_sky function)

**Step 1: Add marker component and imports**

At the top of `sky.rs`, add to the existing imports (line 10):

```rust
use bevy::pbr::{Atmosphere, AtmosphereSettings, DistanceFog, FogFalloff, StandardMaterial};
```

After the `StarField` marker (line 20), add:

```rust
/// Marker component for the ground plane mesh
#[derive(Component)]
pub struct GroundPlane;
```

**Step 2: Spawn ground plane in setup_sky**

Add to the end of `setup_sky` (after the star field spawn, around line 155), before the closing brace:

```rust
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
```

Update `setup_sky` signature to accept mesh and material assets:

```rust
pub fn setup_sky(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
```

Note: `Plane3d::new(Vec3::Z, ...)` creates a plane with Z as the normal (facing up), which matches the coordinate system where Z is altitude. Verify this is correct in Bevy 0.18 — the plane should lie in the XY plane. If `Plane3d::new` takes different args, consult Bevy 0.18 docs for the correct constructor.

**Step 3: Build to verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles. Ground plane entity spawns hidden.

**Step 4: Commit**

```
git add src/view3d/sky.rs
git commit -m "Add ground plane mesh entity for 3D horizon"
```

---

### Task 3: Manage ground plane visibility and position in 3D mode

**Files:**
- Modify: `src/view3d/sky.rs:272-320` (manage_atmosphere_camera function)

**Step 1: Add ground plane query and visibility logic**

Update `manage_atmosphere_camera` to also control the ground plane. Add a ground plane query parameter:

```rust
mut ground_query: Query<(&mut Transform, &mut Visibility), With<GroundPlane>>,
```

Inside the `if state.is_3d_active()` block, after the atmosphere insertion (around line 300), add:

```rust
// Show ground plane at ground elevation, centered on camera target
if let Ok((mut gp_transform, mut gp_vis)) = ground_query.single_mut() {
    *gp_vis = Visibility::Inherited;
    gp_transform.translation.z = state.altitude_to_z(state.ground_elevation_ft);
    gp_transform.translation.x = state.saved_2d_center.x;
    gp_transform.translation.y = state.saved_2d_center.y;
}
```

In the `else` block (2D mode, around line 313), add:

```rust
// Hide ground plane in 2D
if let Ok((_, mut gp_vis)) = ground_query.single_mut() {
    *gp_vis = Visibility::Hidden;
}
```

**Step 2: Build and run to verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles.

Run: `cargo run`
Expected: In 3D mode (press 3), the dark ground plane is visible beyond tile edges. In 2D mode, it's hidden.

**Step 3: Commit**

```
git add src/view3d/sky.rs
git commit -m "Show ground plane at terrain elevation in 3D mode"
```

---

### Task 4: Keep ground plane centered on camera during pan

**Files:**
- Modify: `src/view3d/sky.rs:272-320` (manage_atmosphere_camera, or create a dedicated sync system)

The ground plane needs to follow the camera's XY position as the user pans, so it always appears as infinite ground. Since `manage_atmosphere_camera` already reads `View3DState`, update the ground plane position there. However, it only runs when atmosphere state changes. A lightweight dedicated system is better.

**Step 1: Create sync system**

Add a new system in `sky.rs` after `update_atmosphere_scale`:

```rust
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
```

Remove the ground plane code added in Task 3 from `manage_atmosphere_camera` — this dedicated system replaces it.

**Step 2: Register the system**

In `src/view3d/mod.rs`, plugin `build` (line 656), add:

```rust
.add_systems(Update, sky::sync_ground_plane.after(update_3d_camera))
```

**Step 3: Build and run to verify**

Run: `cargo build && cargo run`
Expected: Ground plane follows camera during pan in 3D mode. No visible edges.

**Step 4: Commit**

```
git add src/view3d/sky.rs src/view3d/mod.rs
git commit -m "Keep ground plane centered on camera during pan"
```

---

### Task 5: Add DistanceFog to Camera3d

**Files:**
- Modify: `src/view3d/sky.rs:272-320` (manage_atmosphere_camera)

**Step 1: Insert DistanceFog alongside Atmosphere**

In `manage_atmosphere_camera`, inside the `if has_atmo.is_none()` block (where atmosphere is inserted, line 294), also insert `DistanceFog`:

```rust
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
```

In the `else` block (2D mode), also remove `DistanceFog`:

```rust
commands.entity(cam3d_entity)
    .remove::<Atmosphere>()
    .remove::<AtmosphereSettings>()
    .remove::<DistanceFog>();
```

The `manage_atmosphere_camera` query for Camera3d needs `Option<&DistanceFog>` if we want to guard double-insertion, but since it's already guarded by `has_atmo.is_none()` (atmosphere and fog are inserted together), we can rely on the same guard.

**Step 2: Build and run**

Run: `cargo build && cargo run`
Expected: In 3D mode, the ground plane fades into haze at distance. The horizon shows a smooth ground-to-sky transition.

**Step 3: Commit**

```
git add src/view3d/sky.rs
git commit -m "Add DistanceFog to Camera3d for atmospheric haze"
```

---

### Task 6: Dynamic fog color from sun position

**Files:**
- Modify: `src/view3d/sky.rs` (new system)

**Step 1: Create the fog update system**

Add after `update_atmosphere_scale`:

```rust
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
```

**Step 2: Register the system**

In `src/view3d/mod.rs`, plugin `build`, add:

```rust
.add_systems(Update, sky::update_fog_parameters.after(sky::update_sun_position))
```

**Step 3: Build and run**

Run: `cargo build && cargo run`
Expected: Fog color shifts with time of day. Warm tones near sunset, blue-gray at midday, dark at night.

**Step 4: Commit**

```
git add src/view3d/sky.rs src/view3d/mod.rs
git commit -m "Dynamically update fog color from sun position"
```

---

### Task 7: Distance-based tile and aircraft sprite fading

**Files:**
- Modify: `src/view3d/mod.rs` (new system)

**Step 1: Create the fade system**

Add a new system in `mod.rs`, after `update_aircraft_altitude_z`:

```rust
/// Fade tile and aircraft sprites based on distance from Camera2d in 3D mode.
/// This makes tiles transparent at distance, revealing the fogged ground plane beneath.
pub fn fade_distant_sprites(
    state: Res<View3DState>,
    camera_query: Query<&Transform, With<Camera2d>>,
    mut tile_query: Query<(&Transform, &mut Sprite, &mut TileFadeState), (With<bevy_slippy_tiles::MapTile>, Without<Camera2d>)>,
    mut aircraft_query: Query<(&Transform, &mut Sprite), (With<crate::Aircraft>, Without<bevy_slippy_tiles::MapTile>, Without<Camera2d>)>,
) {
    if !state.is_3d_active() {
        return;
    }

    let Ok(cam_transform) = camera_query.single() else {
        return;
    };

    let cam_pos = cam_transform.translation;

    // Fade range matches the fog: starts at 40% of visibility_range, fully gone at 100%
    let fade_start = state.visibility_range * 0.4;
    let fade_end = state.visibility_range;
    let fade_range = fade_end - fade_start;

    if fade_range <= 0.0 {
        return;
    }

    // Fade tiles
    for (transform, mut sprite, fade_state) in tile_query.iter_mut() {
        let dist = cam_pos.distance(transform.translation);
        let distance_alpha = if dist <= fade_start {
            1.0
        } else if dist >= fade_end {
            0.0
        } else {
            1.0 - ((dist - fade_start) / fade_range)
        };
        // Combine with tile's own fade alpha (from zoom transitions)
        let final_alpha = fade_state.alpha * distance_alpha;
        sprite.color = Color::srgba(1.0, 1.0, 1.0, final_alpha);
    }

    // Fade aircraft
    for (transform, mut sprite) in aircraft_query.iter_mut() {
        let dist = cam_pos.distance(transform.translation);
        let alpha = if dist <= fade_start {
            1.0
        } else if dist >= fade_end {
            0.0
        } else {
            1.0 - ((dist - fade_start) / fade_range)
        };
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
}
```

Note: `TileFadeState` is defined in `src/main.rs:377` as `pub(crate)`. Make it `pub(crate)` if not already, since `view3d/mod.rs` needs to read it. Check if `TileFadeState` and its fields are accessible — if the struct is private, change `struct TileFadeState` to `pub(crate) struct TileFadeState` and fields to `pub(crate)`.

**Step 2: Restore alpha when returning to 2D**

The existing `manage_tile_fade` system in `main.rs:1457` already sets `sprite.color` based on `fade_state.alpha`. When 3D mode is exited, the `fade_distant_sprites` system stops running (early return when `!state.is_3d_active()`), and the normal `manage_tile_fade` system restores the correct alpha values. No explicit reset needed.

However, aircraft sprites may retain faded alpha. Add a reset at the top of `fade_distant_sprites`:

In `update_aircraft_altitude_z` (line 619), when transitioning back to 2D, the labels are already restored. Add aircraft sprite alpha reset there:

```rust
} else if !state.is_transitioning() {
    // Restore aircraft to fixed Z layer in 2D mode
    for (_aircraft, mut transform) in aircraft_query.iter_mut() {
        transform.translation.z = crate::constants::AIRCRAFT_Z_LAYER;
    }
```

This requires also querying `&mut Sprite` in `update_aircraft_altitude_z`. However, that might complicate the query. An alternative is to add the reset to `fade_distant_sprites` itself — when `!state.is_3d_active()`, iterate and reset to 1.0, then return. This runs once per frame in 2D mode but is cheap (early return after one pass to reset).

Update the early return in `fade_distant_sprites`:

```rust
if !state.is_3d_active() {
    // Reset aircraft alpha when leaving 3D mode
    for (_, mut sprite) in aircraft_query.iter_mut() {
        sprite.color = Color::srgba(1.0, 1.0, 1.0, 1.0);
    }
    return;
}
```

**Step 3: Register the system**

In plugin `build`, add:

```rust
.add_systems(Update, fade_distant_sprites
    .after(update_3d_camera)
    .after(update_tile_elevation))
```

**Step 4: Build and run**

Run: `cargo build && cargo run`
Expected: In 3D mode, distant tiles fade to transparent, revealing fogged ground. Aircraft also fade. In 2D, everything is fully opaque.

**Step 5: Commit**

```
git add src/view3d/mod.rs src/main.rs
git commit -m "Add distance-based tile and aircraft fading in 3D mode"
```

---

### Task 8: Add visibility range slider to 3D settings panel

**Files:**
- Modify: `src/tools_window.rs:316-378` (render_view3d_tab)

**Step 1: Add the slider**

In `render_view3d_tab`, after the ground elevation section (around line 377), add:

```rust
ui.separator();
ui.label("Atmosphere:");

ui.horizontal(|ui| {
    ui.label("Visibility:");
    ui.add(egui::Slider::new(&mut state.visibility_range, 1000.0..=20000.0)
        .suffix(" units")
        .logarithmic(true));
});
```

**Step 2: Build and run**

Run: `cargo build && cargo run`
Expected: The 3D View tab in the Tools window shows a "Visibility" slider. Moving it changes how far you can see before fog/fade.

**Step 3: Commit**

```
git add src/tools_window.rs
git commit -m "Add visibility range slider to 3D settings panel"
```

---

### Task 9: Tune defaults and visual polish

**Files:**
- Modify: `src/view3d/mod.rs` (default visibility_range)
- Modify: `src/view3d/sky.rs` (ground plane material, fog initial values)

This task is for manual tuning after running the app. Adjust:

**Step 1: Run and observe**

Run: `cargo run`

Check in 3D mode:
- Does the ground plane color match the tile edges? Adjust `Color::srgb(0.1, 0.1, 0.12)` in `setup_sky`.
- Is the default `visibility_range: 5000.0` a good starting point? Tiles should fade at roughly the coverage edge distance.
- Does the fog color blend smoothly with the atmosphere sky at the horizon?
- Do tiles fade too abruptly or too gradually? Adjust `fade_start` ratio (currently 0.4).

**Step 2: Adjust values based on observations**

Common tuning points:
- `visibility_range` default: Try 3000-8000 range
- `fade_start` ratio: Try 0.3-0.6 of visibility_range
- Ground plane color: Match by taking a screenshot of a dark tile and sampling the color
- Fog base color values in `update_fog_parameters`

**Step 3: Build and verify**

Run: `cargo build && cargo run`
Expected: Smooth, natural-looking horizon with no jarring transitions.

**Step 4: Commit**

```
git add src/view3d/mod.rs src/view3d/sky.rs
git commit -m "Tune fog and ground plane defaults for visual quality"
```
