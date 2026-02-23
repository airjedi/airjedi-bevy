# bevy-inspector-egui Integration Design

## Goal

Add a standalone floating inspector window using `bevy-inspector-egui` to AirJedi.
The inspector provides live, editable views of ECS entities, resources, and assets
at runtime. The existing Debug dock pane (Metrics + Log) remains unchanged.

## Approach

Use `bevy-inspector-egui` v0.36 (compatible with Bevy 0.18 / bevy_egui 0.39) via
a custom exclusive system that renders a floating `egui::Window`. This avoids
refactoring the dock system, which uses regular `SystemParam`-based systems
incompatible with the inspector's `&mut World` requirement.

## Inspector Window Layout

The window contains four collapsible sections:

1. **App Resources** (curated, open by default)
   Individual `ui_for_resource::<T>()` calls for key application resources:
   - `MapState` -- map center and tile zoom level
   - `ZoomState` -- continuous camera zoom
   - `View3DState` -- 3D mode settings
   - `AppConfig` -- user configuration
   - `DebugPanelState` -- debug metrics and log state
   - Additional resources added as needed

2. **Entities** (collapsed by default)
   `bevy_inspector::ui_for_entities(world, ui)` -- browse all entities and
   their components.

3. **Resources** (collapsed by default)
   `bevy_inspector::ui_for_resources(world, ui)` -- browse all reflected
   resources in the world.

4. **Assets** (collapsed by default)
   `bevy_inspector::ui_for_all_assets(world, ui)` -- browse all asset types.

All values are fully editable at runtime.

## Toggle Mechanism

- New `InspectorState` resource with `open: bool` field.
- New `PanelId::Inspector` variant in `UiPanelManager`.
- Toolbar button to toggle visibility.
- Keyboard shortcut (F12 or other unused key).
- Follows existing panel toggle patterns (toolbar + keyboard + UiPanelManager sync).

## Reflect Requirements

`bevy-inspector-egui` requires `#[derive(Reflect)]` on types to inspect them.
Add `Reflect` to these resources:

| Type | File |
|------|------|
| `MapState` | `src/map.rs` |
| `ZoomState` | `src/map.rs` |
| `View3DState` | `src/view3d/mod.rs` |
| `DebugPanelState` | `src/debug_panel.rs` |
| `AppConfig` (and sub-structs) | `src/config.rs` |
| `AircraftListState` | `src/aircraft/` |
| `CameraFollowState` | `src/aircraft/` |

Fields containing types from external crates that do not derive `Reflect`
(e.g., `ZoomLevel` from `bevy_slippy_tiles`) must use `#[reflect(ignore)]`.

## System Architecture

```
render_inspector_window (exclusive system, Update schedule)
  1. Check InspectorState.open -- early return if closed
  2. Clone EguiContext from world
  3. Render egui::Window with four collapsible sections
  4. Each section calls bevy_inspector:: functions with &mut World
```

The system runs in `Update`, not `EguiPrimaryContextPass`, because exclusive
systems cannot run in the egui pass alongside regular systems. The egui context
accumulates draw commands across schedules, so the inspector content composites
correctly on top of the dock UI.

## Plugin Setup

Register `DefaultInspectorConfigPlugin` in the app. This provides the default
`InspectorEguiImpl` registrations needed for reflection-based UI rendering.
Do NOT add `WorldInspectorPlugin` -- the custom exclusive system provides
equivalent functionality with toggle control.

## Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Add `bevy-inspector-egui = "0.36"` |
| `src/main.rs` | Register plugin, init InspectorState, add system |
| `src/inspector.rs` | New: InspectorState resource, render_inspector_window system |
| `src/map.rs` | Add `#[derive(Reflect)]` to MapState, ZoomState |
| `src/view3d/mod.rs` | Add `#[derive(Reflect)]` to View3DState |
| `src/debug_panel.rs` | Add `#[derive(Reflect)]` to DebugPanelState |
| `src/config.rs` | Add `#[derive(Reflect)]` to AppConfig and sub-structs |
| `src/ui_panels.rs` | Add `PanelId::Inspector` variant |
| `src/toolbar.rs` | Add inspector toggle button |
| `src/keyboard.rs` | Add keyboard shortcut binding |

## Decisions

- **Standalone window over dock integration**: The dock system uses regular
  `SystemParam`-based rendering incompatible with `&mut World`. Converting to
  an exclusive system would require refactoring all dock pane rendering. The
  standalone window avoids this complexity while providing the same inspector
  functionality.

- **Curated resources tab**: A curated section with the most-used app resources
  appears first and open by default, so common debugging tasks don't require
  scrolling through the full world browser.

- **Fully editable**: All reflected values are editable. This is the primary
  value of bevy-inspector-egui -- live tweaking of game state at runtime.
