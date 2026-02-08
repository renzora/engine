//! Node type definitions and registry
//!
//! Each node type defines its pins, default values, and code generation behavior.

mod ai;
mod animation;
mod arrays;
mod audio;
mod camera;
pub mod components;
mod debug;
mod easing;
mod ecs;
mod events;
mod flow;
mod health;
mod hierarchy;
mod input;
mod logic;
mod math;
mod physics;
mod rendering;
mod scene;
pub mod shader;
mod state;
mod strings;
mod time;
mod transform;
mod ui;
mod utility;
mod window;

use std::collections::HashMap;
use bevy::prelude::*;
use super::{BlueprintNode, NodeId, Pin, PinType, PinValue, PinDirection};

/// Definition of a node type
#[allow(dead_code)]
pub struct NodeTypeDefinition {
    /// Unique type ID (e.g., "math/add")
    pub type_id: &'static str,
    /// Display name in the node palette
    pub display_name: &'static str,
    /// Category for organization (e.g., "Math", "Events")
    pub category: &'static str,
    /// Description shown in tooltips
    pub description: &'static str,
    /// Function to create the node's pins
    pub create_pins: fn() -> Vec<Pin>,
    /// Accent color for the node header [r, g, b]
    pub color: [u8; 3],
    /// Whether this is an event node (entry point)
    pub is_event: bool,
    /// Whether this node can have a comment
    pub is_comment: bool,
}

impl NodeTypeDefinition {
    /// Create a new node instance with this type
    pub fn create_node(&self, id: NodeId) -> BlueprintNode {
        let mut node = BlueprintNode::with_pins(id, self.type_id, (self.create_pins)());

        // Set default values for all input pins that have them
        for pin in &node.pins {
            if pin.direction == PinDirection::Input {
                if let Some(default) = &pin.default_value {
                    node.input_values.insert(pin.name.clone(), default.clone());
                }
            }
        }

        node
    }
}

/// Dynamic node definition for auto-generated component nodes.
/// Unlike NodeTypeDefinition (which uses static refs), this owns its data.
pub struct ComponentNodeDef {
    pub type_id: String,
    pub display_name: String,
    pub category: String,
    pub description: String,
    pub pins: Vec<Pin>,
    pub color: [u8; 3],
}

impl ComponentNodeDef {
    /// Create a node instance from this dynamic definition
    pub fn create_node(&self, id: NodeId) -> BlueprintNode {
        let mut node = BlueprintNode::with_pins(id, &self.type_id, self.pins.clone());

        for pin in &node.pins {
            if pin.direction == PinDirection::Input {
                if let Some(default) = &pin.default_value {
                    node.input_values.insert(pin.name.clone(), default.clone());
                }
            }
        }

        node
    }
}

/// Entry in the node palette — either a static built-in or a dynamic component node
pub enum NodeEntry {
    Static(&'static NodeTypeDefinition),
    Dynamic(String), // key into component_nodes
}

/// Registry of all available node types
#[derive(Resource)]
pub struct NodeRegistry {
    /// Static node types indexed by type_id
    pub types: HashMap<String, &'static NodeTypeDefinition>,
    /// Dynamic component nodes indexed by type_id
    pub component_nodes: HashMap<String, ComponentNodeDef>,
    /// Node entries organized by category (includes both static and dynamic)
    pub by_category: HashMap<String, Vec<NodeEntry>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            component_nodes: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Register a static node type
    pub fn register(&mut self, def: &'static NodeTypeDefinition) {
        self.types.insert(def.type_id.to_string(), def);
        self.by_category
            .entry(def.category.to_string())
            .or_default()
            .push(NodeEntry::Static(def));
    }

    /// Register a dynamic component node
    pub fn register_component_node(&mut self, def: ComponentNodeDef) {
        let type_id = def.type_id.clone();
        let category = def.category.clone();
        self.component_nodes.insert(type_id.clone(), def);
        self.by_category
            .entry(category)
            .or_default()
            .push(NodeEntry::Dynamic(type_id));
    }

    /// Get a static node type by ID
    pub fn get(&self, type_id: &str) -> Option<&'static NodeTypeDefinition> {
        self.types.get(type_id).copied()
    }

    /// Get a dynamic component node by ID
    pub fn get_component_node(&self, type_id: &str) -> Option<&ComponentNodeDef> {
        self.component_nodes.get(type_id)
    }

    /// Get all categories
    pub fn categories(&self) -> impl Iterator<Item = &String> {
        self.by_category.keys()
    }

    /// Get all node entries in a category
    pub fn entries_in_category(&self, category: &str) -> Option<&Vec<NodeEntry>> {
        self.by_category.get(category)
    }

