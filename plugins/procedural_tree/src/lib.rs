//! Procedural trees — seeded branch and leaf meshes, generated on demand.
//!
//! Insert a [`Tree`] on an entity and the generator builds the branch mesh on
//! that entity plus a child leaf-mesh entity. In the editor, Add Entity →
//! "Procedural Tree".
//!
//! # What changed when this became a native plugin
//!
//! It used to be three crates: `bevy_procedural_tree` (the vendored generator),
//! `renzora_procedural_tree` (the runtime glue) and
//! `renzora_procedural_tree_editor` (the preset and inspector). All three are
//! here now, and the reason is the native-plugin dependency rule: a plugin is
//! compiled by a bare `rustc` against the staged SDK, and the only engine crates
//! it is handed are `bevy`, `renzora` and `renzora_ember`. A workspace crate
//! that depends on Bevy — which the generator does — cannot be reached at all,
//! and cargo is forbidden from resolving one, because a second Bevy compilation
//! would give this plugin different `TypeId`s from the engine and let it read
//! the host's `World` through the wrong layouts.
//!
//! So the generator was **vendored inward** rather than depended on. It sits
//! under [`tree`] as a module tree, still carrying its upstream licences and
//! `README.md` at the plugin root. Its only third-party dependency, `fastrand`,
//! has no Bevy in its graph and is built by cargo into a private rlib.
//!
//! The editor half merged in for a different reason: a native plugin is compiled
//! with **no cargo features**, so the `#[cfg(feature = "editor")]` that used to
//! gate this registration would be permanently false and the inspector would
//! vanish without a word. The registrations are unconditional now — the editor
//! contract is always present in the SDK's `renzora`, and in a shipped game the
//! registries simply go unread.

use bevy::prelude::*;

pub mod tree;

use renzora::{
    AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, HideInHierarchy, InspectorEntry,
    WindSway,
};
use tree::{Leaves, Tree, TreeMeshSettings, TreeProceduralGenerationPlugin, TreeType};

use std::sync::atomic::{AtomicU64, Ordering};

/// Runtime-scope plugin that installs the procedural tree generator.
#[derive(Default)]
pub struct ProceduralTreePlugin;

impl Plugin for ProceduralTreePlugin {
    fn build(&self, app: &mut App) {
        info!("[procedural_tree] native plugin");
        app.add_plugins(TreeProceduralGenerationPlugin);
        app.add_systems(
            Update,
            (tag_generated_leaves, prune_stale_leaves, sway_generated_trees),
        );

        // No `cfg(feature = "editor")` — see the module docs. A native plugin
        // has no cargo features, so the gate would be permanently false.
        app.register_entity_preset(EntityPreset {
            id: "procedural_tree",
            display_name: "Procedural Tree",
            icon: "tree",
            category: "general",
            spawn_fn: |world| {
                world
                    .spawn((
                        Name::new("Procedural Tree"),
                        Transform::default(),
                        Tree {
                            seed: next_seed(),
                            ..default()
                        },
                    ))
                    .id()
            },
        });
        app.register_inspector(inspector_entry());
    }
}

// ---------------------------------------------------------------------------
// Runtime glue
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Editor integration
// ---------------------------------------------------------------------------

