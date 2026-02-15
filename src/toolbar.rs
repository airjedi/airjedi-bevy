/// Left-side egui icon toolbar.
///
/// Replaces the Bevy native buttons (Clear Cache, Settings, Aircraft List)
/// with a narrow egui side panel containing icon toggle buttons for all features.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use bevy_slippy_tiles::{MapTile, SlippyTileDownloadStatus, DownloadSlippyTilesMessage};

use crate::ui_panels::{UiPanelManager, PanelId};
use crate::theme::{AppTheme, to_egui_color32, to_egui_color32_alpha};
use crate::MapState;
use crate::adsb::AdsbAircraftData;

/// Width of the toolbar in pixels.
const TOOLBAR_WIDTH: f32 = 44.0;

/// Render the left-side icon toolbar as a narrow egui SidePanel.
pub fn render_toolbar(
    mut contexts: EguiContexts,
    mut panels: ResMut<UiPanelManager>,
    map_state: Res<MapState>,
    mut download_events: MessageWriter<DownloadSlippyTilesMessage>,
    mut commands: Commands,
    tile_query: Query<Entity, With<MapTile>>,
    mut slippy_tile_download_status: ResMut<SlippyTileDownloadStatus>,
    adsb_data: Option<Res<AdsbAircraftData>>,
    theme: Res<AppTheme>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let panel_bg = to_egui_color32_alpha(theme.bg_secondary(), 245);
    let border_color = to_egui_color32(theme.bg_contrast());

    let toolbar_frame = egui::Frame::default()
        .fill(panel_bg)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::symmetric(4, 8));

    egui::SidePanel::left("toolbar")
        .exact_width(TOOLBAR_WIDTH)
        .resizable(false)
        .frame(toolbar_frame)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let active_color = to_egui_color32(theme.accent_primary());
                let inactive_color = to_egui_color32(theme.text_dim());
                let active_bg = to_egui_color32_alpha(theme.accent_primary(), 30);

                // -- Connection status indicator at top --
                render_connection_indicator(ui, &adsb_data, &theme);

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // -- Panel toggle buttons --
                toolbar_button(ui, &mut panels, PanelId::Settings, "\u{2699}", "Settings", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::AircraftList, "\u{2708}", "Aircraft List (L)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::Bookmarks, "\u{2605}", "Bookmarks (B)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::Statistics, "S", "Statistics (S)", active_color, inactive_color, active_bg);

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                toolbar_button(ui, &mut panels, PanelId::Measurement, "\u{21A6}", "Measurement (M)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::Export, "\u{21E9}", "Export (E)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::Coverage, "\u{25CE}", "Coverage (V)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::Airspace, "\u{25A1}", "Airspace (Shift+A)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::DataSources, "\u{2637}", "Data Sources (Shift+D)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::Recording, "\u{23FA}", "Recording (Ctrl+R)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::View3D, "\u{2B1A}", "3D View (3)", active_color, inactive_color, active_bg);

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                toolbar_button(ui, &mut panels, PanelId::Debug, "#", "Debug (`)", active_color, inactive_color, active_bg);
                toolbar_button(ui, &mut panels, PanelId::Help, "?", "Help (H)", active_color, inactive_color, active_bg);

                // -- Clear Cache button (action, not a panel toggle) --
                ui.add_space(4.0);
                let icon_dim = to_egui_color32(theme.text_dim());
                let clear_btn = ui.add(
                    egui::Button::new(
                        egui::RichText::new("\u{2716}")
                            .size(16.0)
                            .color(icon_dim),
                    )
                    .min_size(egui::vec2(32.0, 32.0))
                ).on_hover_text("Clear tile cache");

                if clear_btn.clicked() {
                    // Clear download status tracking
                    slippy_tile_download_status.0.clear();

                    // Despawn all tile entities
                    for entity in tile_query.iter() {
                        commands.entity(entity).despawn();
                    }

                    // Clear tile cache from disk
                    crate::clear_tile_cache();

                    // Request fresh tiles
                    crate::request_tiles_at_location(
                        &mut download_events,
                        map_state.latitude,
                        map_state.longitude,
                        map_state.zoom_level,
                        false,
                    );

                    info!("Tile cache cleared via toolbar");
                }
            });
        });
}

/// Render a toolbar toggle button that highlights when its panel is open.
fn toolbar_button(
    ui: &mut egui::Ui,
    panels: &mut UiPanelManager,
    panel_id: PanelId,
    icon: &str,
    tooltip: &str,
    active_color: egui::Color32,
    inactive_color: egui::Color32,
    active_bg: egui::Color32,
) {
    let is_open = panels.is_open(panel_id);
    let icon_color = if is_open { active_color } else { inactive_color };

    let btn = ui.add(
        egui::Button::new(
            egui::RichText::new(icon)
                .size(18.0)
                .color(icon_color),
        )
        .fill(if is_open { active_bg } else { egui::Color32::TRANSPARENT })
        .min_size(egui::vec2(32.0, 32.0))
    ).on_hover_text(tooltip);

    if btn.clicked() {
        panels.toggle_panel(panel_id);
    }
}

/// Render the ADS-B connection status indicator at the top of the toolbar.
fn render_connection_indicator(
    ui: &mut egui::Ui,
    adsb_data: &Option<Res<AdsbAircraftData>>,
    theme: &AppTheme,
) {
    let Some(adsb_data) = adsb_data else {
        ui.label(egui::RichText::new("\u{25CF}").size(12.0).color(to_egui_color32(theme.bg_overlay())))
            .on_hover_text("ADS-B: No client");
        return;
    };

    let connection_state = adsb_data.get_connection_state();
    let aircraft_count = adsb_data.get_aircraft().len();

    use adsb_client::ConnectionState;
    let (color, tooltip) = match connection_state {
        ConnectionState::Connected => (
            to_egui_color32(theme.text_success()),
            format!("ADS-B: {} aircraft", aircraft_count),
        ),
        ConnectionState::Connecting => (
            to_egui_color32(theme.text_warn()),
            "ADS-B: Connecting...".to_string(),
        ),
        ConnectionState::Disconnected => (
            to_egui_color32(theme.text_error()),
            "ADS-B: Disconnected".to_string(),
        ),
        ConnectionState::Error(ref msg) => (
            to_egui_color32(theme.text_error()),
            format!("ADS-B: Error - {}", msg),
        ),
    };

    ui.label(egui::RichText::new("\u{25CF}").size(12.0).color(color))
        .on_hover_text(tooltip);
}

/// Render map attribution as an egui overlay at the bottom of the screen.
pub fn render_map_attribution(
    mut contexts: EguiContexts,
    theme: Res<AppTheme>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Area::new(egui::Id::new("map_attribution"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-5.0, -5.0))
        .interactable(false)
        .show(ctx, |ui| {
            let bg = egui::Frame::default()
                .fill(to_egui_color32_alpha(theme.bg_triage(), 128))
                .inner_margin(egui::Margin::same(4));
            bg.show(ui, |ui| {
                ui.label(
                    egui::RichText::new("\u{00A9} OpenStreetMap contributors, \u{00A9} CartoDB")
                        .size(11.0)
                        .color(to_egui_color32(theme.text_dim())),
                );
            });
        });
}
