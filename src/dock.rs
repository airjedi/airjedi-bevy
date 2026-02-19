use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui_tiles::{Behavior, SimplificationOptions, TabState, TileId, Tiles, UiResponse};

use crate::aircraft::{
    AircraftListState, AircraftDisplayList, CameraFollowState, DetailPanelState,
    SessionClock, StatsPanelState, TrailHistory,
    list_panel::render_aircraft_list_pane_content,
    stats_panel::render_stats_pane_content,
};
use crate::airspace::{AirspaceData, AirspaceDisplayState};
use crate::bookmarks::{self, BookmarksPanelState};
use crate::config::{self, AppConfig, SettingsUiState};
use crate::coverage::CoverageState;
use crate::data_sources::DataSourceManager;
use crate::debug_panel::{self, DebugPanelState};
use crate::export::ExportState;
use crate::recording::{PlaybackState, RecordingState};
use crate::theme::{AppTheme, ThemeRegistry, to_egui_color32, to_egui_color32_alpha};
use crate::tools_window;
use crate::ui_panels::{PanelId, UiPanelManager};
use crate::view3d::View3DState;
use crate::{Aircraft, MapState, ZoomState};

// =============================================================================
// Bundled system params to stay within Bevy's 16-param limit
// =============================================================================

#[derive(SystemParam)]
pub struct DockPanelResources<'w> {
    pub debug_state: ResMut<'w, DebugPanelState>,
    pub settings_ui: ResMut<'w, SettingsUiState>,
    pub app_config: ResMut<'w, AppConfig>,
    pub list_state: ResMut<'w, AircraftListState>,
    pub detail_state: ResMut<'w, DetailPanelState>,
    pub follow_state: ResMut<'w, CameraFollowState>,
    pub display_list: Res<'w, AircraftDisplayList>,
    pub clock: Res<'w, SessionClock>,
    pub stats_state: Res<'w, StatsPanelState>,
    pub bookmarks_state: ResMut<'w, BookmarksPanelState>,
}

#[derive(SystemParam)]
pub struct DockToolResources<'w> {
    pub coverage: ResMut<'w, CoverageState>,
    pub airspace_display: ResMut<'w, AirspaceDisplayState>,
    pub airspace_data: ResMut<'w, AirspaceData>,
    pub datasource_mgr: ResMut<'w, DataSourceManager>,
    pub export_state: ResMut<'w, ExportState>,
    pub recording: ResMut<'w, RecordingState>,
    pub playback: ResMut<'w, PlaybackState>,
    pub view3d_state: ResMut<'w, View3DState>,
}

// =============================================================================
// DockPane - identifies each dockable panel
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockPane {
    MapViewport,
    Debug,
    Coverage,
    Airspace,
    DataSources,
    Export,
    Recording,
    View3D,
    Settings,
    AircraftList,
    AircraftDetail,
    Bookmarks,
    Stats,
}

impl DockPane {
    pub fn display_name(&self) -> &'static str {
        match self {
            DockPane::MapViewport => "Map",
            DockPane::Debug => "Debug",
            DockPane::Coverage => "Coverage",
            DockPane::Airspace => "Airspace",
            DockPane::DataSources => "Data Sources",
            DockPane::Export => "Export",
            DockPane::Recording => "Recording",
            DockPane::View3D => "3D View",
            DockPane::Settings => "Settings",
            DockPane::AircraftList => "Aircraft",
            DockPane::AircraftDetail => "Detail",
            DockPane::Bookmarks => "Bookmarks",
            DockPane::Stats => "Statistics",
        }
    }
}

// =============================================================================
// DockTreeState - persistent Bevy resource holding the tile tree
// =============================================================================

