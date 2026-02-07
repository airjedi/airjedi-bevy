use bevy::prelude::*;
use std::collections::HashMap;

use crate::{constants, Aircraft, AircraftLabel};
use crate::aircraft::TrailHistory;
use super::connection::{AdsbAircraftData, ConnectionStatusText};

/// Resource to hold the aircraft icon texture handle
#[derive(Resource)]
pub struct AircraftTexture {
    pub handle: Handle<Image>,
}

/// Load the aircraft icon texture
pub fn setup_aircraft_texture(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("airplane1.png");
    commands.insert_resource(AircraftTexture { handle });
}

/// Sync aircraft entities from the shared ADS-B data.
/// This system runs every frame and updates Bevy entities to match the ADS-B client state.
pub fn sync_aircraft_from_adsb(
    mut commands: Commands,
    aircraft_texture: Option<Res<AircraftTexture>>,
    adsb_data: Option<Res<AdsbAircraftData>>,
    mut aircraft_query: Query<(Entity, &mut Aircraft, &mut Transform)>,
    label_query: Query<(Entity, &AircraftLabel)>,
) {
    let Some(adsb_data) = adsb_data else {
        return; // ADS-B client not yet initialized
    };
    let Some(aircraft_texture) = aircraft_texture else {
        return; // Aircraft texture not yet loaded
    };

    let adsb_aircraft = adsb_data.get_aircraft();

    // Build a map of existing aircraft entities by ICAO
    let mut existing_aircraft: HashMap<String, Entity> = aircraft_query
        .iter()
        .map(|(entity, aircraft, _)| (aircraft.icao.clone(), entity))
        .collect();

    // Update or spawn aircraft
    for adsb_ac in &adsb_aircraft {
        // Skip aircraft without position data
        let (Some(lat), Some(lon)) = (adsb_ac.latitude, adsb_ac.longitude) else {
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
            // Spawn new aircraft with sprite icon
            let aircraft_entity = commands
                .spawn((
                    Sprite {
                        image: aircraft_texture.handle.clone(),
                        custom_size: Some(Vec2::splat(constants::AIRCRAFT_MARKER_RADIUS * 4.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, constants::AIRCRAFT_Z_LAYER),
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
                .id();

            // Spawn label for this aircraft
            let callsign_display = adsb_ac.callsign.as_deref().unwrap_or(&adsb_ac.icao);
            let alt_display = adsb_ac
                .altitude
                .map(|a| format!("{} ft", a))
                .unwrap_or_default();
            let label_text = format!("{}\n{}", callsign_display, alt_display);

            commands.spawn((
                Text2d::new(label_text),
                TextFont {
                    font_size: constants::BASE_FONT_SIZE,
                    ..default()
                },
                TextColor(Color::WHITE),
                Transform::from_xyz(0.0, 0.0, constants::LABEL_Z_LAYER),
                AircraftLabel {
                    aircraft_entity,
                },
            ));
        }
    }

    // Remove aircraft that are no longer in the ADS-B data
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
