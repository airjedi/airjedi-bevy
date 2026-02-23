use bevy::prelude::*;
use bevy_slippy_tiles::ZoomLevel;

use crate::constants;

/// Resource to track map state (center position and zoom level)
#[derive(Resource, Clone, Reflect)]
#[reflect(Default)]
pub struct MapState {
    /// Current map center latitude
    pub latitude: f64,
    /// Current map center longitude
    pub longitude: f64,
    /// Current discrete tile zoom level
    #[reflect(ignore)]
    pub zoom_level: ZoomLevel,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            latitude: constants::DEFAULT_LATITUDE,
            longitude: constants::DEFAULT_LONGITUDE,
            zoom_level: ZoomLevel::L10,
        }
    }
}

/// Resource to track camera zoom (continuous zoom within tile zoom levels)
#[derive(Resource, Reflect)]
pub struct ZoomState {
    /// Continuous camera zoom level (1.0 = normal, 2.0 = 2x zoomed in, 0.5 = 2x zoomed out)
    pub camera_zoom: f32,
    /// Minimum camera zoom
    pub min_zoom: f32,
    /// Maximum camera zoom
    pub max_zoom: f32,
}

impl ZoomState {
    pub fn new() -> Self {
        Self {
            camera_zoom: 1.0,
            min_zoom: constants::MIN_CAMERA_ZOOM,
            max_zoom: constants::MAX_CAMERA_ZOOM,
        }
    }
}

impl Default for ZoomState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::reflect::Reflect;

    #[test]
    fn map_state_implements_reflect() {
        let state = MapState::default();
        let _: &dyn Reflect = &state;
    }

    #[test]
    fn zoom_state_implements_reflect() {
        let state = ZoomState::new();
        let _: &dyn Reflect = &state;
    }
}
