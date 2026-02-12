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
