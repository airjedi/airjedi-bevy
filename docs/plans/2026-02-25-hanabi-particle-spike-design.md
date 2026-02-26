# Hanabi Particle System Spike

## Goal

Evaluate bevy_hanabi as a particle effects system for aircraft trails and selection fog, integrated behind a cargo feature flag in the main app.

## Dependency

`bevy_hanabi = { version = "0.18", optional = true }` gated behind a `hanabi` cargo feature.

## Use Case 1: Particle Ribbon Trails

Replace gizmo-based trail rendering with Hanabi ribbon particle trails when the feature is enabled.

### Approach

- Each aircraft gets a `ParticleEffect` entity with `SimulationSpace::Global` so particles detach from the aircraft and stay fixed in world space.
- Particles spawn continuously and link into a ribbon via `PREV`/`NEXT`/`RIBBON_ID` attributes, producing a smooth contrail.
- Particle lifetime matches the existing `max_age_seconds` (300s default).
- Opacity fades over lifetime via `ColorOverLifetimeModifier` (alpha channel only).

### Altitude-Based Coloring

- Each particle's color is set at spawn time using `SetAttributeModifier` on `Attribute::COLOR`, based on the aircraft's current altitude.
- Uses the same 5-stop gradient as gizmo trails:
  - 0-10k ft: Cyan to Green
  - 10k-20k ft: Green to Yellow
  - 20k-30k ft: Yellow to Orange
  - 30k-40k ft: Orange to Purple
  - 40k+ ft: Purple
- As an aircraft climbs or descends, the trail naturally shows altitude history as a color gradient along its length.

### Integration

- New `hanabi_trail_renderer.rs` module behind `#[cfg(feature = "hanabi")]`.
- Existing `trail_renderer.rs` systems get `#[cfg(not(feature = "hanabi"))]`.
- Uses existing `TrailHistory` data for positioning.
- Works in both 2D and 3D view modes.

## Use Case 2: Selection Fog Effect

Add a particle fog selection indicator around selected aircraft.

### Approach

- When an aircraft is selected, spawn a `ParticleEffect` as a child entity with `SimulationSpace::Local` (particles move with the aircraft).
- `SetPositionSphereModifier` spawns particles on/near a sphere surface.
- `ConformToSphereModifier` keeps particles hovering near the sphere surface for a churning fog look.
- Short lifetime (1-2s) with continuous respawning.
- Semi-transparent white/cyan color with low alpha.
- Sphere radius sized so nose, wingtips, and tail protrude beyond the fog boundary.

### Integration

- New `hanabi_selection.rs` module behind `#[cfg(feature = "hanabi")]`.
- Hooks into existing `SelectionOutline` component: added = spawn fog, removed = despawn fog.
- Existing material-swap selection (emissive glow) remains active alongside the fog.

## Camera Compatibility

- Both 2D and 3D modes supported.
- In 2D, particles render as flat billboards.
- In 3D, particles render in full 3D space.
- `ParticleEffect` entities inherit aircraft transforms, which already handle both modes.

## Performance Budget

- Conservative particle caps: 1024 per trail, 256 per fog sphere.
- Hanabi is GPU-accelerated via compute shaders.
- Should handle dozens of aircraft without issue.

## Evaluation Criteria

- Does the ribbon trail look better than gizmo lines?
- Does the fog selection effect read clearly at various zoom levels?
- What's the GPU/CPU overhead with 20+ aircraft?
- Does Hanabi compose correctly with the dual-camera setup?
- Any Z-fighting or rendering order issues?
