use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

use super::{
    TrailConfig, TrailRecordTimer, draw_trails, prune_trails, record_trail_points,
    AircraftListState, AircraftDisplayList,
    render_aircraft_list_panel, toggle_aircraft_list, update_aircraft_display_list,
    highlight_selected_aircraft,
};

pub struct AircraftPlugin;

impl Plugin for AircraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<TrailConfig>()
            .init_resource::<TrailRecordTimer>()
            .init_resource::<AircraftListState>()
            .init_resource::<AircraftDisplayList>()
            .add_systems(Update, (
                record_trail_points,
                draw_trails,
                prune_trails,
                toggle_aircraft_list,
                update_aircraft_display_list,
                highlight_selected_aircraft,
            ))
            .add_systems(EguiPrimaryContextPass, render_aircraft_list_panel);
    }
}
