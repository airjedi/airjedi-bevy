# bevy-inspector-egui Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a standalone floating inspector window using `bevy-inspector-egui` that provides live, editable views of ECS entities, resources, and assets at runtime.

**Architecture:** A custom exclusive system renders an `egui::Window` with four collapsible sections (curated app resources, entities, resources, assets). The inspector requires `&mut World` access, so it runs as an exclusive system in the `Update` schedule, separate from the regular dock rendering. Key resource types get `#[derive(Reflect)]` so the inspector can display them.

**Tech Stack:** Bevy 0.18, bevy_egui 0.39, bevy-inspector-egui 0.36, egui_phosphor (icons)

---

### Task 1: Add bevy-inspector-egui dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add the dependency**

Add `bevy-inspector-egui` to the `[dependencies]` section in `Cargo.toml`:

```toml
bevy-inspector-egui = "0.36"
```

Place it after the `bevy_egui` line to keep dependencies grouped logically.

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors (warnings OK)

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Add bevy-inspector-egui dependency"
```

---

### Task 2: Add Reflect derives to MapState and ZoomState

**Files:**
- Modify: `src/map.rs:1-50`

**Context:** `bevy-inspector-egui` requires `#[derive(Reflect)]` on types to inspect them. `MapState` contains a `ZoomLevel` field from the `bevy_slippy_tiles` crate which does not derive `Reflect`, so it must be ignored.

**Step 1: Write the failing test**

