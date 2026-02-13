/// Consolidated "Tools" window with tabs for infrequent-use feature panels.
///
/// Groups Coverage, Airspace, Data Sources, Export, and 3D View into a single
/// tabbed window to reduce floating window clutter.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::path::Path;

use crate::coverage::CoverageState;
use crate::airspace::{AirspaceDisplayState, AirspaceData};
use crate::data_sources::DataSourceManager;
use crate::export::{ExportState, ExportFormat};
use crate::view3d::{View3DState, ViewMode};
use crate::ui_panels::{UiPanelManager, PanelId};

/// Which tab is currently active in the tools window.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum ToolsTab {
    #[default]
    Coverage,
    Airspace,
    DataSources,
    Export,
    View3D,
}

/// Whether the tools window is open.
#[derive(Resource, Default)]
pub struct ToolsWindowState {
    pub open: bool,
    pub active_tab: ToolsTab,
}

/// System to render the consolidated tools window.
pub fn render_tools_window(
    mut contexts: EguiContexts,
    mut tools_state: ResMut<ToolsWindowState>,
    mut coverage: ResMut<CoverageState>,
    mut airspace_display: ResMut<AirspaceDisplayState>,
    mut airspace_data: ResMut<AirspaceData>,
    mut datasource_mgr: ResMut<DataSourceManager>,
    mut export_state: ResMut<ExportState>,
    mut view3d_state: ResMut<View3DState>,
    panels: Res<UiPanelManager>,
) {
    // The tools window opens when any of its constituent panels are toggled open
    let any_tool_open = panels.is_open(PanelId::Coverage)
        || panels.is_open(PanelId::Airspace)
        || panels.is_open(PanelId::DataSources)
        || panels.is_open(PanelId::Export)
        || panels.is_open(PanelId::View3D);

    if !any_tool_open {
        tools_state.open = false;
        return;
    }
    tools_state.open = true;

    // Auto-select the tab that was most recently toggled on
    if panels.is_open(PanelId::Coverage) && tools_state.active_tab != ToolsTab::Coverage {
        if coverage.is_changed() {
            tools_state.active_tab = ToolsTab::Coverage;
        }
    }
    if panels.is_open(PanelId::Airspace) && tools_state.active_tab != ToolsTab::Airspace {
        if airspace_display.is_changed() {
            tools_state.active_tab = ToolsTab::Airspace;
        }
    }
    if panels.is_open(PanelId::DataSources) && tools_state.active_tab != ToolsTab::DataSources {
        if datasource_mgr.is_changed() {
            tools_state.active_tab = ToolsTab::DataSources;
        }
    }
    if panels.is_open(PanelId::Export) && tools_state.active_tab != ToolsTab::Export {
        if export_state.is_changed() {
            tools_state.active_tab = ToolsTab::Export;
        }
    }
    if panels.is_open(PanelId::View3D) && tools_state.active_tab != ToolsTab::View3D {
        if view3d_state.is_changed() {
            tools_state.active_tab = ToolsTab::View3D;
        }
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let panel_bg = egui::Color32::from_rgba_unmultiplied(25, 30, 35, 240);
    let border_color = egui::Color32::from_rgb(60, 80, 100);

    let window_frame = egui::Frame::default()
        .fill(panel_bg)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::same(8));

    egui::Window::new("Tools")
        .collapsible(true)
        .resizable(true)
        .default_width(320.0)
        .default_height(350.0)
        .frame(window_frame)
        .show(ctx, |ui| {
            // Tab bar
            ui.horizontal(|ui| {
                if panels.is_open(PanelId::Coverage) {
                    let selected = tools_state.active_tab == ToolsTab::Coverage;
                    if ui.selectable_label(selected, "Coverage").clicked() {
                        tools_state.active_tab = ToolsTab::Coverage;
                    }
                }
                if panels.is_open(PanelId::Airspace) {
                    let selected = tools_state.active_tab == ToolsTab::Airspace;
                    if ui.selectable_label(selected, "Airspace").clicked() {
                        tools_state.active_tab = ToolsTab::Airspace;
                    }
                }
                if panels.is_open(PanelId::DataSources) {
                    let selected = tools_state.active_tab == ToolsTab::DataSources;
                    if ui.selectable_label(selected, "Sources").clicked() {
                        tools_state.active_tab = ToolsTab::DataSources;
                    }
                }
                if panels.is_open(PanelId::Export) {
                    let selected = tools_state.active_tab == ToolsTab::Export;
                    if ui.selectable_label(selected, "Export").clicked() {
                        tools_state.active_tab = ToolsTab::Export;
                    }
                }
                if panels.is_open(PanelId::View3D) {
                    let selected = tools_state.active_tab == ToolsTab::View3D;
                    if ui.selectable_label(selected, "3D View").clicked() {
                        tools_state.active_tab = ToolsTab::View3D;
                    }
                }
            });

            ui.separator();

            // Tab content
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    match tools_state.active_tab {
                        ToolsTab::Coverage => render_coverage_tab(ui, &mut coverage),
                        ToolsTab::Airspace => render_airspace_tab(ui, &mut airspace_display, &mut airspace_data),
                        ToolsTab::DataSources => render_data_sources_tab(ui, &mut datasource_mgr),
                        ToolsTab::Export => render_export_tab(ui, &mut export_state),
                        ToolsTab::View3D => render_view3d_tab(ui, &mut view3d_state),
                    }
                });
        });
}