#[derive(Resource)]
pub struct DockTreeState {
    pub tree: egui_tiles::Tree<DockPane>,
    pub pane_tile_ids: HashMap<DockPane, TileId>,
    /// Container tile IDs for auto-collapse when all children are hidden.
    pub bottom_tabs_id: TileId,
    pub right_tabs_id: TileId,
    /// Captured each frame from MapViewport pane for camera viewport adjustment
    pub map_viewport_rect: Option<egui::Rect>,
}

/// Panes grouped in the bottom tab container.
const BOTTOM_PANES: &[DockPane] = &[
    DockPane::Debug,
    DockPane::Coverage,
    DockPane::Airspace,
    DockPane::DataSources,
    DockPane::Export,
    DockPane::Recording,
];

/// Panes grouped in the right tab container.
const RIGHT_PANES: &[DockPane] = &[
    DockPane::AircraftList,
    DockPane::AircraftDetail,
    DockPane::Bookmarks,
    DockPane::Stats,
    DockPane::Settings,
    DockPane::View3D,
];

impl Default for DockTreeState {
    fn default() -> Self {
        let mut tiles = Tiles::default();
        let mut pane_tile_ids = HashMap::new();

        // Insert all panes and record their TileIds
        let all_panes = [
            DockPane::MapViewport,
            DockPane::Debug,
            DockPane::Coverage,
            DockPane::Airspace,
            DockPane::DataSources,
            DockPane::Export,
            DockPane::Recording,
            DockPane::View3D,
            DockPane::Settings,
            DockPane::AircraftList,
            DockPane::AircraftDetail,
            DockPane::Bookmarks,
            DockPane::Stats,
        ];

        for pane in all_panes {
            let id = tiles.insert_pane(pane);
            pane_tile_ids.insert(pane, id);
        }

        // Bottom tabs: Debug, Coverage, Airspace, DataSources, Export, Recording
        let bottom_tabs_id = tiles.insert_tab_tile(vec![
            pane_tile_ids[&DockPane::Debug],
            pane_tile_ids[&DockPane::Coverage],
            pane_tile_ids[&DockPane::Airspace],
            pane_tile_ids[&DockPane::DataSources],
            pane_tile_ids[&DockPane::Export],
            pane_tile_ids[&DockPane::Recording],
        ]);

        // Right tabs: AircraftList, AircraftDetail, Bookmarks, Stats, Settings, View3D
        let right_tabs_id = tiles.insert_tab_tile(vec![
            pane_tile_ids[&DockPane::AircraftList],
            pane_tile_ids[&DockPane::AircraftDetail],
            pane_tile_ids[&DockPane::Bookmarks],
            pane_tile_ids[&DockPane::Stats],
            pane_tile_ids[&DockPane::Settings],
            pane_tile_ids[&DockPane::View3D],
        ]);

        // Top area: horizontal split - MapViewport (left, ~75%) + right tabs (right, ~25%)
        let map_id = pane_tile_ids[&DockPane::MapViewport];
        let top_area_id = tiles.insert_horizontal_tile(vec![map_id, right_tabs_id]);
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) =
            tiles.get_mut(top_area_id)
        {
            linear.shares.set_share(map_id, 3.0);
            linear.shares.set_share(right_tabs_id, 1.0);
        }

        // Root: vertical split - top area (~75%) + bottom tabs (~25%)
        let root_id = tiles.insert_vertical_tile(vec![top_area_id, bottom_tabs_id]);
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) =
            tiles.get_mut(root_id)
        {
            linear.shares.set_share(top_area_id, 3.0);
            linear.shares.set_share(bottom_tabs_id, 1.0);
        }

        // Set most panes to not visible initially (only MapViewport and AircraftList visible)
        let hidden_panes = [
            DockPane::Debug,
            DockPane::Coverage,
            DockPane::Airspace,
            DockPane::DataSources,
            DockPane::Export,
            DockPane::Recording,
            DockPane::View3D,
            DockPane::AircraftDetail,
            DockPane::Bookmarks,
            DockPane::Stats,
            DockPane::Settings,
        ];
        for pane in hidden_panes {
            tiles.set_visible(pane_tile_ids[&pane], false);
        }

        let tree = egui_tiles::Tree::new("dock_tree", root_id, tiles);

        Self {
            tree,
            pane_tile_ids,
            bottom_tabs_id,
            right_tabs_id,
            map_viewport_rect: None,
        }
    }
}

