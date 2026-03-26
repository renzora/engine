//! Particle effect preview mode.

use bevy::prelude::*;

use crate::bridge::{PreviewCommand, PreviewCommandQueue};
use crate::scene::PreviewSubject;
use super::PreviewMode;

#[derive(Component)]
pub struct PreviewParticle;

#[derive(Resource, Default)]
pub struct ParticlePreviewState {
    pub loaded: bool,
}

fn handle_particle_commands(
    mut queue: ResMut<PreviewCommandQueue>,
    mut state: ResMut<ParticlePreviewState>,
    mut commands: Commands,
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

                match serde_json::from_str::<renzora_hanabi::HanabiEffectDefinition>(&event.definition) {
                    Ok(definition) => {
                        commands.spawn((
                            Transform::IDENTITY,
                            Visibility::Visible,
                            PreviewParticle,
                            renzora_hanabi::HanabiEffect {
                                source: renzora_hanabi::EffectSource::Inline { definition },
                                playing: true,
                                ..default()
                            },
                        ));

                        state.loaded = true;
                        next_mode.set(PreviewMode::Particle);
                        info!("[preview] Particle effect loaded");
                    }
                    Err(err) => {
                        warn!("[preview] Failed to parse particle definition: {err}");
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
