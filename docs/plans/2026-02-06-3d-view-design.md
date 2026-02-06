# 3D View Experimental Feature Design

## Overview

A tilted perspective view that shows aircraft at their altitudes above a flat map plane. Users can orbit the camera around the map center to view from different angles. This provides visual context for altitude differences while maintaining the familiar map reference.

## Mode Switching

- Press `3` key to toggle between 2D and 3D modes
- Camera smoothly animates from orthographic top-down to perspective tilted view (~0.5 seconds)
- Map center position preserved during transitions
- Current zoom level translates to initial camera distance in 3D
- Pan/zoom controls disabled during transition animation

**Technical approach:**
- Single camera entity switches between `Projection::Orthographic` and `Projection::Perspective`
- Animate camera transform (position, rotation) using linear interpolation
- Store transition progress in `View3DState` resource

## Camera System

**Positioning in 3D mode:**
- Camera orbits around map center at configurable distance and pitch
- Default: 45 degree pitch, ~10km distance
- Camera always looks at map center point

**Controls:**

| Input | Action |
|-------|--------|
| Left drag | Orbit camera (change yaw angle) |
| Right drag / Two-finger | Pan map center |
| Scroll | Adjust camera distance |
| Shift + scroll | Adjust pitch angle |

**Constraints:**
- Pitch: 15 to 89 degrees
- Distance: 1km to 100km equivalent
- Yaw: unconstrained (full 360 degree orbit)

**Implementation:**
- Store `camera_yaw`, `camera_pitch`, `camera_distance` in `View3DState`
- Calculate camera position from spherical coordinates each frame
- Use `Transform::look_at()` to point at map center

## Aircraft 3D Representation

**Shape:** Cone mesh pointing in heading direction
- Dimensions: ~500m tall, ~300m base radius (exaggerated for visibility)
- Tip points in flight direction
- Color: bright cyan to stand out

**Positioning:**
- X/Z from lat/lon using local tangent plane projection
- Y from altitude with 10x exaggeration: `y = altitude_feet * 0.3048 * 10 / 1000`
- Example: 35,000 ft appears ~107 units above ground

**Orientation:**
- Rotate cone around Y-axis to match heading
- No pitch/roll

**Labels:**
- Billboard text always facing camera
- Positioned above cone
- Shows callsign and altitude
- Scale inversely with distance

**Rendering:**
- Create cone mesh once at startup as resource
- In 3D mode: `Mesh3d` + `MeshMaterial3d` entities
- In 2D mode: existing `Sprite` entities
- Toggle visibility by mode

## Ground Plane

**Approach:** Reuse existing tile sprites, transform for 3D
- Rotate tile sprites 90 degrees to lie flat (X/Z plane)
- Position at Y=0
- X/Z positions same as current X/Y
- Tiles continue to load/unload via `bevy_slippy_tiles`

**Visibility by mode:**
- Aircraft sprites: 2D only
- Aircraft 3D meshes: 3D only
- Tile sprites: always visible, transform changes

## Implementation Plan

**Files to modify:**
- `src/view3d/mod.rs` - Main implementation
- `src/main.rs` - Add systems, lighting

**New systems:**
1. `animate_view_transition` - Lerp camera between states
2. `handle_3d_camera_controls` - Orbit, pitch, distance
3. `update_aircraft_3d` - Position/rotate cone meshes
4. `update_3d_labels` - Billboard labels
5. `transform_tiles_for_3d` - Rotate tiles flat

**Startup additions:**
- Cone mesh resource
- Directional light (sun)
- Ambient light

## Scope

**In v1 (experimental):**
- Mode toggle with smooth transition
- Orbit camera controls
- Cone aircraft with heading
- Flat map tiles
- Billboard labels

**Not in v1:**
- Terrain elevation
- 3D trail rendering
- Aircraft model loading
- Shadows
