# UI Consolidation Review

## Inventory of All UI Elements

18+ distinct UI elements rendered across the application, in three rendering categories:

### A. Bevy Native UI (spawned as entities in `setup_ui` in `main.rs:565-672`)
1. **Map Attribution Text** -- bottom-right, absolute positioned
2. **Controls Instructions Text** -- top-left, absolute positioned
3. **"Clear Cache" Button** -- top-left at y=50px, absolute positioned
4. **"Settings" Button** -- top-left at y=100px, absolute positioned
5. **"Aircraft (L)" Button** -- top-left at y=150px, absolute positioned
6. **ADS-B Connection Status Text** -- top-right, absolute positioned
7. **Help Overlay** (keyboard.rs:207-233) -- center screen, toggled with H key
8. **Emergency Alert Banner** (emergency.rs:181-204) -- top-center at y=40px

### B. egui Panels (rendered in `EguiPrimaryContextPass`)
9. **Settings Panel** (config.rs:340-436) -- `egui::SidePanel::left`, 300px wide
10. **Aircraft List Panel** (list_panel.rs:280-552) -- `egui::SidePanel::right`, 304px wide
11. **Aircraft Detail Panel** (detail_panel.rs:110-302) -- `egui::Window`, anchored RIGHT_BOTTOM offset -320px
12. **Statistics Panel** (stats_panel.rs:115-238) -- `egui::Window`, anchored LEFT_BOTTOM
13. **Bookmarks Panel** (bookmarks/mod.rs:65-108) -- `egui::Window`, anchored LEFT_TOP at y=150
14. **Recording Panel** (recorder.rs:254-356) -- `egui::Window`, anchored RIGHT_TOP + recording indicator via `egui::Area`
15. **Coverage Statistics Panel** (coverage/mod.rs:287-353) -- `egui::Window`, default position
16. **Airspace Panel** (airspace/mod.rs:337-375) -- `egui::Window`, default position
17. **Data Sources Panel** (data_sources/mod.rs:273-346) -- `egui::Window`, default position
18. **Export Data Panel** (export/mod.rs:307-391) -- `egui::Window`, default position
19. **3D View Panel** (view3d/mod.rs:196-271) -- `egui::Window`, default position
20. **Measurement Tooltip** (measurement.rs:336-385) -- `egui::Area`, fixed position near cursor + mode indicator

---

## Issues Identified

### 1. Mixed UI Systems (Bevy native vs egui)
The most significant issue. The left-side buttons (Clear Cache, Settings, Aircraft List) are Bevy native UI entities with absolute pixel positioning, while the panels they toggle are egui panels. This creates:
- **Inconsistent styling** -- Bevy buttons use hardcoded RGBA colors; egui panels use their own theming
- **Overlap conflicts** -- The Bevy buttons at fixed y=50/100/150px overlap with the egui Settings SidePanel when it opens (both occupy the left side)
- **The Bookmarks panel (egui::Window anchored LEFT_TOP at y=150)** directly overlaps the Aircraft List button
- **Duplicated input guarding** -- Each button handler independently checks `ctx.is_pointer_over_area()` or `ui_state.open` to avoid conflicts

### 2. Scattered Toggle Logic
Panel toggles are spread across three places:
- **Bevy button click handlers** in `main.rs` (lines 1350-1421)
- **Keyboard shortcuts** in `keyboard.rs` (lines 19-127)
- **Individual toggle systems** in each module (e.g., `toggle_bookmarks_panel`, `toggle_aircraft_list`, `toggle_3d_view`, etc.)

Some panels have duplicate toggle handlers. For example, `toggle_aircraft_list` in `list_panel.rs:556-563` handles the L key, but `handle_keyboard_shortcuts` in `keyboard.rs:41-43` also handles L for the same action. The `toggle_settings_panel` in `config.rs:439-450` uses Escape, while `handle_keyboard_shortcuts` in `keyboard.rs:53-63` also handles Escape with cascading logic.

### 3. Inconsistent Panel Types
- Settings uses `egui::SidePanel::left` (takes full height, pushes content)
- Aircraft List uses `egui::SidePanel::right` (takes full height, pushes content)
- Everything else uses `egui::Window` (floating, overlapping, draggable)
- This means Settings and Aircraft List are "hard" panels that resize the content area, while others float freely

### 4. Hardcoded Position Conflicts
- Bookmarks window anchor: `LEFT_TOP, (10, 150)` -- sits directly under the Bevy buttons
- Statistics window anchor: `LEFT_BOTTOM, (10, -10)` -- could overlap bookmarks if both open
- Detail panel anchor: `RIGHT_BOTTOM, (-320, -10)` -- hardcoded offset to avoid aircraft list
- Recording panel anchor: `RIGHT_TOP, (-10, 50)` -- overlaps connection status text

### 5. No Panel Mutual Exclusivity or Stacking
Multiple panels can be open simultaneously with no coordination, leading to overlapping windows. The only mutual exclusivity is implicit (Escape cascading in keyboard.rs).

