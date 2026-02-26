use bevy::prelude::*;
use bevy_hanabi::prelude::{self as hanabi, *};

use super::picking::SelectionOutline;
use crate::Aircraft;

/// Marker linking a selection fog particle effect to its aircraft.
#[derive(Component)]
pub struct SelectionFog {
    pub effect_entity: Entity,
}

/// Resource holding the pre-built fog effect asset handle.
#[derive(Resource)]
pub struct FogEffectHandle(pub Handle<EffectAsset>);

/// Startup system that creates the fog effect asset once and stores the handle.
pub fn setup_fog_effect(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
) {
    let mut module = Module::default();

    let center = module.lit(Vec3::ZERO);
    let sphere_radius = module.lit(20.0);

    let init_pos = SetPositionSphereModifier {
        center,
        radius: sphere_radius,
        dimension: ShapeDimension::Surface,
    };

    let lifetime = module.lit(1.5);
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    // Subtle cyan fog that fades in and out
    let mut gradient = hanabi::Gradient::new();
    gradient.add_key(0.0, Vec4::new(0.0, 0.8, 1.0, 0.0));
    gradient.add_key(0.2, Vec4::new(0.0, 0.8, 1.0, 0.15));
    gradient.add_key(0.8, Vec4::new(0.0, 0.8, 1.0, 0.15));
    gradient.add_key(1.0, Vec4::new(0.0, 0.8, 1.0, 0.0));

    let conform = ConformToSphereModifier::new(
        center,
        sphere_radius,
        module.lit(5.0),  // influence_dist
        module.lit(10.0), // attraction_accel
        module.lit(2.0),  // max_attraction_speed
    );

    let effect = EffectAsset::new(256, SpawnerSettings::rate(100.0.into()), module)
        .with_name("SelectionFog")
        .with_simulation_space(SimulationSpace::Local)
        .init(init_pos)
        .init(init_lifetime)
        .update(conform)
        .render(ColorOverLifetimeModifier {
            gradient,
            blend: ColorBlendMode::Overwrite,
            mask: ColorBlendMask::RGBA,
        });

    let handle = effects.add(effect);
    commands.insert_resource(FogEffectHandle(handle));
}

/// System that spawns/despawns fog particle effects based on SelectionOutline presence.
pub fn manage_selection_fog(
    mut commands: Commands,
    fog_handle: Res<FogEffectHandle>,
    selected_aircraft: Query<Entity, (With<Aircraft>, With<SelectionOutline>, Without<SelectionFog>)>,
    deselected_aircraft: Query<(Entity, &SelectionFog), (With<Aircraft>, Without<SelectionOutline>)>,
) {
    // Spawn fog for newly selected aircraft
    for aircraft_entity in selected_aircraft.iter() {
        let effect_entity = commands
            .spawn(ParticleEffect::new(fog_handle.0.clone()))
            .id();

        // Attach as child so it inherits the aircraft transform
        commands.entity(aircraft_entity).add_child(effect_entity);
        commands
            .entity(aircraft_entity)
            .insert(SelectionFog { effect_entity });
    }

    // Despawn fog for deselected aircraft
    for (aircraft_entity, fog) in deselected_aircraft.iter() {
        commands.entity(fog.effect_entity).despawn();
        commands
            .entity(aircraft_entity)
            .remove::<SelectionFog>();
    }
}
