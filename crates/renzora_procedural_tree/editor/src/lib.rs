//! Editor-only half of `renzora_procedural_tree`.
//!
//! `renzora_procedural_tree` compiles lean (no editor contract). This crate adds
//! the editor integration — an Add-Entity preset and a curated `Tree` inspector —
//! registered via `renzora::add!(ProceduralTreeEditorPlugin, Editor)` and linked
//! only by the editor bundle.
//!
//! The inspector exposes the high-signal knobs (seed, tree type, leaf count/size)
//! plus a Regenerate button. The deeply nested per-level branch/leaf arrays
//! (`[f32; 4]` etc.) stay at their defaults for now — the declarative inspector
//! has no array widget.

use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use bevy_procedural_tree::{Leaves, Tree, TreeMeshSettings, TreeType};
use renzora::{AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry};

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
                    settings(world, entity)
                        .map(|s| FieldValue::Float(s.branch.trunk_base_radius))
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

/// Editor-scope companion to `renzora_procedural_tree::ProceduralTreePlugin`.
#[derive(Default)]
pub struct ProceduralTreeEditorPlugin;

impl Plugin for ProceduralTreeEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ProceduralTreeEditorPlugin");
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

renzora::add!(ProceduralTreeEditorPlugin, Editor);
