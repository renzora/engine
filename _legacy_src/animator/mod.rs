//! AnimatorComponent plugin — manages skeletal animation clips via .anim files.

pub mod anim_file;
pub mod anim_loader;
pub mod component;
pub mod systems;

pub use component::{AnimatorComponent, AnimClipSlot};
pub use anim_loader::AnimFileLoader;
pub use anim_file::{AnimFile, BoneTrack};

use bevy::prelude::*;
use systems::{setup_animator_graphs, update_animator_playback};

pub struct AnimatorPlugin;

impl Plugin for AnimatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset_loader::<AnimFileLoader>()
            .register_type::<AnimatorComponent>()
            .register_type::<AnimClipSlot>()
            .add_systems(
                Update,
                (setup_animator_graphs, update_animator_playback).chain(),
            );
    }
}
