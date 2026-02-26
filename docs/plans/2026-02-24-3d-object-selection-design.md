# 3D Object Selection Design

## Overview

Enable clicking on aircraft in 3D mode to select them, with visual outline feedback and camera follow. Uses Bevy 0.18's built-in `MeshPickingPlugin` for raycasting and a scaled-clone technique for outlines.

## Requirements

- Click aircraft in 3D mode to select them
- Selected aircraft gets a cyan outline (scaled clone, back-face rendered)
- Hover shows a subtle outline before clicking
- Camera orbits around the selected aircraft (follow mode)
- Click empty space or press ESC to deselect
- Existing `AircraftListState.selected_icao` remains the source of truth

## Picking

Bevy 0.18 ships `MeshPickingPlugin` as part of `DefaultPlugins`. It raycasts against `Mesh3d` entities automatically.

Each aircraft entity gets:
- `Pickable::default()` component
- `.observe(on_aircraft_click)` — sets `selected_icao`, spawns `SelectionOutline`
- `.observe(on_aircraft_hover)` — spawns `HoverOutline`
- `.observe(on_aircraft_out)` — despawns `HoverOutline`

The ground plane gets a `Pointer<Click>` observer that clears selection.

## Outline Effect

Scaled-clone technique:
- Spawn a child entity under the aircraft with the same `SceneRoot` (airplane.glb)
- Selection outline: scale 1.05x, cyan, unlit, front-face culled
- Hover outline: scale 1.03x, dimmer cyan, unlit, front-face culled
- A system watches for newly-spawned outline children and replaces their `MeshMaterial3d<StandardMaterial>` with the flat outline material
- Tagged with `SelectionOutline` / `HoverOutline` marker components for cleanup

## Camera Follow

When an aircraft is selected in 3D mode:
- 3D camera orbit center moves to the aircraft's world position
- Smooth interpolation via lerp (~5.0 * delta_time)
- Maintains current pitch, yaw, and altitude offset
- Pan drag breaks follow mode (existing behavior)

## Data Flow

```
Pointer<Over>  on aircraft  → spawn HoverOutline child
Pointer<Out>   on aircraft  → despawn HoverOutline child
Pointer<Click> on aircraft  → set selected_icao, spawn SelectionOutline
Pointer<Click> on ground    → clear selected_icao, despawn SelectionOutline
ESC key                     → clear selected_icao, despawn SelectionOutline

Each frame (3D mode):
  if selected_icao → orbit center = aircraft world pos (lerped)
  outline_material_swap → replace materials on outline clone children
  if aircraft despawned → auto-clear selection
```

## Files

- `src/aircraft/picking.rs` (new) — observers, outline spawn/despawn, material swap system
- `src/adsb/sync.rs` — add Pickable and observers to spawned aircraft
- `src/view3d/mod.rs` — integrate follow target into 3D camera, ESC handler
- `src/aircraft/mod.rs` — add module declaration
- `src/main.rs` — register picking systems

## Dependencies

None new. `MeshPickingPlugin` is part of Bevy's `DefaultPlugins`.