// =============================================================================
// DockBehavior - transient struct built each frame with all needed state
// =============================================================================

pub struct DockBehavior<'a, 'w, 's> {
    pub map_viewport_rect: &'a mut Option<egui::Rect>,
    pub theme: &'a mut AppTheme,
    /// Panes closed via dock tab X button, processed after tree rendering.
    pub closed_panes: Vec<DockPane>,
    // Debug
    pub debug_state: &'a mut DebugPanelState,
    pub map_state: Option<&'a mut MapState>,
    pub zoom_state: Option<&'a mut ZoomState>,
    // Settings
    pub settings_ui: &'a mut SettingsUiState,
    pub app_config: &'a mut AppConfig,
    pub theme_registry: &'a ThemeRegistry,
    // Aircraft list
    pub list_state: &'a mut AircraftListState,
    pub detail_state: &'a mut DetailPanelState,
    pub follow_state: &'a mut CameraFollowState,
    pub display_list: &'a AircraftDisplayList,
    pub clock: &'a SessionClock,
    pub aircraft_trail_query: &'a Query<'w, 's, (&'static Aircraft, &'static TrailHistory)>,
    // Stats
    pub stats_state: &'a StatsPanelState,
    pub aircraft_query: &'a Query<'w, 's, &'static Aircraft>,
    // Bookmarks
    pub bookmarks_state: &'a mut BookmarksPanelState,
    // Tools
    pub coverage: &'a mut CoverageState,
    pub airspace_display: &'a mut AirspaceDisplayState,
    pub airspace_data: &'a mut AirspaceData,
    pub datasource_mgr: &'a mut DataSourceManager,
    pub export_state: &'a mut ExportState,
    pub recording: &'a mut RecordingState,
    pub playback: &'a mut PlaybackState,
    pub view3d_state: &'a mut View3DState,
}

