use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.toml";

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub feed: FeedConfig,
    pub map: MapConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FeedConfig {
    pub endpoint_url: String,
    pub refresh_interval_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapConfig {
    pub default_latitude: f64,
    pub default_longitude: f64,
    pub default_zoom: u8,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            feed: FeedConfig {
                endpoint_url: "http://192.168.1.63:8080/aircraft.json".to_string(),
                refresh_interval_ms: 1000,
            },
            map: MapConfig {
                default_latitude: 37.6872,
                default_longitude: -97.3301,
                default_zoom: 10,
            },
        }
    }
}

fn config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join(CONFIG_FILE)
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => {
                    info!("Loaded config from {:?}", path);
                    return config;
                }
                Err(e) => {
                    warn!("Failed to parse config: {}, using defaults", e);
                }
            },
            Err(e) => {
                warn!("Failed to read config: {}, using defaults", e);
            }
        }
    }

    let config = AppConfig::default();
    save_config(&config);
    config
}

pub fn save_config(config: &AppConfig) {
    let path = config_path();
    match toml::to_string_pretty(config) {
        Ok(contents) => {
            if let Err(e) = fs::write(&path, contents) {
                error!("Failed to write config: {}", e);
            } else {
                info!("Saved config to {:?}", path);
            }
        }
        Err(e) => {
            error!("Failed to serialize config: {}", e);
        }
    }
}

#[derive(Resource, Default)]
pub struct SettingsUiState {
    pub open: bool,
    pub endpoint_url: String,
    pub refresh_interval_ms: String,
    pub default_latitude: String,
    pub default_longitude: String,
    pub default_zoom: String,
    pub error_message: Option<String>,
}

impl SettingsUiState {
    pub fn populate_from_config(&mut self, config: &AppConfig) {
        self.endpoint_url = config.feed.endpoint_url.clone();
        self.refresh_interval_ms = config.feed.refresh_interval_ms.to_string();
        self.default_latitude = config.map.default_latitude.to_string();
        self.default_longitude = config.map.default_longitude.to_string();
        self.default_zoom = config.map.default_zoom.to_string();
        self.error_message = None;
    }

    pub fn validate_and_build(&self) -> Result<AppConfig, String> {
        // Validate endpoint URL
        let endpoint = self.endpoint_url.trim();
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err("Endpoint URL must start with http:// or https://".to_string());
        }

        // Validate refresh interval
        let refresh_ms: u64 = self.refresh_interval_ms.trim().parse()
            .map_err(|_| "Refresh interval must be a number")?;
        if refresh_ms < 100 || refresh_ms > 60000 {
            return Err("Refresh interval must be 100-60000 ms".to_string());
        }

        // Validate latitude
        let lat: f64 = self.default_latitude.trim().parse()
            .map_err(|_| "Latitude must be a number")?;
        if lat < -90.0 || lat > 90.0 {
            return Err("Latitude must be -90 to 90".to_string());
        }

        // Validate longitude
        let lon: f64 = self.default_longitude.trim().parse()
            .map_err(|_| "Longitude must be a number")?;
        if lon < -180.0 || lon > 180.0 {
            return Err("Longitude must be -180 to 180".to_string());
        }

        // Validate zoom
        let zoom: u8 = self.default_zoom.trim().parse()
            .map_err(|_| "Zoom must be a number")?;
        if zoom > 19 {
            return Err("Zoom must be 0-19".to_string());
        }

        Ok(AppConfig {
            feed: FeedConfig {
                endpoint_url: endpoint.to_string(),
                refresh_interval_ms: refresh_ms,
            },
            map: MapConfig {
                default_latitude: lat,
                default_longitude: lon,
                default_zoom: zoom,
            },
        })
    }
}
