//! Scene nodes
//!
//! Nodes for scene management, loading, and instantiation.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// SCENE LOADING
// =============================================================================

/// Load scene
pub static LOAD_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/load",
    display_name: "Load Scene",
    category: "Scene",
    description: "Load a scene file",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("path", "Path", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("on_loaded", "On Loaded", PinType::Execution),
        Pin::output("root", "Root Entity", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Load scene async
pub static LOAD_SCENE_ASYNC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/load_async",
    display_name: "Load Scene Async",
    category: "Scene",
    description: "Load a scene file asynchronously",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("path", "Path", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("handle", "Handle", PinType::SceneHandle),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Spawn scene
pub static SPAWN_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/spawn",
    display_name: "Spawn Scene",
    category: "Scene",
    description: "Spawn a loaded scene into the world",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::SceneHandle),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("rotation", "Rotation", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("root", "Root Entity", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Unload scene
pub static UNLOAD_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/unload",
    display_name: "Unload Scene",
    category: "Scene",
    description: "Unload a scene and free its resources",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::SceneHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Is scene loaded
pub static IS_SCENE_LOADED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/is_loaded",
    display_name: "Is Scene Loaded",
    category: "Scene",
    description: "Check if a scene is fully loaded",
    create_pins: || vec![
        Pin::input("handle", "Handle", PinType::SceneHandle),
        Pin::output("loaded", "Is Loaded", PinType::Bool),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// On scene loaded
pub static ON_SCENE_LOADED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/on_loaded",
    display_name: "On Scene Loaded",
    category: "Scene Events",
    description: "Triggered when a scene finishes loading",
    create_pins: || vec![
        Pin::input("handle", "Handle", PinType::SceneHandle),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("root", "Root Entity", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// SCENE TRANSITIONS
// =============================================================================

/// Change scene
pub static CHANGE_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/change",
    display_name: "Change Scene",
    category: "Scene",
    description: "Unload current scene and load a new one",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("path", "Path", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Reload current scene
pub static RELOAD_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/reload",
    display_name: "Reload Scene",
    category: "Scene",
    description: "Reload the current scene",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Get current scene
pub static GET_CURRENT_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/get_current",
    display_name: "Get Current Scene",
    category: "Scene",
    description: "Get the current scene name/path",
    create_pins: || vec![
        Pin::output("name", "Name", PinType::String),
        Pin::output("path", "Path", PinType::String),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// PREFABS/INSTANTIATION
// =============================================================================

/// Load prefab
pub static LOAD_PREFAB: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/load_prefab",
    display_name: "Load Prefab",
    category: "Scene",
    description: "Load a prefab asset",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("path", "Path", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("prefab", "Prefab", PinType::PrefabHandle),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Instantiate prefab
pub static INSTANTIATE_PREFAB: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/instantiate",
    display_name: "Instantiate Prefab",
    category: "Scene",
    description: "Instantiate a prefab at a position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("prefab", "Prefab", PinType::PrefabHandle),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("rotation", "Rotation", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("scale", "Scale", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Instantiate at transform
pub static INSTANTIATE_AT_TRANSFORM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/instantiate_at",
    display_name: "Instantiate At Transform",
    category: "Scene",
    description: "Instantiate a prefab at another entity's transform",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("prefab", "Prefab", PinType::PrefabHandle),
        Pin::input("transform_entity", "At Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// GLTF/GLB LOADING
// =============================================================================

/// Load GLTF
pub static LOAD_GLTF: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/load_gltf",
    display_name: "Load GLTF",
    category: "Scene",
    description: "Load a GLTF/GLB file",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("path", "Path", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("handle", "Handle", PinType::GltfHandle),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Spawn GLTF scene
pub static SPAWN_GLTF_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/spawn_gltf",
    display_name: "Spawn GLTF Scene",
    category: "Scene",
    description: "Spawn a scene from a GLTF file",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("gltf", "GLTF", PinType::GltfHandle),
        Pin::input("scene_index", "Scene Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("entity", "Entity", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Get GLTF scene count
pub static GET_GLTF_SCENE_COUNT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/gltf_scene_count",
    display_name: "Get GLTF Scene Count",
    category: "Scene",
    description: "Get the number of scenes in a GLTF file",
    create_pins: || vec![
        Pin::input("gltf", "GLTF", PinType::GltfHandle),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SCENE QUERIES
// =============================================================================

/// Find entity in scene
pub static FIND_IN_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/find",
    display_name: "Find In Scene",
    category: "Scene",
    description: "Find an entity in a scene by name",
    create_pins: || vec![
        Pin::input("root", "Scene Root", PinType::Entity),
        Pin::input("name", "Name", PinType::String),
        Pin::output("entity", "Entity", PinType::Entity),
        Pin::output("found", "Found", PinType::Bool),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Find all by name in scene
pub static FIND_ALL_IN_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/find_all",
    display_name: "Find All In Scene",
    category: "Scene",
    description: "Find all entities with a name in a scene",
    create_pins: || vec![
        Pin::input("root", "Scene Root", PinType::Entity),
        Pin::input("name", "Name", PinType::String),
        Pin::output("entities", "Entities", PinType::EntityArray),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Get scene root
pub static GET_SCENE_ROOT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/get_root",
    display_name: "Get Scene Root",
    category: "Scene",
    description: "Get the root entity of a spawned scene",
    create_pins: || vec![
        Pin::input("handle", "Handle", PinType::SceneHandle),
        Pin::output("root", "Root", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SCENE SERIALIZATION
// =============================================================================

/// Save scene
pub static SAVE_SCENE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/save",
    display_name: "Save Scene",
    category: "Scene",
    description: "Save entities to a scene file",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("root", "Root Entity", PinType::Entity),
        Pin::input("path", "Path", PinType::String),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("success", "Success", PinType::Bool),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};

/// Clone entity tree
pub static CLONE_ENTITY_TREE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "scene/clone_tree",
    display_name: "Clone Entity Tree",
    category: "Scene",
    description: "Clone an entity and all its children",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("source", "Source", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("clone", "Clone", PinType::Entity),
    ],
    color: [200, 160, 100],
    is_event: false,
    is_comment: false,
};
