use bevy::prelude::*;
use bevy::picking::prelude::*;

use super::list_panel::AircraftListState;
use crate::Aircraft;

/// Marker component added to aircraft entities when selected via click.
#[derive(Component)]
pub struct SelectionOutline;

/// Marker component added to aircraft entities when hovered.
#[derive(Component)]
pub struct HoverOutline;

/// Pre-built materials for selection and hover outlines.
#[derive(Resource)]
pub struct OutlineMaterials {
    /// Bright cyan material for selected aircraft.
    pub selected: Handle<StandardMaterial>,
    /// Dimmer cyan material for hovered aircraft.
    pub hover: Handle<StandardMaterial>,
    /// Original materials stashed before outline swap, keyed by entity.
    pub originals: Vec<(Entity, Handle<StandardMaterial>)>,
}

/// Startup system that creates the outline materials.
pub fn setup_outline_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let selected = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 1.0),
        emissive: LinearRgba::new(0.0, 4.0, 4.0, 1.0),
        unlit: false,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });

    let hover = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.6, 0.6),
        emissive: LinearRgba::new(0.0, 1.5, 1.5, 1.0),
        unlit: false,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });

    commands.insert_resource(OutlineMaterials {
        selected,
        hover,
        originals: Vec::new(),
    });
}

/// Observer triggered when an aircraft entity is clicked.
/// Since Pointer events auto-propagate up the hierarchy, clicks on child
/// mesh entities bubble up to the aircraft entity where this observer lives.
pub fn on_aircraft_click(
    event: On<Pointer<Click>>,
    aircraft_query: Query<&Aircraft>,
    mut list_state: ResMut<AircraftListState>,
) {
    // The observer is attached to the aircraft entity, so we use observer()
    // to get the entity this observer belongs to.
    let aircraft_entity = event.observer();

    if let Ok(aircraft) = aircraft_query.get(aircraft_entity) {
        info!("Aircraft clicked: {}", aircraft.icao);
        list_state.selected_icao = Some(aircraft.icao.clone());
    }
}

/// Observer triggered when the pointer enters an aircraft entity.
pub fn on_aircraft_hover(
    event: On<Pointer<Over>>,
    mut commands: Commands,
    hover_query: Query<(), With<HoverOutline>>,
) {
    let aircraft_entity = event.observer();

    if hover_query.get(aircraft_entity).is_err() {
        commands.entity(aircraft_entity).insert(HoverOutline);
    }
}

/// Observer triggered when the pointer leaves an aircraft entity.
pub fn on_aircraft_out(
    event: On<Pointer<Out>>,
    mut commands: Commands,
    hover_query: Query<(), With<HoverOutline>>,
) {
    let aircraft_entity = event.observer();

    if hover_query.get(aircraft_entity).is_ok() {
        commands.entity(aircraft_entity).remove::<HoverOutline>();
    }
}
