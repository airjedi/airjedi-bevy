use bevy::prelude::*;
use std::collections::HashMap;

use crate::{constants, Aircraft, AircraftLabel};
use crate::aircraft::TrailHistory;
use crate::aircraft::picking::{on_aircraft_click, on_aircraft_hover, on_aircraft_out};
use crate::debug_panel::DebugPanelState;
use super::connection::{AdsbAircraftData, ConnectionStatusText};

use crate::theme::AppTheme;

/// Type codes that should use the B737 model
const B737_TYPES: &[&str] = &[
    "B731", "B732", "B733", "B734", "B735", "B736", "B737", "B738", "B739",
    "B37M", "B38M", "B39M",
];

/// Resource holding aircraft 3D model handles keyed by type code
#[derive(Resource)]
pub struct AircraftModelRegistry {
    pub default_model: Handle<Scene>,
    pub type_models: HashMap<String, Handle<Scene>>,
}

impl AircraftModelRegistry {
    /// Get the model handle for a given type code, falling back to the default
    pub fn get_model(&self, type_code: Option<&str>) -> Handle<Scene> {
        if let Some(code) = type_code {
            if let Some(handle) = self.type_models.get(code) {
                return handle.clone();
            }
        }
        self.default_model.clone()
    }
}

/// Load aircraft 3D models and build the registry.
/// The default GLB is loaded with MAIN_WORLD asset usage so mesh data
/// is retained on the CPU for picking raycasts (not just uploaded to GPU).
pub fn setup_aircraft_models(mut commands: Commands, asset_server: Res<AssetServer>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::gltf::GltfLoaderSettings;

    let default_model = asset_server.load_with_settings(
        "airplane.glb#Scene0",
        |settings: &mut GltfLoaderSettings| {
            settings.load_meshes = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
        },
    );
    let b737_model: Handle<Scene> = asset_server.load("models/b737/78349.obj");

    let mut type_models = HashMap::new();
    for code in B737_TYPES {
        type_models.insert(code.to_string(), b737_model.clone());
    }

    commands.insert_resource(AircraftModelRegistry {
        default_model,
        type_models,
    });
}

