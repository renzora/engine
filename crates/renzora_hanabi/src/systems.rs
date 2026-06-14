//! Runtime systems for syncing HanabiEffect with bevy_hanabi ParticleEffect.

use bevy::prelude::*;
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_hanabi::prelude::*;
use bevy_hanabi::EffectMaterial;
use std::path::PathBuf;

use crate::builder::build_complete_effect;
use crate::data::*;
use renzora::CurrentProject;

/// A built-in soft radial sprite bound to every particle effect, so quads render
/// as soft round blobs (modulated by the particle color) instead of hard squares
/// — the cheapest, highest-impact "de-blocking" step. Generated procedurally so
/// there's no asset file / VFS path to resolve.
#[derive(Resource, Default)]
pub struct ParticleSoftTexture(pub Handle<Image>);

/// Create the soft radial sprite (grayscale falloff in all RGBA channels, so it
/// softens both additive RGB and alpha-blended effects via Modulate).
pub fn setup_soft_particle_texture(
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    let size = 64u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = (size as f32 - 1.0) * 0.5;
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - c) / c;
            let dy = (y as f32 - c) / c;
            let d = (dx * dx + dy * dy).sqrt();
            let mut a = (1.0 - d).clamp(0.0, 1.0);
            a = a * a * (3.0 - 2.0 * a); // smoothstep for a soft edge
            let v = (a * 255.0) as u8;
            let i = ((y * size + x) * 4) as usize;
            data[i] = v;
            data[i + 1] = v;
            data[i + 2] = v;
            data[i + 3] = v;
        }
    }
    let mut image = Image {
        data: Some(data),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::linear();
    commands.insert_resource(ParticleSoftTexture(images.add(image)));
}

/// Grayscale fbm noise bound as the 2nd texture of effects with `erosion`, used
/// by the `ErosionModifier` to dissolve particles in organic wisps as they fade.
#[derive(Resource, Default)]
pub struct ParticleErosionNoise(pub Handle<Image>);

fn noise_hash(x: i32, y: i32) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393).wrapping_add(y.wrapping_mul(668_265_263))) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h as f32) / (u32::MAX as f32)
}

fn value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (xf, yf) = (x - x.floor(), y - y.floor());
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let a = noise_hash(xi, yi);
    let b = noise_hash(xi + 1, yi);
    let c = noise_hash(xi, yi + 1);
    let d = noise_hash(xi + 1, yi + 1);
    let ab = a + (b - a) * u;
    let cd = c + (d - c) * u;
    ab + (cd - ab) * v
}

/// Create the erosion noise texture (4-octave fbm, grayscale in all channels).
pub fn setup_erosion_noise_texture(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let size = 128u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (mut f, mut amp, mut freq) = (0.0f32, 0.5f32, 4.0f32);
            for _ in 0..4 {
                let nx = x as f32 / size as f32 * freq;
                let ny = y as f32 / size as f32 * freq;
                f += value_noise(nx, ny) * amp;
                freq *= 2.0;
                amp *= 0.5;
            }
            let val = (f.clamp(0.0, 1.0) * 255.0) as u8;
            let i = ((y * size + x) * 4) as usize;
            data[i] = val;
            data[i + 1] = val;
            data[i + 2] = val;
            data[i + 3] = 255;
        }
    }
    let mut image = Image {
        data: Some(data),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::linear();
    commands.insert_resource(ParticleErosionNoise(images.add(image)));
}

/// Build the `EffectMaterial` image list for an effect: soft sprite in slot 0,
/// plus erosion noise in slot 1 when the effect uses erosion. The order MUST
/// match the texture slots declared in `build_complete_effect`.
fn effect_images(
    def: &HanabiEffectDefinition,
    soft: &ParticleSoftTexture,
    noise: &ParticleErosionNoise,
) -> Vec<Handle<Image>> {
    if def.erosion {
        vec![soft.0.clone(), noise.0.clone()]
    } else {
        vec![soft.0.clone()]
    }
}

