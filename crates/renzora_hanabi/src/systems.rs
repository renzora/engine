//! Runtime systems for syncing HanabiEffect with bevy_hanabi ParticleEffect.

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use std::path::PathBuf;

use crate::builder::build_complete_effect;
use crate::data::*;
use renzora_core::CurrentProject;

/// Resolve an effect definition from its source.
fn resolve_effect_definition(source: &EffectSource, project: Option<&CurrentProject>) -> HanabiEffectDefinition {
    match source {
        EffectSource::Asset { path } => {
            if let Some(proj) = project {
                let full_path = proj.path.join(path);
                load_effect_from_file(&full_path).unwrap_or_default()
            } else {
                load_effect_from_file(&PathBuf::from(path)).unwrap_or_default()
            }
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
                HanabiEffectSynced { effect_handle },
            ));
        }
    }

    for (entity, _synced) in removed_query.iter() {
        commands.entity(entity).remove::<(
            ParticleEffect,
            CompiledParticleEffect,
            HanabiEffectSynced,
        )>();
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
) {
    for (entity, effect_data) in query.iter() {
        let definition = resolve_effect_definition(&effect_data.source, project.as_deref());
        let effect_asset = build_complete_effect(&definition);
        let effect_handle = effects.add(effect_asset);
        commands.entity(entity).try_insert((
            ParticleEffect::new(effect_handle.clone()),
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
    Burst { entity: Entity, count: u32 },
    SetRate { entity: Entity, multiplier: f32 },
    SetScale { entity: Entity, multiplier: f32 },
    SetTint { entity: Entity, r: f32, g: f32, b: f32, a: f32 },
    SetVariable { entity: Entity, name: String, value: EffectVariable },
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
                    if let Some(mut s) = spawner { s.active = true; }
                }
            }
            ParticleCommand::Pause(entity) => {
                if let Ok((mut data, spawner)) = effect_query.get_mut(entity) {
                    data.playing = false;
                    if let Some(mut s) = spawner { s.active = false; }
                }
            }
            ParticleCommand::Stop(entity) => {
                if let Ok((mut data, spawner)) = effect_query.get_mut(entity) {
                    data.playing = false;
                    if let Some(mut s) = spawner { s.active = false; s.reset(); }
                }
            }
            ParticleCommand::Reset(entity) => {
                if let Ok((_, spawner)) = effect_query.get_mut(entity) {
                    if let Some(mut s) = spawner { s.reset(); }
                }
            }
            ParticleCommand::Burst { entity, count: _ } => {
                if let Ok((_, spawner)) = effect_query.get_mut(entity) {
                    if let Some(mut s) = spawner { s.reset(); }
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
            ParticleCommand::SetVariable { entity, name, value } => {
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
