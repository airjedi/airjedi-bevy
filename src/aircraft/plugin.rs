use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

use super::{
    SessionClock,
    TrailConfig, TrailRecordTimer, draw_trails, prune_trails, record_trail_points,
    AircraftListState, AircraftDisplayList,
    render_aircraft_list_panel, toggle_aircraft_list, update_aircraft_display_list,
    highlight_selected_aircraft,
    DetailPanelState, CameraFollowState,
    render_detail_panel, toggle_detail_panel, open_detail_on_selection, detect_aircraft_click,
    EmergencyAlertState,
    detect_emergencies, draw_emergency_rings, update_emergency_banner, update_emergency_banner_text,
    PredictionConfig, draw_predictions,
    StatsPanelState, render_stats_panel,
};

pub struct AircraftPlugin;

impl Plugin for AircraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SessionClock>()
            .init_resource::<TrailConfig>()
            .init_resource::<TrailRecordTimer>()
            .init_resource::<AircraftListState>()
            .init_resource::<AircraftDisplayList>()
            .init_resource::<DetailPanelState>()
            .init_resource::<CameraFollowState>()
            .init_resource::<EmergencyAlertState>()
            .init_resource::<PredictionConfig>()
            .init_resource::<StatsPanelState>()
            .add_systems(Update, (
                record_trail_points,
                draw_trails,
                prune_trails,
                toggle_aircraft_list,
                update_aircraft_display_list,
                highlight_selected_aircraft,
                toggle_detail_panel,
                open_detail_on_selection,
                detect_aircraft_click,
                detect_emergencies,
                draw_emergency_rings,
                update_emergency_banner,
                update_emergency_banner_text,
                draw_predictions,
            ))
            .add_systems(EguiPrimaryContextPass, (
                render_aircraft_list_panel,
                render_detail_panel,
                render_stats_panel,
            ));
    }
}
