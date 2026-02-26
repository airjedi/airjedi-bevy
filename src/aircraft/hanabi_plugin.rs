use bevy::prelude::*;
use bevy_hanabi::prelude::*;

use super::hanabi_selection;

pub struct HanabiEffectsPlugin;

impl Plugin for HanabiEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin);
        app.add_systems(Startup, hanabi_selection::setup_fog_effect);
        app.add_systems(Update, hanabi_selection::manage_selection_fog);
        info!("HanabiEffectsPlugin loaded — particle trails and selection fog enabled");
    }
}
