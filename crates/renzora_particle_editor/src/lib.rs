//! Particle Editor Panel & Preview
//!
//! Full-featured editor for bevy_hanabi particle effects with live preview.

mod inspector;
mod native_editor_panel;
mod native_graph;
mod native_preview_panel;
mod preview;

use bevy::prelude::*;
use renzora_hanabi::data::{HanabiEffectDefinition, ParticleEditorState};

#[derive(Default)]
pub struct ParticleEditorPlugin;

impl Plugin for ParticleEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ParticleEditorPlugin");
        inspector::register_inspector(app);
        app.add_plugins(preview::ParticlePreviewPlugin);
        app.add_plugins(native_preview_panel::NativeParticlePreview);
        app.add_plugins(native_editor_panel::NativeParticleEditor);
        app.add_plugins(native_graph::NativeParticleGraph);

        app.init_resource::<PartUndoShadow>();
        app.add_systems(
            Update,
            particle_undo_observer.run_if(|s: Option<Res<ParticleEditorState>>| {
                s.is_some_and(|s| s.current_effect.is_some())
            }),
        );
    }
}

renzora::add!(ParticleEditorPlugin, Editor);

// ── Undo integration ─────────────────────────────────────────────────────────
//
// The particle editor edits an in-memory `.particle` buffer
// (`ParticleEditorState.current_effect`). A change-observer records a coarse
// snapshot of that buffer whenever it changes, covering every param/gradient/
// graph edit from one place. Full RON serialization is the change signal (the
// preview's own hash is partial), so no field is missed. Per-frame scrub spam
// collapses via the snapshot merge key; the global gesture seal splits gestures.

/// Shadow of the effect the observer last saw, its serialized form (the diff
/// key), and the document it belongs to. Switching files reseeds, not records.
#[derive(Resource, Default)]
struct PartUndoShadow {
    doc_id: String,
    serialized: Option<String>,
    effect: Option<HanabiEffectDefinition>,
}

/// Restore a snapshotted effect — the `restore` fn for the particle `SnapshotCmd`.
/// Writes the buffer, forces the node graph to regenerate from it, and keeps the
/// observer shadow in sync. The preview's effect-hash watcher respawns the live
/// preview from the restored buffer.
fn restore_particle(world: &mut World, effect: &HanabiEffectDefinition) {
    if let Some(mut s) = world.get_resource_mut::<ParticleEditorState>() {
        s.current_effect = Some(effect.clone());
        s.is_modified = true;
        // ensure_node_graph rebuilds the graph from the restored effect.
        s.node_graph = None;
    }
    if let Some(mut sh) = world.get_resource_mut::<PartUndoShadow>() {
        sh.serialized = ron::to_string(effect).ok();
        sh.effect = Some(effect.clone());
    }
}

fn particle_undo_observer(world: &mut World) {
    let (cur, doc_id) = {
        let Some(s) = world.get_resource::<ParticleEditorState>() else {
            return;
        };
        let Some(cur) = s.current_effect.clone() else {
            return;
        };
        (cur, s.current_file_path.clone().unwrap_or_default())
    };
    let serialized = match ron::to_string(&cur) {
        Ok(s) => s,
        Err(_) => return,
    };
    let (prev_id, prev_serialized, prev_effect) = {
        let sh = world.resource::<PartUndoShadow>();
        (sh.doc_id.clone(), sh.serialized.clone(), sh.effect.clone())
    };
    if prev_id != doc_id || prev_effect.is_none() {
        let mut sh = world.resource_mut::<PartUndoShadow>();
        sh.doc_id = doc_id;
        sh.serialized = Some(serialized);
        sh.effect = Some(cur);
        return;
    }
    if prev_serialized.as_deref() == Some(serialized.as_str()) {
        return;
    }
    let before = prev_effect.unwrap();
    let ctx = renzora_undo::active_context(world);
    renzora_undo::record(
        world,
        ctx,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Particle".to_string(),
            before,
            after: cur.clone(),
            restore: restore_particle,
            merge_key: Some("particle-effect".to_string()),
        }),
    );
    let mut sh = world.resource_mut::<PartUndoShadow>();
    sh.serialized = Some(serialized);
    sh.effect = Some(cur);
}