pub(crate) fn render_coverage_tab(ui: &mut egui::Ui, coverage: &mut CoverageState) {
    let stats = coverage.get_stats();

    ui.horizontal(|ui| {
        ui.label("Tracking: ");
        if coverage.enabled {
            ui.colored_label(egui::Color32::GREEN, "ACTIVE");
        } else {
            ui.colored_label(egui::Color32::GRAY, "INACTIVE");
        }
    });

    ui.separator();

    egui::Grid::new("coverage_stats_grid_tab")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Max Range:");
            ui.label(format!("{:.1} NM", stats.max_range_nm));
            ui.end_row();

            ui.label("Avg Max Range:");
            ui.label(format!("{:.1} NM", stats.avg_max_range_nm));
            ui.end_row();

            ui.label("Active Sectors:");
            ui.label(format!("{}/{}", stats.active_sectors, stats.total_sectors));
            ui.end_row();

            ui.label("Unique Aircraft:");
            ui.label(format!("{}", stats.unique_aircraft));
            ui.end_row();

            ui.label("Total Observations:");
            ui.label(format!("{}", stats.total_observations));
            ui.end_row();
        });

    ui.separator();

    ui.horizontal(|ui| {
        let enable_text = if coverage.enabled { "Disable" } else { "Enable" };
        if ui.button(enable_text).clicked() {
            coverage.enabled = !coverage.enabled;
        }
        if ui.button("Reset").clicked() {
            coverage.reset();
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Receiver:");
        ui.label(format!("{:.4}, {:.4}", coverage.receiver_location.0, coverage.receiver_location.1));
    });
}

fn render_airspace_tab(
    ui: &mut egui::Ui,
    display_state: &mut AirspaceDisplayState,
    airspace_data: &mut AirspaceData,
) {
    if !airspace_data.loaded {
        ui.label("No airspace data loaded");
        ui.separator();
        if ui.button("Load Sample Data").clicked() {
            airspace_data.load_sample_data();
        }
        ui.label(
            egui::RichText::new("Note: Full implementation requires\nintegration with FAA/OpenAIP data")
                .size(11.0)
                .color(egui::Color32::GRAY),
        );
    } else {
        if let Some(ref source) = airspace_data.source {
            ui.label(format!("Source: {}", source));
        }
        ui.label(format!("{} airspaces loaded", airspace_data.airspaces.len()));
        ui.separator();
        ui.label("Display Options:");
        ui.checkbox(&mut display_state.show_class_b, "Class B");
        ui.checkbox(&mut display_state.show_class_c, "Class C");
        ui.checkbox(&mut display_state.show_class_d, "Class D");
        ui.checkbox(&mut display_state.show_restricted, "Restricted");
        ui.checkbox(&mut display_state.show_moa, "MOA");
        ui.checkbox(&mut display_state.show_tfr, "TFR");
        ui.separator();
        ui.checkbox(&mut display_state.show_labels, "Show Labels");
    }
}

fn render_data_sources_tab(ui: &mut egui::Ui, manager: &mut DataSourceManager) {
    let stats = manager.get_stats();
    ui.label(format!(
        "{}/{} sources connected, {} aircraft",
        stats.connected_sources, stats.total_sources, stats.total_aircraft
    ));

    ui.separator();

    for source in &manager.sources {
        let status = manager.states.get(&source.name);
        let status_text = status
            .map(|s| format!("{:?}", s.status))
            .unwrap_or_else(|| "Unknown".to_string());

        ui.horizontal(|ui| {
            let enabled_icon = if source.enabled { "\u{25CF}" } else { "\u{25CB}" };
            ui.label(enabled_icon);
            ui.label(&source.name);
            ui.label(
                egui::RichText::new(&status_text)
                    .size(10.0)
                    .color(egui::Color32::GRAY),
            );
        });

        ui.label(
            egui::RichText::new(format!("  {}", source.endpoint))
                .size(10.0)
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );
    }
}

