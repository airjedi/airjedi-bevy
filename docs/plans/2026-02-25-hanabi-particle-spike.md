# Hanabi Particle System Spike Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Integrate bevy_hanabi behind a cargo feature flag to replace gizmo trails with ribbon particle trails (altitude-colored) and add a particle fog selection effect around selected aircraft.

**Architecture:** Add an optional `hanabi` cargo feature. When enabled, a `HanabiPlugin` registers particle-based trail and selection systems that replace the gizmo trail renderer and augment the material-swap selection. New modules `hanabi_trails.rs` and `hanabi_selection.rs` live alongside existing renderers with cfg gates.

**Tech Stack:** bevy_hanabi 0.18 (GPU particle system), Bevy 0.18 ECS

---

### Task 1: Add bevy_hanabi dependency and feature flag

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add the optional dependency and feature**

Add to `Cargo.toml`:

```toml
[features]
default = []
hanabi = ["dep:bevy_hanabi"]

[dependencies]
bevy_hanabi = { version = "0.18", optional = true }
```

**Step 2: Verify it compiles without the feature**

Run: `cargo build`
Expected: Compiles successfully, no changes to default behavior.

**Step 3: Verify it compiles with the feature**

Run: `cargo build --features hanabi`
Expected: Compiles successfully, bevy_hanabi is downloaded and built.

**Step 4: Commit**

```
feat: add bevy_hanabi optional dependency with hanabi feature flag
```

---

### Task 2: Create HanabiPlugin skeleton and gate existing trail renderer

**Files:**
- Create: `src/aircraft/hanabi_plugin.rs`
- Modify: `src/aircraft/mod.rs`
- Modify: `src/aircraft/plugin.rs`

**Step 1: Create the hanabi_plugin module**

Create `src/aircraft/hanabi_plugin.rs`:

```rust
use bevy::prelude::*;

pub struct HanabiPlugin;

impl Plugin for HanabiPlugin {
    fn build(&self, _app: &mut App) {
        info!("HanabiPlugin loaded — particle trails and selection fog enabled");
    }
}
```

**Step 2: Register the module and plugin**

In `src/aircraft/mod.rs`, add:

```rust
#[cfg(feature = "hanabi")]
pub mod hanabi_plugin;
```

In `src/aircraft/plugin.rs`, conditionally add the hanabi plugin and gate gizmo trail systems:

- Add `#[cfg(feature = "hanabi")]` to import and register `HanabiPlugin` as a sub-plugin.
- Wrap `draw_trails` registration with `#[cfg(not(feature = "hanabi"))]` so gizmo trails are skipped when hanabi is active. Keep `record_trail_points` and `prune_trails` unconditional since hanabi trails still use `TrailHistory` data.

The plugin.rs `build` method should look like:

```rust
// Trail drawing: gizmo (default) or hanabi (feature)
#[cfg(not(feature = "hanabi"))]
{
    app.add_systems(Update, draw_trails);
}
#[cfg(feature = "hanabi")]
{
    app.add_plugins(hanabi_plugin::HanabiPlugin);
}
```

Note: `record_trail_points` and `prune_trails` stay unconditional.

**Step 3: Verify both configurations compile**

Run: `cargo build && cargo build --features hanabi`
Expected: Both compile. Default build uses gizmo trails. Hanabi build prints the info log and skips gizmo trails.

**Step 4: Commit**

```
feat: add HanabiPlugin skeleton and gate gizmo trail renderer behind feature flag
```

---

### Task 3: Implement particle ribbon trail system

**Files:**
- Create: `src/aircraft/hanabi_trails.rs`
- Modify: `src/aircraft/hanabi_plugin.rs`
- Modify: `src/aircraft/mod.rs`

**Step 1: Create the hanabi_trails module**

Create `src/aircraft/hanabi_trails.rs` with:

1. A `TrailEffect` marker component to link a `ParticleEffect` entity to its aircraft.
2. A startup or on-spawn system `attach_trail_effects` that creates the `EffectAsset` for ribbon trails:
   - `SimulationSpace::Global` so particles stay in world space
   - Ribbon attributes (`PREV`, `NEXT`, `RIBBON_ID`) for linked trails
   - `ColorOverLifetimeModifier` for alpha fade only (RGB set per-particle at spawn)
   - Particle lifetime matching `TrailConfig::max_age_seconds` (300s default)
   - Max particles: 1024 per effect
   - Spawn rate: 0.5/sec (matching the 2-second trail record interval)
