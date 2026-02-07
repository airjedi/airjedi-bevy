use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::aircraft::{AircraftListState, DetailPanelState, CameraFollowState, StatsPanelState};
use crate::config::{AppConfig, SettingsUiState};
use crate::ui_panels::{UiPanelManager, PanelId};
use crate::{MapState, ZoomState, Aircraft};

/// Resource for help overlay visibility
#[derive(Resource, Default)]
pub struct HelpOverlayState {
    pub visible: bool,
}

/// Component for help overlay UI
#[derive(Component)]
pub struct HelpOverlay;

/// System to handle keyboard shortcuts.
///
/// Panel toggles go through UiPanelManager; non-panel actions (zoom, follow,
/// center, reset) are handled directly.
pub fn handle_keyboard_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panels: ResMut<UiPanelManager>,
    mut list_state: ResMut<AircraftListState>,
    mut follow_state: ResMut<CameraFollowState>,
    mut zoom_state: ResMut<ZoomState>,
    mut map_state: ResMut<MapState>,
    app_config: Res<AppConfig>,
    aircraft_query: Query<&Aircraft>,
    mut contexts: EguiContexts,
) {
    // Check if egui wants keyboard input (e.g., typing in a text field)
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() {
            return;
        }
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    // L - Toggle aircraft list
    if keyboard.just_pressed(KeyCode::KeyL) {
        panels.toggle_panel(PanelId::AircraftList);
    }

    // D - Toggle detail panel (if aircraft selected) / Shift+D - Data sources
    if keyboard.just_pressed(KeyCode::KeyD) {
        if shift {
            panels.toggle_panel(PanelId::DataSources);
        } else if list_state.selected_icao.is_some() {
            panels.toggle_panel(PanelId::AircraftDetail);
        }
    }

    // S - Toggle statistics panel
    if keyboard.just_pressed(KeyCode::KeyS) {
        panels.toggle_panel(PanelId::Statistics);
    }

    // B - Toggle bookmarks
    if keyboard.just_pressed(KeyCode::KeyB) {
        panels.toggle_panel(PanelId::Bookmarks);
    }

    // M - Toggle measurement mode
    if keyboard.just_pressed(KeyCode::KeyM) {
        panels.toggle_panel(PanelId::Measurement);
    }

    // E - Toggle export panel
    if keyboard.just_pressed(KeyCode::KeyE) {
        panels.toggle_panel(PanelId::Export);
    }

    // V - Toggle coverage / Shift+V - Coverage stats
    if keyboard.just_pressed(KeyCode::KeyV) {
        if shift {
            // Toggle coverage stats panel visibility handled in coverage module
            // via its own resource (show_stats), keep that behaviour
        } else {
            panels.toggle_panel(PanelId::Coverage);
        }
    }

    // A - Toggle airports / Shift+A - Airspace
    if keyboard.just_pressed(KeyCode::KeyA) {
        if shift {
            panels.toggle_panel(PanelId::Airspace);
        }
        // Non-shift A (airports toggle) is handled by toggle_overlays_keyboard
    }

    // 3 - Toggle 3D view panel
    if keyboard.just_pressed(KeyCode::Digit3) {
        panels.toggle_panel(PanelId::View3D);
    }

    // H - Toggle help overlay
    if keyboard.just_pressed(KeyCode::KeyH) {
        panels.toggle_panel(PanelId::Help);
    }

    // Ctrl+R - Toggle recording
    if ctrl && keyboard.just_pressed(KeyCode::KeyR) {
        panels.toggle_panel(PanelId::Recording);
    }

    // Escape - Deselect aircraft, cancel follow, close panels (cascading)
    if keyboard.just_pressed(KeyCode::Escape) {
        if follow_state.following_icao.is_some() {
            follow_state.following_icao = None;
        } else if list_state.selected_icao.is_some() {
            list_state.selected_icao = None;
            panels.close_panel(PanelId::AircraftDetail);
        } else if panels.is_open(PanelId::Settings) {
            panels.close_panel(PanelId::Settings);
        } else if panels.is_open(PanelId::Help) {
            panels.close_panel(PanelId::Help);
        }
    }

    // F - Follow selected aircraft
    if keyboard.just_pressed(KeyCode::KeyF) {
        if let Some(ref icao) = list_state.selected_icao {
            if follow_state.following_icao.as_ref() == Some(icao) {
                follow_state.following_icao = None;
            } else {
                follow_state.following_icao = Some(icao.clone());
            }
        }
    }

    // C - Center on selected aircraft (one-time center, not follow)
    if keyboard.just_pressed(KeyCode::KeyC) {
        if let Some(ref icao) = list_state.selected_icao {
            if let Some(aircraft) = aircraft_query.iter().find(|a| &a.icao == icao) {
                map_state.latitude = aircraft.latitude;
                map_state.longitude = aircraft.longitude;
            }
        }
    }

    // + or = (same key, shift for +) - Zoom in
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        zoom_state.camera_zoom = (zoom_state.camera_zoom * 1.2)
            .clamp(zoom_state.min_zoom, zoom_state.max_zoom);
    }

    // - (minus) - Zoom out
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        zoom_state.camera_zoom = (zoom_state.camera_zoom / 1.2)
            .clamp(zoom_state.min_zoom, zoom_state.max_zoom);
    }

    // R - Reset view to default (only when Ctrl is NOT pressed, so Ctrl+R goes to recording)
    if keyboard.just_pressed(KeyCode::KeyR) && !ctrl {
        map_state.latitude = app_config.map.default_latitude;
        map_state.longitude = app_config.map.default_longitude;
        zoom_state.camera_zoom = 1.0;
        follow_state.following_icao = None;
    }
}

