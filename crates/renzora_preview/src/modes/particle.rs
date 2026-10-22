//! Particle effect preview mode.
//!
//! The page hands over an effect's text. That is a `.vfx` document for an
//! effect made with the current particle system, and for one listed before it
//! the old Hanabi definition, which the page has always sent as JSON (a
//! `.particle` file's RON is accepted too). Both play through
//! `renzora_particles`: the old one is converted on the spot by the same code
//! that loads an old `.particle` in the engine, so a listing never has to be
//! re-uploaded for its preview to keep working.

use bevy::prelude::*;
use renzora_particles::convert::format::HanabiEffectDefinition;
use renzora_particles::{ParticleSystem, VfxAsset};

use super::PreviewMode;
use crate::bridge::{PreviewCommand, PreviewCommandQueue};
use crate::scene::PreviewSubject;

#[derive(Component)]
pub struct PreviewParticle;

#[derive(Resource, Default)]
pub struct ParticlePreviewState {
    pub loaded: bool,
}

/// The effect in `text`, in whichever of the accepted forms it is.
fn parse_effect(text: &str) -> Result<VfxAsset, String> {
    let vfx_error = match renzora_particles::asset::parse_vfx(text) {
        // An empty document also parses as a `.vfx` (every field defaults),
        // so a result with no emitters is not taken as a match: an old
        // definition gets its chance first.
        Ok(asset) if !asset.emitters.is_empty() => return Ok(asset),
        Ok(_) => "the effect has no emitters".to_string(),
        Err(e) => e.to_string(),
    };
    let old = serde_json::from_str::<HanabiEffectDefinition>(text)
        .map_err(|e| e.to_string())
        .or_else(|_| renzora_particles::convert::parse_particle(text));
    match old {
        Ok(definition) => Ok(renzora_particles::convert::convert_definition(&definition).asset),
        Err(_) => Err(format!("not a .vfx effect ({vfx_error}) nor an old particle definition")),
    }
}

fn handle_particle_commands(
    mut queue: ResMut<PreviewCommandQueue>,
    mut state: ResMut<ParticlePreviewState>,
    mut commands: Commands,
    mut assets: ResMut<Assets<VfxAsset>>,
    particle_q: Query<Entity, With<PreviewParticle>>,
    subject_q: Query<Entity, With<PreviewSubject>>,
    mut next_mode: ResMut<NextState<PreviewMode>>,
) {
    let mut remaining = Vec::new();

    for cmd in queue.commands.drain(..) {
        match cmd {
            PreviewCommand::LoadParticle(event) => {
                for entity in particle_q.iter() {
                    commands.entity(entity).despawn();
                }
                for entity in subject_q.iter() {
                    commands.entity(entity).insert(Visibility::Hidden);
                }

                match parse_effect(&event.definition) {
                    Ok(asset) => {
                        commands.spawn((
                            Transform::IDENTITY,
                            Visibility::Visible,
                            PreviewParticle,
                            ParticleSystem::from_handle(assets.add(asset)),
                        ));

                        state.loaded = true;
                        next_mode.set(PreviewMode::Particle);
                        info!("[preview] Particle effect loaded");
                    }
                    Err(err) => {
                        warn!("[preview] Failed to parse particle effect: {err}");
                    }
                }
            }
            other => remaining.push(other),
        }
    }

    queue.commands = remaining;
}

fn on_exit_particle(
    mut commands: Commands,
    particle_q: Query<Entity, With<PreviewParticle>>,
    subject_q: Query<Entity, With<PreviewSubject>>,
) {
    for entity in particle_q.iter() {
        commands.entity(entity).despawn();
    }
    for entity in subject_q.iter() {
        commands.entity(entity).insert(Visibility::Visible);
    }
}

pub struct ParticlePreviewPlugin;

impl Plugin for ParticlePreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticlePreviewState>()
            .add_systems(Update, handle_particle_commands)
            .add_systems(OnExit(PreviewMode::Particle), on_exit_particle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_json_definition_still_previews() {
        let json = serde_json::to_string(&HanabiEffectDefinition { name: "Old".into(), ..Default::default() }).unwrap();
        let asset = parse_effect(&json).expect("old JSON converts");
        assert_eq!(asset.name, "Old");
        assert_eq!(asset.emitters.len(), 1);
    }

    #[test]
    fn a_vfx_document_previews_as_itself() {
        let text = renzora_particles::presets::PRESETS[0].source;
        assert_eq!(parse_effect(text).unwrap(), renzora_particles::asset::parse_vfx(text).unwrap());
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse_effect("not an effect").is_err());
    }
}