fn render_export_tab(ui: &mut egui::Ui, export_state: &mut ExportState) {
    ui.label("Export recorded flight data to various formats.");
    ui.separator();

    ui.label("Format:");
    egui::ComboBox::from_id_salt("export_format_tab")
        .selected_text(export_state.format.display_name())
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut export_state.format, ExportFormat::KML, ExportFormat::KML.display_name());
            ui.selectable_value(&mut export_state.format, ExportFormat::CSV, ExportFormat::CSV.display_name());
            ui.selectable_value(&mut export_state.format, ExportFormat::GeoJSON, ExportFormat::GeoJSON.display_name());
        });

    ui.add_space(8.0);
    ui.label("Available Recordings:");

    let recordings = crate::export::list_available_recordings();
    if recordings.is_empty() {
        ui.label(
            egui::RichText::new("No recordings found in tmp/")
                .color(egui::Color32::GRAY),
        );
    } else {
        for recording in &recordings {
            let name = recording.file_name()
                .unwrap_or_default()
                .to_string_lossy();

            ui.horizontal(|ui| {
                ui.label(&*name);
                if ui.button("Export").clicked() {
                    let output_name = format!(
                        "{}.{}",
                        name.trim_end_matches(".ndjson"),
                        export_state.format.extension()
                    );
                    let output_path = recording.parent()
                        .unwrap_or(Path::new("."))
                        .join(&output_name);

                    match crate::export::export_recording(recording, &output_path, export_state.format) {
                        Ok(()) => {
                            export_state.status_message = Some(format!("Exported to {}", output_name));
                            export_state.last_export_path = Some(output_path);
                        }
                        Err(e) => {
                            export_state.status_message = Some(format!("Error: {}", e));
                        }
                    }
                }
            });
        }
    }

    if let Some(ref msg) = export_state.status_message {
        ui.separator();
        let color = if msg.starts_with("Error") {
            egui::Color32::RED
        } else {
            egui::Color32::GREEN
        };
        ui.colored_label(color, msg);
    }
}

pub(crate) fn render_view3d_tab(ui: &mut egui::Ui, state: &mut View3DState) {
    ui.colored_label(egui::Color32::YELLOW, "This feature is in research/prototype stage");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("View Mode:");
        if ui.selectable_label(state.mode == ViewMode::Map2D, "2D Map").clicked() {
            state.mode = ViewMode::Map2D;
        }
        if ui.selectable_label(state.mode == ViewMode::Perspective3D, "3D View").clicked() {
            state.mode = ViewMode::Perspective3D;
        }
    });

    if state.mode == ViewMode::Perspective3D {
        ui.colored_label(egui::Color32::RED, "3D rendering not yet implemented");
    }

    ui.separator();
    ui.label("Camera Settings (for future use):");

    ui.horizontal(|ui| {
        ui.label("Pitch:");
        ui.add(egui::Slider::new(&mut state.camera_pitch, 15.0..=89.0).suffix("\u{00B0}"));
    });
    ui.horizontal(|ui| {
        ui.label("Altitude:");
        ui.add(egui::Slider::new(&mut state.camera_altitude, 1000.0..=60000.0).suffix(" ft"));
    });
    ui.horizontal(|ui| {
        ui.label("Yaw:");
        ui.add(egui::Slider::new(&mut state.camera_yaw, 0.0..=360.0).suffix("\u{00B0}"));
    });
    ui.horizontal(|ui| {
        ui.label("Alt Scale:");
        ui.add(egui::Slider::new(&mut state.altitude_scale, 0.1..=10.0));
    });

}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};
    use crate::coverage::CoverageState;
    use crate::view3d::{View3DState, ViewMode};

    #[test]
    fn test_coverage_tab_shows_inactive() {
        let harness = Harness::new_ui_state(
            |ui, state: &mut CoverageState| {
                render_coverage_tab(ui, state);
            },
            CoverageState::default(),
        );

        harness.get_by_label("INACTIVE");
    }

    #[test]
    fn test_coverage_tab_shows_enable_button() {
        let harness = Harness::new_ui_state(
            |ui, state: &mut CoverageState| {
                render_coverage_tab(ui, state);
            },
            CoverageState::default(),
        );

        harness.get_by_label("Enable");
    }

    #[test]
    fn test_view3d_tab_shows_pitch_label() {
        let harness = Harness::new_ui_state(
            |ui, state: &mut View3DState| {
                render_view3d_tab(ui, state);
            },
            View3DState::default(),
        );

        harness.get_by_label("Pitch:");
    }

    #[test]
    fn test_view3d_tab_shows_altitude_label() {
        let harness = Harness::new_ui_state(
            |ui, state: &mut View3DState| {
                render_view3d_tab(ui, state);
            },
            View3DState::default(),
        );

        harness.get_by_label("Altitude:");
    }

    #[test]
    fn test_view3d_tab_shows_mode_selectable_labels() {
        let harness = Harness::new_ui_state(
            |ui, state: &mut View3DState| {
                render_view3d_tab(ui, state);
            },
            View3DState::default(),
        );

        harness.get_by_label("2D Map");
        harness.get_by_label("3D View");
    }
}