/// System to toggle overlay settings with keyboard (airports, trails)
pub fn toggle_overlays_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut airport_state: Option<ResMut<crate::aviation::AirportRenderState>>,
    mut trail_config: Option<ResMut<crate::aircraft::TrailConfig>>,
    mut contexts: EguiContexts,
) {
    // Check if egui wants keyboard input
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() {
            return;
        }
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // A - Toggle airports (only without Shift; Shift+A is airspace, handled above)
    if keyboard.just_pressed(KeyCode::KeyA) && !shift {
        if let Some(ref mut state) = airport_state {
            state.show_airports = !state.show_airports;
        }
    }

    // T - Toggle trails
    if keyboard.just_pressed(KeyCode::KeyT) {
        if let Some(ref mut config) = trail_config {
            config.enabled = !config.enabled;
        }
    }
}

/// Sync UiPanelManager state to individual per-module resources.
///
/// This runs after keyboard shortcuts so that the per-module rendering systems
/// see the correct open/closed state each frame.
pub fn sync_panel_manager_to_resources(
    panels: Res<UiPanelManager>,
    mut settings_ui: ResMut<SettingsUiState>,
    mut list_state: ResMut<AircraftListState>,
    mut detail_state: ResMut<DetailPanelState>,
    mut bookmarks_state: ResMut<crate::bookmarks::BookmarksPanelState>,
    mut stats_state: ResMut<StatsPanelState>,
    mut help_state: ResMut<HelpOverlayState>,
    mut export_state: ResMut<crate::export::ExportState>,
    mut coverage_state: ResMut<crate::coverage::CoverageState>,
    mut airspace_state: ResMut<crate::airspace::AirspaceDisplayState>,
    mut datasource_mgr: ResMut<crate::data_sources::DataSourceManager>,
    mut view3d_state: ResMut<crate::view3d::View3DState>,
    mut measurement_state: ResMut<crate::tools::MeasurementState>,
    app_config: Res<AppConfig>,
) {
    if !panels.is_changed() {
        return;
    }

    // Settings - also populate form data when opening
    let settings_open = panels.is_open(PanelId::Settings);
    if settings_open && !settings_ui.open {
        settings_ui.populate_from_config(&app_config);
    }
    settings_ui.open = settings_open;

    list_state.expanded = panels.is_open(PanelId::AircraftList);
    detail_state.open = panels.is_open(PanelId::AircraftDetail);
    bookmarks_state.open = panels.is_open(PanelId::Bookmarks);
    stats_state.expanded = panels.is_open(PanelId::Statistics);
    help_state.visible = panels.is_open(PanelId::Help);
    export_state.panel_open = panels.is_open(PanelId::Export);
    coverage_state.show_stats = panels.is_open(PanelId::Coverage);
    airspace_state.enabled = panels.is_open(PanelId::Airspace);
    datasource_mgr.show_panel = panels.is_open(PanelId::DataSources);
    view3d_state.show_panel = panels.is_open(PanelId::View3D);
    measurement_state.active = panels.is_open(PanelId::Measurement);
}

