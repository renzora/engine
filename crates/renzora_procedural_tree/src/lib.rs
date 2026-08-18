//! Procedural tree distribution plugin.
//!
//! Wraps the vendored [`bevy_procedural_tree`] generator and registers it with
//! the renzora runtime via `renzora::add!`. Built as a `cdylib` and dlopen'd
//! from `plugins/` at startup — the same distribution-plugin model the cloth /
//! postprocess effects use, so a shipped game gets procedural trees only if this
//! plugin sits in `plugins/`.
//!
//! Spawn a tree by inserting a [`bevy_procedural_tree::Tree`] on an entity (the
//! generator's `on_add` hook builds the branch mesh on that entity and a child
//! leaf-mesh entity). In the editor, use Add Entity → "Procedural Tree" (see
//! `renzora_procedural_tree_editor`).
//!
//! This wrapper adds the renzora-specific glue that keeps trees clean across
//! scene save/load:
//! - [`tag_generated_leaves`] hides the generated leaf child from the hierarchy
//!   and excludes it from scene saves (renzora's saver skips `HideInHierarchy`
//!   entities). The `Tree` parent is the single source of truth and regenerates
//!   the leaf child on load.
//! - [`prune_stale_leaves`] cleans up any orphaned/empty leaf entities left by
//!   scenes saved before the tag took effect.
//! - [`sway_generated_trees`] tags both meshes with [`renzora::WindSway`] so the
//!   tree moves in the world wind. The generator writes per-vertex wind weights
//!   into `UV_1`, so this is the piece that turns those weights into motion.

use bevy::prelude::*;
use bevy_procedural_tree::{Leaves, Tree, TreeProceduralGenerationPlugin};
use renzora::{HideInHierarchy, WindSway};

/// Runtime-scope plugin that installs the procedural tree generator.
#[derive(Default)]
pub struct ProceduralTreePlugin;

impl Plugin for ProceduralTreePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ProceduralTreePlugin (bevy_procedural_tree)");
        app.add_plugins(TreeProceduralGenerationPlugin);
        app.add_systems(
            Update,
            (tag_generated_leaves, prune_stale_leaves, sway_generated_trees),
        );
    }
}

/// Tag each tree's generated leaf-mesh child with [`HideInHierarchy`] so it stays
/// out of the outliner and out of scene saves (it's regenerated from the parent
/// `Tree` on load). The branch mesh lives on the parent entity, which stays
/// selectable.
fn tag_generated_leaves(
    mut commands: Commands,
    trees: Query<&Leaves>,
    needs_tag: Query<(), (With<Mesh3d>, Without<HideInHierarchy>)>,
) {
    for leaves in trees.iter() {
        if needs_tag.get(leaves.0).is_ok() {
            commands.entity(leaves.0).insert(HideInHierarchy);
        }
    }
}

/// Give every generated tree its wind response.
///
/// Two different tunings, because they are two different materials on two
/// different meshes: the trunk bends slowly and does not flutter at all, while
/// the leaf canopy is floppier and flutters fully. Sharing one `WindSway`
/// between them would either give the trunk a rustle or take the rustle off the
/// leaves.
///
/// Only ever *inserts*, so a value an author changed in the inspector — or one
/// restored from a scene — is never overwritten on the next frame.
fn sway_generated_trees(
    mut commands: Commands,
    trees: Query<(Entity, &Leaves), With<Tree>>,
    needs_sway: Query<(), Without<WindSway>>,
) {
    for (trunk, leaves) in trees.iter() {
        if needs_sway.get(trunk).is_ok() {
            commands.entity(trunk).insert(WindSway {
                // Wood is stiff, and the trunk mesh's `UV_1` weights already
                // ramp from 0 at the base — this scales what is left.
                response: 0.55,
                flutter: 0.0,
                amplitude: 0.25,
                ..default()
            });
        }
        if needs_sway.get(leaves.0).is_ok() {
            commands.entity(leaves.0).insert(WindSway {
                response: 1.0,
                flutter: 1.0,
                amplitude: 0.4,
                ..default()
            });
        }
    }
}

/// Despawn stray leaf entities with no mesh — the empty husks a pre-tag scene
/// save could leave behind. A freshly generated leaf child always carries a
/// `Mesh3d`, so this only ever removes orphans.
fn prune_stale_leaves(mut commands: Commands, stale: Query<(Entity, &Name), Without<Mesh3d>>) {
    for (entity, name) in stale.iter() {
        if name.as_str() == "ProcGenTreeLeaves" {
            commands.entity(entity).despawn();
        }
    }
}

renzora::add!(ProceduralTreePlugin);