/// Resolve an effect definition from its source.
fn resolve_effect_definition(
    source: &EffectSource,
    project: Option<&CurrentProject>,
) -> HanabiEffectDefinition {
    match source {
        EffectSource::Asset { path } => {
            // Prefer the VFS-aware byte loader so `.particle` files bundled in
            // an exported `.rpak` load correctly (the editor's disk read can't
            // see into the archive). Falls back to a direct disk read.
            if let Some(bytes) = renzora::core::load_asset_bytes(path) {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    if let Ok(def) = ron::from_str::<HanabiEffectDefinition>(text) {
                        return def;
                    }
                }
            }
            let disk = match project {
                Some(proj) => proj.path.join(path),
                None => PathBuf::from(path),
            };
            load_effect_from_file(&disk).unwrap_or_default()
        }
        EffectSource::Inline { definition } => definition.clone(),
    }
}

/// Marker component to track that we've created the hanabi effect for this entity.
#[derive(Component)]
pub struct HanabiEffectSynced {
    pub effect_handle: Handle<EffectAsset>,
}

/// Sync HanabiEffect with bevy_hanabi ParticleEffect.
pub fn sync_hanabi_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    query: Query<(Entity, &HanabiEffect, Option<&HanabiEffectSynced>), Changed<HanabiEffect>>,
    removed_query: Query<(Entity, &HanabiEffectSynced), Without<HanabiEffect>>,
    project: Option<Res<CurrentProject>>,
    soft: Res<ParticleSoftTexture>,
    noise: Res<ParticleErosionNoise>,
) {
    for (entity, effect_data, maybe_synced) in query.iter() {
        let definition = resolve_effect_definition(&effect_data.source, project.as_deref());
        let effect_asset = build_complete_effect(&definition);

        if let Some(synced) = maybe_synced {
            if let Some(existing) = effects.get_mut(&synced.effect_handle) {
                *existing = effect_asset;
            }
        } else {
            let effect_handle = effects.add(effect_asset);
            commands.entity(entity).try_insert((
                ParticleEffect::new(effect_handle.clone()),
                EffectMaterial { images: effect_images(&definition, &soft, &noise) },
                HanabiEffectSynced { effect_handle },
            ));
        }
    }

    for (entity, _synced) in removed_query.iter() {
        commands
            .entity(entity)
            .remove::<(ParticleEffect, CompiledParticleEffect, HanabiEffectSynced)>();
    }
}

/// Apply runtime overrides (play/pause) to particle effects.
pub fn apply_runtime_overrides(
    mut effects_query: Query<(&HanabiEffect, &mut EffectSpawner), Changed<HanabiEffect>>,
) {
    for (effect_data, mut spawner) in effects_query.iter_mut() {
        spawner.active = effect_data.playing;
    }
}

/// Rehydrate particle effects after scene load.
pub fn rehydrate_hanabi_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    query: Query<(Entity, &HanabiEffect), Without<HanabiEffectSynced>>,
    project: Option<Res<CurrentProject>>,
    soft: Res<ParticleSoftTexture>,
    noise: Res<ParticleErosionNoise>,
) {
    for (entity, effect_data) in query.iter() {
        let definition = resolve_effect_definition(&effect_data.source, project.as_deref());
        let effect_asset = build_complete_effect(&definition);
        let effect_handle = effects.add(effect_asset);
        commands.entity(entity).try_insert((
            ParticleEffect::new(effect_handle.clone()),
            EffectMaterial { images: effect_images(&definition, &soft, &noise) },
            HanabiEffectSynced { effect_handle },
        ));
    }
}

/// Command queue for particle script commands.
#[derive(Resource, Default)]
pub struct ParticleCommandQueue {
    pub commands: Vec<ParticleCommand>,
}

