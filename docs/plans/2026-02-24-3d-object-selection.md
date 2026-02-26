# 3D Object Selection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable clicking aircraft in 3D mode with outline highlight, hover feedback, camera follow, and deselection.

**Architecture:** Bevy 0.18's built-in `MeshPickingPlugin` (part of DefaultPlugins) raycasts against Mesh3d entities. Aircraft get `Pickable` component and observer callbacks. Selected aircraft get a scaled-up clone child rendered with front-face culling as an outline. Camera orbit center lerps to the followed aircraft's world position.

**Tech Stack:** Bevy 0.18 built-in picking (`bevy::picking`), `StandardMaterial` with `cull_mode`, observers (`Pointer<Click>`, `Pointer<Over>`, `Pointer<Out>`)

---

### Task 1: Create picking module with marker components

**Files:**
- Create: `src/aircraft/picking.rs`
- Modify: `src/aircraft/mod.rs:1-19`

**Step 1: Create `src/aircraft/picking.rs` with marker components and resources**

```rust
use bevy::prelude::*;

/// Marker for the outline entity spawned on the selected aircraft.
#[derive(Component)]
pub struct SelectionOutline;

/// Marker for the outline entity spawned on hover.
#[derive(Component)]
pub struct HoverOutline;

/// Resource holding the flat-color material used for selection outlines.
#[derive(Resource)]
pub struct OutlineMaterials {
    pub selection: Handle<StandardMaterial>,
    pub hover: Handle<StandardMaterial>,
}
```

**Step 2: Add module declaration to `src/aircraft/mod.rs`**

Add `pub mod picking;` after line 9 (`pub mod stats_panel;`) and add to the re-exports:

```rust
pub mod picking;

pub use picking::{SelectionOutline, HoverOutline};
```

**Step 3: Build and verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Successful compilation (warnings are OK).

**Step 4: Commit**

```
git add src/aircraft/picking.rs src/aircraft/mod.rs
git commit -m "Add picking module with outline marker components"
```

---

### Task 2: Create outline materials at startup

**Files:**
- Modify: `src/aircraft/picking.rs`

**Step 1: Add setup system that creates outline materials**

Add to `src/aircraft/picking.rs`:

```rust
use bevy::render::mesh::MeshCullMode;

/// Startup system: create shared outline materials.
pub fn setup_outline_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let selection_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 1.0, 1.0, 1.0), // cyan
        unlit: true,
        cull_mode: Some(MeshCullMode::Front), // render back faces only = outline
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });

    let hover_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.6, 0.8, 0.7), // dimmer cyan
        unlit: true,
        cull_mode: Some(MeshCullMode::Front),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.insert_resource(OutlineMaterials {
        selection: selection_mat,
        hover: hover_mat,
    });
}
```

**Step 2: Register the startup system in `src/aircraft/plugin.rs`**

Add to the `AircraftPlugin::build` method, in the startup systems:

```rust
.add_systems(Startup, super::picking::setup_outline_materials)
```

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles successfully.

**Step 4: Commit**

```
git add src/aircraft/picking.rs src/aircraft/plugin.rs
git commit -m "Add outline material creation at startup"
```

---

### Task 3: Add Pickable component to spawned aircraft

**Files:**
- Modify: `src/adsb/sync.rs:86-106`

**Step 1: Add `Pickable` to the aircraft spawn bundle**

In `src/adsb/sync.rs`, modify the `commands.spawn((...))` block at line 88-106. Add `bevy::picking::Pickable::default()` to the tuple:

```rust
let aircraft_entity = commands
    .spawn((
        Name::new(format!("Aircraft: {}", aircraft_name)),
        SceneRoot(aircraft_model.handle.clone()),
        Transform::from_xyz(0.0, 0.0, constants::AIRCRAFT_Z_LAYER),
        Aircraft {
            icao: adsb_ac.icao.clone(),
            callsign: adsb_ac.callsign.clone(),
            latitude: lat,
            longitude: lon,
            altitude: adsb_ac.altitude,
            heading: adsb_ac.track.map(|t| t as f32),
            velocity: adsb_ac.velocity,
            vertical_rate: adsb_ac.vertical_rate,
            squawk: None,
        },
        TrailHistory::default(),
        Pickable::default(),
    ))
    .id();
```

