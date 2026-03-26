//! Preview modes — each module handles a specific asset type.

pub mod shader;
pub mod model;
pub mod animation;
pub mod particle;
pub mod texture;

use bevy::prelude::*;
use crate::bridge::{PreviewCommand, PreviewCommandQueue};

/// The active preview mode.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PreviewMode {
    #[default]
    Idle,
    Shader,
    Model,
    Animation,
    Particle,
    Texture,
}

impl PreviewMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "shader" | "material" | "postprocess" | "post-process" => Self::Shader,
            "model" | "3d" => Self::Model,
            "animation" | "anim" => Self::Animation,
            "particle" | "particles" | "fx" => Self::Particle,
            "texture" | "hdri" => Self::Texture,
            _ => Self::Idle,
        }
    }
}

/// Handle mode switching from command queue.
pub fn handle_set_mode(
    mut queue: ResMut<PreviewCommandQueue>,
    mut next_state: ResMut<NextState<PreviewMode>>,
) {
    let mut remaining = Vec::new();

    for cmd in queue.commands.drain(..) {
        match cmd {
            PreviewCommand::SetMode(c) => {
                next_state.set(PreviewMode::from_str(&c.mode));
            }
            other => remaining.push(other),
        }
    }

    queue.commands = remaining;
}

/// Plugin that registers all preview modes.
pub struct ModesPlugin;

impl Plugin for ModesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<PreviewMode>()
            .add_systems(Update, handle_set_mode)
            .add_plugins((
                shader::ShaderPreviewPlugin,
                model::ModelPreviewPlugin,
                animation::AnimationPreviewPlugin,
                particle::ParticlePreviewPlugin,
                texture::TexturePreviewPlugin,
            ));
    }
}
