# 3D Horizon, Atmospheric Haze, and Realism

**Date:** 2026-02-18
**Approach:** Ground Plane + DistanceFog + Distance-Based Sprite Fading
**Fidelity:** Functional realism (no custom shaders, all Bevy built-in features)

## Problem

The 3D view has three visual gaps compared to a flight simulator:

1. **Tiles end abruptly** at their coverage edges, revealing atmosphere/void with no transition
2. **No atmospheric perspective** - distant terrain and aircraft are as crisp as nearby ones
3. **No continuous ground surface** - just floating tile sprites with gaps between coverage and sky

## Solution Overview

Three components work together within the existing dual-camera architecture:

1. A **ground plane mesh** extends beyond tile edges to the horizon
2. **DistanceFog** on Camera3d fades the ground plane into atmospheric haze
3. **Distance-based alpha** on tile/aircraft sprites matches the fog fade, revealing the fogged ground beneath

## Component 1: Ground Plane

A large `Plane3d` mesh with `MeshMaterial3d<StandardMaterial>` at ground elevation Z.

- Material color: dark gray matching CartoDB dark basemap (~`Color::srgb(0.1, 0.1, 0.12)`)
- Size: 500,000 x 500,000 world units (edges never visible through fog)
- Rendered by Camera3d, sits behind Camera2d tile compositing
- `GroundPlane` marker component, hidden in 2D mode
- Spawned during setup alongside star field

**Camera layering:**
- Camera3d (order 0): atmosphere sky + ground plane mesh
- Camera2d (order 1): tiles composited on top with `Color::NONE` clear
- Where tiles exist, they cover the ground. Beyond tile edges, the ground plane is visible.

## Component 2: DistanceFog

Bevy's built-in `DistanceFog` component attached to Camera3d.

- **Falloff:** `FogFalloff::Exponential` with density derived from user-configurable visibility range
- **Density formula:** `density = 3.0 / visibility_range`
- **Fog color:** dynamically tracks sun elevation
  - Daytime: muted blue-gray matching Rayleigh scattering at the horizon
  - Sunset: warm amber tones
  - Night: near-black
- **Directional light color:** enabled for sun glow through fog
- **Lifecycle:** inserted/removed alongside `Atmosphere` in `manage_atmosphere_camera`
- **Only affects** PBR meshes (ground plane), not Camera2d sprites

## Component 3: Distance-Based Sprite Fading

A system `fade_distant_sprites` that adjusts alpha on Camera2d entities.

- Calculates 3D distance from camera to each tile and aircraft entity
- Applies alpha via `Sprite.color` alpha channel:
  - Below near threshold: alpha = 1.0
  - Between near and far: smooth interpolation to 0.0
  - Beyond far threshold: alpha = 0.0 (transparent, revealing fogged ground)
- Fade thresholds match DistanceFog parameters for visual consistency
- Faded entities: `MapTile`, `Aircraft`
- Alpha reset to 1.0 when returning to 2D mode

## State Changes

New field on `View3DState`:
- `visibility_range: f32` - distance before fog reaches full opacity (world units)

## UI Changes

New "Visibility" section in the 3D View Settings panel:
- Slider for visibility range

## System Ordering

- `manage_atmosphere_camera` (existing, after `animate_view_transition`): also inserts/removes DistanceFog and shows/hides ground plane
- `fade_distant_sprites` (new): after `update_3d_camera` and `update_tile_elevation`
- `update_fog_parameters` (new): after `update_sun_position`, updates fog color from sun state

## Files Modified

- `src/view3d/sky.rs` - ground plane setup, fog management, fog color updates
- `src/view3d/mod.rs` - `View3DState` field, sprite fading system, settings panel, system registration

## Performance

- 2D mode: no cost (ground plane hidden, fog removed, no alpha calculations)
- 3D mode: one extra mesh (ground plane), per-frame distance calculation for visible tiles/aircraft (lightweight)

## Trade-offs Accepted

- Per-tile alpha fading (not per-pixel) - acceptable at the distances where fading occurs
- Flat ground plane (no earth curvature) - sufficient for the altitude range (1,000-60,000 ft)
- Ground plane is flat color, not textured - matches the dark basemap aesthetic
