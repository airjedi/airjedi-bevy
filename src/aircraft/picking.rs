use bevy::prelude::*;

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