Add a test to `src/map.rs` that verifies `MapState` implements `Reflect`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::reflect::Reflect;

    #[test]
    fn map_state_implements_reflect() {
        let state = MapState::default();
        // This compiles only if MapState: Reflect
        let _: &dyn Reflect = &state;
    }

    #[test]
    fn zoom_state_implements_reflect() {
        let state = ZoomState::new();
        let _: &dyn Reflect = &state;
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib map::tests`
Expected: FAIL — "the trait bound `MapState: Reflect` is not satisfied"

**Step 3: Add Reflect derives**

In `src/map.rs`, modify the derive attributes:

For `MapState` (line 7):
```rust
#[derive(Resource, Clone, Reflect)]
pub struct MapState {
    pub latitude: f64,
    pub longitude: f64,
    #[reflect(ignore)]
    pub zoom_level: ZoomLevel,
}
```

For `ZoomState` (line 28):
```rust
#[derive(Resource, Reflect)]
pub struct ZoomState {
    pub camera_zoom: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
}
```

Also add `use bevy::reflect::Reflect;` if not already imported via the prelude (it is — `bevy::prelude::*` includes `Reflect`).

**Step 4: Run test to verify it passes**

Run: `cargo test --lib map::tests`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: all 25 tests pass

**Step 6: Commit**

```bash
git add src/map.rs
git commit -m "Add Reflect derives to MapState and ZoomState"
```

---

### Task 3: Add Reflect derives to View3DState and related enums

**Files:**
- Modify: `src/view3d/mod.rs:45-100`

**Context:** `View3DState` contains `ViewMode` and `TransitionState` enums plus a `Vec2` field (already Reflect). All three types need `Reflect`. `ViewMode` and `TransitionState` are simple enums with no external types.

**Step 1: Add Reflect to ViewMode**

In `src/view3d/mod.rs` (line 47), change:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum ViewMode {
```

**Step 2: Add Reflect to TransitionState**

In `src/view3d/mod.rs` (line 55), change:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Default, Reflect)]
pub enum TransitionState {
```

**Step 3: Add Reflect to View3DState**

In `src/view3d/mod.rs` (line 64), change:
```rust
#[derive(Resource, Reflect)]
pub struct View3DState {
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 5: Run full test suite**

Run: `cargo test`
Expected: all tests pass

**Step 6: Commit**

```bash
git add src/view3d/mod.rs
git commit -m "Add Reflect derives to View3DState and view mode enums"
```

---

### Task 4: Add Reflect derives to DebugPanelState

**Files:**
- Modify: `src/debug_panel.rs:17-47`

**Context:** `DebugPanelState` contains a `VecDeque<String>` which implements `Reflect` in Bevy 0.18. The private rate computation fields can be reflected normally since they're primitive types.

**Step 1: Add Reflect derive**

In `src/debug_panel.rs` (line 18), change:
```rust
#[derive(Resource, Reflect)]
pub struct DebugPanelState {
```

Mark the private fields with `#[reflect(ignore)]` since they're implementation details:
```rust
#[derive(Resource, Reflect)]
pub struct DebugPanelState {
    pub open: bool,
    pub log_messages: VecDeque<String>,
    pub aircraft_count: usize,
    pub messages_processed: u64,
    pub positions_rejected: u64,
    pub message_rate: f64,
    pub fps: f32,
    #[reflect(ignore)]
    last_rate_time: f64,
    #[reflect(ignore)]
    last_rate_count: u64,
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Run full test suite**

Run: `cargo test`
Expected: all tests pass

**Step 4: Commit**

```bash
git add src/debug_panel.rs
git commit -m "Add Reflect derive to DebugPanelState"
```

---

### Task 5: Add PanelId::Inspector to UiPanelManager

**Files:**
- Modify: `src/ui_panels.rs:10-88`
- Modify: `src/ui_panels.rs:129-253` (tests)

**Step 1: Write the failing tests**

Add these tests at the end of the `tests` module in `src/ui_panels.rs`:

```rust
#[test]
fn display_name_inspector() {
    assert_eq!(PanelId::Inspector.display_name(), "Inspector");
}

#[test]
fn shortcut_label_inspector() {
    assert_eq!(PanelId::Inspector.shortcut_label(), "F12");
}

#[test]
fn toggle_inspector_panel() {
    let mut mgr = UiPanelManager::default();
    assert!(!mgr.is_open(PanelId::Inspector));
    mgr.toggle_panel(PanelId::Inspector);
    assert!(mgr.is_open(PanelId::Inspector));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib ui_panels::tests`
Expected: FAIL — "no variant named `Inspector`"

**Step 3: Add PanelId::Inspector variant**

In `src/ui_panels.rs`, add `Inspector` to the `PanelId` enum (line 25, before `Help`):

```rust
pub enum PanelId {
    Settings,
    AircraftList,
    AircraftDetail,
    Bookmarks,
    Statistics,
    Recording,
    Measurement,
    Export,
    Coverage,
    Airspace,
    DataSources,
    View3D,
    Debug,
    Inspector,
    Help,
}
```

Add the match arms in `shortcut_label()`:
```rust
PanelId::Inspector => "F12",
```

Add the match arm in `display_name()`:
```rust
PanelId::Inspector => "Inspector",
```

Add the match arm in `icon()`:
```rust
PanelId::Inspector => "\u{1F50D}",  // magnifying glass
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib ui_panels::tests`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: all tests pass

**Step 6: Commit**

```bash
git add src/ui_panels.rs
git commit -m "Add PanelId::Inspector variant for inspector panel toggle"
```

---

### Task 6: Add keyboard shortcut for inspector toggle

**Files:**
- Modify: `src/keyboard.rs:99-102`

**Step 1: Add F12 shortcut**

In `src/keyboard.rs`, after the backtick debug toggle (line 102), add:

```rust
// F12 - Toggle inspector
if keyboard.just_pressed(KeyCode::F12) {
    panels.toggle_panel(PanelId::Inspector);
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/keyboard.rs
git commit -m "Add F12 keyboard shortcut for inspector toggle"
```

---

### Task 7: Add inspector toggle button to toolbar

**Files:**
- Modify: `src/toolbar.rs:69-71`

**Step 1: Add toolbar button**

In `src/toolbar.rs`, after the Debug button (line 70) and before the Help button (line 71), add the Inspector button:

```rust
toolbar_button(ui, &mut panels, PanelId::Debug, regular::HASH, "Debug (`)", active_color, inactive_color, active_bg);
toolbar_button(ui, &mut panels, PanelId::Inspector, regular::MAGNIFYING_GLASS, "Inspector (F12)", active_color, inactive_color, active_bg);
toolbar_button(ui, &mut panels, PanelId::Help, regular::QUESTION, "Help (H)", active_color, inactive_color, active_bg);
```

Note: `regular::MAGNIFYING_GLASS` is from `egui_phosphor::regular`. If this exact constant name doesn't exist, check with `cargo check` and use the correct constant (may be `MAGNIFYING_GLASS_PLUS` or similar — check egui-phosphor docs).

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/toolbar.rs
git commit -m "Add inspector toggle button to toolbar"
```

---

### Task 8: Create the inspector module with InspectorState and render system

**Files:**
- Create: `src/inspector.rs`
- Modify: `src/main.rs`

**Context:** This is the core task. The inspector window is rendered by an exclusive system that takes `&mut World`. It clones the `EguiContext`, checks `InspectorState.open`, and renders an `egui::Window` with four collapsible sections. The system must run in `Update` (not `EguiPrimaryContextPass`) because it's exclusive.

**Step 1: Create `src/inspector.rs`**

```rust
/// ECS Inspector window using bevy-inspector-egui.
///
/// Provides a floating egui window with live, editable views of
/// entities, resources, and assets. Rendered by an exclusive system
/// that requires &mut World access.

use bevy::prelude::*;
use bevy_egui::EguiContext;
use bevy_inspector_egui::bevy_inspector;

use crate::debug_panel::DebugPanelState;
use crate::map::{MapState, ZoomState};
use crate::ui_panels::{PanelId, UiPanelManager};
use crate::view3d::View3DState;

/// Resource controlling inspector window visibility.
#[derive(Resource, Default)]
pub struct InspectorState {
    pub open: bool,
}

/// Exclusive system that renders the inspector window.
///
/// Must be exclusive because `bevy_inspector` functions require `&mut World`.
/// Runs in `Update` schedule, not `EguiPrimaryContextPass`.
pub fn render_inspector_window(world: &mut World) {
    // Check if inspector should be shown
    let open = world
        .get_resource::<UiPanelManager>()
        .is_some_and(|panels| panels.is_open(PanelId::Inspector));

    if !open {
        return;
    }

    // Clone the egui context so we can release the world borrow
    let mut egui_context = world
        .query_filtered::<&mut EguiContext, With<bevy_egui::PrimaryEguiContext>>()
        .single(world)
        .expect("EguiContext not found")
        .clone();

    let ctx = egui_context.get_mut();

    egui::Window::new("Inspector")
        .default_size([400.0, 500.0])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                // Section 1: Curated app resources (open by default)
                egui::CollapsingHeader::new("App Resources")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("MapState");
                        bevy_inspector::ui_for_resource::<MapState>(world, ui);
                        ui.separator();

                        ui.label("ZoomState");
                        bevy_inspector::ui_for_resource::<ZoomState>(world, ui);
                        ui.separator();

                        ui.label("View3DState");
                        bevy_inspector::ui_for_resource::<View3DState>(world, ui);
                        ui.separator();

                        ui.label("DebugPanelState");
                        bevy_inspector::ui_for_resource::<DebugPanelState>(world, ui);
                    });

                ui.separator();

                // Section 2: All entities
                egui::CollapsingHeader::new("Entities")
                    .default_open(false)
                    .show(ui, |ui| {
                        bevy_inspector::ui_for_entities(world, ui);
                    });

                ui.separator();

                // Section 3: All resources
                egui::CollapsingHeader::new("Resources")
                    .default_open(false)
                    .show(ui, |ui| {
                        bevy_inspector::ui_for_resources(world, ui);
                    });

                ui.separator();

                // Section 4: All assets
                egui::CollapsingHeader::new("Assets")
                    .default_open(false)
                    .show(ui, |ui| {
                        bevy_inspector::ui_for_all_assets(world, ui);
                    });
            });
        });
}
```

**Important implementation note:** The `bevy_inspector::ui_for_resource::<T>()` calls borrow from `world` inside the egui closure. The egui context was cloned before this, so `world` is free to use. However, the closure captures `world` by reference. Since the `egui::Window::show()` callback executes immediately (not deferred), this should work. If lifetime issues arise, the alternative is to call `ui_for_world(world, ui)` instead of individual resource calls.

**Step 2: Register the module and systems in `src/main.rs`**

Add the module declaration (after `mod dock;`, around line 28):
```rust
mod inspector;
```

Add the plugin registration (after `DefaultPlugins` plugins, around line 172):
```rust
bevy_inspector_egui::DefaultInspectorConfigPlugin,
```

Add the resource initialization (after `init_resource::<dock::DockTreeState>()`, around line 185):
```rust
.init_resource::<inspector::InspectorState>()
```

Add the system to the `Update` schedule (after `debug_panel::update_debug_metrics`, around line 213):
```rust
.add_systems(Update, inspector::render_inspector_window)
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

If there are lifetime issues with the `world` borrow inside the egui closure, restructure as noted in the implementation note above.

**Step 4: Run full test suite**

Run: `cargo test`
Expected: all tests pass

**Step 5: Manual smoke test**

Run: `cargo run`
- Press F12 — inspector window should appear
- Verify "App Resources" section shows MapState, ZoomState, View3DState, DebugPanelState
- Verify values are editable (try changing MapState.latitude — map should move)
- Expand "Entities" — should show all entities
- Expand "Resources" — should show all reflected resources
- Expand "Assets" — should show loaded assets
- Press F12 again — inspector should close
- Click the inspector toolbar button — should toggle

**Step 6: Commit**

```bash
git add src/inspector.rs src/main.rs
git commit -m "Add inspector window with curated resources and world browser"
```

---

### Task 9: Add InspectorState sync to keyboard.rs

**Files:**
- Modify: `src/keyboard.rs:201-246`

**Context:** The existing `sync_panel_manager_to_resources` pushes `UiPanelManager` state to per-module resources. Add `InspectorState` sync so the toolbar/keyboard toggle updates `InspectorState.open`.

**Step 1: Add sync for InspectorState**

In `src/keyboard.rs`, in the `sync_panel_manager_to_resources` function signature, add the `InspectorState` parameter:

```rust
pub fn sync_panel_manager_to_resources(
    panels: Res<UiPanelManager>,
    // ... existing params ...
    mut inspector_state: ResMut<crate::inspector::InspectorState>,
    // ... existing params ...
) {
```

Inside the function body, after the debug state sync (line 245), add:

```rust
let v = panels.is_open(PanelId::Inspector);
if inspector_state.open != v { inspector_state.open = v; }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Run full test suite**

Run: `cargo test`
Expected: all tests pass

**Step 4: Commit**

```bash
git add src/keyboard.rs
git commit -m "Sync InspectorState with UiPanelManager"
```

---

### Task 10: Update help overlay with inspector shortcut

**Files:**
- Modify: `src/keyboard.rs:300-328`

**Step 1: Add inspector shortcut to help text**

In `src/keyboard.rs`, in the `update_help_overlay` function, add the inspector shortcut to the `help_text` string (around line 316, after the debug panel line):

```
F12   Toggle inspector
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/keyboard.rs
git commit -m "Add inspector shortcut to help overlay"
```

---

### Task 11: Final integration test and cleanup

**Files:**
- All modified files

**Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass

**Step 2: Run clippy**

Run: `cargo clippy -- -W clippy::all`
Expected: no new warnings from our changes

**Step 3: Full manual test**

Run: `cargo run`

Test checklist:
- [ ] F12 toggles inspector window
- [ ] Toolbar button toggles inspector window
- [ ] Inspector shows "App Resources" section with MapState, ZoomState, View3DState, DebugPanelState
- [ ] Values are editable in the inspector
- [ ] "Entities" section expands and shows entity list
- [ ] "Resources" section expands and shows all reflected resources
- [ ] "Assets" section expands and shows asset types
- [ ] Inspector window is resizable and collapsible
- [ ] Debug dock pane still works independently
- [ ] No visual glitches or panics
- [ ] Help overlay shows F12 shortcut

**Step 4: Commit any cleanup**

```bash
git add -A
git commit -m "Final cleanup for bevy-inspector-egui integration"
```