**Step 2: Add `use bevy::picking::Pickable;` to the imports at the top of `sync.rs`**

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles. Note: `Pickable` is re-exported from `bevy::prelude` so the existing import may already cover it. If so, skip the explicit import.

**Step 4: Commit**

```
git add src/adsb/sync.rs
git commit -m "Add Pickable component to spawned aircraft entities"
```

---

### Task 4: Implement click observer for aircraft selection

**Files:**
- Modify: `src/aircraft/picking.rs`
- Modify: `src/adsb/sync.rs:88-106`

**Step 1: Add the click observer handler to `picking.rs`**

```rust
use crate::Aircraft;
use crate::aircraft::AircraftListState;
use crate::adsb::sync::AircraftModel;

/// Observer: when an aircraft is clicked, select it.
pub fn on_aircraft_click(
    trigger: On<Pointer<Click>>,
    aircraft_query: Query<&Aircraft>,
    mut list_state: ResMut<AircraftListState>,
) {
    let entity = trigger.target();
    // The click may hit a child mesh, so also check the parent
    if let Ok(aircraft) = aircraft_query.get(entity) {
        list_state.selected_icao = Some(aircraft.icao.clone());
    }
}
```

**Step 2: Attach the observer when spawning aircraft in `sync.rs`**

After the `.spawn((...))` call at line 88, chain `.observe(crate::aircraft::picking::on_aircraft_click)`:

```rust
let aircraft_entity = commands
    .spawn((
        // ... existing components + Pickable ...
    ))
    .observe(crate::aircraft::picking::on_aircraft_click)
    .id();
```

Note: Bevy's picking system propagates pointer events up the entity hierarchy, so clicking a child mesh of the `SceneRoot` will bubble up to the aircraft entity that has the `Aircraft` component.

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles successfully.

**Step 4: Run and test manually**

Run: `cargo run`
- Enter 3D mode (press `3`)
- Click on an aircraft
- Expected: The aircraft list panel should show it as selected (same as 2D click)

**Step 5: Commit**

```
git add src/aircraft/picking.rs src/adsb/sync.rs
git commit -m "Add click observer for aircraft selection in 3D mode"
```

---

### Task 5: Implement hover observer

**Files:**
- Modify: `src/aircraft/picking.rs`
- Modify: `src/adsb/sync.rs`

**Step 1: Add hover/out observer handlers to `picking.rs`**

```rust
/// Observer: when pointer enters an aircraft, spawn a hover outline.
pub fn on_aircraft_hover(
    trigger: On<Pointer<Over>>,
    mut commands: Commands,
    outline_materials: Res<OutlineMaterials>,
    aircraft_model: Res<AircraftModel>,
    aircraft_query: Query<&Aircraft>,
    existing_hover: Query<Entity, With<HoverOutline>>,
) {
    let entity = trigger.target();
    if aircraft_query.get(entity).is_err() {
        return;
    }

    // Remove any existing hover outline
    for hover_entity in existing_hover.iter() {
        commands.entity(hover_entity).despawn();
    }

    // Spawn hover outline as child of the aircraft
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Name::new("Hover Outline"),
            SceneRoot(aircraft_model.handle.clone()),
            Transform::from_scale(Vec3::splat(1.03)),
            HoverOutline,
        ));
    });
}

/// Observer: when pointer leaves an aircraft, remove hover outline.
pub fn on_aircraft_out(
    trigger: On<Pointer<Out>>,
    mut commands: Commands,
    children_query: Query<&Children>,
    hover_query: Query<Entity, With<HoverOutline>>,
) {
    let entity = trigger.target();
    if let Ok(children) = children_query.get(entity) {
        for &child in children.iter() {
            if hover_query.get(child).is_ok() {
                commands.entity(child).despawn();
            }
        }
    }
}
```