impl<'a, 'w, 's> Behavior<DockPane> for DockBehavior<'a, 'w, 's> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        pane: &mut DockPane,
    ) -> UiResponse {
        match pane {
            DockPane::MapViewport => {
                *self.map_viewport_rect = Some(ui.max_rect());
            }
            DockPane::Debug => {
                self.render_with_bg(ui, |ui, this| {
                    debug_panel::render_debug_pane_content(
                        ui,
                        this.debug_state,
                        this.map_state.as_deref(),
                        this.zoom_state.as_deref(),
                    );
                });
            }
            DockPane::Settings => {
                self.render_with_bg(ui, |ui, this| {
                    config::render_settings_pane_content(
                        ui,
                        this.settings_ui,
                        this.app_config,
                        this.theme,
                        this.theme_registry,
                    );
                });
            }
            DockPane::AircraftList => {
                self.render_with_bg(ui, |ui, this| {
                    render_aircraft_list_pane_content(
                        ui,
                        this.list_state,
                        this.detail_state,
                        this.follow_state,
                        this.display_list,
                        this.map_state.as_deref().unwrap(),
                        this.clock,
                        this.aircraft_trail_query,
                        this.theme,
                    );
                });
            }
            DockPane::AircraftDetail => {
                self.render_with_bg(ui, |ui, _this| {
                    ui.label("Select an aircraft from the Aircraft tab to view details.");
                });
            }
            DockPane::Stats => {
                self.render_with_bg(ui, |ui, this| {
                    render_stats_pane_content(
                        ui,
                        this.stats_state,
                        this.aircraft_query,
                        this.theme,
                    );
                });
            }
            DockPane::Bookmarks => {
                self.render_with_bg(ui, |ui, this| {
                    bookmarks::render_bookmarks_pane_content(
                        ui,
                        this.bookmarks_state,
                        this.app_config,
                        this.map_state.as_deref_mut().unwrap(),
                        this.zoom_state.as_deref_mut().unwrap(),
                        this.list_state,
                        this.aircraft_query,
                        this.theme,
                    );
                });
            }
            DockPane::Coverage => {
                self.render_with_bg(ui, |ui, this| {
                    tools_window::render_coverage_tab(ui, this.coverage);
                });
            }
            DockPane::Airspace => {
                self.render_with_bg(ui, |ui, this| {
                    tools_window::render_airspace_tab(
                        ui,
                        this.airspace_display,
                        this.airspace_data,
                    );
                });
            }
            DockPane::DataSources => {
                self.render_with_bg(ui, |ui, this| {
                    tools_window::render_data_sources_tab(ui, this.datasource_mgr);
                });
            }
            DockPane::Export => {
                self.render_with_bg(ui, |ui, this| {
                    tools_window::render_export_tab(ui, this.export_state);
                });
            }
            DockPane::Recording => {
                self.render_with_bg(ui, |ui, this| {
                    tools_window::render_recording_tab(
                        ui,
                        this.recording,
                        this.playback,
                    );
                });
            }
            DockPane::View3D => {
                self.render_with_bg(ui, |ui, this| {
                    tools_window::render_view3d_tab(ui, this.view3d_state);
                });
            }
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &DockPane) -> egui::WidgetText {
        pane.display_name().into()
    }

    fn is_tab_closable(&self, _tiles: &Tiles<DockPane>, _tile_id: TileId) -> bool {
        true
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<DockPane>, tile_id: TileId) -> bool {
        tiles.set_visible(tile_id, false);
        if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(tile_id) {
            self.closed_panes.push(*pane);
        }
        false // prevent removal, just hide
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_empty_tabs: false,
            prune_empty_containers: false,
            prune_single_child_tabs: false,
            prune_single_child_containers: false,
            all_panes_must_have_tabs: true,
            join_nested_linear_containers: false,
        }
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        1.0
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        to_egui_color32(self.theme.bg_secondary())
    }

    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<DockPane>,
        _tile_id: TileId,
        state: &TabState,
    ) -> egui::Color32 {
        if state.active {
            to_egui_color32(self.theme.bg_primary())
        } else {
            to_egui_color32_alpha(self.theme.bg_secondary(), 180)
        }
    }

    fn tab_text_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<DockPane>,
        _tile_id: TileId,
        state: &TabState,
    ) -> egui::Color32 {
        if state.active {
            to_egui_color32(self.theme.text_primary())
        } else {
            to_egui_color32(self.theme.text_dim())
        }
    }
}

impl<'a, 'w, 's> DockBehavior<'a, 'w, 's> {
    fn render_with_bg(
        &mut self,
        ui: &mut egui::Ui,
        content: impl FnOnce(&mut egui::Ui, &mut Self),
    ) {
        let pane_bg = to_egui_color32(self.theme.bg_primary());
        // Paint opaque background across the entire pane area
        ui.painter().rect_filled(ui.max_rect(), 0.0, pane_bg);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                content(ui, self);
            });
    }
}

// =============================================================================
// Mapping from UiPanelManager / ToolsWindowState to DockPane visibility
// =============================================================================

const PANEL_DOCK_MAP: &[(PanelId, DockPane)] = &[
    (PanelId::Debug, DockPane::Debug),
    (PanelId::Settings, DockPane::Settings),
    (PanelId::AircraftList, DockPane::AircraftList),
    (PanelId::AircraftDetail, DockPane::AircraftDetail),
    (PanelId::Bookmarks, DockPane::Bookmarks),
    (PanelId::Statistics, DockPane::Stats),
    (PanelId::Coverage, DockPane::Coverage),
    (PanelId::Airspace, DockPane::Airspace),
    (PanelId::DataSources, DockPane::DataSources),
    (PanelId::Export, DockPane::Export),
    (PanelId::Recording, DockPane::Recording),
    (PanelId::View3D, DockPane::View3D),
];