/// Sync per-module resource changes back to UiPanelManager.
///
/// Some panels can be closed from within their own UI (e.g., clicking an X
/// button). This system detects those changes and updates UiPanelManager so
/// it stays in sync.
pub fn sync_resources_to_panel_manager(
    mut panels: ResMut<UiPanelManager>,
    settings_ui: Res<SettingsUiState>,
    list_state: Res<AircraftListState>,
    detail_state: Res<DetailPanelState>,
    bookmarks_state: Res<crate::bookmarks::BookmarksPanelState>,
    stats_state: Res<StatsPanelState>,
    help_state: Res<HelpOverlayState>,
    export_state: Res<crate::export::ExportState>,
    coverage_state: Res<crate::coverage::CoverageState>,
    airspace_state: Res<crate::airspace::AirspaceDisplayState>,
    datasource_mgr: Res<crate::data_sources::DataSourceManager>,
    view3d_state: Res<crate::view3d::View3DState>,
    measurement_state: Res<crate::tools::MeasurementState>,
) {
    // Only run when any resource actually changed
    let any_changed = settings_ui.is_changed()
        || list_state.is_changed()
        || detail_state.is_changed()
        || bookmarks_state.is_changed()
        || stats_state.is_changed()
        || help_state.is_changed()
        || export_state.is_changed()
        || coverage_state.is_changed()
        || airspace_state.is_changed()
        || datasource_mgr.is_changed()
        || view3d_state.is_changed()
        || measurement_state.is_changed();

    if !any_changed {
        return;
    }

    sync_one(&mut panels, PanelId::Settings, settings_ui.open);
    sync_one(&mut panels, PanelId::AircraftList, list_state.expanded);
    sync_one(&mut panels, PanelId::AircraftDetail, detail_state.open);
    sync_one(&mut panels, PanelId::Bookmarks, bookmarks_state.open);
    sync_one(&mut panels, PanelId::Statistics, stats_state.expanded);
    sync_one(&mut panels, PanelId::Help, help_state.visible);
    sync_one(&mut panels, PanelId::Export, export_state.panel_open);
    sync_one(&mut panels, PanelId::Coverage, coverage_state.show_stats);
    sync_one(&mut panels, PanelId::Airspace, airspace_state.enabled);
    sync_one(&mut panels, PanelId::DataSources, datasource_mgr.show_panel);
    sync_one(&mut panels, PanelId::View3D, view3d_state.show_panel);
    sync_one(&mut panels, PanelId::Measurement, measurement_state.active);
}

fn sync_one(panels: &mut UiPanelManager, id: PanelId, resource_open: bool) {
    if resource_open && !panels.is_open(id) {
        panels.open_panel(id);
    } else if !resource_open && panels.is_open(id) {
        panels.close_panel(id);
    }
}

/// System to create/update help overlay
pub fn update_help_overlay(
    mut commands: Commands,
    help_state: Res<HelpOverlayState>,
    existing_overlay: Query<Entity, With<HelpOverlay>>,
) {
    // Remove existing overlay if not visible
    if !help_state.visible {
        for entity in existing_overlay.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // If overlay already exists, leave it
    if !existing_overlay.is_empty() {
        return;
    }

    // Create help overlay
    let help_text = "\
Keyboard Shortcuts
------------------
L     Toggle aircraft list
D     Toggle detail panel
S     Toggle statistics
B     Toggle bookmarks
M     Measurement mode
E     Export data panel
V     Toggle coverage tracking
3     Toggle 3D view panel
Esc   Deselect / cancel follow
F     Follow selected aircraft
C     Center on selected
+/-   Zoom in / out
H     Toggle this help
R     Reset view
A     Toggle airports
T     Toggle trails
W     Toggle weather overlay

Shift+ Modifiers:
Shift+A  Toggle airspace
Shift+D  Data sources panel
Shift+V  Coverage stats

Ctrl+R  Record/Stop recording
";

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            left: Val::Percent(50.0),
            margin: UiRect {
                left: Val::Px(-150.0),
                top: Val::Px(-180.0),
                ..default()
            },
            width: Val::Px(300.0),
            padding: UiRect::all(Val::Px(20.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 0.95)),
        HelpOverlay,
    )).with_children(|parent| {
        parent.spawn((
            Text::new(help_text),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.9, 0.9)),
        ));
    });
}