**Step 2: Attach observers in `sync.rs`**

Chain after the click observer:

```rust
.observe(crate::aircraft::picking::on_aircraft_click)
.observe(crate::aircraft::picking::on_aircraft_hover)
.observe(crate::aircraft::picking::on_aircraft_out)
```

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles.

**Step 4: Commit**

```
git add src/aircraft/picking.rs src/adsb/sync.rs
git commit -m "Add hover and out observers for aircraft outline preview"
```

---

### Task 6: Implement selection outline spawning/despawning

**Files:**
- Modify: `src/aircraft/picking.rs`

**Step 1: Add a system that manages selection outlines based on `AircraftListState`**

```rust
/// System: spawn/despawn selection outline when selected_icao changes.
pub fn manage_selection_outline(
    mut commands: Commands,
    list_state: Res<AircraftListState>,
    aircraft_query: Query<(Entity, &Aircraft)>,
    outline_materials: Res<OutlineMaterials>,
    aircraft_model: Res<AircraftModel>,
    existing_outlines: Query<(Entity, &ChildOf), With<SelectionOutline>>,
) {
    if !list_state.is_changed() {
        return;
    }

    // Despawn all existing selection outlines
    for (outline_entity, _) in existing_outlines.iter() {
        commands.entity(outline_entity).despawn();
    }

    // Spawn new outline if an aircraft is selected
    let Some(ref selected_icao) = list_state.selected_icao else {
        return;
    };

    let Some((aircraft_entity, _)) = aircraft_query
        .iter()
        .find(|(_, ac)| &ac.icao == selected_icao)
    else {
        return;
    };

    commands.entity(aircraft_entity).with_children(|parent| {
        parent.spawn((
            Name::new("Selection Outline"),
            SceneRoot(aircraft_model.handle.clone()),
            Transform::from_scale(Vec3::splat(1.05)),
            SelectionOutline,
        ));
    });
}
```

**Step 2: Register in `src/aircraft/plugin.rs`**

Add to Update systems:

```rust
super::picking::manage_selection_outline,
```

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles.

**Step 4: Commit**

```
git add src/aircraft/picking.rs src/aircraft/plugin.rs
git commit -m "Add selection outline spawn/despawn system"
```

---

### Task 7: Implement outline material swap system

**Files:**
- Modify: `src/aircraft/picking.rs`

The outline clone spawns with the original GLB materials. We need a system that detects outline entities and replaces their child mesh materials with the flat outline material.

**Step 1: Add material swap system**

```rust
/// System: replace materials on outline entity children with flat outline material.
/// Runs every frame to catch newly-loaded scene children (SceneRoot loads asynchronously).
pub fn swap_outline_materials(
    outline_materials: Option<Res<OutlineMaterials>>,
    selection_query: Query<&Children, With<SelectionOutline>>,
    hover_query: Query<&Children, With<HoverOutline>>,
    children_query: Query<&Children>,
    mesh_query: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(outline_mats) = outline_materials else {
        return;
    };

    // Swap materials for selection outlines
    for children in selection_query.iter() {
        swap_materials_recursive(
            children,
            &children_query,
            &mesh_query,
            &mut materials,
            &outline_mats.selection,
        );
    }

    // Swap materials for hover outlines
    for children in hover_query.iter() {
        swap_materials_recursive(
            children,
            &children_query,
            &mesh_query,
            &mut materials,
            &outline_mats.hover,
        );
    }
}

fn swap_materials_recursive(
    children: &Children,
    children_query: &Query<&Children>,
    mesh_query: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
    target_handle: &Handle<StandardMaterial>,
) {
    let target_mat = materials.get(target_handle).cloned();
    let Some(target_mat) = target_mat else { return; };

    for child in children.iter() {
        if let Ok(mat_handle) = mesh_query.get(child) {
            // Check if this material is already the outline material
            let needs_swap = materials
                .get(mat_handle.id())
                .is_some_and(|m| m.cull_mode != target_mat.cull_mode);
            if needs_swap {
                if let Some(material) = materials.get_mut(mat_handle.id()) {
                    material.base_color = target_mat.base_color;
                    material.unlit = target_mat.unlit;
                    material.cull_mode = target_mat.cull_mode;
                    material.alpha_mode = target_mat.alpha_mode.clone();
                    material.emissive = LinearRgba::NONE;
                }
            }
        }
        if let Ok(grandchildren) = children_query.get(child) {
            swap_materials_recursive(grandchildren, children_query, mesh_query, materials, target_handle);
        }
    }
}
```

