# Aircraft Model Registry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Load aircraft-type-specific 3D models (starting with B737) using `bevy_obj`, replacing the single generic model with a registry that maps type codes to scene handles.

**Architecture:** Add `bevy_obj` ObjPlugin for OBJ file loading. Replace the single `AircraftModel` resource with `AircraftModelRegistry` containing a default model and a HashMap of type-code-to-handle mappings. The sync system looks up the type code at spawn time via the `AircraftTypeDatabase`.

**Tech Stack:** Bevy 0.18, bevy_obj 0.18, Wavefront OBJ format

---

### Task 1: Copy B737 Model Assets

**Files:**
- Create: `assets/models/b737/78349.obj` (copy from ~/Downloads/)
- Create: `assets/models/b737/scene1.mtl` (copy from ~/Downloads/)

**Step 1: Create the models directory and copy files**

```bash
mkdir -p assets/models/b737
cp ~/Downloads/78349.obj assets/models/b737/
cp ~/Downloads/78349.mtl assets/models/b737/scene1.mtl
```

Note: The OBJ file references `scene1.mtl` in its header (`mtllib scene1.mtl`), so the MTL file must be named `scene1.mtl` and placed alongside the OBJ.

**Step 2: Verify the MTL filename matches the OBJ reference**

```bash
head -10 assets/models/b737/78349.obj | grep mtllib
```

Expected: `mtllib scene1.mtl`

**Step 3: Commit**

```bash
git add assets/models/b737/
git commit -m "Add B737 3D model assets (OBJ + MTL)"
```

---

### Task 2: Add bevy_obj Dependency and ObjPlugin

**Files:**
- Modify: `Cargo.toml:6-25` (add dependency)
- Modify: `src/main.rs:145-173` (add ObjPlugin)

**Step 1: Add bevy_obj to Cargo.toml**

Add after line 7 (bevy dependency):
```toml
bevy_obj = "0.18"
```

**Step 2: Add ObjPlugin to app plugins in src/main.rs**

Add `bevy_obj::ObjPlugin` to the plugins tuple at line 159, after `SlippyTilesPlugin`:
```rust
            SlippyTilesPlugin,
            bevy_obj::ObjPlugin,
```

**Step 3: Build to verify compilation**

```bash
cargo build
```

Expected: Compiles with 0 errors (warnings OK).

**Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs
git commit -m "Add bevy_obj dependency and ObjPlugin"
```

---

### Task 3: Replace AircraftModel with AircraftModelRegistry

**Files:**
- Modify: `src/adsb/sync.rs:1-21` (replace AircraftModel resource and setup function)

**Step 1: Replace the AircraftModel resource and setup function**

Replace lines 1-21 of `src/adsb/sync.rs` with:

```rust
use bevy::prelude::*;
use std::collections::HashMap;

use crate::{constants, Aircraft, AircraftLabel};
use crate::aircraft::TrailHistory;
use crate::debug_panel::DebugPanelState;
use super::connection::{AdsbAircraftData, ConnectionStatusText};

use crate::theme::AppTheme;

/// Type codes that should use the B737 model
const B737_TYPES: &[&str] = &[
    "B731", "B732", "B733", "B734", "B735", "B736", "B737", "B738", "B739",
    "B37M", "B38M", "B39M",
];

/// Resource holding aircraft 3D model handles keyed by type code
#[derive(Resource)]
pub struct AircraftModelRegistry {
    pub default_model: Handle<Scene>,
    pub type_models: HashMap<String, Handle<Scene>>,
}

impl AircraftModelRegistry {
    /// Get the model handle for a given type code, falling back to the default
    pub fn get_model(&self, type_code: Option<&str>) -> Handle<Scene> {
        if let Some(code) = type_code {
            if let Some(handle) = self.type_models.get(code) {
                return handle.clone();
            }
        }
        self.default_model.clone()
    }
}

