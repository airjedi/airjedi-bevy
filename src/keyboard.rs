use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::aircraft::{AircraftListState, DetailPanelState, CameraFollowState, StatsPanelState};
use crate::config::{AppConfig, SettingsUiState};
use crate::{MapState, ZoomState, Aircraft};

/// Resource for help overlay visibility
#[derive(Resource, Default)]
pub struct HelpOverlayState {
    pub visible: bool,
}

/// Component for help overlay UI
#[derive(Component)]
pub struct HelpOverlay;

/// System to handle keyboard shortcuts
pub fn handle_keyboard_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut list_state: ResMut<AircraftListState>,
    mut detail_state: ResMut<DetailPanelState>,
    mut follow_state: ResMut<CameraFollowState>,
    mut help_state: ResMut<HelpOverlayState>,
    mut stats_state: ResMut<StatsPanelState>,
    mut zoom_state: ResMut<ZoomState>,
    mut map_state: ResMut<MapState>,
    mut settings_ui: ResMut<SettingsUiState>,
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

    // L - Toggle aircraft list
    if keyboard.just_pressed(KeyCode::KeyL) {
        list_state.expanded = !list_state.expanded;
    }

    // D - Toggle detail panel (if aircraft selected)
    if keyboard.just_pressed(KeyCode::KeyD) {
        if list_state.selected_icao.is_some() {
            detail_state.open = !detail_state.open;
        }
    }

    // Escape - Deselect aircraft, cancel follow, close panels
    if keyboard.just_pressed(KeyCode::Escape) {
        if follow_state.following_icao.is_some() {
            follow_state.following_icao = None;
        } else if list_state.selected_icao.is_some() {
            list_state.selected_icao = None;
            detail_state.open = false;
        } else if settings_ui.open {
            settings_ui.open = false;
        } else if help_state.visible {
            help_state.visible = false;
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

    // H - Toggle help overlay
    if keyboard.just_pressed(KeyCode::KeyH) {
        help_state.visible = !help_state.visible;
    }

    // R - Reset view to default
    if keyboard.just_pressed(KeyCode::KeyR) {
        map_state.latitude = app_config.map.default_latitude;
        map_state.longitude = app_config.map.default_longitude;
        zoom_state.camera_zoom = 1.0;
        follow_state.following_icao = None;
    }

    // A - Toggle airports (via config sync)
    if keyboard.just_pressed(KeyCode::KeyA) {
        // This needs to go through the overlay state system
        // We'll emit an event or modify a resource that sync_config_to_render_states reads
    }

    // T - Toggle trails (via config sync)
    if keyboard.just_pressed(KeyCode::KeyT) {
        // Similar to airports, handled via config sync
    }

    // S - Toggle statistics panel
    if keyboard.just_pressed(KeyCode::KeyS) {
        stats_state.expanded = !stats_state.expanded;
    }
}

/// System to toggle overlay settings with keyboard
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

    // A - Toggle airports
    if keyboard.just_pressed(KeyCode::KeyA) {
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
