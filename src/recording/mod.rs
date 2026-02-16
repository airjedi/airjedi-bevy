mod recorder;
mod player;

pub use recorder::*;
pub use player::*;

use bevy::prelude::*;

pub struct RecordingPlugin;

impl Plugin for RecordingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecordingState>()
            .init_resource::<PlaybackState>()
            .add_systems(Update, (
                record_frame,
                playback_frame,
                toggle_recording,
            ));
    }
}
