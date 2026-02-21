# Y-up Migration for Native Bevy Atmosphere

## Problem

The 3D view uses Z-up coordinates (map on XY plane, altitude on Z), but Bevy's
built-in Atmosphere component assumes Y-up. The atmosphere shader interprets the
camera's Y position as altitude, producing a black sky when the camera is at
Y=0 in our Z-up layout. The current workaround is a hand-rolled sky system
(compute_sky_color + DistanceFog) that approximates atmospheric scattering with
hardcoded color gradients. This blocks use of Bevy's physically-based Rayleigh/Mie
scattering, atmosphere-aware PBR shading, environment map reflections, and any
future Bevy 3D features that assume Y-up.

## Solution

Migrate the 3D view to use Bevy's native Y-up convention for Camera3d and all
3D-rendered entities. Camera2d (tiles) stays in Z-up space. A fixed 90-degree
rotation bridges the two coordinate systems at the camera sync boundary.

## Coordinate System Mapping

Two coordinate domains coexist:

**Z-up (data/tiles):** Map tiles on XY plane, Z = layer depth. All geo-to-pixel
conversion, `saved_2d_center`, `MapState`, tile code unchanged. Camera2d renders
tiles here.

**Y-up (3D rendering):** Ground plane on XZ, Y = altitude. Camera3d, Atmosphere,
DirectionalLight, aircraft meshes, ground plane all operate here.

The transform between them:

    Z-up to Y-up:  (x, y, z) -> (x, z, -y)    Quat::from_rotation_x(-PI/2)
    Y-up to Z-up:  (x, y, z) -> (x, -z, y)    Quat::from_rotation_x(PI/2)

This maps: X(east) stays, Y(north) becomes -Z(forward), Z(up) becomes Y(up).
Encapsulated as utility functions `zup_to_yup` / `yup_to_zup` and a constant
`COORD_ROTATION: Quat`.

## Camera Architecture

Camera3d becomes primary in 3D mode. Camera2d derives from it.

**3D mode:**
- Camera3d computes orbit in Y-up space (altitude on Y, orbit in XZ plane,
  `looking_at(center, Vec3::Y)`)
- Camera3d holds Atmosphere + AtmosphereSettings + AtmosphereEnvironmentMapLight
- Camera3d renders first (order=0), atmosphere paints sky
- Camera2d transform derived from Camera3d via Y-up-to-Z-up rotation
- Camera2d renders second (order=1), transparent clear, tiles composite over sky

**2D mode:** Unchanged. Camera2d is orthographic, Camera3d mirrors it.

Sync direction in `sync_aircraft_camera`:
- 2D mode: Camera2d -> Camera3d (copy, as today)
- 3D mode: Camera3d -> Camera2d (apply Y-up-to-Z-up rotation)

### Orbit Computation

`calculate_camera_transform` changes from Z-up to Y-up:

    center_yup = (saved_2d_center.x, ground_altitude_y, -saved_2d_center.y)

    camera_pos = (
        center.x - horizontal_dist * sin(yaw),
        center.y + vertical_dist,                  // altitude on Y
        center.z - horizontal_dist * cos(yaw),     // depth on Z
    )
    looking_at(center, Vec3::Y)

### Transition Animation

During 2D-to-3D transition:
- Camera3d interpolates in Y-up space (position lerp, rotation slerp)
- Camera2d transform derived each frame from Camera3d via rotation
- Both cameras stay in lockstep because the Y-up/Z-up transform is a rigid
  rotation (preserves lerp/slerp curves)

Pan/drag: `saved_2d_center` stays in Z-up pixel space. Drag deltas modify it
using yaw-aware basis vectors. Conversion to Y-up happens downstream in
`calculate_camera_transform`.

## 3D Entity Positioning

### Aircraft Models

`update_aircraft_positions` (camera.rs) sets .x and .y from lat/lon pixel
coords -- unchanged. A renamed `update_aircraft_3d_transform` runs after it
and remaps to Y-up in 3D mode:

    3D mode: translation = (px, altitude_y, -py)
             rotation = heading around Y axis * BASE_ROT_YUP
    2D mode: translation.z = AIRCRAFT_Z_LAYER (unchanged)

Aircraft model base rotation (BASE_ROT_YUP) recalculated for Y-up from GLB
model local axes (nose=+Z, top=+Y, right-wing=+X).

### Ground Plane

Changes from `Plane3d::new(Vec3::Z, ...)` to `Plane3d::new(Vec3::Y, ...)`.
Position: `(saved_2d_center.x, ground_altitude_y, -saved_2d_center.y)`.

### DirectionalLight (Sun/Moon)

The existing euler rotation `Quat::from_euler(EulerRot::YXZ, -azim, -elev, 0)`
already uses a Y-up convention (azimuth around Y, elevation around X). No change
needed. The atmosphere shader reads the light direction from this transform.

## Atmosphere Setup

Bevy's native Atmosphere fully replaces the hand-rolled sky system.

**Added to Camera3d when entering 3D mode:**
- `Atmosphere::earthlike(medium_handle)` -- Rayleigh + Mie scattering
- `AtmosphereSettings { scene_units_to_m }` -- pixel-scale to meters conversion
- `AtmosphereEnvironmentMapLight` -- IBL reflections and ambient lighting

**Removed (replaced by Atmosphere):**
- `compute_sky_color()` (~50 lines) -- atmosphere paints sky
- `update_fog_parameters()` (~65 lines) -- atmosphere handles scattering/haze
- DistanceFog insert/remove in manage_atmosphere_camera (~20 lines)
- Manual ambient brightness tuning for 3D mode

**Kept:**
- Sun/moon position calculations and DirectionalLight euler rotations
- Star field sprite system (Camera2d, atmosphere does not render stars)
- 2D tint overlay (2D mode only, unaffected)
- `fade_distant_sprites` -- may be simplified later as follow-up

## Files Changed

| File | Changes |
|------|---------|
| src/view3d/mod.rs | calculate_camera_transform to Y-up orbit. update_3d_camera sets Camera3d primary, derives Camera2d. Rename update_aircraft_altitude_z to update_aircraft_3d_transform with Y-up remapping. Ground plane normal to Vec3::Y. Add coord conversion utilities. |
| src/view3d/sky.rs | manage_atmosphere_camera inserts Atmosphere + AtmosphereSettings + AtmosphereEnvironmentMapLight, removes DistanceFog. Delete compute_sky_color and update_fog_parameters. sync_ground_plane uses Y-up positioning. |
| src/camera.rs | sync_aircraft_camera conditional: 2D copies Camera2d to Camera3d, 3D applies rotation Camera3d to Camera2d. Aircraft model base rotation constant for Y-up. |
| src/main.rs | ScatteringMedium::earthlike() + AtmosphereMediumHandle resource if not already present. |

## Unchanged

- All 2D mode behavior
- Tile system, geo conversions, slippy tiles
- Sun/moon position calculations
- Star field sprite system
- 2D tint overlay
- Pan/drag data model (saved_2d_center stays Z-up)
- UI panels, egui, toolbar, settings
