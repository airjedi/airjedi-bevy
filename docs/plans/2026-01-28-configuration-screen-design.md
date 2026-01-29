# Configuration Screen Design

## Overview

Add a configuration screen to AirJedi for managing application settings, starting with feed endpoint URL, refresh rate, and map defaults. Settings persist to a TOML config file.

## Decisions

| Decision | Choice |
|----------|--------|
| Persistence | Config file (TOML) |
| File format | TOML |
| UI library | bevy_egui |
| Access method | Keyboard shortcut (Esc) + settings button |
| Panel style | Left side panel |
| Initial parameters | Endpoint URL, refresh rate, map defaults |

## Data Model

```rust
#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub feed: FeedConfig,
    pub map: MapConfig,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FeedConfig {
    pub endpoint_url: String,
    pub refresh_interval_ms: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MapConfig {
    pub default_latitude: f64,
    pub default_longitude: f64,
    pub default_zoom: u8,
}
```

### Default Config File (config.toml)

```toml
[feed]
endpoint_url = "http://192.168.1.63:8080/aircraft.json"
refresh_interval_ms = 1000

[map]
default_latitude = 51.5074
default_longitude = -0.1278
default_zoom = 10
```

File location: Application working directory. Created with defaults if missing on startup.

## UI State

```rust
#[derive(Resource, Default)]
pub struct SettingsUiState {
    pub open: bool,
    // Temporary edit buffers (strings for text input)
    pub endpoint_url: String,
    pub refresh_interval_ms: String,
    pub default_latitude: String,
    pub default_longitude: String,
    pub default_zoom: String,
}
```

### Panel Layout

1. Header: "Settings" with close button (X)
2. Feed section: endpoint URL text field, refresh interval field
3. Map section: latitude, longitude, zoom fields
4. Footer: Cancel and Save buttons

Panel width: ~300px fixed, using `egui::SidePanel::left()`.

### Panel Behavior

- Toggle via Esc key or settings button
- On open: populate buffers from current AppConfig
- Save: validate, write to AppConfig resource and config.toml
- Cancel: discard changes, close panel
- Invalid inputs show inline error messages

## Bevy Systems

### New Systems

1. **load_config** (Startup)
   - Read config.toml or create default
   - Insert AppConfig as resource
   - Initialize SettingsUiState resource

2. **toggle_settings_panel** (Update)
   - Listen for Esc key press
   - Toggle SettingsUiState.open
   - Copy AppConfig values to edit buffers on open

3. **render_settings_panel** (EguiPrimaryContextPass)
   - Render left side panel when open
   - Handle all egui widgets
   - Process Save/Cancel clicks

4. **save_config** (helper function)
   - Validate all fields
   - Update AppConfig resource
   - Write config.toml to disk
   - Close panel on success

### Settings Button

Add gear icon button to existing UI overlay (near cache clear button). Toggles panel on click.

### Input Absorption

Use `EguiContexts::ctx_mut().wants_keyboard_input()` to prevent map pan/zoom while typing.

## Dependencies

Add to Cargo.toml:

```toml
bevy_egui = "0.33"
toml = "0.8"
```

## File Structure

Create `src/config.rs` containing:
- AppConfig, FeedConfig, MapConfig structs
- SettingsUiState struct
- load_config() startup system
- toggle_settings_panel() update system
- render_settings_panel() egui system
- save_config_to_file() helper
- validate_config() helper

Wrap in ConfigPlugin for clean main.rs integration:

```rust
app.add_plugins(ConfigPlugin);
```

## Runtime Behavior

### How Config Affects the App

1. **Endpoint URL**: Modify ADS-B polling to read from Res<AppConfig>. Changes take effect on next poll cycle.

2. **Refresh interval**: Configure Timer resource from AppConfig.feed.refresh_interval_ms. On save, reset timer with new duration.

3. **Map defaults**: Used on startup for initial MapState. Optional "Reset to defaults" button can re-center map.

### Validation Rules

| Field | Validation |
|-------|------------|
| Endpoint URL | Must start with http:// or https:// |
| Refresh interval | 100-60000 ms |
| Latitude | -90 to 90 |
| Longitude | -180 to 180 |
| Zoom | 0 to 19 |
