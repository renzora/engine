//! Texture & HDRI preview mode.

use bevy::prelude::*;

use crate::bridge::{PreviewCommand, PreviewCommandQueue};
use crate::scene::PreviewSubject;
use super::PreviewMode;

#[derive(Resource, Default)]
pub struct TexturePreviewState {
    pub texture_type: Option<String>,
    pub loaded: bool,
}

fn handle_texture_commands(
    mut queue: ResMut<PreviewCommandQueue>,
    mut state: ResMut<TexturePreviewState>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut subject_q: Query<&mut MeshMaterial3d<StandardMaterial>, With<PreviewSubject>>,
    mut next_mode: ResMut<NextState<PreviewMode>>,
) {
    let mut remaining = Vec::new();

    for cmd in queue.commands.drain(..) {
        match cmd {
            PreviewCommand::LoadTexture(event) => {
                let texture_handle: Handle<Image> = asset_server.load(&event.url);

                match event.texture_type.as_str() {
                    "texture" => {
                        let material = materials.add(StandardMaterial {
                            base_color_texture: Some(texture_handle),
                            ..default()
                        });
                        for mut mat in subject_q.iter_mut() {
                            mat.0 = material.clone();
                        }
                        state.texture_type = Some("texture".into());
                        info!("[preview] Texture loaded: {}", event.url);
                    }
                    "hdri" => {
                        let material = materials.add(StandardMaterial {
                            emissive_texture: Some(texture_handle),
                            emissive: LinearRgba::WHITE,
                            unlit: true,
                            cull_mode: None,
                            ..default()
                        });
                        for mut mat in subject_q.iter_mut() {
                            mat.0 = material.clone();
                        }
                        state.texture_type = Some("hdri".into());
                        info!("[preview] HDRI loaded: {}", event.url);
                    }
                    _ => {
                        warn!("[preview] Unknown texture type: {}", event.texture_type);
                        remaining.push(PreviewCommand::LoadTexture(event));
                        continue;
                    }
                }

                state.loaded = true;
                next_mode.set(PreviewMode::Texture);
            }
            other => remaining.push(other),
        }
    }

    queue.commands = remaining;
}

pub struct TexturePreviewPlugin;

impl Plugin for TexturePreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TexturePreviewState>()
            .add_systems(Update, handle_texture_commands);
    }
}