    /// Create a node instance from a type ID (checks both static and dynamic)
    pub fn create_node(&self, type_id: &str, id: NodeId) -> Option<BlueprintNode> {
        if let Some(def) = self.get(type_id) {
            return Some(def.create_node(id));
        }
        if let Some(def) = self.get_component_node(type_id) {
            return Some(def.create_node(id));
        }
        None
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Register all built-in node types
pub fn register_all_nodes(registry: &mut NodeRegistry) {
    // Events
    registry.register(&events::ON_READY);
    registry.register(&events::ON_UPDATE);

    // Math
    registry.register(&math::ADD);
    registry.register(&math::SUBTRACT);
    registry.register(&math::MULTIPLY);
    registry.register(&math::DIVIDE);
    registry.register(&math::LERP);
    registry.register(&math::CLAMP);
    registry.register(&math::ABS);
    registry.register(&math::MIN);
    registry.register(&math::MAX);
    registry.register(&math::SIN);
    registry.register(&math::COS);

    // Logic
    registry.register(&logic::IF_BRANCH);
    registry.register(&logic::COMPARE);
    registry.register(&logic::AND);
    registry.register(&logic::OR);
    registry.register(&logic::NOT);

    // Transform
    registry.register(&transform::GET_POSITION);
    registry.register(&transform::SET_POSITION);
    registry.register(&transform::TRANSLATE);
    registry.register(&transform::GET_ROTATION);
    registry.register(&transform::SET_ROTATION);
    registry.register(&transform::ROTATE);

    // Input
    registry.register(&input::GET_INPUT_AXIS);
    registry.register(&input::IS_KEY_PRESSED);
    registry.register(&input::IS_KEY_JUST_PRESSED);
    registry.register(&input::IS_KEY_JUST_RELEASED);
    registry.register(&input::GET_MOUSE_POSITION);
    registry.register(&input::GET_MOUSE_DELTA);
    registry.register(&input::IS_MOUSE_BUTTON_PRESSED);
    registry.register(&input::GET_MOUSE_SCROLL);
    registry.register(&input::GET_GAMEPAD_LEFT_STICK);
    registry.register(&input::GET_GAMEPAD_RIGHT_STICK);
    registry.register(&input::IS_GAMEPAD_BUTTON_PRESSED);

    // Utility
    registry.register(&utility::PRINT);
    registry.register(&utility::SEQUENCE);
    registry.register(&utility::COMMENT);
    registry.register(&utility::GET_DELTA);
    registry.register(&utility::GET_ELAPSED);

    // Variables
    registry.register(&utility::GET_VARIABLE);
    registry.register(&utility::SET_VARIABLE);

    // Shader Input
    registry.register(&shader::UV);
    registry.register(&shader::WORLD_POSITION);
    registry.register(&shader::WORLD_NORMAL);
    registry.register(&shader::VIEW_DIRECTION);
    registry.register(&shader::TIME);
    registry.register(&shader::VERTEX_COLOR);

    // Shader Texture
    registry.register(&shader::TEXTURE_COLOR);
    registry.register(&shader::TEXTURE_NORMAL_DX);
    registry.register(&shader::TEXTURE_NORMAL_GL);
    registry.register(&shader::TEXTURE_ROUGHNESS);
    registry.register(&shader::TEXTURE_METALLIC);
    registry.register(&shader::TEXTURE_DISPLACEMENT);
    registry.register(&shader::TEXTURE_AO);
    registry.register(&shader::TEXTURE_EMISSIVE);
    registry.register(&shader::TEXTURE_OPACITY);
    registry.register(&shader::TEXTURE_GENERIC);

    // Shader Math
    registry.register(&shader::DOT);
    registry.register(&shader::CROSS);
    registry.register(&shader::NORMALIZE);
    registry.register(&shader::LENGTH);
    registry.register(&shader::DISTANCE);
    registry.register(&shader::REFLECT);
    registry.register(&shader::FRESNEL);
    registry.register(&shader::POW);
    registry.register(&shader::SMOOTHSTEP);
    registry.register(&shader::STEP);
    registry.register(&shader::FRACT);
    registry.register(&shader::FLOOR);
    registry.register(&shader::CEIL);
    registry.register(&shader::ONE_MINUS);
    registry.register(&shader::SATURATE);

    // Shader Vector
    registry.register(&shader::MAKE_VEC2);
    registry.register(&shader::MAKE_VEC3);
    registry.register(&shader::MAKE_VEC4);
    registry.register(&shader::MAKE_COLOR);
    registry.register(&shader::SPLIT_VEC2);
    registry.register(&shader::SPLIT_VEC3);
    registry.register(&shader::SPLIT_COLOR);
    registry.register(&shader::COLOR_CONSTANT);
    registry.register(&shader::FLOAT_CONSTANT);

    // Shader Output
    registry.register(&shader::PBR_OUTPUT);
    registry.register(&shader::UNLIT_OUTPUT);

    // Shader Noise/Procedural
    registry.register(&shader::NOISE_SIMPLE);
    registry.register(&shader::NOISE_GRADIENT);
    registry.register(&shader::NOISE_VORONOI);
    registry.register(&shader::CHECKERBOARD);
    registry.register(&shader::GRADIENT);

    // Shader Color Manipulation
    registry.register(&shader::RGB_TO_HSV);
    registry.register(&shader::HSV_TO_RGB);
    registry.register(&shader::HUE_SHIFT);
    registry.register(&shader::SATURATION);
    registry.register(&shader::BRIGHTNESS);
    registry.register(&shader::CONTRAST);
    registry.register(&shader::DESATURATE);
    registry.register(&shader::INVERT_COLOR);
    registry.register(&shader::LERP_COLOR);
    registry.register(&shader::LERP_VEC3);

    // Shader UV Manipulation
    registry.register(&shader::UV_TILING);
    registry.register(&shader::UV_OFFSET);
    registry.register(&shader::UV_ROTATE);
    registry.register(&shader::UV_FLIPBOOK);
    registry.register(&shader::TRIPLANAR);

    // Shader Advanced Noise
    registry.register(&shader::NOISE_FBM);
    registry.register(&shader::NOISE_TURBULENCE);
    registry.register(&shader::NOISE_RIDGED);
    registry.register(&shader::DOMAIN_WARP);

    // Shader Effects
    registry.register(&shader::RIM_LIGHT);
    registry.register(&shader::PARALLAX);
    registry.register(&shader::NORMAL_BLEND);
    registry.register(&shader::DETAIL_BLEND);
    registry.register(&shader::POSTERIZE);
    registry.register(&shader::DITHER);
    registry.register(&shader::PIXELATE);
    registry.register(&shader::EDGE_DETECT);

    // Shader Blend Modes
    registry.register(&shader::BLEND_MULTIPLY);
    registry.register(&shader::BLEND_SCREEN);
    registry.register(&shader::BLEND_OVERLAY);
    registry.register(&shader::BLEND_ADD);
    registry.register(&shader::BLEND_SOFTLIGHT);

    // Shader Patterns
    registry.register(&shader::BRICK);
    registry.register(&shader::WAVE_SINE);
    registry.register(&shader::WAVE_SQUARE);
    registry.register(&shader::WAVE_SAWTOOTH);
    registry.register(&shader::RADIAL_GRADIENT);
    registry.register(&shader::SPIRAL);

    // Shader SDF (Signed Distance Fields)
    registry.register(&shader::SDF_CIRCLE);
    registry.register(&shader::SDF_BOX);
    registry.register(&shader::SDF_UNION);
    registry.register(&shader::SDF_INTERSECTION);
    registry.register(&shader::SDF_SMOOTH_UNION);

    // Shader Color (Additional)
    registry.register(&shader::GAMMA_TO_LINEAR);
    registry.register(&shader::LINEAR_TO_GAMMA);
    registry.register(&shader::LEVELS);
    registry.register(&shader::GRADIENT_MAP);

    // =========================================================================
    // BEHAVIOR NODES (for gameplay scripting)
    // =========================================================================

    // ECS - Entity Management
    registry.register(&ecs::SPAWN_ENTITY);
    registry.register(&ecs::DESPAWN_ENTITY);
    registry.register(&ecs::SELF_ENTITY);
    registry.register(&ecs::ENTITY_VALID);
    registry.register(&ecs::FIND_ENTITY_BY_NAME);
    registry.register(&ecs::GET_ENTITY_NAME);
    registry.register(&ecs::SET_ENTITY_NAME);

    // ECS - Components
    registry.register(&ecs::ADD_COMPONENT);
    registry.register(&ecs::REMOVE_COMPONENT);
    registry.register(&ecs::HAS_COMPONENT);

    // ECS - Tags
    registry.register(&ecs::ADD_TAG);
    registry.register(&ecs::REMOVE_TAG);
    registry.register(&ecs::HAS_TAG);
    registry.register(&ecs::FIND_BY_TAG);

    // ECS - Queries
    registry.register(&ecs::GET_ALL_ENTITIES);
    registry.register(&ecs::FOR_EACH_ENTITY);
    registry.register(&ecs::GET_CLOSEST_ENTITY);
    registry.register(&ecs::GET_ENTITIES_IN_RADIUS);

    // Health - Data
    registry.register(&health::GET_HEALTH);
    registry.register(&health::IS_DEAD);
    registry.register(&health::IS_INVINCIBLE);

    // Health - Actions
    registry.register(&health::DAMAGE);
    registry.register(&health::HEAL);
    registry.register(&health::SET_HEALTH);
    registry.register(&health::SET_MAX_HEALTH);
    registry.register(&health::SET_INVINCIBLE);
    registry.register(&health::KILL);
    registry.register(&health::REVIVE);

    // Health - Events
    registry.register(&health::ON_DAMAGE);
    registry.register(&health::ON_DEATH);
    registry.register(&health::ON_HEAL);

    // Physics - Rigid Body
    registry.register(&physics::ADD_RIGID_BODY);
    registry.register(&physics::SET_BODY_TYPE);
    registry.register(&physics::SET_MASS);
    registry.register(&physics::GET_VELOCITY);
    registry.register(&physics::SET_VELOCITY);
    registry.register(&physics::GET_ANGULAR_VELOCITY);
    registry.register(&physics::SET_ANGULAR_VELOCITY);

    // Physics - Forces
    registry.register(&physics::APPLY_FORCE);
    registry.register(&physics::APPLY_FORCE_AT_POINT);
    registry.register(&physics::APPLY_IMPULSE);
    registry.register(&physics::APPLY_TORQUE);
    registry.register(&physics::APPLY_TORQUE_IMPULSE);

    // Physics - Colliders
    registry.register(&physics::ADD_BOX_COLLIDER);
    registry.register(&physics::ADD_SPHERE_COLLIDER);
    registry.register(&physics::ADD_CAPSULE_COLLIDER);
    registry.register(&physics::ADD_CYLINDER_COLLIDER);
    registry.register(&physics::ADD_MESH_COLLIDER);
    registry.register(&physics::SET_FRICTION);
    registry.register(&physics::SET_RESTITUTION);

    // Physics - Raycasting
    registry.register(&physics::RAYCAST);
    registry.register(&physics::RAYCAST_ALL);
    registry.register(&physics::SPHERECAST);

    // Physics - Collision Events
    registry.register(&physics::ON_COLLISION_ENTER);
    registry.register(&physics::ON_COLLISION_EXIT);
    registry.register(&physics::ON_COLLISION_STAY);
    registry.register(&physics::ON_TRIGGER_ENTER);
    registry.register(&physics::ON_TRIGGER_EXIT);

    // Physics - Settings
    registry.register(&physics::SET_GRAVITY_SCALE);
    registry.register(&physics::SET_LINEAR_DAMPING);
    registry.register(&physics::SET_ANGULAR_DAMPING);
    registry.register(&physics::LOCK_ROTATION);
    registry.register(&physics::LOCK_POSITION);

    // Physics - Character Controller
    registry.register(&physics::ADD_CHARACTER_CONTROLLER);
    registry.register(&physics::MOVE_CHARACTER);
    registry.register(&physics::IS_GROUNDED);

    // Audio - Sound Playback
    registry.register(&audio::PLAY_SOUND);
    registry.register(&audio::PLAY_SOUND_AT);
    registry.register(&audio::PLAY_SOUND_ATTACHED);
    registry.register(&audio::STOP_SOUND);
    registry.register(&audio::PAUSE_SOUND);
    registry.register(&audio::RESUME_SOUND);

    // Audio - Music
    registry.register(&audio::PLAY_MUSIC);
    registry.register(&audio::STOP_MUSIC);
    registry.register(&audio::CROSSFADE_MUSIC);

    // Audio - Properties
    registry.register(&audio::SET_VOLUME);
    registry.register(&audio::SET_PITCH);
    registry.register(&audio::SET_PANNING);
    registry.register(&audio::SET_MASTER_VOLUME);

    // Audio - Queries
    registry.register(&audio::IS_PLAYING);
    registry.register(&audio::GET_PLAYBACK_POSITION);
    registry.register(&audio::SET_PLAYBACK_POSITION);

    // Audio - Events
    registry.register(&audio::ON_SOUND_FINISHED);

    // Audio - Spatial
    registry.register(&audio::SET_AUDIO_LISTENER);
    registry.register(&audio::SET_SPATIAL_PROPERTIES);

    // Animation - Skeletal
    registry.register(&animation::PLAY_ANIMATION);
    registry.register(&animation::PLAY_ANIMATION_ONCE);
    registry.register(&animation::STOP_ANIMATION);
    registry.register(&animation::PAUSE_ANIMATION);
    registry.register(&animation::RESUME_ANIMATION);
    registry.register(&animation::SET_ANIMATION_SPEED);
    registry.register(&animation::SET_ANIMATION_TIME);
    registry.register(&animation::GET_ANIMATION_TIME);
    registry.register(&animation::IS_ANIMATION_PLAYING);

    // Animation - Blending
    registry.register(&animation::CROSSFADE_ANIMATION);
    registry.register(&animation::SET_ANIMATION_WEIGHT);

    // Animation - Events
    registry.register(&animation::ON_ANIMATION_FINISHED);
    registry.register(&animation::ON_ANIMATION_LOOP);

    // Animation - Tweening
    registry.register(&animation::TWEEN_POSITION);
    registry.register(&animation::TWEEN_ROTATION);
    registry.register(&animation::TWEEN_SCALE);
    registry.register(&animation::TWEEN_FLOAT);
    registry.register(&animation::TWEEN_COLOR);
    registry.register(&animation::CANCEL_TWEEN);

    // Animation - Sprite
    registry.register(&animation::PLAY_SPRITE_ANIMATION);
    registry.register(&animation::SET_SPRITE_FRAME);
    registry.register(&animation::GET_SPRITE_FRAME);

    // Camera - Control
    registry.register(&camera::GET_MAIN_CAMERA);
    registry.register(&camera::SET_MAIN_CAMERA);
    registry.register(&camera::CAMERA_LOOK_AT);
    registry.register(&camera::CAMERA_FOLLOW);
    registry.register(&camera::CAMERA_ORBIT);

    // Camera - Projection
    registry.register(&camera::SET_PERSPECTIVE);
    registry.register(&camera::SET_ORTHOGRAPHIC);
    registry.register(&camera::SET_FOV);
    registry.register(&camera::GET_FOV);

    // Camera - Screen Space
    registry.register(&camera::WORLD_TO_SCREEN);
    registry.register(&camera::SCREEN_TO_WORLD);
    registry.register(&camera::SCREEN_TO_WORLD_PLANE);
    registry.register(&camera::GET_VIEWPORT_SIZE);

    // Camera - Effects
    registry.register(&camera::CAMERA_SHAKE);
    registry.register(&camera::CAMERA_ZOOM);
    registry.register(&camera::SET_CLEAR_COLOR);
    registry.register(&camera::SET_CAMERA_ACTIVE);
    registry.register(&camera::SET_CAMERA_ORDER);

    // Rendering - Mesh
    registry.register(&rendering::SPAWN_MESH);
    registry.register(&rendering::SET_MESH);
    registry.register(&rendering::SPAWN_PRIMITIVE);

    // Rendering - Material
    registry.register(&rendering::SET_MATERIAL);
    registry.register(&rendering::SET_COLOR);
    registry.register(&rendering::GET_COLOR);
    registry.register(&rendering::SET_EMISSIVE);
    registry.register(&rendering::SET_PBR_PROPERTIES);
    registry.register(&rendering::SET_TEXTURE);

    // Rendering - Visibility
    registry.register(&rendering::SET_VISIBILITY);
    registry.register(&rendering::GET_VISIBILITY);
    registry.register(&rendering::TOGGLE_VISIBILITY);

    // Rendering - Lights
    registry.register(&rendering::SPAWN_POINT_LIGHT);
    registry.register(&rendering::SPAWN_SPOT_LIGHT);
    registry.register(&rendering::SPAWN_DIRECTIONAL_LIGHT);
    registry.register(&rendering::SET_LIGHT_COLOR);
    registry.register(&rendering::SET_LIGHT_INTENSITY);
    registry.register(&rendering::SET_LIGHT_RANGE);
    registry.register(&rendering::SET_LIGHT_SHADOWS);
    registry.register(&rendering::SET_AMBIENT_LIGHT);

    // Rendering - Environment
    registry.register(&rendering::SET_FOG);
    registry.register(&rendering::SET_SKYBOX);

    // Rendering - 2D
    registry.register(&rendering::SPAWN_SPRITE);
    registry.register(&rendering::SET_SPRITE);
    registry.register(&rendering::SET_SPRITE_COLOR);
    registry.register(&rendering::SET_SPRITE_FLIP);

    // Rendering - Particles
    registry.register(&rendering::SPAWN_PARTICLES);
    registry.register(&rendering::PLAY_PARTICLES);
    registry.register(&rendering::STOP_PARTICLES);

    // UI - Text
    registry.register(&ui::SPAWN_TEXT);
    registry.register(&ui::SET_TEXT);
    registry.register(&ui::GET_TEXT);
    registry.register(&ui::SET_TEXT_COLOR);
    registry.register(&ui::SET_FONT_SIZE);

    // UI - Buttons
    registry.register(&ui::SPAWN_BUTTON);
    registry.register(&ui::ON_BUTTON_CLICKED);
    registry.register(&ui::ON_BUTTON_HOVERED);
    registry.register(&ui::SET_BUTTON_ENABLED);

    // UI - Images
    registry.register(&ui::SPAWN_UI_IMAGE);
    registry.register(&ui::SET_UI_IMAGE);
    registry.register(&ui::SET_IMAGE_COLOR);

    // UI - Containers
    registry.register(&ui::SPAWN_UI_NODE);
    registry.register(&ui::SET_UI_POSITION);
    registry.register(&ui::SET_UI_SIZE);
    registry.register(&ui::GET_UI_SIZE);
    registry.register(&ui::SET_BACKGROUND_COLOR);
    registry.register(&ui::SET_UI_BORDER);
    registry.register(&ui::SET_BORDER_RADIUS);

    // UI - Visibility
    registry.register(&ui::SET_UI_VISIBILITY);
    registry.register(&ui::TOGGLE_UI_VISIBILITY);

    // UI - Input Fields
    registry.register(&ui::SPAWN_TEXT_INPUT);
    registry.register(&ui::GET_TEXT_INPUT_VALUE);
    registry.register(&ui::SET_TEXT_INPUT_VALUE);
    registry.register(&ui::ON_TEXT_INPUT_CHANGED);
    registry.register(&ui::ON_TEXT_INPUT_SUBMITTED);

    // UI - Sliders
    registry.register(&ui::SPAWN_SLIDER);
    registry.register(&ui::GET_SLIDER_VALUE);
    registry.register(&ui::SET_SLIDER_VALUE);
    registry.register(&ui::ON_SLIDER_CHANGED);

    // UI - Progress Bar
    registry.register(&ui::SPAWN_PROGRESS_BAR);
    registry.register(&ui::SET_PROGRESS_VALUE);

    // UI - Parenting
    registry.register(&ui::ADD_UI_CHILD);
    registry.register(&ui::REMOVE_UI_CHILD);

    // UI - Z-Order
    registry.register(&ui::SET_Z_INDEX);
    registry.register(&ui::BRING_TO_FRONT);
    registry.register(&ui::SEND_TO_BACK);

    // Hierarchy - Parenting
    registry.register(&hierarchy::SET_PARENT);
    registry.register(&hierarchy::REMOVE_PARENT);
    registry.register(&hierarchy::GET_PARENT);
    registry.register(&hierarchy::HAS_PARENT);

    // Hierarchy - Children
    registry.register(&hierarchy::ADD_CHILD);
    registry.register(&hierarchy::REMOVE_CHILD);
    registry.register(&hierarchy::GET_CHILDREN);
    registry.register(&hierarchy::GET_CHILD_AT);
    registry.register(&hierarchy::GET_CHILD_COUNT);
    registry.register(&hierarchy::HAS_CHILDREN);
    registry.register(&hierarchy::FOR_EACH_CHILD);

    // Hierarchy - Queries
    registry.register(&hierarchy::GET_ROOT);
    registry.register(&hierarchy::IS_ROOT);
    registry.register(&hierarchy::IS_ANCESTOR_OF);
    registry.register(&hierarchy::IS_DESCENDANT_OF);
    registry.register(&hierarchy::GET_ALL_DESCENDANTS);
    registry.register(&hierarchy::GET_DEPTH);

    // Hierarchy - Transforms
    registry.register(&hierarchy::GET_LOCAL_POSITION);
    registry.register(&hierarchy::SET_LOCAL_POSITION);
    registry.register(&hierarchy::GET_LOCAL_ROTATION);
    registry.register(&hierarchy::SET_LOCAL_ROTATION);
    registry.register(&hierarchy::GET_LOCAL_SCALE);
    registry.register(&hierarchy::SET_LOCAL_SCALE);
    registry.register(&hierarchy::LOCAL_TO_WORLD);
    registry.register(&hierarchy::WORLD_TO_LOCAL);

    // Scene - Loading
    registry.register(&scene::LOAD_SCENE);
    registry.register(&scene::LOAD_SCENE_ASYNC);
    registry.register(&scene::SPAWN_SCENE);
    registry.register(&scene::UNLOAD_SCENE);
    registry.register(&scene::IS_SCENE_LOADED);
    registry.register(&scene::ON_SCENE_LOADED);

    // Scene - Transitions
    registry.register(&scene::CHANGE_SCENE);
    registry.register(&scene::RELOAD_SCENE);
    registry.register(&scene::GET_CURRENT_SCENE);

    // Scene - Prefabs
    registry.register(&scene::LOAD_PREFAB);
    registry.register(&scene::INSTANTIATE_PREFAB);
    registry.register(&scene::INSTANTIATE_AT_TRANSFORM);

    // Scene - GLTF
    registry.register(&scene::LOAD_GLTF);
    registry.register(&scene::SPAWN_GLTF_SCENE);
    registry.register(&scene::GET_GLTF_SCENE_COUNT);

    // Scene - Queries
    registry.register(&scene::FIND_IN_SCENE);
    registry.register(&scene::FIND_ALL_IN_SCENE);
    registry.register(&scene::GET_SCENE_ROOT);

    // Scene - Serialization
    registry.register(&scene::SAVE_SCENE);
    registry.register(&scene::CLONE_ENTITY_TREE);

    // State - App States
    registry.register(&state::GET_CURRENT_STATE);
    registry.register(&state::SET_STATE);
    registry.register(&state::PUSH_STATE);
    registry.register(&state::POP_STATE);
    registry.register(&state::ON_STATE_ENTER);
    registry.register(&state::ON_STATE_EXIT);
    registry.register(&state::ON_STATE_TRANSITION);
    registry.register(&state::IS_IN_STATE);

    // State - Pause
    registry.register(&state::PAUSE_GAME);
    registry.register(&state::RESUME_GAME);
    registry.register(&state::TOGGLE_PAUSE);
    registry.register(&state::IS_PAUSED);
    registry.register(&state::ON_PAUSE);
    registry.register(&state::ON_RESUME);

    // State - Global Variables
    registry.register(&state::SET_GLOBAL_VAR);
    registry.register(&state::GET_GLOBAL_VAR);
    registry.register(&state::HAS_GLOBAL_VAR);
    registry.register(&state::REMOVE_GLOBAL_VAR);

    // State - Persistence
    registry.register(&state::SAVE_GAME_DATA);
    registry.register(&state::LOAD_GAME_DATA);
    registry.register(&state::DELETE_SAVE_DATA);
    registry.register(&state::HAS_SAVE_DATA);
    registry.register(&state::GET_SAVE_SLOTS);

    // State - Lifecycle
    registry.register(&state::QUIT_GAME);
    registry.register(&state::RESTART_GAME);
    registry.register(&state::ON_QUIT_REQUESTED);

    // Debug - Drawing
    registry.register(&debug::DEBUG_LINE);
    registry.register(&debug::DEBUG_RAY);
    registry.register(&debug::DEBUG_SPHERE);
    registry.register(&debug::DEBUG_BOX);
    registry.register(&debug::DEBUG_CAPSULE);
    registry.register(&debug::DEBUG_POINT);
    registry.register(&debug::DEBUG_ARROW);
    registry.register(&debug::DEBUG_AXES);
    registry.register(&debug::CLEAR_DEBUG_DRAWS);

    // Debug - Text
    registry.register(&debug::DEBUG_TEXT_3D);
    registry.register(&debug::DEBUG_TEXT_2D);

    // Debug - Logging
    registry.register(&debug::LOG_MESSAGE);
    registry.register(&debug::LOG_WARNING);
    registry.register(&debug::LOG_ERROR);
    registry.register(&debug::LOG_VALUE);

    // Debug - Performance
    registry.register(&debug::GET_FPS);
    registry.register(&debug::START_TIMER);
    registry.register(&debug::STOP_TIMER);
    registry.register(&debug::GET_ENTITY_COUNT);

    // Debug - Assertions
    registry.register(&debug::ASSERT);
    registry.register(&debug::ASSERT_EQUAL);

    // Debug - Toggles
    registry.register(&debug::TOGGLE_PHYSICS_DEBUG);
    registry.register(&debug::TOGGLE_WIREFRAME);
    registry.register(&debug::TOGGLE_BOUNDING_BOXES);

    // Debug - Breakpoints
    registry.register(&debug::BREAKPOINT);

    // Flow - Loops
    registry.register(&flow::FOR_LOOP);
    registry.register(&flow::FOR_EACH);
    registry.register(&flow::WHILE_LOOP);
    registry.register(&flow::DO_WHILE);
    registry.register(&flow::BREAK);
    registry.register(&flow::CONTINUE);

    // Flow - Conditionals
    registry.register(&flow::IF);
    registry.register(&flow::SWITCH_INT);
    registry.register(&flow::SWITCH_STRING);
    registry.register(&flow::MULTI_GATE);
    registry.register(&flow::DO_ONCE);
    registry.register(&flow::DO_N);
    registry.register(&flow::FLIP_FLOP);
    registry.register(&flow::GATE);

    // Flow - Sequence/Parallel
    registry.register(&flow::SEQUENCE);
    registry.register(&flow::PARALLEL);

    // Flow - Selection
    registry.register(&flow::SELECT_INT);
    registry.register(&flow::SELECT_FLOAT);
    registry.register(&flow::SELECT_STRING);
    registry.register(&flow::SELECT_VEC3);
    registry.register(&flow::SELECT_ENTITY);

    // Flow - Return
    registry.register(&flow::RETURN);

    // Time - Values
    registry.register(&time::GET_DELTA_TIME);
    registry.register(&time::GET_ELAPSED_TIME);
    registry.register(&time::GET_UNSCALED_DELTA);
    registry.register(&time::GET_UNSCALED_ELAPSED);
    registry.register(&time::GET_FRAME_COUNT);

    // Time - Scale
    registry.register(&time::GET_TIME_SCALE);
    registry.register(&time::SET_TIME_SCALE);

    // Time - Timers
    registry.register(&time::CREATE_TIMER);
    registry.register(&time::START_TIMER);
    registry.register(&time::STOP_TIMER);
    registry.register(&time::PAUSE_TIMER);
    registry.register(&time::RESUME_TIMER);
    registry.register(&time::RESET_TIMER);
    registry.register(&time::GET_TIMER_PROGRESS);
    registry.register(&time::IS_TIMER_FINISHED);
    registry.register(&time::IS_TIMER_RUNNING);
    registry.register(&time::ON_TIMER_FINISHED);

    // Time - Delays
    registry.register(&time::DELAY);
    registry.register(&time::DELAY_FRAMES);
    registry.register(&time::WAIT_UNTIL);
    registry.register(&time::RETRIGGERABLE_DELAY);

    // Time - Cooldown
    registry.register(&time::COOLDOWN);
    registry.register(&time::IS_ON_COOLDOWN);

    // Time - Periodic
    registry.register(&time::EVERY_N_SECONDS);
    registry.register(&time::EVERY_N_FRAMES);

    // Time - Real Time
    registry.register(&time::GET_SYSTEM_TIME);
    registry.register(&time::GET_SYSTEM_DATE);
    registry.register(&time::GET_TIMESTAMP);

    // Window - Properties
    registry.register(&window::GET_WINDOW_SIZE);
    registry.register(&window::SET_WINDOW_SIZE);
    registry.register(&window::GET_WINDOW_POSITION);
    registry.register(&window::SET_WINDOW_POSITION);
    registry.register(&window::CENTER_WINDOW);
    registry.register(&window::GET_WINDOW_TITLE);
    registry.register(&window::SET_WINDOW_TITLE);

    // Window - Modes
    registry.register(&window::SET_FULLSCREEN);
    registry.register(&window::TOGGLE_FULLSCREEN);
    registry.register(&window::IS_FULLSCREEN);
    registry.register(&window::SET_BORDERLESS);
    registry.register(&window::MINIMIZE_WINDOW);
    registry.register(&window::MAXIMIZE_WINDOW);
    registry.register(&window::RESTORE_WINDOW);
    registry.register(&window::IS_MINIMIZED);
    registry.register(&window::IS_MAXIMIZED);

    // Window - Decorations
    registry.register(&window::SET_RESIZABLE);
    registry.register(&window::SET_DECORATIONS);
    registry.register(&window::SET_ALWAYS_ON_TOP);

    // Window - Cursor
    registry.register(&window::GET_CURSOR_POSITION);
    registry.register(&window::SET_CURSOR_POSITION);
    registry.register(&window::SHOW_CURSOR);
    registry.register(&window::HIDE_CURSOR);
    registry.register(&window::LOCK_CURSOR);
    registry.register(&window::CONFINE_CURSOR);
    registry.register(&window::SET_CURSOR_ICON);

    // Window - Display Info
    registry.register(&window::GET_MONITOR_SIZE);
    registry.register(&window::GET_MONITOR_COUNT);
    registry.register(&window::GET_SCALE_FACTOR);

    // Window - Events
    registry.register(&window::ON_WINDOW_RESIZED);
    registry.register(&window::ON_WINDOW_MOVED);
    registry.register(&window::ON_WINDOW_FOCUSED);
    registry.register(&window::IS_WINDOW_FOCUSED);
    registry.register(&window::ON_CLOSE_REQUESTED);

    // Window - VSync
    registry.register(&window::SET_VSYNC);
    registry.register(&window::IS_VSYNC_ENABLED);

    // =========================================================================
    // NEW MATH NODES
    // =========================================================================
    registry.register(&math::TAN);
    registry.register(&math::ASIN);
    registry.register(&math::ACOS);
    registry.register(&math::ATAN);
    registry.register(&math::ATAN2);
    registry.register(&math::FLOOR);
    registry.register(&math::CEIL);
    registry.register(&math::ROUND);
    registry.register(&math::SQRT);
    registry.register(&math::POW);
    registry.register(&math::LOG);
    registry.register(&math::EXP);
    registry.register(&math::SIGN);
    registry.register(&math::MOD);
    registry.register(&math::FRACT);
    registry.register(&math::NEGATE);
    registry.register(&math::ONE_MINUS);
    registry.register(&math::RECIPROCAL);
    registry.register(&math::SMOOTHSTEP);
    registry.register(&math::STEP);
    registry.register(&math::RANDOM);
    registry.register(&math::RANDOM_RANGE);
    registry.register(&math::RANDOM_INT);
    registry.register(&math::MAP_RANGE);
    registry.register(&math::DEG_TO_RAD);
    registry.register(&math::RAD_TO_DEG);

    // Math Vector
    registry.register(&math::DOT);
    registry.register(&math::CROSS);
    registry.register(&math::NORMALIZE);
    registry.register(&math::LENGTH);
    registry.register(&math::DISTANCE);
    registry.register(&math::DIRECTION_TO);
    registry.register(&math::ANGLE_BETWEEN);
    registry.register(&math::REFLECT);
    registry.register(&math::LERP_VEC3);
    registry.register(&math::MAKE_VEC3);
    registry.register(&math::BREAK_VEC3);
    registry.register(&math::RANDOM_VEC3);
    registry.register(&math::RANDOM_DIRECTION);

    // =========================================================================
    // STRING NODES
    // =========================================================================
    registry.register(&strings::CONCAT);
    registry.register(&strings::CONCAT_MULTI);
    registry.register(&strings::JOIN);
    registry.register(&strings::STRING_LENGTH);
    registry.register(&strings::IS_EMPTY);
    registry.register(&strings::CONTAINS);
    registry.register(&strings::STARTS_WITH);
    registry.register(&strings::ENDS_WITH);
    registry.register(&strings::INDEX_OF);
    registry.register(&strings::STRING_EQUALS);
    registry.register(&strings::STRING_EQUALS_IGNORE_CASE);
    registry.register(&strings::SUBSTRING);
    registry.register(&strings::CHAR_AT);
    registry.register(&strings::REPLACE);
    registry.register(&strings::SPLIT);
    registry.register(&strings::TO_UPPER);
    registry.register(&strings::TO_LOWER);
    registry.register(&strings::CAPITALIZE);
    registry.register(&strings::TRIM);
    registry.register(&strings::TRIM_START);
    registry.register(&strings::TRIM_END);
    registry.register(&strings::PAD_LEFT);
    registry.register(&strings::PAD_RIGHT);
    registry.register(&strings::FORMAT);
    registry.register(&strings::INT_TO_STRING);
    registry.register(&strings::FLOAT_TO_STRING);
    registry.register(&strings::BOOL_TO_STRING);
    registry.register(&strings::STRING_TO_INT);
    registry.register(&strings::STRING_TO_FLOAT);
    registry.register(&strings::REPEAT);
    registry.register(&strings::REVERSE);

    // =========================================================================
    // ARRAY NODES
    // =========================================================================
    registry.register(&arrays::CREATE_ARRAY);
    registry.register(&arrays::CREATE_ARRAY_WITH);
    registry.register(&arrays::CREATE_INT_ARRAY);
    registry.register(&arrays::CREATE_FLOAT_ARRAY);
    registry.register(&arrays::ARRAY_PUSH);
    registry.register(&arrays::ARRAY_POP);
    registry.register(&arrays::ARRAY_INSERT);
    registry.register(&arrays::ARRAY_REMOVE_AT);
    registry.register(&arrays::ARRAY_REMOVE);
    registry.register(&arrays::ARRAY_SET);
    registry.register(&arrays::ARRAY_CLEAR);
    registry.register(&arrays::ARRAY_GET);
    registry.register(&arrays::ARRAY_FIRST);
    registry.register(&arrays::ARRAY_LAST);
    registry.register(&arrays::ARRAY_RANDOM);
    registry.register(&arrays::ARRAY_LENGTH);
    registry.register(&arrays::ARRAY_IS_EMPTY);
    registry.register(&arrays::ARRAY_CONTAINS);
    registry.register(&arrays::ARRAY_FIND);
    registry.register(&arrays::ARRAY_IS_VALID_INDEX);
    registry.register(&arrays::ARRAY_SHUFFLE);
    registry.register(&arrays::ARRAY_REVERSE);
    registry.register(&arrays::ARRAY_SORT);
    registry.register(&arrays::ARRAY_COPY);
    registry.register(&arrays::ARRAY_SLICE);
    registry.register(&arrays::ARRAY_CONCAT);
    registry.register(&arrays::ARRAY_SUM);
    registry.register(&arrays::ARRAY_AVERAGE);
    registry.register(&arrays::ARRAY_MIN);
    registry.register(&arrays::ARRAY_MAX);

    // =========================================================================
    // EASING NODES
    // =========================================================================
    registry.register(&easing::EASE_IN_QUAD);
    registry.register(&easing::EASE_OUT_QUAD);
    registry.register(&easing::EASE_INOUT_QUAD);
    registry.register(&easing::EASE_IN_CUBIC);
    registry.register(&easing::EASE_OUT_CUBIC);
    registry.register(&easing::EASE_INOUT_CUBIC);
    registry.register(&easing::EASE_IN_QUART);
    registry.register(&easing::EASE_OUT_QUART);
    registry.register(&easing::EASE_INOUT_QUART);
    registry.register(&easing::EASE_IN_QUINT);
    registry.register(&easing::EASE_OUT_QUINT);
    registry.register(&easing::EASE_INOUT_QUINT);
    registry.register(&easing::EASE_IN_SINE);
    registry.register(&easing::EASE_OUT_SINE);
    registry.register(&easing::EASE_INOUT_SINE);
    registry.register(&easing::EASE_IN_EXPO);
    registry.register(&easing::EASE_OUT_EXPO);
    registry.register(&easing::EASE_INOUT_EXPO);
    registry.register(&easing::EASE_IN_CIRC);
    registry.register(&easing::EASE_OUT_CIRC);
    registry.register(&easing::EASE_INOUT_CIRC);
    registry.register(&easing::EASE_IN_BACK);
    registry.register(&easing::EASE_OUT_BACK);
    registry.register(&easing::EASE_INOUT_BACK);
    registry.register(&easing::EASE_IN_ELASTIC);
    registry.register(&easing::EASE_OUT_ELASTIC);
    registry.register(&easing::EASE_INOUT_ELASTIC);
    registry.register(&easing::EASE_IN_BOUNCE);
    registry.register(&easing::EASE_OUT_BOUNCE);
    registry.register(&easing::EASE_INOUT_BOUNCE);
    registry.register(&easing::EASE_LINEAR);
    registry.register(&easing::APPLY_EASING);
    registry.register(&easing::INVERSE_LERP);

    // =========================================================================
    // AI / PATHFINDING NODES
    // =========================================================================
    registry.register(&ai::FIND_PATH);
    registry.register(&ai::GET_NEXT_WAYPOINT);
    registry.register(&ai::IS_REACHABLE);
    registry.register(&ai::MOVE_TO);
    registry.register(&ai::MOVE_ALONG_PATH);
    registry.register(&ai::STOP_MOVEMENT);
    registry.register(&ai::LOOK_AT_POSITION);
    registry.register(&ai::LOOK_AT_TARGET);
    registry.register(&ai::IS_FACING);
    registry.register(&ai::DISTANCE_TO_TARGET);
    registry.register(&ai::DISTANCE_TO_POSITION);
    registry.register(&ai::IS_IN_RANGE);
    registry.register(&ai::HAS_LINE_OF_SIGHT);
    registry.register(&ai::FIND_NEAREST);
    registry.register(&ai::FIND_IN_RANGE);
    registry.register(&ai::FLEE_FROM);
    registry.register(&ai::WANDER);
    registry.register(&ai::PATROL);
    registry.register(&ai::SET_AI_STATE);
    registry.register(&ai::GET_AI_STATE);
    registry.register(&ai::IS_AI_STATE);
}