/// Source of fresh, distinct (and small, so the seed reads cleanly as an integer
/// in the inspector drag) seeds for newly spawned / regenerated trees.
static SEED_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_seed() -> u64 {
    SEED_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Read the entity's per-tree settings override (every editor-spawned tree owns
/// one — see the spawn preset), falling back to `None` for code-spawned trees
/// that use the global resource.
fn settings(world: &World, e: Entity) -> Option<&TreeMeshSettings> {
    world
        .get::<Tree>(e)
        .and_then(|t| t.tree_mesh_settings_override.as_ref())
}

/// Mutate the entity's per-tree settings override, lazily materialising it from
/// defaults on first edit. Going through `get_mut` flags `Changed<Tree>`, which
/// drives the generator's regeneration system.
fn with_settings_mut(world: &mut World, e: Entity, f: impl FnOnce(&mut TreeMeshSettings)) {
    if let Some(mut tree) = world.get_mut::<Tree>(e) {
        let s = tree
            .tree_mesh_settings_override
            .get_or_insert_with(TreeMeshSettings::default);
        f(s);
    }
}

/// The curated `Tree` inspector: the high-signal knobs plus a Regenerate button.
/// The deeply nested per-level branch/leaf arrays (`[f32; 4]` etc.) stay at their
/// defaults — the declarative inspector has no array widget.
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "procedural_tree",
        display_name: "Procedural Tree",
        icon: "tree",
        category: "general",
        has_fn: |world, entity| world.get::<Tree>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Tree::default());
        }),
        remove_fn: Some(|world, entity| {
            // Despawn the generated leaf child and strip the generated mesh /
            // material so removing the component leaves a clean entity.
            if let Some(leaf) = world.get::<Leaves>(entity).map(|l| l.0) {
                if world.get_entity(leaf).is_ok() {
                    world.entity_mut(leaf).despawn();
                }
            }
            let mut em = world.entity_mut(entity);
            em.remove::<Tree>();
            em.remove::<Leaves>();
            em.remove::<Mesh3d>();
            em.remove::<MeshMaterial3d<StandardMaterial>>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            // Seed is a direct field of `Tree`; the macro's `get_mut` write
            // flags `Changed<Tree>` and regenerates the mesh.
            renzora::int_field!("Seed", Tree, seed, u64, 1.0, 0.0, 1_000_000.0),
            FieldDef {
                name: "Tree Type",
                field_type: FieldType::Enum {
                    options: &["Deciduous", "Evergreen"],
                },
                get_fn: |world, entity| {
                    settings(world, entity).map(|s| {
                        let label = match s.tree_type {
                            TreeType::Deciduous => "Deciduous",
                            TreeType::Evergreen => "Evergreen",
                        };
                        FieldValue::Enum(label.to_string())
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(label) = val {
                        with_settings_mut(world, entity, |s| {
                            s.tree_type = match label.as_str() {
                                "Evergreen" => TreeType::Evergreen,
                                _ => TreeType::Deciduous,
                            };
                        });
                    }
                },
            },
            FieldDef {
                name: "Leaf Count",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 50.0,
                },
                get_fn: |world, entity| {
                    settings(world, entity).map(|s| FieldValue::Float(s.leaves.count as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(f) = val {
                        with_settings_mut(world, entity, |s| s.leaves.count = f.max(0.0) as u32);
                    }
                },
            },
            FieldDef {
                name: "Leaf Size",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    settings(world, entity).map(|s| FieldValue::Float(s.leaves.size))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(f) = val {
                        with_settings_mut(world, entity, |s| s.leaves.size = f);
                    }
                },
            },
            FieldDef {
                name: "Trunk Radius",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.01,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    settings(world, entity).map(|s| FieldValue::Float(s.branch.trunk_base_radius))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(f) = val {
                        with_settings_mut(world, entity, |s| s.branch.trunk_base_radius = f);
                    }
                },
            },
            FieldDef {
                name: "Regenerate",
                field_type: FieldType::Button {
                    icon: "arrows-clockwise",
                },
                get_fn: |_world, _entity| None,
                set_fn: |world, entity, _val| {
                    if let Some(mut tree) = world.get_mut::<Tree>(entity) {
                        tree.seed = next_seed();
                    }
                },
            },
        ],
    }
}

// `Runtime`, explicitly. `plugin!` defaults to `Editor` where `add!` defaulted
// to `Runtime`, and a tree is scene content a shipped game renders — the editor
// preset and inspector above ride along harmlessly.
renzora::plugin!(ProceduralTreePlugin, Runtime);
