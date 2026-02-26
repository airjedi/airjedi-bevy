use bevy::prelude::*;
use bevy_hanabi::prelude::*;

use super::hanabi_selection;
use super::hanabi_trails;

pub struct HanabiEffectsPlugin;

impl Plugin for HanabiEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin);
        app.add_systems(Startup, hanabi_selection::setup_fog_effect);
        app.add_systems(Update, (
            hanabi_selection::manage_selection_fog,
            hanabi_selection::sync_fog_position,
        ));
        app.add_systems(Update, (
            hanabi_trails::spawn_trail_effects,
            hanabi_trails::update_trail_particles,
            hanabi_trails::cleanup_trail_effects,
        ));
        info!("HanabiEffectsPlugin loaded — particle trails and selection fog enabled");
    }
}
