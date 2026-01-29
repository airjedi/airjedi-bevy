use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
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
                // Raw TCP address for ADS-B connection (host:port format)
                endpoint_url: "98.186.33.60:30003".to_string(),
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
        // Validate endpoint address (host:port format for raw TCP connection)
        let endpoint = self.endpoint_url.trim();
        if endpoint.is_empty() {
            return Err("Endpoint address is required".to_string());
        }
        // Check for host:port format
        let parts: Vec<&str> = endpoint.split(':').collect();
        if parts.len() != 2 {
            return Err("Endpoint must be in host:port format (e.g., 192.168.1.1:30003)".to_string());
        }
        if parts[1].parse::<u16>().is_err() {
            return Err("Port must be a valid number (1-65535)".to_string());
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

pub fn render_settings_panel(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<SettingsUiState>,
    mut app_config: ResMut<AppConfig>,
) {
    if !ui_state.open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::SidePanel::left("settings_panel")
        .default_width(300.0)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Settings");
            ui.separator();

            // Feed section
            ui.collapsing("Feed", |ui| {
                ui.label("Endpoint (host:port):");
                ui.text_edit_singleline(&mut ui_state.endpoint_url);
                ui.add_space(8.0);

                ui.label("Refresh Interval (ms):");
                ui.text_edit_singleline(&mut ui_state.refresh_interval_ms);
            });

            ui.add_space(12.0);

            // Map section
            ui.collapsing("Map Defaults", |ui| {
                ui.label("Default Latitude:");
                ui.text_edit_singleline(&mut ui_state.default_latitude);
                ui.add_space(8.0);

                ui.label("Default Longitude:");
                ui.text_edit_singleline(&mut ui_state.default_longitude);
                ui.add_space(8.0);

                ui.label("Default Zoom (0-19):");
                ui.text_edit_singleline(&mut ui_state.default_zoom);
            });

            ui.add_space(16.0);

            // Error message
            if let Some(ref error) = ui_state.error_message {
                ui.colored_label(egui::Color32::RED, error);
                ui.add_space(8.0);
            }

            // Buttons
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    ui_state.open = false;
                    ui_state.error_message = None;
                }

                if ui.button("Save").clicked() {
                    match ui_state.validate_and_build() {
                        Ok(new_config) => {
                            save_config(&new_config);
                            *app_config = new_config;
                            ui_state.open = false;
                            ui_state.error_message = None;
                            info!("Configuration saved");
                        }
                        Err(e) => {
                            ui_state.error_message = Some(e);
                        }
                    }
                }
            });
        });
}

pub fn toggle_settings_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<SettingsUiState>,
    app_config: Res<AppConfig>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        ui_state.open = !ui_state.open;
        if ui_state.open {
            ui_state.populate_from_config(&app_config);
        }
    }
}

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        let config = load_config();

        app.add_plugins(EguiPlugin::default())
            .insert_resource(config)
            .init_resource::<SettingsUiState>()
            .add_systems(Update, toggle_settings_panel)
            .add_systems(EguiPrimaryContextPass, render_settings_panel);
    }
}
