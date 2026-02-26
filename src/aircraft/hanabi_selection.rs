use bevy::prelude::*;
use bevy::image::{ImageSampler, ImageAddressMode, ImageFilterMode, ImageSamplerDescriptor};
use bevy_hanabi::prelude::{self as hanabi, *};

use super::picking::SelectionOutline;
use crate::Aircraft;

/// Marker linking a selection particle effect to its aircraft.
#[derive(Component)]
pub struct SelectionFog {
    pub effect_entity: Entity,
}

/// Resource holding the pre-built selection effect asset handle and fog texture.
#[derive(Resource)]
pub struct FogEffectHandle {
    pub effect: Handle<EffectAsset>,
    pub texture: Handle<Image>,
}

/// Create a soft radial gradient texture for fog particles.
/// White center fading to transparent edges — eliminates visible square borders.
fn create_fog_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let size = 64u32;
    let center = size as f32 / 2.0;
    let mut data = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center + 0.5;
            let dy = y as f32 - center + 0.5;
            let dist = (dx * dx + dy * dy).sqrt() / center;

            // Smooth radial falloff: solid center, soft edges
            let alpha = if dist >= 1.0 {
                0.0
            } else {
                // Cubic falloff for soft, organic edges
                let t = 1.0 - dist;
                t * t * t
            };

            let idx = ((y * size + x) * 4) as usize;
            data[idx] = 255;     // R
            data[idx + 1] = 255; // G
            data[idx + 2] = 255; // B
            data[idx + 3] = (alpha * 255.0) as u8; // A
        }
    }

    let mut image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        default(),
    );

    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });

    images.add(image)
}

/// Startup system that creates the purple fog sphere selection effect.
pub fn setup_fog_effect(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut images: ResMut<Assets<Image>>,
) {
    let fog_texture = create_fog_texture(&mut images);

    let writer = ExprWriter::new();

    // Single particle at center — the soft texture does all the visual work
    let init_pos = SetAttributeModifier::new(
        Attribute::POSITION,
        writer.lit(Vec3::ZERO).expr(),
    );

    // No velocity — particle stays centered
    let init_vel = SetAttributeModifier::new(
        Attribute::VELOCITY,
        writer.lit(Vec3::ZERO).expr(),
    );

    // Long lifetime, continuously respawned for seamless presence
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(2.0).expr(),
    );
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());

    // Purple fog color — brighter and more visible
    let mut gradient = hanabi::Gradient::new();
    gradient.add_key(0.0, Vec4::new(0.45, 0.1, 0.65, 0.0));   // transparent start
    gradient.add_key(0.1, Vec4::new(0.5, 0.15, 0.7, 0.55));    // fade in
    gradient.add_key(0.9, Vec4::new(0.5, 0.15, 0.7, 0.55));    // hold
    gradient.add_key(1.0, Vec4::new(0.45, 0.1, 0.65, 0.0));    // fade out

    // Large particle to wrap the aircraft — model is ~4 units but scale is 8x
    let mut size_gradient = hanabi::Gradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.0));
    size_gradient.add_key(0.1, Vec3::splat(24.0));
    size_gradient.add_key(0.9, Vec3::splat(24.0));
    size_gradient.add_key(1.0, Vec3::splat(0.0));

    // Texture slot
    let texture_slot = writer.lit(0u32).expr();

    let mut module = writer.finish();
    module.add_texture_slot("fog");

    let texture_mod = ParticleTextureModifier::new(texture_slot);

    // Always face camera
    let orient = OrientModifier::new(OrientMode::ParallelCameraDepthPlane);

    // Spawn 2 particles staggered (so one is always at full opacity while the
    // other cycles) — gives seamless coverage with zero flicker
    let effect = EffectAsset::new(4, SpawnerSettings::rate(1.0.into()), module)
        .with_name("SelectionFog")
        .with_simulation_space(SimulationSpace::Local)
        .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .render(ColorOverLifetimeModifier {
            gradient,
            blend: ColorBlendMode::Overwrite,
            mask: ColorBlendMask::RGBA,
        })
        .render(SizeOverLifetimeModifier {
            gradient: size_gradient,
            screen_space_size: false,
        })
        .render(texture_mod)
        .render(orient);

    let handle = effects.add(effect);
    commands.insert_resource(FogEffectHandle {
        effect: handle,
        texture: fog_texture,
    });
}

/// System that spawns/despawns fog particle effects based on SelectionOutline presence.
pub fn manage_selection_fog(
    mut commands: Commands,
    fog_handle: Res<FogEffectHandle>,
    selected_aircraft: Query<Entity, (With<Aircraft>, With<SelectionOutline>, Without<SelectionFog>)>,
    deselected_aircraft: Query<(Entity, &SelectionFog), (With<Aircraft>, Without<SelectionOutline>)>,
) {
    for aircraft_entity in selected_aircraft.iter() {
        let effect_entity = commands
            .spawn((
                ParticleEffect::new(fog_handle.effect.clone()),
                EffectMaterial {
                    images: vec![fog_handle.texture.clone()],
                },
                Transform::default(),
            ))
            .id();

        commands
            .entity(aircraft_entity)
            .insert(SelectionFog { effect_entity });
    }

    for (aircraft_entity, fog) in deselected_aircraft.iter() {
        commands.entity(fog.effect_entity).despawn();
        commands
            .entity(aircraft_entity)
            .remove::<SelectionFog>();
    }
}

/// System that syncs fog effect position to its aircraft's global transform.
pub fn sync_fog_position(
    aircraft_query: Query<(&GlobalTransform, &SelectionFog)>,
    mut effect_query: Query<&mut Transform, With<ParticleEffect>>,
) {
    for (global_tf, fog) in aircraft_query.iter() {
        if let Ok(mut effect_tf) = effect_query.get_mut(fog.effect_entity) {
            let (scale, rotation, translation) = global_tf.to_scale_rotation_translation();
            effect_tf.translation = translation;
            effect_tf.rotation = rotation;
            effect_tf.scale = scale;
        }
    }
}
