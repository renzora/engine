//! Particle effect systems
//!
//! Runtime systems for syncing HanabiEffectData components with
//! bevy_hanabi ParticleEffect components.

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use std::path::PathBuf;

use super::builder::build_complete_effect;
use super::data::*;
use crate::project::CurrentProject;
use crate::ui::load_effect_from_file;

/// Resolve an effect definition from its source, loading from disk if needed
fn resolve_effect_definition(source: &EffectSource, project: Option<&CurrentProject>) -> HanabiEffectDefinition {
    match source {
        EffectSource::Asset { path } => {
            if let Some(proj) = project {
                let full_path = proj.path.join("assets").join(path);
                load_effect_from_file(&full_path).unwrap_or_default()
            } else {
                // Try as absolute path
                let full_path = PathBuf::from(path);
                load_effect_from_file(&full_path).unwrap_or_default()
            }
        }
        EffectSource::Inline { definition } => definition.clone(),
    }
}

/// Marker component to track that we've created the hanabi effect for this entity
#[derive(Component)]
pub struct HanabiEffectSynced {
    /// The handle to the effect asset we created
    pub effect_handle: Handle<EffectAsset>,
}

/// System to sync HanabiEffectData with bevy_hanabi ParticleEffect
///
/// When HanabiEffectData is added or changed, this system creates/updates
/// the corresponding EffectAsset and ParticleEffect components.
pub fn sync_hanabi_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    query: Query<
        (Entity, &HanabiEffectData, Option<&HanabiEffectSynced>),
        Changed<HanabiEffectData>,
    >,
    // Query for entities that lost HanabiEffectData
    removed_query: Query<(Entity, &HanabiEffectSynced), Without<HanabiEffectData>>,
    project: Option<Res<CurrentProject>>,
) {
    // Handle added/changed effects
    for (entity, effect_data, maybe_synced) in query.iter() {
        // Get the effect definition
        let definition = resolve_effect_definition(&effect_data.source, project.as_deref());

        // Build the effect asset
        let effect_asset = build_complete_effect(&definition);

        // If we already have a synced effect, update it
        if let Some(synced) = maybe_synced {
            if let Some(existing) = effects.get_mut(&synced.effect_handle) {
                *existing = effect_asset;
            }
        } else {
            // Create new effect asset and components
            let effect_handle = effects.add(effect_asset);

            // In bevy_hanabi 0.18, we just add ParticleEffect directly
            // The required components are added automatically by Bevy
            commands.entity(entity).insert((
                ParticleEffect::new(effect_handle.clone()),
                HanabiEffectSynced { effect_handle },
            ));
        }
    }

    // Clean up entities that lost HanabiEffectData
    for (entity, _synced) in removed_query.iter() {
        commands.entity(entity).remove::<(
            ParticleEffect,
            CompiledParticleEffect,
            HanabiEffectSynced,
        )>();
    }
}

/// System to apply runtime overrides to particle effects
///
/// Handles play/pause, rate multiplier, color tint, etc.
pub fn apply_runtime_overrides(
    mut effects_query: Query<
        (&HanabiEffectData, &mut EffectSpawner),
        Changed<HanabiEffectData>,
    >,
) {
    for (effect_data, mut spawner) in effects_query.iter_mut() {
        // Apply play/pause state by setting the active field directly
        spawner.active = effect_data.playing;
    }
}

/// Rehydrate particle effects after scene load
///
/// When a scene is loaded, entities have HanabiEffectData but not the
/// runtime ParticleEffect components. This system recreates them.
pub fn rehydrate_hanabi_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    query: Query<(Entity, &HanabiEffectData), Without<HanabiEffectSynced>>,
    project: Option<Res<CurrentProject>>,
) {
    for (entity, effect_data) in query.iter() {
        // Get the effect definition
        let definition = resolve_effect_definition(&effect_data.source, project.as_deref());

        // Build and add the effect
        let effect_asset = build_complete_effect(&definition);
        let effect_handle = effects.add(effect_asset);

        // In bevy_hanabi 0.18, just add ParticleEffect
        commands.entity(entity).insert((
            ParticleEffect::new(effect_handle.clone()),
            HanabiEffectSynced { effect_handle },
        ));
    }
}

/// Command queue for particle script commands
#[derive(Resource, Default)]
pub struct ParticleCommandQueue {
    pub commands: Vec<ParticleCommand>,
}

/// Commands that can be issued to particle effects from scripts
pub enum ParticleCommand {
    /// Start playing the effect
    Play(Entity),
    /// Pause the effect
    Pause(Entity),
    /// Stop and reset the effect
    Stop(Entity),
    /// Reset the effect to initial state
    Reset(Entity),
    /// Emit a burst of particles
    Burst { entity: Entity, count: u32 },
    /// Set the rate multiplier
    SetRate { entity: Entity, multiplier: f32 },
    /// Set the scale multiplier
    SetScale { entity: Entity, multiplier: f32 },
    /// Set the color tint
    SetTint { entity: Entity, r: f32, g: f32, b: f32, a: f32 },
    /// Set a custom variable
    SetVariable { entity: Entity, name: String, value: EffectVariable },
}

/// Process particle commands from scripts
pub fn process_particle_commands(
    mut commands: ResMut<ParticleCommandQueue>,
    mut effect_query: Query<(&mut HanabiEffectData, Option<&mut EffectSpawner>)>,
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
                if let Ok((_, spawner)) = effect_query.get_mut(entity) {
                    if let Some(mut s) = spawner {
                        s.reset();
                    }
                }
            }
            ParticleCommand::Burst { entity, count: _ } => {
                // Burst spawning would require more complex handling
                // For now, just trigger a reset which may cause a burst
                if let Ok((_, spawner)) = effect_query.get_mut(entity) {
                    if let Some(mut s) = spawner {
                        s.reset();
                    }
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

/// Hot reload system: when .effect files are saved, update all entities referencing them
pub fn hot_reload_saved_effects(
    mut editor_state: ResMut<ParticleEditorState>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut query: Query<(&mut HanabiEffectData, Option<&HanabiEffectSynced>)>,
    project: Option<Res<CurrentProject>>,
) {
    if editor_state.recently_saved_paths.is_empty() {
        return;
    }

    let saved_paths: Vec<String> = editor_state.recently_saved_paths.drain(..).collect();

    for (mut effect_data, maybe_synced) in query.iter_mut() {
        if let EffectSource::Asset { path } = &effect_data.source {
            // Check if this entity references any of the saved paths
            let matches = saved_paths.iter().any(|saved| {
                // Normalize for comparison: compare the asset-relative path portion
                let saved_normalized = saved.replace('\\', "/");
                let path_normalized = path.replace('\\', "/");
                saved_normalized.ends_with(&path_normalized) || path_normalized.ends_with(&saved_normalized) || saved_normalized == path_normalized
            });

            if matches {
                // Reload the definition from disk
                let definition = resolve_effect_definition(&effect_data.source, project.as_deref());
                let effect_asset = build_complete_effect(&definition);

                // Update the existing asset handle if synced
                if let Some(synced) = maybe_synced {
                    if let Some(existing) = effects.get_mut(&synced.effect_handle) {
                        *existing = effect_asset;
                    }
                }

                // Trigger change detection so sync_hanabi_effects picks it up if needed
                effect_data.set_changed();
            }
        }
    }
}