**Important consideration:** The outline clone shares material handles with the original aircraft. Mutating the material in-place would also change the original. Instead, we should clone the material handle for outline children. Let me revise — we need to replace the `MeshMaterial3d` component with a new handle pointing to our outline material.

**Revised Step 1: Replace material handles instead of mutating**

```rust
/// System: replace material handles on outline entity children with the flat outline material.
pub fn swap_outline_materials(
    mut commands: Commands,
    outline_materials: Option<Res<OutlineMaterials>>,
    selection_query: Query<&Children, With<SelectionOutline>>,
    hover_query: Query<&Children, With<HoverOutline>>,
    children_query: Query<&Children>,
    mesh_query: Query<(Entity, &MeshMaterial3d<StandardMaterial>)>,
    materials: Res<Assets<StandardMaterial>>,
) {
    let Some(outline_mats) = outline_materials else {
        return;
    };

    for children in selection_query.iter() {
        replace_materials_recursive(
            &mut commands,
            children,
            &children_query,
            &mesh_query,
            &materials,
            &outline_mats.selection,
        );
    }

    for children in hover_query.iter() {
        replace_materials_recursive(
            &mut commands,
            children,
            &children_query,
            &mesh_query,
            &materials,
            &outline_mats.hover,
        );
    }
}

fn replace_materials_recursive(
    commands: &mut Commands,
    children: &Children,
    children_query: &Query<&Children>,
    mesh_query: &Query<(Entity, &MeshMaterial3d<StandardMaterial>)>,
    materials: &Res<Assets<StandardMaterial>>,
    target_handle: &Handle<StandardMaterial>,
) {
    for child in children.iter() {
        if let Ok((entity, current_handle)) = mesh_query.get(child) {
            // Skip if already using the outline material
            if current_handle.id() != target_handle.id() {
                commands.entity(entity).insert(MeshMaterial3d(target_handle.clone()));
            }
        }
        if let Ok(grandchildren) = children_query.get(child) {
            replace_materials_recursive(commands, grandchildren, children_query, mesh_query, materials, target_handle);
        }
    }
}
```

**Step 2: Register in `src/aircraft/plugin.rs`**

Add to Update systems:

```rust
super::picking::swap_outline_materials,
```

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles.

**Step 4: Run and test**

Run: `cargo run`
- Enter 3D mode, hover over an aircraft → should see dim cyan outline
- Click an aircraft → should see bright cyan outline
- Expected: Outline renders as visible edges around the aircraft model

**Step 5: Commit**

```
git add src/aircraft/picking.rs src/aircraft/plugin.rs
git commit -m "Add outline material swap system for selection and hover"
```

---

### Task 8: Add ground plane click-to-deselect

**Files:**
- Modify: `src/aircraft/picking.rs`
- Modify: `src/view3d/sky.rs` (find ground plane spawn)

**Step 1: Find the ground plane entity in `sky.rs`**

The ground plane is spawned in `sky::setup_sky`. We need to add `Pickable` and a click observer to it.

Read `src/view3d/sky.rs` to find the `GroundPlane` component and spawn location. Add `Pickable::default()` and an observer.

**Step 2: Add deselect observer to `picking.rs`**

