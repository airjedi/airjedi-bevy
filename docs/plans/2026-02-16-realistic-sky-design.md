# Realistic Sky for 3D Mode

## Overview

Add a physically-based sky with full day/night cycle to the 3D perspective view, using Bevy 0.18's built-in `Atmosphere` component and a procedural star field.

## Rendering Pipeline

Three cameras render in order in 3D mode:

| Order | Camera | Renders | Clear Color |
|-------|--------|---------|-------------|
| -1 | Camera3d (sky) | Atmosphere + star sphere | Solid black |
| 0 | Camera2d (map) | Tiles, trails, labels | Transparent (3D) / Default (2D) |
| 1 | Camera3d (aircraft) | 3D aircraft models | Transparent |

The sky camera is only active in 3D mode. Its transform mirrors the main camera's orientation (pitch/yaw from View3DState) so the horizon aligns with the map tilt. Only rotation matters -- the sky is infinitely far away.

Camera2d switches to transparent clear color in 3D mode so the sky shows through behind tiles.

## Sun Position from Real Time

A system computes sun azimuth and elevation each frame using:
- System clock (UTC)
- Map center lat/lon from MapState

Algorithm:
1. Compute day-of-year and fractional hour
2. Calculate solar declination and hour angle
3. Derive elevation and azimuth via standard solar position formulas
4. Apply as DirectionalLight rotation

DirectionalLight illuminance scales with sun elevation -- bright at midday, dim at twilight, off at night. GlobalAmbientLight brightness also scales down at night.

No external crates needed -- ~20 lines of trig.

## Procedural Star Field

- Large inverted Sphere mesh centered on sky camera (faces inward)
- Emissive texture generated at startup with ~500-1000 procedural stars at varying brightness
- Visibility fades in as sun drops below horizon (elevation < 0), fully visible at astronomical twilight (elevation < -12)
- Fixed relative to camera -- stars don't shift with map panning
- Renders at sky camera layer, behind the atmosphere (atmosphere is semi-transparent at night)

## Integration with Existing 3D Mode

- Sky camera activates/deactivates with View3DState transitions
- Sky opacity lerps with transition progress for smooth 2D/3D switching
- Sky camera transform synced from View3DState pitch/yaw each frame
- No performance cost in 2D mode (sky camera and star sphere hidden)
- No new UI controls -- time driven by real clock

## Files

| File | Change |
|------|--------|
| src/view3d/sky.rs | New -- sky camera, atmosphere setup, sun position system, star field |
| src/view3d/mod.rs | Add mod sky, register sky systems in View3DPlugin |
| src/main.rs | Camera2d clear color toggling, DirectionalLight adjustments |

## Dependencies

No new crates. Uses Bevy 0.18 built-in: Atmosphere, AtmosphereSettings, SunDisk, Bloom, Hdr.