### 6. Duplicate `haversine_distance_nm` Function
This function is defined identically in three places:
- `detail_panel.rs:39-52`
- `list_panel.rs:110-123`
- `coverage/mod.rs:107-123` (as `calculate_range_nm`)

---

## Recommendations

### Recommendation 1: Consolidate Left-Side Buttons into an egui Toolbar
Replace the three Bevy native buttons (Clear Cache, Settings, Aircraft List) with a single egui side toolbar. This eliminates the mixed UI systems problem and the overlap with egui panels.

**Proposed approach:** Create a narrow `egui::SidePanel::left` (about 40-50px wide) containing icon-style toggle buttons for all features. This acts as a permanent toolbar strip. When a panel is activated, it opens as a secondary panel or window to the right of the toolbar.

Buttons to include in the toolbar:
- Settings (gear icon)
- Aircraft List (airplane icon)
- Bookmarks (star icon)
- Statistics (chart icon)
- Recording (record/circle icon)
- Measurement (ruler icon)
- Export (download icon)
- Coverage (radar icon)
- Airspace (layers icon)
- Data Sources (database icon)
- 3D View (cube icon)
- Clear Cache (trash icon)

This approach is common in map applications (Google Earth, FlightRadar24) and gives a clean, organized feel.

### Recommendation 2: Create a Stacked Right Panel System
Instead of having the aircraft list as a full-height SidePanel and the detail panel as a floating window at RIGHT_BOTTOM, combine them:

- **Upper section:** Aircraft list (scrollable, takes most of the space)
- **Lower section:** Detail panel for the selected aircraft (collapsible)

This eliminates the hardcoded `-320px` X offset on the detail panel and keeps related aircraft information together. The `SidePanel::right` already exists for the aircraft list -- just add the detail panel within it when an aircraft is selected.

### Recommendation 3: Group Feature Panels into a Tabbed Window
Many of the floating `egui::Window` panels are infrequently used and overlap chaotically:
- Coverage Statistics
- Airspace Settings
- Data Sources
- Export Data
- 3D View Settings

These could be consolidated into a single "Tools" window with tabs, opened via a toolbar button. This reduces window clutter significantly.

### Recommendation 4: Move Bevy Native UI to egui
Convert the remaining Bevy native UI elements to egui for consistency:
- **Connection Status** -- move to the toolbar or a status bar
- **Map Attribution** -- render as egui overlay at bottom-right
- **Controls Instructions** -- remove entirely (redundant with help overlay)
- **Help Overlay** -- convert to an egui modal/window
- **Emergency Banner** -- convert to an egui top bar notification

This eliminates the entire `setup_ui` function and all the Bevy button interaction handlers (`handle_clear_cache_button`, `handle_settings_button`, `handle_aircraft_list_button`), simplifying the codebase considerably.

### Recommendation 5: Centralize Toggle/Keyboard Logic
Create a single `UiPanelManager` resource that tracks which panels are open and provides methods like `toggle_panel(PanelId)`, `close_all()`, `is_open(PanelId)`. This replaces:
- The scattered `panel_state.open` booleans across ~10 different resources
- The duplicated keyboard handling between `keyboard.rs` and individual module toggle functions
- The ad-hoc mutual exclusivity logic in the Escape handler

### Recommendation 6: Extract Common Utility Functions
Move `haversine_distance_nm` to a shared utility module (e.g., `src/geo.rs` or `src/utils/geo.rs`) to eliminate the three duplicate implementations.

---

## Suggested Layout

```
+--+----------------------------------------------+--------+
|  | [Connection Status] [Emergency Banner]        |        |
|T |                                               | A/C    |
|O |                                               | List   |
|O |                                               |--------|
|L |              MAP VIEWPORT                     | Detail |
|B |                                               | Panel  |
|A |                                               |        |
|R |                                               |        |
|  | [Measurement tooltip]                         |        |
+--+----------------------------------------------+--------+
|  [Map Attribution]                    [Zoom: L12]         |
+-----------------------------------------------------------+
```

- **Left toolbar:** ~40px, always visible, icon buttons
- **Right panel:** Aircraft list + detail, collapsible
- **Top bar:** Status indicators (connection, recording, emergency)
- **Center:** Map viewport (untouched)
- **Feature panels (Tools, Coverage, etc.):** Open as floating windows or tabs within a dedicated panel area

---

## Priority Order
1. **Recommendation 1** (egui toolbar) -- highest impact, eliminates mixed UI systems
2. **Recommendation 4** (migrate remaining Bevy UI) -- removes ~100 lines of boilerplate
3. **Recommendation 2** (stacked right panel) -- better UX for aircraft interaction
4. **Recommendation 5** (centralized panel manager) -- reduces maintenance burden
5. **Recommendation 3** (tabbed tools window) -- nice-to-have for clutter reduction
6. **Recommendation 6** (deduplicate haversine) -- trivial fix