3. A system `update_trail_particles` that runs each frame:
   - For each aircraft with `TrailHistory`, find its associated `ParticleEffect` entity
   - When new trail points are recorded, set particle color via `altitude_color()` from `trails.rs`
   - Position the emitter at the aircraft's current world position (the `ParticleEffect` transform)
4. A system `spawn_trail_effects` that watches for new `Aircraft` entities without a `TrailEffect` and attaches one
5. A system `cleanup_trail_effects` that despawns trail effects when their aircraft is removed

Key implementation details:
- Use `ExprWriter` to build the effect module
- Set `Attribute::COLOR` at spawn time using `SetAttributeModifier` with the altitude color
- The aircraft's altitude determines the color at spawn; as altitude changes, new particles get new colors
- In 2D mode, set particle Z to 0. In 3D mode, use `view3d_state.altitude_to_z()`.
- Use `CoordinateConverter::latlon_to_world()` for XY positioning, same as `trail_renderer.rs`

```rust
use bevy::prelude::*;
use bevy_hanabi::prelude::*;

use super::trails::altitude_color;
use super::{TrailHistory, TrailConfig, SessionClock};
use crate::{Aircraft, MapState};
use crate::geo::CoordinateConverter;
use crate::view3d::View3DState;

/// Marker linking an aircraft entity to its trail particle effect entity.
#[derive(Component)]
pub struct TrailEffect {
    pub effect_entity: Entity,
}
```

The `EffectAsset` setup:

```rust
fn create_trail_effect(
    module: &mut Module,
    config: &TrailConfig,
) -> EffectAsset {
    let lifetime = module.lit(config.max_age_seconds as f32);
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    let init_pos = SetPositionSphereModifier {
        center: module.lit(Vec3::ZERO),
        radius: module.lit(0.1),
        dimension: ShapeDimension::Volume,
    };

    // Alpha-only gradient: full opacity -> transparent over lifetime
    let mut alpha_gradient = Gradient::new();
    alpha_gradient.add_key(0.0, Vec4::new(1., 1., 1., 1.));
    alpha_gradient.add_key(0.75, Vec4::new(1., 1., 1., 1.)); // solid for 75% of life
    alpha_gradient.add_key(1.0, Vec4::new(1., 1., 1., 0.));  // fade out last 25%

    EffectAsset::new(1024, SpawnerSettings::rate(0.5.into()), module.clone())
        .with_simulation_space(SimulationSpace::Global)
        .init(init_pos)
        .init(init_lifetime)
        .render(ColorOverLifetimeModifier {
            gradient: alpha_gradient,
            blend: ColorBlendMode::Modulate,
            mask: ColorBlendMask::ALPHA,
        })
}
```

Note: The ribbon attributes and exact API may need adjustment based on what compiles. This is a spike — iterate on the visual quality.

**Step 2: Register in hanabi_plugin.rs**

```rust
use bevy::prelude::*;
use bevy_hanabi::prelude::*;

use super::hanabi_trails;

pub struct HanabiPlugin;

impl Plugin for HanabiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_systems(Update, (
                hanabi_trails::spawn_trail_effects,
                hanabi_trails::update_trail_particles,
                hanabi_trails::cleanup_trail_effects,
            ));
    }
}
```

