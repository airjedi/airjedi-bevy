/// Centralized UI panel state management.
///
/// Provides a single resource that tracks which panels are open, replacing
/// scattered boolean flags across many individual resources.

use bevy::prelude::*;
use std::collections::HashSet;

/// Identifies every toggleable panel/overlay in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelId {
    Settings,
    AircraftList,
    AircraftDetail,
    Bookmarks,
    Statistics,
    Recording,
    Measurement,
    Export,
    Coverage,
    Airspace,
    DataSources,
    View3D,
    Debug,
    Help,
}

impl PanelId {
    /// Keyboard shortcut label for the help overlay.
    pub fn shortcut_label(&self) -> &'static str {
        match self {
            PanelId::Settings => "Esc",
            PanelId::AircraftList => "L",
            PanelId::AircraftDetail => "D",
            PanelId::Bookmarks => "B",
            PanelId::Statistics => "S",
            PanelId::Recording => "Ctrl+R",
            PanelId::Measurement => "M",
            PanelId::Export => "E",
            PanelId::Coverage => "V",
            PanelId::Airspace => "Shift+A",
            PanelId::DataSources => "Shift+D",
            PanelId::View3D => "3",
            PanelId::Debug => "`",
            PanelId::Help => "H",
        }
    }

    /// Display name for UI labels.
    pub fn display_name(&self) -> &'static str {
        match self {
            PanelId::Settings => "Settings",
            PanelId::AircraftList => "Aircraft List",
            PanelId::AircraftDetail => "Aircraft Detail",
            PanelId::Bookmarks => "Bookmarks",
            PanelId::Statistics => "Statistics",
            PanelId::Recording => "Recording",
            PanelId::Measurement => "Measurement",
            PanelId::Export => "Export",
            PanelId::Coverage => "Coverage",
            PanelId::Airspace => "Airspace",
            PanelId::DataSources => "Data Sources",
            PanelId::View3D => "3D View",
            PanelId::Debug => "Debug",
            PanelId::Help => "Help",
        }
    }

    /// Unicode icon character for toolbar display.
    pub fn icon(&self) -> &'static str {
        match self {
            PanelId::Settings => "\u{2699}",     // gear
            PanelId::AircraftList => "\u{2708}",  // airplane
            PanelId::AircraftDetail => "\u{1F4CB}", // clipboard
            PanelId::Bookmarks => "\u{2B50}",     // star
            PanelId::Statistics => "\u{1F4CA}",   // bar chart
            PanelId::Recording => "\u{23FA}",     // record
            PanelId::Measurement => "\u{1F4CF}",  // ruler
            PanelId::Export => "\u{1F4E5}",       // inbox tray / download
            PanelId::Coverage => "\u{1F4E1}",     // satellite antenna
            PanelId::Airspace => "\u{1F5FA}",     // map
            PanelId::DataSources => "\u{1F5C4}",  // file cabinet
            PanelId::View3D => "\u{1F4E6}",       // package / cube
            PanelId::Debug => "#",
            PanelId::Help => "?",
        }
    }
}

/// Centralized resource tracking which panels are currently open.
#[derive(Resource, Default)]
pub struct UiPanelManager {
    open_panels: HashSet<PanelId>,
}

impl UiPanelManager {
    /// Toggle a panel open/closed. Returns the new state (true = open).
    pub fn toggle_panel(&mut self, panel: PanelId) -> bool {
        if self.open_panels.contains(&panel) {
            self.open_panels.remove(&panel);
            false
        } else {
            self.open_panels.insert(panel);
            true
        }
    }

    /// Open a specific panel.
    pub fn open_panel(&mut self, panel: PanelId) {
        self.open_panels.insert(panel);
    }

    /// Close a specific panel.
    pub fn close_panel(&mut self, panel: PanelId) {
        self.open_panels.remove(&panel);
    }

    /// Check whether a panel is currently open.
    pub fn is_open(&self, panel: PanelId) -> bool {
        self.open_panels.contains(&panel)
    }