/// Sync aircraft entities from the shared ADS-B data.
/// This system runs every frame and updates Bevy entities to match the ADS-B client state.
pub fn sync_aircraft_from_adsb(
    mut commands: Commands,
    model_registry: Option<Res<AircraftModelRegistry>>,
    adsb_data: Option<Res<AdsbAircraftData>>,
    mut aircraft_query: Query<(Entity, &mut Aircraft, &mut Transform)>,
    label_query: Query<(Entity, &AircraftLabel)>,
    mut debug: Option<ResMut<DebugPanelState>>,
    theme: Res<AppTheme>,
    type_db: Option<Res<crate::aircraft::AircraftTypeDatabase>>,
) {
    let Some(adsb_data) = adsb_data else {
        return; // ADS-B client not yet initialized
    };
    let Some(model_registry) = model_registry else {
        return; // Aircraft model registry not yet loaded
    };

    // Use try_get to avoid blocking the main thread if the background ADS-B
    // thread currently holds the lock. We'll just skip this frame and retry next frame.
    let Some(adsb_aircraft) = adsb_data.try_get_aircraft() else {
        return;
    };

    // Build a map of existing aircraft entities by ICAO
    let mut existing_aircraft: HashMap<String, Entity> = aircraft_query
        .iter()
        .map(|(entity, aircraft, _)| (aircraft.icao.clone(), entity))
        .collect();

    // Update or spawn aircraft
    for adsb_ac in &adsb_aircraft {
        if let Some(ref mut dbg) = debug {
            dbg.messages_processed += 1;
        }

        // Skip aircraft without position data
        let (Some(lat), Some(lon)) = (adsb_ac.latitude, adsb_ac.longitude) else {
            if let Some(ref mut dbg) = debug {
                dbg.positions_rejected += 1;
            }
            continue;
        };

        if let Some(&entity) = existing_aircraft.get(&adsb_ac.icao) {
            // Update existing aircraft
            if let Ok((_, mut aircraft, _)) = aircraft_query.get_mut(entity) {
                aircraft.latitude = lat;
                aircraft.longitude = lon;
                aircraft.altitude = adsb_ac.altitude;
                aircraft.heading = adsb_ac.track.map(|t| t as f32);
                aircraft.velocity = adsb_ac.velocity;
                aircraft.vertical_rate = adsb_ac.vertical_rate;
                aircraft.callsign = adsb_ac.callsign.clone();
                aircraft.squawk = None;
            }
            existing_aircraft.remove(&adsb_ac.icao);
        } else {
            // Log new aircraft
            if let Some(ref mut dbg) = debug {
                let callsign = adsb_ac.callsign.as_deref().unwrap_or("?");
                dbg.push_log(format!("New aircraft: {} ({})", adsb_ac.icao, callsign));
            }
            // Spawn new aircraft with 3D model
            let aircraft_name = adsb_ac.callsign.as_deref().unwrap_or(&adsb_ac.icao);

            // Look up type code for model selection
            let type_code = type_db
                .as_ref()
                .and_then(|db| db.lookup(&adsb_ac.icao))
                .and_then(|info| info.type_code.clone());

            let model_handle = model_registry.get_model(type_code.as_deref());

            let aircraft_entity = commands
                .spawn((
                    Name::new(format!("Aircraft: {}", aircraft_name)),
                    SceneRoot(model_handle),
                    Transform::from_xyz(0.0, 0.0, constants::AIRCRAFT_Z_LAYER),
                    Pickable::default(),
                    Aircraft {
                        icao: adsb_ac.icao.clone(),
                        callsign: adsb_ac.callsign.clone(),
                        latitude: lat,
                        longitude: lon,
                        altitude: adsb_ac.altitude,
                        heading: adsb_ac.track.map(|t| t as f32),
                        velocity: adsb_ac.velocity,
                        vertical_rate: adsb_ac.vertical_rate,
                        squawk: None,
                    },
                    TrailHistory::default(),
                ))
                .observe(on_aircraft_click)
                .observe(on_aircraft_hover)
                .observe(on_aircraft_out)
                .id();

            // Spawn label for this aircraft
            let callsign_display = adsb_ac.callsign.as_deref().unwrap_or(&adsb_ac.icao);
            let alt_display = adsb_ac
                .altitude
                .map(|a| format!("{} ft", a))
                .unwrap_or_default();
            let label_text = format!("{}\n{}", callsign_display, alt_display);

            commands.spawn((
                Name::new(format!("Label: {}", aircraft_name)),
                Text2d::new(label_text),
                TextFont {
                    font_size: constants::BASE_FONT_SIZE,
                    ..default()
                },
                TextColor(theme.text_primary()),
                Transform::from_xyz(0.0, 0.0, constants::LABEL_Z_LAYER),
                AircraftLabel {
                    aircraft_entity,
                },
            ));
        }
    }

    // Remove aircraft that are no longer in the ADS-B data
    for (icao, _entity) in &existing_aircraft {
        if let Some(ref mut dbg) = debug {
            dbg.push_log(format!("Removed aircraft: {}", icao));
        }
    }
    for (icao, entity) in existing_aircraft {
        // Find and despawn the label first
        for (label_entity, label) in label_query.iter() {
            if label.aircraft_entity == entity {
                commands.entity(label_entity).despawn();
                break;
            }
        }
        commands.entity(entity).despawn();
        info!("Removed aircraft {} from display", icao);
    }
}

/// Update aircraft labels with current data
pub fn update_aircraft_label_text(
    aircraft_query: Query<&Aircraft>,
    mut label_query: Query<(&AircraftLabel, &mut Text2d)>,
) {
    for (label, mut text) in label_query.iter_mut() {
        if let Ok(aircraft) = aircraft_query.get(label.aircraft_entity) {
            let callsign_display = aircraft.callsign.as_deref().unwrap_or(&aircraft.icao);
            let alt_display = aircraft
                .altitude
                .map(|a| format!("{} ft", a))
                .unwrap_or_default();
            **text = format!("{}\n{}", callsign_display, alt_display);
        }
    }
}
