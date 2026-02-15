use std::collections::HashMap;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui_tiles::{Behavior, SimplificationOptions, TabState, TileId, Tiles, UiResponse};

use crate::theme::{AppTheme, to_egui_color32, to_egui_color32_alpha};

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
    /// Captured each frame from MapViewport pane for camera viewport adjustment
    pub map_viewport_rect: Option<egui::Rect>,
}

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

        // Build layout structure:
        // Bottom tabs: Debug, Coverage, Airspace, DataSources, Export, Recording, View3D
        let bottom_tabs_id = tiles.insert_tab_tile(vec![
            pane_tile_ids[&DockPane::Debug],
            pane_tile_ids[&DockPane::Coverage],
            pane_tile_ids[&DockPane::Airspace],
            pane_tile_ids[&DockPane::DataSources],
            pane_tile_ids[&DockPane::Export],
            pane_tile_ids[&DockPane::Recording],
            pane_tile_ids[&DockPane::View3D],
        ]);

        // Right tabs: AircraftList, AircraftDetail, Bookmarks, Stats
        let right_tabs_id = tiles.insert_tab_tile(vec![
            pane_tile_ids[&DockPane::AircraftList],
            pane_tile_ids[&DockPane::AircraftDetail],
            pane_tile_ids[&DockPane::Bookmarks],
            pane_tile_ids[&DockPane::Stats],
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
            map_viewport_rect: None,
        }
    }
}

// =============================================================================
// DockBehavior - transient struct built each frame
// =============================================================================

pub struct DockBehavior<'a> {
    pub map_viewport_rect: &'a mut Option<egui::Rect>,
    pub theme: &'a AppTheme,
}

impl Behavior<DockPane> for DockBehavior<'_> {
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
            _ => {
                ui.label(format!("{} (TODO)", pane.display_name()));
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

// =============================================================================
// render_dock_tree - Bevy system that renders the dock each frame
// =============================================================================

pub fn render_dock_tree(
    mut contexts: EguiContexts,
    mut dock_state: ResMut<DockTreeState>,
    theme: Res<AppTheme>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut map_viewport_rect: Option<egui::Rect> = None;

    let mut behavior = DockBehavior {
        map_viewport_rect: &mut map_viewport_rect,
        theme: &theme,
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
        .show(ctx, |ui| {
            dock_state.tree.ui(&mut behavior, ui);
        });

    dock_state.map_viewport_rect = map_viewport_rect;
}
