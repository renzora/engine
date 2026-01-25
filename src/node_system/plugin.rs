use bevy::prelude::*;

use super::inspector::InspectorRegistry;
use super::nodes;
use super::registry::NodeRegistry;

/// Plugin that sets up the node system with all built-in node types
pub struct NodeSystemPlugin;

impl Plugin for NodeSystemPlugin {
    fn build(&self, app: &mut App) {
        // Create and populate the node registry
        let mut registry = NodeRegistry::new();

        // Register all built-in node types
        register_builtin_nodes(&mut registry);

        // Create the inspector registry
        let inspector_registry = InspectorRegistry::new();

        // Add registries as resources
        app.insert_resource(registry);
        app.insert_resource(inspector_registry);

        info!(
            "NodeSystemPlugin initialized with {} node types",
            app.world().resource::<NodeRegistry>().len()
        );
    }
}

/// Register all built-in node types
fn register_builtin_nodes(registry: &mut NodeRegistry) {
    // Scene Roots (only one should exist per scene)
    registry.register(&nodes::SCENE3D);
    registry.register(&nodes::SCENE2D);
    registry.register(&nodes::UI_ROOT);
    registry.register(&nodes::OTHER_ROOT);

    // 3D Nodes
    registry.register(&nodes::NODE3D);

    // Meshes
    registry.register(&nodes::CUBE);
    registry.register(&nodes::SPHERE);
    registry.register(&nodes::CYLINDER);
    registry.register(&nodes::PLANE);
    registry.register(&nodes::MESH_INSTANCE);

    // Lights
    registry.register(&nodes::POINT_LIGHT);
    registry.register(&nodes::DIRECTIONAL_LIGHT);
    registry.register(&nodes::SPOT_LIGHT);

    // Physics - Bodies
    registry.register(&nodes::RIGIDBODY3D);
    registry.register(&nodes::STATICBODY3D);
    registry.register(&nodes::KINEMATICBODY3D);

    // Physics - Collision Shapes
    registry.register(&nodes::COLLISION_BOX);
    registry.register(&nodes::COLLISION_SPHERE);
    registry.register(&nodes::COLLISION_CAPSULE);
    registry.register(&nodes::COLLISION_CYLINDER);

    // Environment
    registry.register(&nodes::WORLD_ENVIRONMENT);
    registry.register(&nodes::AUDIO_LISTENER);

    // Cameras
    registry.register(&nodes::CAMERA3D);
    registry.register(&nodes::CAMERA_RIG);

    // 2D Nodes
    registry.register(&nodes::NODE2D);
    registry.register(&nodes::SPRITE2D);
    registry.register(&nodes::CAMERA2D);

    // UI Nodes
    registry.register(&nodes::UI_PANEL);
    registry.register(&nodes::UI_LABEL);
    registry.register(&nodes::UI_BUTTON);
    registry.register(&nodes::UI_IMAGE);
}
