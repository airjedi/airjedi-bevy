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