```rust
/// Observer: clicking the ground plane deselects the current aircraft.
pub fn on_ground_click(
    _trigger: On<Pointer<Click>>,
    mut list_state: ResMut<AircraftListState>,
    mut follow_state: ResMut<crate::aircraft::CameraFollowState>,
) {
    list_state.selected_icao = None;
    follow_state.following_icao = None;
}
```

**Step 3: Add `Pickable` and observer to ground plane spawn in `sky.rs`**

Add `Pickable::default()` to the ground plane bundle and chain `.observe(crate::aircraft::picking::on_ground_click)`.

**Step 4: Add ESC key deselect system to `picking.rs`**

```rust
/// System: pressing Escape clears aircraft selection.
pub fn deselect_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut list_state: ResMut<AircraftListState>,
    mut follow_state: ResMut<crate::aircraft::CameraFollowState>,
    view3d_state: Res<crate::view3d::View3DState>,
) {
    if !view3d_state.is_3d_active() {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        list_state.selected_icao = None;
        follow_state.following_icao = None;
    }
}
```

**Step 5: Register `deselect_on_escape` in `src/aircraft/plugin.rs`**

Add to Update systems:

```rust
super::picking::deselect_on_escape,
```

**Step 6: Build and verify**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles.

**Step 7: Run and test**

- Select an aircraft in 3D mode
- Click empty ground → selection clears
- Press ESC → selection clears

**Step 8: Commit**

```
git add src/aircraft/picking.rs src/aircraft/plugin.rs src/view3d/sky.rs
git commit -m "Add ground click and ESC deselect in 3D mode"
```

---

### Task 9: Integrate camera follow in 3D mode

**Files:**
- Modify: `src/aircraft/picking.rs`
- Modify: `src/view3d/mod.rs:475-595` (handle_3d_camera_controls)
- Modify: `src/camera.rs:81-105` (follow_aircraft)

**Step 1: Set follow state on selection**

Modify `on_aircraft_click` in `picking.rs` to also set `CameraFollowState`:

```rust
pub fn on_aircraft_click(
    trigger: On<Pointer<Click>>,
    aircraft_query: Query<&Aircraft>,
    mut list_state: ResMut<AircraftListState>,
    mut follow_state: ResMut<crate::aircraft::CameraFollowState>,
) {
    let entity = trigger.target();
    if let Ok(aircraft) = aircraft_query.get(entity) {
        list_state.selected_icao = Some(aircraft.icao.clone());
        follow_state.following_icao = Some(aircraft.icao.clone());
    }
}
```

**Step 2: Extend 3D camera to follow aircraft orbit center**

In `src/view3d/mod.rs`, modify `update_3d_camera` to lerp the orbit center toward the followed aircraft's world position when in 3D mode.

Add after the `center_yup` calculation (around line 382):

```rust
// If following an aircraft, lerp orbit center toward its 3D position
let center_yup = if let Some(ref following_icao) = follow_state.following_icao {
    if let Some((_, aircraft, ac_transform)) = aircraft_query
        .iter()
        .find(|(_, ac, _)| &ac.icao == *following_icao)
    {
        // Use the aircraft's current Y-up position as orbit center
        let target = ac_transform.translation;
        // Lerp saved_2d_center toward aircraft pixel position for smooth follow
        let lerp_t = (5.0 * time.delta_secs()).min(1.0);
        // Update saved center so pan stays smooth
        state.saved_2d_center = state.saved_2d_center.lerp(
            Vec2::new(target.x, -target.z), // Y-up back to Z-up pixel space
            lerp_t,
        );
        // Recompute center_yup from updated saved_2d_center
        let ground_alt = state.altitude_to_z(state.ground_elevation_ft);
        zup_to_yup(Vec3::new(
            state.saved_2d_center.x,
            state.saved_2d_center.y,
            ground_alt,
        ))
    } else {
        center_yup // aircraft not found, keep current center
    }
} else {
    center_yup
};
```

This requires adding `follow_state`, `aircraft_query`, and `time` parameters to `update_3d_camera`. The function signature will need:

```rust
pub fn update_3d_camera(
    mut state: ResMut<View3DState>,
    follow_state: Res<crate::aircraft::CameraFollowState>,
    aircraft_query: Query<(Entity, &crate::Aircraft, &Transform), Without<crate::MapCamera>>,
    time: Res<Time>,
    // ... existing params ...
)
```

**Step 3: Also update `map_state` so tiles follow**

After updating `saved_2d_center`, call `sync_center_to_map_state` so tiles load around the followed aircraft.

**Step 4: Build and verify**

Run: `cargo build 2>&1 | head -20`

**Step 5: Run and test**

- Select an aircraft in 3D mode
- Camera should smoothly orbit to center on the aircraft
- Aircraft moves → camera follows
- Drag to pan → follow mode breaks

**Step 6: Commit**

```
git add src/aircraft/picking.rs src/view3d/mod.rs
git commit -m "Integrate camera follow for selected aircraft in 3D mode"
```

---

### Task 10: Auto-clear selection when aircraft despawns

**Files:**
- Modify: `src/aircraft/picking.rs`

**Step 1: Add system to detect when the followed aircraft is gone**

```rust
/// System: clear selection if the selected aircraft entity no longer exists.
pub fn clear_stale_selection(
    mut list_state: ResMut<AircraftListState>,
    mut follow_state: ResMut<crate::aircraft::CameraFollowState>,
    aircraft_query: Query<&Aircraft>,
) {
    let Some(ref selected_icao) = list_state.selected_icao else {
        return;
    };

    let still_exists = aircraft_query.iter().any(|ac| &ac.icao == selected_icao);
    if !still_exists {
        list_state.selected_icao = None;
        follow_state.following_icao = None;
    }
}
```

**Step 2: Register in `src/aircraft/plugin.rs`**

```rust
super::picking::clear_stale_selection,
```

**Step 3: Build and verify**

Run: `cargo build 2>&1 | head -20`

**Step 4: Commit**

```
git add src/aircraft/picking.rs src/aircraft/plugin.rs
git commit -m "Auto-clear selection when aircraft despawns"
```

---

### Task 11: Disable 2D click detection in 3D mode

**Files:**
- Modify: `src/aircraft/detail_panel.rs:90-143`

**Step 1: Guard `detect_aircraft_click` to skip in 3D mode**

Add `view3d_state: Res<crate::view3d::View3DState>` parameter and early return:

```rust
pub fn detect_aircraft_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::MapCamera>>,
    aircraft_query: Query<(&crate::Aircraft, &Transform)>,
    mut list_state: ResMut<AircraftListState>,
    zoom_state: Res<ZoomState>,
    view3d_state: Res<crate::view3d::View3DState>,
) {
    // In 3D mode, picking is handled by Bevy's MeshPickingPlugin
    if view3d_state.is_3d_active() {
        return;
    }

    // ... rest unchanged ...
}
```

**Step 2: Build and verify**

Run: `cargo build 2>&1 | head -20`

**Step 3: Commit**

```
git add src/aircraft/detail_panel.rs
git commit -m "Disable 2D click detection when 3D mode is active"
```

---

### Task 12: Final integration test

**Files:** None modified — manual testing.

**Step 1: Full test run**

Run: `cargo run`

Test checklist:
- [ ] 2D mode: click aircraft → selects (existing behavior preserved)
- [ ] Press `3` to enter 3D mode
- [ ] Hover over aircraft → dim cyan outline appears
- [ ] Move pointer away → outline disappears
- [ ] Click aircraft → bright cyan outline, detail panel opens, camera follows
- [ ] Camera smoothly tracks moving aircraft
- [ ] Pan/drag → follow mode breaks
- [ ] Click ground → deselects, outline removed
- [ ] Press ESC → deselects
- [ ] Aircraft goes out of range (ADS-B timeout) → auto-deselects
- [ ] Press `3` to return to 2D → outline cleaned up, 2D click works

**Step 2: Fix any issues found**

**Step 3: Final commit if any fixes needed**
