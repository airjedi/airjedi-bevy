use bevy::prelude::*;
use bevy_brp_extras::BrpExtrasPlugin;

pub struct BrpPlugin;

impl Plugin for BrpPlugin {
    fn build(&self, app: &mut App) {
        // BrpExtrasPlugin internally adds RemotePlugin (with extra methods)
        // and RemoteHttpPlugin (localhost:15702), so we only need this one.
        app.add_plugins(BrpExtrasPlugin::default());
    }
}
