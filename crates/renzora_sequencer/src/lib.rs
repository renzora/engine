//! Renzora Sequencer — multi-track timeline for cinematics.
//!
//! Sequences arrange typed tracks (camera moves, entity transforms, markers,
//! and pre-baked media) on a shared playhead. While playing, each track type
//! has its own apply system that writes to the world (e.g. the camera track
//! drives the editor camera transform). The result can be baked to video via
//! the `renzora_record` plugin.
//!
//! The MVP scope is intentionally narrow:
//! - Camera + Transform + Marker + Media tracks.
//! - Linear/smoothstep interpolation between keyframes (no bezier handles).
//! - Bake-to-video is wired as a stub that drops a placeholder MediaClip;
//!   real recording is a follow-up that hooks into `renzora_record`'s
//!   encoder pipeline.

mod model;
mod panel;
mod runtime;

use bevy::prelude::*;
use renzora_editor::AppEditorExt;

pub use model::{
    CameraClip, CameraKey, MarkerClip, MediaClip, Sequence, Track, TrackKind, TransformClip,
    TransformKey,
};
pub use runtime::{SequencerAction, SequencerBridge, SequencerState};

#[derive(Default)]
pub struct SequencerPlugin;

impl Plugin for SequencerPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SequencerPlugin");

        let bridge = runtime::SequencerBridge::default();

        app.init_resource::<runtime::SequencerState>();
        app.insert_resource(bridge.clone());

        app.add_systems(
            Update,
            (
                runtime::apply_bridge_actions,
                runtime::advance_playhead,
                runtime::apply_camera_tracks,
                runtime::apply_transform_tracks,
            )
                .chain(),
        );

        app.register_panel(panel::SequencerPanel::new(bridge));
    }
}

