use bevy::prelude::*;
use bevy::picking::prelude::*;

use super::detail_panel::CameraFollowState;
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
    mut follow_state: ResMut<CameraFollowState>,
) {
    // The observer is attached to the aircraft entity, so we use observer()
    // to get the entity this observer belongs to.
    let aircraft_entity = event.observer();

    if let Ok(aircraft) = aircraft_query.get(aircraft_entity) {
        info!("Aircraft clicked: {}", aircraft.icao);
        list_state.selected_icao = Some(aircraft.icao.clone());
        follow_state.following_icao = Some(aircraft.icao.clone());
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

/// Observer for ground plane clicks — clears the current selection.
pub fn on_ground_click(
    _event: On<Pointer<Click>>,
    mut list_state: ResMut<AircraftListState>,
    mut follow_state: ResMut<CameraFollowState>,
) {
    if list_state.selected_icao.is_some() {
        info!("Ground clicked, clearing selection");
        list_state.selected_icao = None;
        follow_state.following_icao = None;
    }
}

/// System that clears selection when ESC is pressed.
pub fn deselect_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut list_state: ResMut<AircraftListState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        if list_state.selected_icao.is_some() {
            list_state.selected_icao = None;
        }
    }
}

/// System that clears selection when the selected aircraft no longer exists.
pub fn clear_stale_selection(
    mut list_state: ResMut<AircraftListState>,
    mut follow_state: ResMut<CameraFollowState>,
    aircraft_query: Query<&Aircraft>,
) {
    let Some(ref selected_icao) = list_state.selected_icao else {
        return;
    };

    let still_exists = aircraft_query.iter().any(|a| a.icao == *selected_icao);
    if !still_exists {
        info!("Selected aircraft {} no longer exists, clearing selection", selected_icao);
        list_state.selected_icao = None;
        follow_state.following_icao = None;
    }
}

/// System that keeps SelectionOutline marker in sync with AircraftListState.
/// Runs every frame but only does work when selected_icao changes.
pub fn manage_selection_outline(
    mut commands: Commands,
    list_state: Res<AircraftListState>,
    aircraft_query: Query<(Entity, &Aircraft)>,
    selected_query: Query<Entity, With<SelectionOutline>>,
) {
    if !list_state.is_changed() {
        return;
    }

    // Remove SelectionOutline from all currently selected entities
    for entity in selected_query.iter() {
        commands.entity(entity).remove::<SelectionOutline>();
    }

    // Add SelectionOutline to the newly selected aircraft
    if let Some(ref selected_icao) = list_state.selected_icao {
        for (entity, aircraft) in aircraft_query.iter() {
            if aircraft.icao == *selected_icao {
                commands.entity(entity).insert(SelectionOutline);
                break;
            }
        }
    }
}

/// System that swaps materials on child meshes of aircraft with SelectionOutline or HoverOutline.
/// SelectionOutline takes priority over HoverOutline.
pub fn swap_outline_materials(
    mut outline_mats: ResMut<OutlineMaterials>,
    selected_query: Query<&Children, With<SelectionOutline>>,
    hover_query: Query<&Children, (With<HoverOutline>, Without<SelectionOutline>)>,
    normal_query: Query<
        (Entity, &Children),
        (With<Aircraft>, Without<SelectionOutline>, Without<HoverOutline>),
    >,
    children_query: Query<&Children>,
    mesh_query: Query<&MeshMaterial3d<StandardMaterial>>,
    mut commands: Commands,
) {
    let selected_mat = outline_mats.selected.clone();
    let hover_mat = outline_mats.hover.clone();

    // Apply selected material to selected aircraft children
    for children in selected_query.iter() {
        apply_material_to_hierarchy(
            children,
            &children_query,
            &mesh_query,
            &mut outline_mats.originals,
            &selected_mat,
            &mut commands,
        );
    }

    // Apply hover material to hovered (non-selected) aircraft children
    for children in hover_query.iter() {
        apply_material_to_hierarchy(
            children,
            &children_query,
            &mesh_query,
            &mut outline_mats.originals,
            &hover_mat,
            &mut commands,
        );
    }

    // Restore original materials for normal (non-selected, non-hovered) aircraft
    for (entity, children) in normal_query.iter() {
        restore_materials_in_hierarchy(
            entity,
            children,
            &children_query,
            &mesh_query,
            &mut outline_mats.originals,
            &mut commands,
        );
    }
}

/// Recursively apply outline material to all meshes in the hierarchy.
fn apply_material_to_hierarchy(
    children: &Children,
    children_query: &Query<&Children>,
    mesh_query: &Query<&MeshMaterial3d<StandardMaterial>>,
    originals: &mut Vec<(Entity, Handle<StandardMaterial>)>,
    outline_mat: &Handle<StandardMaterial>,
    commands: &mut Commands,
) {
    for child in children.iter() {
        if let Ok(mat_handle) = mesh_query.get(child) {
            // Stash the original material if not already stashed
            if !originals.iter().any(|(e, _)| *e == child) {
                originals.push((child, mat_handle.0.clone()));
            }
            // Replace with outline material
            if mat_handle.0 != *outline_mat {
                commands
                    .entity(child)
                    .insert(MeshMaterial3d(outline_mat.clone()));
            }
        }
        if let Ok(grandchildren) = children_query.get(child) {
            apply_material_to_hierarchy(
                grandchildren,
                children_query,
                mesh_query,
                originals,
                outline_mat,
                commands,
            );
        }
    }
}

/// Recursively restore original materials for all meshes in the hierarchy.
fn restore_materials_in_hierarchy(
    aircraft_entity: Entity,
    children: &Children,
    children_query: &Query<&Children>,
    mesh_query: &Query<&MeshMaterial3d<StandardMaterial>>,
    originals: &mut Vec<(Entity, Handle<StandardMaterial>)>,
    commands: &mut Commands,
) {
    for child in children.iter() {
        if mesh_query.get(child).is_ok() {
            // Find and restore the original material
            if let Some(pos) = originals.iter().position(|(e, _)| *e == child) {
                let (_, original_mat) = originals.remove(pos);
                commands
                    .entity(child)
                    .insert(MeshMaterial3d(original_mat));
            }
        }
        if let Ok(grandchildren) = children_query.get(child) {
            restore_materials_in_hierarchy(
                aircraft_entity,
                grandchildren,
                children_query,
                mesh_query,
                originals,
                commands,
            );
        }
    }
}
