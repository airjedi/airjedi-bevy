# Statusbar Design

## Summary

Add a bottom statusbar to the main window using an egui `TopBottomPanel::bottom()`. The statusbar consolidates quick-glance statistics and status indicators into a single compact bar, replacing the floating recording indicator overlay and map attribution overlay.

## Layout

A thin (~22px) horizontal bar at the bottom of the window, rendered before the CentralPanel in the `EguiPrimaryContextPass` schedule.

```
[*] Connected  |  42 aircraft  |  128 msg/s  |  24.3 FPS  |  [REC]  |          37.6872, -97.3301  Z10  (c) OSM
```

Left-aligned status sections separated by `|` dividers. Map position and attribution right-aligned.

## Sections

1. **Connection status** -- colored dot (green=connected, yellow=connecting, red=disconnected/error) + short text label
2. **Aircraft count** -- number of currently tracked aircraft
3. **Message rate** -- ADS-B messages per second (from `StatsPanelState`)
4. **FPS** -- frames per second, smoothed over ~1 second using exponential moving average
5. **Recording indicator** -- red "REC" text when recording is active, hidden otherwise. Replaces the current floating overlay in `recording/recorder.rs`
6. **Map position + zoom** (right-aligned) -- current center lat/lon and tile zoom level
7. **Map attribution** (right-aligned, after position) -- condensed "(c) OSM, CartoDB" text. Replaces the current floating overlay in `toolbar.rs`

## Styling

- Background: `theme.bg_secondary()` with subtle top border using `theme.bg_contrast()`
- Label text: `theme.text_dim()` for static labels
- Value text: `theme.text_primary()` for dynamic values
- Connection dot: same color logic as toolbar indicator (green/yellow/red via theme success/warn/error colors)
- Recording: `theme.text_error()` color when active
- Font size: 11px for all text (compact)

## Architecture

### New files
- `src/statusbar.rs` -- new module containing:
  - `StatusBarState` resource: holds FPS smoothing state (EMA of frame times)
  - `render_statusbar` system: renders the egui BottomPanel

### Modified files
- `src/main.rs`: add `mod statusbar`, register `StatusBarState` resource, add `render_statusbar` to `EguiPrimaryContextPass` (before `render_dock_tree`)
- `src/recording/recorder.rs`: remove `render_recording_indicator` floating overlay system
- `src/recording/mod.rs`: remove `render_recording_indicator` from plugin systems
- `src/toolbar.rs`: remove `render_map_attribution` function
- `src/main.rs`: remove `render_map_attribution` from system schedule

### System parameters for `render_statusbar`
- `EguiContexts` -- egui context
- `Res<AppTheme>` -- theme colors
- `Option<Res<AdsbAircraftData>>` -- connection state + aircraft count
- `Res<StatsPanelState>` -- message rate
- `Res<RecordingState>` -- recording active flag
- `Res<MapState>` -- lat/lon/zoom
- `Res<Time>` -- frame delta for FPS calculation
- `ResMut<StatusBarState>` -- FPS smoothing state

### Rendering order
The egui BottomPanel must be added before the CentralPanel (egui requirement). The system ordering in `EguiPrimaryContextPass` should be:
1. `apply_egui_theme`
2. `render_toolbar` (SidePanel::left)
3. `render_statusbar` (TopBottomPanel::bottom) -- NEW
4. `render_dock_tree` (CentralPanel)

The `render_map_attribution` system is removed entirely since attribution moves into the statusbar.