    /// Close all panels.
    pub fn close_all(&mut self) {
        self.open_panels.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_panel_opens_closed_panel() {
        let mut mgr = UiPanelManager::default();
        assert!(!mgr.is_open(PanelId::Settings));
        let result = mgr.toggle_panel(PanelId::Settings);
        assert!(result);
        assert!(mgr.is_open(PanelId::Settings));
    }

    #[test]
    fn toggle_panel_closes_open_panel() {
        let mut mgr = UiPanelManager::default();
        mgr.toggle_panel(PanelId::Debug);
        assert!(mgr.is_open(PanelId::Debug));
        let result = mgr.toggle_panel(PanelId::Debug);
        assert!(!result);
        assert!(!mgr.is_open(PanelId::Debug));
    }

    #[test]
    fn open_panel_makes_panel_open() {
        let mut mgr = UiPanelManager::default();
        mgr.open_panel(PanelId::Help);
        assert!(mgr.is_open(PanelId::Help));
    }

    #[test]
    fn open_panel_is_idempotent() {
        let mut mgr = UiPanelManager::default();
        mgr.open_panel(PanelId::Help);
        mgr.open_panel(PanelId::Help);
        assert!(mgr.is_open(PanelId::Help));
    }

    #[test]
    fn close_panel_makes_panel_closed() {
        let mut mgr = UiPanelManager::default();
        mgr.open_panel(PanelId::Statistics);
        mgr.close_panel(PanelId::Statistics);
        assert!(!mgr.is_open(PanelId::Statistics));
    }

    #[test]
    fn close_panel_on_already_closed_is_noop() {
        let mut mgr = UiPanelManager::default();
        mgr.close_panel(PanelId::Export);
        assert!(!mgr.is_open(PanelId::Export));
    }

    #[test]
    fn is_open_returns_correct_state() {
        let mut mgr = UiPanelManager::default();
        assert!(!mgr.is_open(PanelId::Bookmarks));
        mgr.open_panel(PanelId::Bookmarks);
        assert!(mgr.is_open(PanelId::Bookmarks));
        assert!(!mgr.is_open(PanelId::Airspace));
        mgr.close_panel(PanelId::Bookmarks);
        assert!(!mgr.is_open(PanelId::Bookmarks));
    }

    #[test]
    fn close_all_clears_all_open_panels() {
        let mut mgr = UiPanelManager::default();
        mgr.open_panel(PanelId::Settings);
        mgr.open_panel(PanelId::Debug);
        mgr.open_panel(PanelId::View3D);
        mgr.open_panel(PanelId::Help);
        mgr.close_all();
        assert!(!mgr.is_open(PanelId::Settings));
        assert!(!mgr.is_open(PanelId::Debug));
        assert!(!mgr.is_open(PanelId::View3D));
        assert!(!mgr.is_open(PanelId::Help));
    }

    #[test]
    fn close_all_on_empty_is_noop() {
        let mut mgr = UiPanelManager::default();
        mgr.close_all();
        assert!(!mgr.is_open(PanelId::Settings));
    }

    #[test]
    fn display_name_settings() {
        assert_eq!(PanelId::Settings.display_name(), "Settings");
    }

    #[test]
    fn display_name_view3d() {
        assert_eq!(PanelId::View3D.display_name(), "3D View");
    }

    #[test]
    fn display_name_data_sources() {
        assert_eq!(PanelId::DataSources.display_name(), "Data Sources");
    }

    #[test]
    fn display_name_aircraft_list() {
        assert_eq!(PanelId::AircraftList.display_name(), "Aircraft List");
    }

    #[test]
    fn shortcut_label_settings() {
        assert_eq!(PanelId::Settings.shortcut_label(), "Esc");
    }

    #[test]
    fn shortcut_label_debug() {
        assert_eq!(PanelId::Debug.shortcut_label(), "`");
    }

    #[test]
    fn shortcut_label_view3d() {
        assert_eq!(PanelId::View3D.shortcut_label(), "3");
    }

    #[test]
    fn shortcut_label_recording() {
        assert_eq!(PanelId::Recording.shortcut_label(), "Ctrl+R");
    }
}
