use bevy::prelude::*;

use super::{
    AviationData, AirportRenderState, RunwayRenderState, NavaidRenderState,
    spawn_airports, update_airport_positions, update_airport_visibility,
    draw_runways, draw_navaids,
    start_aviation_data_loading, poll_aviation_data_loading,
};

pub struct AviationPlugin;

impl Plugin for AviationPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<AviationData>()
            .init_resource::<AirportRenderState>()
            .init_resource::<RunwayRenderState>()
            .init_resource::<NavaidRenderState>()
            .add_systems(Startup, start_aviation_data_loading)
            .add_systems(Update, (
                poll_aviation_data_loading,
                spawn_airports,
                update_airport_positions,
                update_airport_visibility,
                draw_runways,
                draw_navaids,
            ));
    }
}
