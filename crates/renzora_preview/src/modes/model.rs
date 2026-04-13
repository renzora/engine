//! 3D Model preview mode.

use bevy::prelude::*;

use crate::bridge::{PreviewCommand, PreviewCommandQueue};
use crate::scene::{PreviewSubject, OrbitState};
use super::PreviewMode;

#[derive(Resource, Default)]
pub struct ModelPreviewState {
    pub scene_handle: Option<Handle<Scene>>,
    pub fitted: bool,
}

#[derive(Component)]
pub struct PreviewModel;

fn handle_model_commands(
    mut queue: ResMut<PreviewCommandQueue>,
    mut state: ResMut<ModelPreviewState>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    model_q: Query<Entity, With<PreviewModel>>,
    subject_q: Query<Entity, With<PreviewSubject>>,
    mut next_mode: ResMut<NextState<PreviewMode>>,
) {
    let mut remaining = Vec::new();

    for cmd in queue.commands.drain(..) {
        match cmd {
            PreviewCommand::LoadModel(event) => {
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

                state.scene_handle = Some(scene_handle);
                state.fitted = false;
                next_mode.set(PreviewMode::Model);

                info!("[preview] Model loading: {}", event.url);
            }
            other => remaining.push(other),
        }
    }

    queue.commands = remaining;
}

fn auto_fit_model(
    mut state: ResMut<ModelPreviewState>,
    mut orbit: ResMut<OrbitState>,
    model_q: Query<Entity, With<PreviewModel>>,
    mesh_q: Query<&GlobalTransform, With<Mesh3d>>,
) {
    if state.fitted || state.scene_handle.is_none() {
        return;
    }

    if model_q.is_empty() {
        return;
    }

    // Estimate bounds from mesh entity positions
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found_any = false;

    for gt in mesh_q.iter() {
        let pos = gt.translation();
        min = min.min(pos - Vec3::splat(1.0));
        max = max.max(pos + Vec3::splat(1.0));
        found_any = true;
    }

    if !found_any {
        return;
    }

    let size = (max - min).length().max(2.0);
    orbit.distance = size * 1.2;
    orbit.elevation = 0.4;
    state.fitted = true;

    info!("[preview] Model auto-fit: size={size:.2}, distance={:.2}", orbit.distance);
}

fn on_exit_model(
    mut commands: Commands,
    subject_q: Query<Entity, With<PreviewSubject>>,
    model_q: Query<Entity, With<PreviewModel>>,
) {
    for entity in model_q.iter() {
        commands.entity(entity).despawn();
    }
    for entity in subject_q.iter() {
        commands.entity(entity).insert(Visibility::Visible);
    }
}

pub struct ModelPreviewPlugin;

impl Plugin for ModelPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ModelPreviewState>()
            .add_systems(Update, handle_model_commands)
            .add_systems(Update, auto_fit_model.run_if(in_state(PreviewMode::Model)))
            .add_systems(OnExit(PreviewMode::Model), on_exit_model);
    }
}