/// Load aircraft 3D models and build the registry
pub fn setup_aircraft_models(mut commands: Commands, asset_server: Res<AssetServer>) {
    let default_model = asset_server.load("airplane.glb#Scene0");
    let b737_model: Handle<Scene> = asset_server.load("models/b737/78349.obj");

    let mut type_models = HashMap::new();
    for code in B737_TYPES {
        type_models.insert(code.to_string(), b737_model.clone());
    }

    commands.insert_resource(AircraftModelRegistry {
        default_model,
        type_models,
    });
}
```

**Step 2: Update sync_aircraft_from_adsb to use the registry**

In the same file, update `sync_aircraft_from_adsb` function signature — replace `aircraft_model: Option<Res<AircraftModel>>` with `model_registry: Option<Res<AircraftModelRegistry>>` and update the guard:

Change:
```rust
    let Some(aircraft_model) = aircraft_model else {
        return; // Aircraft model not yet loaded
    };
```
To:
```rust
    let Some(model_registry) = model_registry else {
        return; // Aircraft model registry not yet loaded
    };
```

**Step 3: Update aircraft spawning to use type-aware model selection**

In the spawn block (around line 86-106), replace:
```rust
                    SceneRoot(aircraft_model.handle.clone()),
```
With a type-code-aware lookup. The `AircraftTypeDatabase` needs to be added as a system parameter. Add it to the function signature:

```rust
    type_db: Option<Res<crate::aircraft::AircraftTypeDatabase>>,
```

Then in the spawn block, look up the type code and select the model:

```rust
            // Look up type code for model selection
            let type_code = type_db
                .as_ref()
                .and_then(|db| db.lookup(&adsb_ac.icao))
                .and_then(|info| info.type_code.clone());

            let model_handle = model_registry.get_model(type_code.as_deref());

            let aircraft_entity = commands
                .spawn((
                    Name::new(format!("Aircraft: {}", aircraft_name)),
                    SceneRoot(model_handle),
                    Transform::from_xyz(0.0, 0.0, constants::AIRCRAFT_Z_LAYER),
                    Aircraft {
                        // ... fields unchanged
                    },
                    TrailHistory::default(),
                ))
                .id();
```

**Step 4: Build to verify compilation**

```bash
cargo build
```

Expected: Compiles with 0 errors.

**Step 5: Commit**

```bash
git add src/adsb/sync.rs
git commit -m "Replace AircraftModel with type-aware AircraftModelRegistry"
```

---

### Task 4: Update AdsbPlugin to Use New Function Name

**Files:**
- Modify: `src/adsb/mod.rs:17` (rename function reference)

**Step 1: Update the function name in AdsbPlugin**

In `src/adsb/mod.rs`, line 17, change:
```rust
                setup_aircraft_model,
```
To:
```rust
                setup_aircraft_models,
```

**Step 2: Build and run to verify**

```bash
cargo build
```

Expected: Compiles with 0 errors.

**Step 3: Run the application to visually verify**

```bash
cargo run
```

Expected: Application starts. Aircraft appear. B737-type aircraft (if present in ADS-B feed) should render with the B737 OBJ model. Other aircraft use the generic glb model.

**Step 4: Commit**

```bash
git add src/adsb/mod.rs
git commit -m "Wire up setup_aircraft_models in AdsbPlugin"
```

---

### Task 5: Verify and Adjust Model Scale/Orientation

**Files:**
- Possibly modify: `src/main.rs:91` (AIRCRAFT_MODEL_SCALE constant)
- Possibly modify: `src/adsb/sync.rs` (model-specific scale)

**Step 1: Run and observe the B737 model in 3D view**

```bash
RUST_LOG=airjedi_bevy=debug cargo run
```

Switch to 3D view. Look for B737-type aircraft. Check:
- Is the model visible?
- Is it the right size relative to other aircraft?
- Is the orientation correct (nose forward along heading)?

**Step 2: Adjust if needed**

The OBJ model may have different scale/orientation than the generic GLB. If adjustment is needed, consider:
- Adding a per-model scale factor to the registry
- Applying a rotation correction transform

This step is iterative — adjust, rebuild, check. Document any scale/rotation values found.

**Step 3: Commit any adjustments**

```bash
git add -A
git commit -m "Tune B737 model scale and orientation"
```