pub enum ParticleCommand {
    Play(Entity),
    Pause(Entity),
    Stop(Entity),
    Reset(Entity),
    Burst {
        entity: Entity,
        count: u32,
    },
    SetRate {
        entity: Entity,
        multiplier: f32,
    },
    SetScale {
        entity: Entity,
        multiplier: f32,
    },
    SetTint {
        entity: Entity,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },
    SetVariable {
        entity: Entity,
        name: String,
        value: EffectVariable,
    },
}

/// Process particle commands from scripts.
pub fn process_particle_commands(
    mut commands: ResMut<ParticleCommandQueue>,
    mut effect_query: Query<(&mut HanabiEffect, Option<&mut EffectSpawner>)>,
) {
    for cmd in commands.commands.drain(..) {
        match cmd {
            ParticleCommand::Play(entity) => {
                if let Ok((mut data, spawner)) = effect_query.get_mut(entity) {
                    data.playing = true;
                    if let Some(mut s) = spawner {
                        s.active = true;
                    }
                }
            }
            ParticleCommand::Pause(entity) => {
                if let Ok((mut data, spawner)) = effect_query.get_mut(entity) {
                    data.playing = false;
                    if let Some(mut s) = spawner {
                        s.active = false;
                    }
                }
            }
            ParticleCommand::Stop(entity) => {
                if let Ok((mut data, spawner)) = effect_query.get_mut(entity) {
                    data.playing = false;
                    if let Some(mut s) = spawner {
                        s.active = false;
                        s.reset();
                    }
                }
            }
            ParticleCommand::Reset(entity) => {
                if let Ok((_, Some(mut s))) = effect_query.get_mut(entity) {
                    s.reset();
                }
            }
            ParticleCommand::Burst { entity, count: _ } => {
                if let Ok((_, Some(mut s))) = effect_query.get_mut(entity) {
                    s.reset();
                }
            }
            ParticleCommand::SetRate { entity, multiplier } => {
                if let Ok((mut data, _)) = effect_query.get_mut(entity) {
                    data.rate_multiplier = multiplier;
                }
            }
            ParticleCommand::SetScale { entity, multiplier } => {
                if let Ok((mut data, _)) = effect_query.get_mut(entity) {
                    data.scale_multiplier = multiplier;
                }
            }
            ParticleCommand::SetTint { entity, r, g, b, a } => {
                if let Ok((mut data, _)) = effect_query.get_mut(entity) {
                    data.color_tint = [r, g, b, a];
                }
            }
            ParticleCommand::SetVariable {
                entity,
                name,
                value,
            } => {
                if let Ok((mut data, _)) = effect_query.get_mut(entity) {
                    data.variable_overrides.insert(name, value);
                }
            }
        }
    }
}

/// Hot reload: when .particle files are saved, update all entities referencing them.
pub fn hot_reload_saved_effects(
    mut editor_state: ResMut<ParticleEditorState>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut query: Query<(&mut HanabiEffect, Option<&HanabiEffectSynced>)>,
    project: Option<Res<CurrentProject>>,
) {
    if editor_state.recently_saved_paths.is_empty() {
        return;
    }

    let saved_paths: Vec<String> = editor_state.recently_saved_paths.drain(..).collect();

    for (mut effect_data, maybe_synced) in query.iter_mut() {
        if let EffectSource::Asset { path } = &effect_data.source {
            let matches = saved_paths.iter().any(|saved| {
                let saved_normalized = saved.replace('\\', "/");
                let path_normalized = path.replace('\\', "/");
                saved_normalized.ends_with(&path_normalized)
                    || path_normalized.ends_with(&saved_normalized)
                    || saved_normalized == path_normalized
            });

            if matches {
                let definition = resolve_effect_definition(&effect_data.source, project.as_deref());
                let effect_asset = build_complete_effect(&definition);

                if let Some(synced) = maybe_synced {
                    if let Some(existing) = effects.get_mut(&synced.effect_handle) {
                        *existing = effect_asset;
                    }
                }

                effect_data.set_changed();
            }
        }
    }
}