Note: `bevy_hanabi::HanabiPlugin` (the library's plugin) must be added to the app for particle rendering to work. Add it here.

**Step 3: Register module in mod.rs**

Add to `src/aircraft/mod.rs`:

```rust
#[cfg(feature = "hanabi")]
pub mod hanabi_trails;
```

**Step 4: Test with the hanabi feature**

Run: `cargo run --features hanabi`
Expected: Aircraft appear with particle ribbon trails instead of gizmo lines. Trails should be colored by altitude and fade over time.

**Step 5: Test without the hanabi feature**

Run: `cargo run`
Expected: Normal gizmo trails, no particle effects.

**Step 6: Commit**

```
feat: add particle ribbon trail rendering with altitude-based coloring
```

---

### Task 4: Implement particle fog selection effect

**Files:**
- Create: `src/aircraft/hanabi_selection.rs`
- Modify: `src/aircraft/hanabi_plugin.rs`
- Modify: `src/aircraft/mod.rs`

**Step 1: Create the hanabi_selection module**

Create `src/aircraft/hanabi_selection.rs` with:

1. A `SelectionFog` marker component linking the fog effect entity to its aircraft.
2. A system `manage_selection_fog` that watches for `SelectionOutline` additions/removals:
   - When `SelectionOutline` is added to an aircraft: spawn a `ParticleEffect` child entity with `SimulationSpace::Local`
   - When `SelectionOutline` is removed: despawn the fog effect entity
3. The fog `EffectAsset`:
   - `SetPositionSphereModifier` with radius ~15-25 units (adjust based on aircraft model scale)
   - `ConformToSphereModifier` to keep particles near the sphere surface
   - Short lifetime: 1.5 seconds with continuous respawn
   - Spawn rate: ~100/sec for dense fog
   - Max particles: 256
   - Color: semi-transparent cyan `(0.0, 0.8, 1.0, 0.15)` to complement the emissive selection glow
   - `SimulationSpace::Local` so fog moves with aircraft

```rust
use bevy::prelude::*;
use bevy_hanabi::prelude::*;

use super::picking::SelectionOutline;
use crate::Aircraft;

/// Marker linking a selection fog particle effect to its aircraft.
#[derive(Component)]
pub struct SelectionFog {
    pub effect_entity: Entity,
}
```

The fog `EffectAsset` setup:

```rust
fn create_fog_effect(module: &mut Module) -> EffectAsset {
    let sphere_radius = module.lit(20.0);
    let center = module.lit(Vec3::ZERO);

    let init_pos = SetPositionSphereModifier {
        center,
        radius: sphere_radius,
        dimension: ShapeDimension::Surface,
    };

    let lifetime = module.lit(1.5);
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    // Subtle fade in and out
    let mut gradient = Gradient::new();
    gradient.add_key(0.0, Vec4::new(0.0, 0.8, 1.0, 0.0));
    gradient.add_key(0.2, Vec4::new(0.0, 0.8, 1.0, 0.15));
    gradient.add_key(0.8, Vec4::new(0.0, 0.8, 1.0, 0.15));
    gradient.add_key(1.0, Vec4::new(0.0, 0.8, 1.0, 0.0));

    let conform = ConformToSphereModifier::new(
        center,
        sphere_radius,
        module.lit(5.0),   // influence_dist
        module.lit(10.0),  // attraction_accel
        module.lit(2.0),   // max_attraction_speed
    );

    EffectAsset::new(256, SpawnerSettings::rate(100.0.into()), module.clone())
        .with_simulation_space(SimulationSpace::Local)
        .init(init_pos)
        .init(init_lifetime)
        .update(conform)
        .render(ColorOverLifetimeModifier {
            gradient,
            blend: ColorBlendMode::Overwrite,
            mask: ColorBlendMask::RGBA,
        })
}
```

**Step 2: Register in hanabi_plugin.rs**

Add `hanabi_selection::manage_selection_fog` to the update systems.

**Step 3: Register module in mod.rs**

Add to `src/aircraft/mod.rs`:

```rust
#[cfg(feature = "hanabi")]
pub mod hanabi_selection;
```

**Step 4: Test selection fog**

Run: `cargo run --features hanabi`
Expected: Clicking an aircraft shows a semi-transparent cyan fog sphere around it. Aircraft nose/wingtips/tail should protrude beyond the fog. Deselecting (ESC or clicking empty space) removes the fog.

**Step 5: Commit**

```
feat: add particle fog selection effect around selected aircraft
```

---

### Task 5: Tune visuals and evaluate

**Files:**
- Modify: `src/aircraft/hanabi_trails.rs`
- Modify: `src/aircraft/hanabi_selection.rs`

This is an iterative tuning task. Run the app and adjust:

**Step 1: Trail visual quality**

- Adjust particle size/scale for visibility at different zoom levels
- Tune spawn rate vs. trail record interval for smooth ribbons
- Verify altitude colors match gizmo trail colors visually
- Check that trails work in both 2D and 3D view modes
- Test view mode transitions (2D -> 3D and back)

**Step 2: Selection fog quality**

- Adjust sphere radius relative to aircraft model size
- Tune particle density and alpha for "partial fog" look
- Verify aircraft parts protrude clearly
- Test at various zoom levels / camera distances
- Check that fog moves correctly with aircraft

**Step 3: Performance check**

- Load many aircraft (20+) and observe frame rate
- Check GPU usage with and without hanabi feature
- Note any stutters during particle spawning/despawning

**Step 4: Document findings**

Add evaluation notes to `docs/plans/2026-02-25-hanabi-particle-spike-design.md`:
- Visual quality assessment for trails vs gizmos
- Visual quality assessment for fog selection
- Performance observations
- Any issues with dual-camera composition
- Recommendation: adopt, iterate, or abandon

**Step 5: Commit**

```
feat: tune hanabi particle effects and document evaluation findings
```
