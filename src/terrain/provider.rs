//! Elevation tile providers (AWS Terrain Tiles, etc.)

use bevy::prelude::*;

/// Supported elevation data providers
#[derive(Debug, Clone, Default, Reflect)]
pub(crate) enum TerrainProvider {
    #[default]
    AwsTerrarium,
    // Future: MapboxTerrainRgb, CesiumQuantizedMesh
}

impl TerrainProvider {
    /// Build the URL for an elevation tile at the given zoom/x/y coordinates
    pub(crate) fn tile_url(&self, zoom: u8, x: u32, y: u32) -> String {
        match self {
            TerrainProvider::AwsTerrarium => {
                format!(
                    "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{}/{}/{}.png",
                    zoom, x, y
                )
            }
        }
    }

    /// Decode a raw PNG pixel (R, G, B) into elevation in meters
    pub(crate) fn decode_elevation(&self, r: u8, g: u8, b: u8) -> f32 {
        match self {
            TerrainProvider::AwsTerrarium => {
                // Terrarium encoding: height = R*256 + G + B/256 - 32768
                (r as f32) * 256.0 + (g as f32) + (b as f32) / 256.0 - 32768.0
            }
        }
    }
}
