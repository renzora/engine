//! Animation preview mode.

use bevy::prelude::*;

use crate::bridge::{PreviewCommand, PreviewCommandQueue};
use crate::scene::PreviewSubject;
use super::PreviewMode;
use super::model::{PreviewModel, ModelPreviewState};

#[derive(Resource, Default)]
pub struct AnimationPreviewState {
    pub initialized: bool,
}

fn handle_animation_commands(
    mut queue: ResMut<PreviewCommandQueue>,
    mut state: ResMut<AnimationPreviewState>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    model_q: Query<Entity, With<PreviewModel>>,
    subject_q: Query<Entity, With<PreviewSubject>>,
    mut next_mode: ResMut<NextState<PreviewMode>>,
    mut model_state: ResMut<ModelPreviewState>,
) {
    let mut remaining = Vec::new();

    for cmd in queue.commands.drain(..) {
        match cmd {
            PreviewCommand::LoadAnimation(event) => {
                for entity in model_q.iter() {
                    commands.entity(entity).despawn();
                }
                for entity in subject_q.iter() {
                    commands.entity(entity).insert(Visibility::Hidden);
                }

                let scene_handle: Handle<Scene> = asset_server.load(&event.url);
                commands.spawn((
                    SceneRoot(scene_handle.clone()),
                    Transform::IDENTITY,
                    PreviewModel,
                ));

                model_state.scene_handle = Some(scene_handle);
                model_state.fitted = false;
                state.initialized = false;
                next_mode.set(PreviewMode::Animation);

                info!("[preview] Animated model loading: {}", event.url);
            }
            other => remaining.push(other),
        }
    }

    queue.commands = remaining;
}

fn auto_play_animations(
    mut state: ResMut<AnimationPreviewState>,
    animation_players: Query<Entity, Added<AnimationPlayer>>,
    model_q: Query<Entity, With<PreviewModel>>,
    children_q: Query<&Children>,
) {
    if state.initialized {
        return;
    }

    for entity in animation_players.iter() {
        let belongs_to_model = model_q.iter().any(|model_entity| {
            entity == model_entity || is_descendant(entity, model_entity, &children_q)
        });

        if !belongs_to_model {
            continue;
        }

        info!("[preview] Animation player found, auto-playing");
        state.initialized = true;
    }
}

fn is_descendant(
    entity: Entity,
    ancestor: Entity,
    children_q: &Query<&Children>,
) -> bool {
    if let Ok(children) = children_q.get(ancestor) {
        for child in children.iter() {
            if child == entity || is_descendant(entity, child, children_q) {
                return true;
            }
        }
    }
    false
}

pub struct AnimationPreviewPlugin;

impl Plugin for AnimationPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimationPreviewState>()
            .add_systems(Update, handle_animation_commands)
            .add_systems(Update, auto_play_animations.run_if(in_state(PreviewMode::Animation)));
    }
}
