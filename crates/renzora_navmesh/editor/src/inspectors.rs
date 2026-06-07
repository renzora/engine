//! NavMesh inspector entries — moved out of `renzora_navmesh::lib` because they
//! construct `InspectorEntry` / `FieldDef`, which live behind renzora's `editor`
//! feature. Registered by `NavMeshEditorPlugin`.

use bevy::prelude::*;
use renzora::{FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_navmesh::{NavAgent, NavMeshObstacle, NavMeshVolume, NavPath};
use vleue_navigator::prelude::{
    ManagedNavMesh, NavMeshAgentExclusion, NavMeshDebug, NavMeshSettings, NavMeshUpdateMode,
};

pub fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "nav_mesh_volume",
        display_name: "NavMesh Volume",
        icon: "polygon",
        category: "navigation",
        has_fn: |world, entity| world.get::<NavMeshVolume>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NavMeshVolume::default());
        }),
        remove_fn: Some(|world, entity| {
            let mut e = world.entity_mut(entity);
            e.remove::<NavMeshVolume>();
            e.remove::<ManagedNavMesh>();
            e.remove::<NavMeshSettings>();
            e.remove::<NavMeshUpdateMode>();
            e.remove::<NavMeshDebug>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<NavMeshVolume>(entity)
                .map(|v| v.debug_draw)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                v.debug_draw = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Half Extents",
                field_type: FieldType::Vec3 { speed: 0.25 },
                get_fn: |world, entity| {
                    world.get::<NavMeshVolume>(entity).map(|v| {
                        FieldValue::Vec3([v.half_extents.x, v.half_extents.y, v.half_extents.z])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3([x, y, z]) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.half_extents = Vec3::new(x, y, z);
                        }
                    }
                },
            },
            FieldDef {
                name: "Agent Radius",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.agent_radius))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.agent_radius = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Upward Shift",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.upward_shift))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.upward_shift = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Simplify",
                field_type: FieldType::Float {
                    speed: 0.001,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.simplify))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.simplify = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Merge Steps",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.merge_steps as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.merge_steps = x.round().clamp(0.0, 10.0) as u32;
                        }
                    }
                },
            },
            FieldDef {
                name: "Include Terrain",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Bool(v.include_terrain))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.include_terrain = b;
                        }
                    }
                },
            },
            FieldDef {
                name: "Max Slope (degrees)",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 5.0,
                    max: 89.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.max_slope_degrees))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.max_slope_degrees = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Terrain Sample Step",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 1.0,
                    max: 32.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.terrain_sample_step as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.terrain_sample_step = x.round().clamp(1.0, 32.0) as u32;
                        }
                    }
                },
            },
            FieldDef {
                name: "Debug Draw",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Bool(v.debug_draw))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.debug_draw = b;
                        }
                    }
                },
            },
        ],
    }
}

pub fn obstacle_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "nav_mesh_obstacle",
        display_name: "NavMesh Obstacle",
        icon: "cube",
        category: "navigation",
        has_fn: |world, entity| world.get::<NavMeshObstacle>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NavMeshObstacle);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<NavMeshObstacle>();
        }),
        is_enabled_fn: Some(|world, entity| world.get::<NavMeshObstacle>(entity).is_some()),
        set_enabled_fn: Some(|world, entity, val| {
            if val {
                world.entity_mut(entity).insert(NavMeshObstacle);
            } else {
                world.entity_mut(entity).remove::<NavMeshObstacle>();
            }
        }),
        fields: vec![],
    }
}

pub fn agent_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "nav_agent",
        display_name: "NavMesh Agent",
        icon: "person-simple-walk",
        category: "navigation",
        has_fn: |world, entity| world.get::<NavAgent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NavAgent::default());
        }),
        remove_fn: Some(|world, entity| {
            let mut e = world.entity_mut(entity);
            e.remove::<NavAgent>();
            e.remove::<NavPath>();
            e.remove::<NavMeshAgentExclusion>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Has Target",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<NavAgent>(entity)
                        .map(|a| FieldValue::Bool(a.target.is_some()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.target = if b {
                                Some(a.target.unwrap_or(Vec3::ZERO))
                            } else {
                                None
                            };
                        }
                    }
                },
            },
            FieldDef {
                name: "Speed",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavAgent>(entity)
                        .map(|a| FieldValue::Float(a.speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.speed = x.max(0.0);
                        }
                    }
                },
            },
            FieldDef {
                name: "Turn Speed",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 50.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavAgent>(entity)
                        .map(|a| FieldValue::Float(a.turn_speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.turn_speed = x.max(0.0);
                        }
                    }
                },
            },
            FieldDef {
                name: "Stopping Distance",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<NavAgent>(entity)
                        .map(|a| FieldValue::Float(a.stopping_distance))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.stopping_distance = x.max(0.0);
                        }
                    }
                },
            },
            FieldDef {
                name: "Target",
                field_type: FieldType::Vec3 { speed: 0.25 },
                get_fn: |world, entity| {
                    world.get::<NavAgent>(entity).map(|a| {
                        let t = a.target.unwrap_or(Vec3::ZERO);
                        FieldValue::Vec3([t.x, t.y, t.z])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3([x, y, z]) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.target = Some(Vec3::new(x, y, z));
                        }
                    }
                },
            },
        ],
    }
}