// =============================================================================
// render_dock_tree - Bevy system that renders the dock each frame
// =============================================================================

pub fn render_dock_tree(
    mut contexts: EguiContexts,
    mut dock_state: ResMut<DockTreeState>,
    mut panels: ResMut<UiPanelManager>,
    mut theme: ResMut<AppTheme>,
    theme_registry: Res<ThemeRegistry>,
    map_state: Option<ResMut<MapState>>,
    zoom_state: Option<ResMut<ZoomState>>,
    mut panel_res: DockPanelResources,
    mut tool_res: DockToolResources,
    aircraft_trail_query: Query<(&'static Aircraft, &'static TrailHistory)>,
    aircraft_query: Query<&'static Aircraft>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Sync UiPanelManager state to dock tile visibility
    for &(panel_id, dock_pane) in PANEL_DOCK_MAP {
        if let Some(&tile_id) = dock_state.pane_tile_ids.get(&dock_pane) {
            let should_be_visible = panels.is_open(panel_id);
            dock_state.tree.tiles.set_visible(tile_id, should_be_visible);
        }
    }

    // Auto-collapse containers when all their children are hidden
    let bottom_id = dock_state.bottom_tabs_id;
    let right_id = dock_state.right_tabs_id;
    let bottom_has_visible = BOTTOM_PANES.iter().any(|p| {
        dock_state.pane_tile_ids.get(p)
            .is_some_and(|&id| dock_state.tree.tiles.is_visible(id))
    });
    let right_has_visible = RIGHT_PANES.iter().any(|p| {
        dock_state.pane_tile_ids.get(p)
            .is_some_and(|&id| dock_state.tree.tiles.is_visible(id))
    });
    dock_state.tree.tiles.set_visible(bottom_id, bottom_has_visible);
    dock_state.tree.tiles.set_visible(right_id, right_has_visible);

    let mut map_viewport_rect: Option<egui::Rect> = None;

    let mut behavior = DockBehavior {
        map_viewport_rect: &mut map_viewport_rect,
        theme: &mut theme,
        closed_panes: Vec::new(),
        debug_state: &mut panel_res.debug_state,
        map_state: map_state.map(|r| r.into_inner()),
        zoom_state: zoom_state.map(|r| r.into_inner()),
        settings_ui: &mut panel_res.settings_ui,
        app_config: &mut panel_res.app_config,
        theme_registry: &theme_registry,
        list_state: &mut panel_res.list_state,
        detail_state: &mut panel_res.detail_state,
        follow_state: &mut panel_res.follow_state,
        display_list: &panel_res.display_list,
        clock: &panel_res.clock,
        aircraft_trail_query: &aircraft_trail_query,
        stats_state: &panel_res.stats_state,
        aircraft_query: &aircraft_query,
        bookmarks_state: &mut panel_res.bookmarks_state,
        coverage: &mut tool_res.coverage,
        airspace_display: &mut tool_res.airspace_display,
        airspace_data: &mut tool_res.airspace_data,
        datasource_mgr: &mut tool_res.datasource_mgr,
        export_state: &mut tool_res.export_state,
        recording: &mut tool_res.recording,
        playback: &mut tool_res.playback,
        view3d_state: &mut tool_res.view3d_state,
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
        .show(ctx, |ui| {
            dock_state.tree.ui(&mut behavior, ui);
        });

    // Route dock tab close events back to UiPanelManager
    for closed_pane in &behavior.closed_panes {
        if let Some(&(panel_id, _)) = PANEL_DOCK_MAP.iter().find(|(_, dp)| dp == closed_pane) {
            panels.close_panel(panel_id);
        }
    }

    dock_state.map_viewport_rect = map_viewport_rect;
}
