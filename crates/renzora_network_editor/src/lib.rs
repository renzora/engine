//! Networking editor panels for the Renzora editor.
//!
//! Panels: Network Monitor, Network Entities, Network Settings — all rendered
//! in bevy_ui / ember in [`panels`]. This crate also registers the
//! `Networked` and `NetworkTransform` inspector entries (declarative fields, no
//! egui).

mod panels;

use bevy::prelude::*;

use renzora_editor_api::{
    bool_field, float_field, AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry,
};
use renzora_network::{
    NetworkOwner, NetworkPlayerSpawner, NetworkTransform, Networked, OwnerKind, TransformAuthority,
};

// ============================================================================
// Inspector entries (attachable components)
// ============================================================================

/// Resolve a localized string and leak it to satisfy the `&'static str`
/// inspector-field contract. Inspector entries are constructed once at plugin
/// registration, so this one-time leak per label is bounded (it does not grow
/// per frame). The trade-off: labels won't re-localize on a live language swap.
fn loc(key: &str) -> &'static str {
    Box::leak(renzora_lang_api::t(key).into_boxed_str())
}

/// `Networked` — the "replicate this entity" marker. Adding it makes the
/// server replicate the entity (and its `Transform`) to every client.
fn networked_inspector() -> InspectorEntry {
    InspectorEntry {
        type_id: "networked",
        display_name: loc("comp.networked"),
        icon: "share-network",
        category: "networking",
        has_fn: |world, entity| world.get::<Networked>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Networked);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Networked>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

/// `NetworkTransform` — tunes how the entity's transform replicates.
fn network_transform_inspector() -> InspectorEntry {
    InspectorEntry {
        type_id: "network_transform",
        display_name: loc("comp.network_transform"),
        icon: "arrows-out-cardinal",
        category: "networking",
        has_fn: |world, entity| world.get::<NetworkTransform>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NetworkTransform::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<NetworkTransform>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: loc("comp.authority"),
                field_type: FieldType::Enum { options: &["Server", "Owner"] },
                get_fn: |w, e| {
                    w.get::<NetworkTransform>(e).map(|t| {
                        FieldValue::Enum(
                            match t.authority {
                                TransformAuthority::Server => "Server",
                                TransformAuthority::Owner => "Owner",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(label), Some(mut t)) = (v, w.get_mut::<NetworkTransform>(e)) {
                        t.authority = if label == "Owner" { TransformAuthority::Owner } else { TransformAuthority::Server };
                    }
                },
            },
            bool_field!(loc("comp.interpolate"), NetworkTransform, interpolate),
            bool_field!(loc("comp.sync_rotation"), NetworkTransform, sync_rotation),
            bool_field!(loc("comp.sync_scale"), NetworkTransform, sync_scale),
        ],
    }
}

/// `NetworkOwner`, read-only: ownership is assigned by the server at run time
/// (an avatar belongs to its player), and a scene object is always the
/// server's, so there is nothing for an author to choose.
fn network_owner_inspector() -> InspectorEntry {
    InspectorEntry {
        type_id: "network_owner",
        display_name: loc("comp.network_owner"),
        icon: "user-circle",
        category: "networking",
        has_fn: |world, entity| world.get::<NetworkOwner>(entity).is_some(),
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<NetworkOwner>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: loc("comp.owner"),
            field_type: FieldType::ReadOnly,
            get_fn: |w, e| {
                w.get::<NetworkOwner>(e).map(|o| {
                    FieldValue::ReadOnly(match o.0 {
                        OwnerKind::Server => "Server".to_string(),
                        OwnerKind::Client(id) => format!("Player {id}"),
                    })
                })
            },
            set_fn: |_, _, _| {},
        }],
    }
}

/// `NetworkPlayerSpawner`: give every player who joins an avatar.
fn player_spawner_inspector() -> InspectorEntry {
    InspectorEntry {
        type_id: "network_player_spawner",
        display_name: loc("comp.network_player_spawner"),
        icon: "users-three",
        category: "networking",
        has_fn: |world, entity| world.get::<NetworkPlayerSpawner>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NetworkPlayerSpawner::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<NetworkPlayerSpawner>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: loc("comp.player_prefab"),
                field_type: FieldType::Asset { extensions: vec!["bsn".into()] },
                get_fn: |w, e| {
                    w.get::<NetworkPlayerSpawner>(e)
                        .map(|s| FieldValue::Asset((!s.prefab.is_empty()).then(|| s.prefab.clone())))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Asset(path), Some(mut s)) = (v, w.get_mut::<NetworkPlayerSpawner>(e)) {
                        s.prefab = path.unwrap_or_default();
                    }
                },
            },
            float_field!(loc("comp.spawn_radius"), NetworkPlayerSpawner, spawn_radius, 0.1, 0.0, 1000.0),
        ],
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct NetworkEditorPlugin;

impl Plugin for NetworkEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] NetworkEditorPlugin");
        app.add_plugins(panels::NetworkPanels);
        app.register_inspector(networked_inspector());
        app.register_inspector(network_transform_inspector());
        app.register_inspector(network_owner_inspector());
        app.register_inspector(player_spawner_inspector());
    }
}
