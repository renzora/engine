//! Modular Node System
//!
//! This module provides a registry-based architecture for defining and managing
//! different types of nodes in the editor. Adding a new node type requires:
//! 1. Creating a NodeDefinition in a new file under nodes/
//! 2. Registering it in plugin.rs
//!
//! # Architecture
//!
//! - `definition.rs` - NodeDefinition struct and NodeCategory enum
//! - `registry.rs` - NodeRegistry resource for storing all definitions
//! - `components.rs` - NodeTypeMarker and other data components
//! - `plugin.rs` - NodeSystemPlugin that registers everything
//! - `menu.rs` - Auto-generated menu rendering
//! - `nodes/` - Individual node type definitions
//! - `inspector/` - Inspector widget system
//! - `scene/` - V2 scene format with registry-based serialization

pub mod components;
pub mod definition;
pub mod inspector;
pub mod menu;
pub mod nodes;
pub mod plugin;
pub mod registry;
pub mod scene;

// Re-export commonly used items
// Note: Some exports are for future use (e.g., save_scene)
#[allow(unused_imports)]
pub use components::{
    CameraNodeData, CollisionShapeData, CollisionShapeType, MeshInstanceData, MeshNodeData,
    MeshPrimitiveType, NodeTypeMarker, PhysicsBodyData, PhysicsBodyType, SceneInstanceData,
};
pub use nodes::{SceneRoot, SceneType, is_scene_root_type};
#[allow(unused_imports)]
pub use definition::{NodeCategory, NodeDefinition};
pub use inspector::{
    render_camera_inspector, render_collision_shape_inspector, render_directional_light_inspector,
    render_physics_body_inspector, render_point_light_inspector, render_script_inspector,
    render_spot_light_inspector, render_transform_inspector, render_world_environment_inspector,
};
#[allow(unused_imports)]
pub use inspector::{InspectorRegistry, InspectorWidget};
#[allow(unused_imports)]
pub use menu::{render_add_node_popup, render_node_menu_items, render_node_menu_as_submenus};
#[allow(unused_imports)]
pub use menu::{render_add_child_menu, get_category_icon_public, get_node_icon_for_type_public};
pub use plugin::NodeSystemPlugin;
pub use registry::NodeRegistry;
pub use scene::load_scene;
pub use scene::{assign_scene_tab_ids, handle_save_shortcut, handle_scene_requests};
#[allow(unused_imports)]
pub use scene::{save_scene, NodeData, SceneData};
