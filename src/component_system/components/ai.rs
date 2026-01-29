//! AI component definitions
//!
//! Components for AI behavior, pathfinding, and state machines.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};

use egui_phosphor::regular::{
    BRAIN, PATH, EYE, ROBOT, ARROWS_OUT_CARDINAL, COMPASS, DETECTIVE,
};

// ============================================================================
// AI Controller Component - Basic AI behavior
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum AIBehavior {
    #[default]
    Idle,
    Patrol,
    Follow,
    Flee,
    Wander,
    Guard,
    Custom,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum AIState {
    #[default]
    Idle,
    Moving,
    Attacking,
    Chasing,
    Fleeing,
    Searching,
    Dead,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct AIControllerData {
    pub behavior: AIBehavior,
    pub current_state: AIState,
    pub move_speed: f32,
    pub run_speed: f32,
    pub turn_speed: f32,
    pub acceleration: f32,
    pub stop_distance: f32,
    pub enabled: bool,
}

impl Default for AIControllerData {
    fn default() -> Self {
        Self {
            behavior: AIBehavior::Idle,
            current_state: AIState::Idle,
            move_speed: 3.0,
            run_speed: 6.0,
            turn_speed: 5.0,
            acceleration: 8.0,
            stop_distance: 0.5,
            enabled: true,
        }
    }
}

pub static AI_CONTROLLER: ComponentDefinition = ComponentDefinition {
    type_id: "ai_controller",
    display_name: "AI Controller",
    category: ComponentCategory::Gameplay,
    icon: BRAIN,
    priority: 20,
    add_fn: add_ai_controller,
    remove_fn: remove_ai_controller,
    has_fn: has_ai_controller,
    serialize_fn: serialize_ai_controller,
    deserialize_fn: deserialize_ai_controller,
    inspector_fn: inspect_ai_controller,
    conflicts_with: &["character_controller"],
    requires: &[],
};

// ============================================================================
// Patrol Component - Waypoint-based patrol
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum PatrolMode {
    #[default]
    Loop,
    PingPong,
    Random,
    Once,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct PatrolData {
    pub patrol_mode: PatrolMode,
    pub waypoint_ids: Vec<String>,
    pub wait_time_min: f32,
    pub wait_time_max: f32,
    pub patrol_speed: f32,
    pub start_on_spawn: bool,
    pub current_waypoint: usize,
    pub reverse: bool,
}

impl Default for PatrolData {
    fn default() -> Self {
        Self {
            patrol_mode: PatrolMode::Loop,
            waypoint_ids: Vec::new(),
            wait_time_min: 1.0,
            wait_time_max: 3.0,
            patrol_speed: 2.0,
            start_on_spawn: true,
            current_waypoint: 0,
            reverse: false,
        }
    }
}

pub static PATROL: ComponentDefinition = ComponentDefinition {
    type_id: "patrol",
    display_name: "Patrol",
    category: ComponentCategory::Gameplay,
    icon: PATH,
    priority: 21,
    add_fn: add_patrol,
    remove_fn: remove_patrol,
    has_fn: has_patrol,
    serialize_fn: serialize_patrol,
    deserialize_fn: deserialize_patrol,
    inspector_fn: inspect_patrol,
    conflicts_with: &[],
    requires: &["ai_controller"],
};

// ============================================================================
// Vision Component - Line of sight detection
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct VisionData {
    pub view_distance: f32,
    pub view_angle: f32,
    pub height_offset: f32,
    pub detection_layers: u32,
    pub target_tags: String,
    pub requires_line_of_sight: bool,
    pub remember_time: f32,
    pub peripheral_distance: f32,
}

impl Default for VisionData {
    fn default() -> Self {
        Self {
            view_distance: 15.0,
            view_angle: 120.0,
            height_offset: 1.6,
            detection_layers: 0xFFFFFFFF,
            target_tags: "Player".to_string(),
            requires_line_of_sight: true,
            remember_time: 5.0,
            peripheral_distance: 3.0,
        }
    }
}

pub static VISION: ComponentDefinition = ComponentDefinition {
    type_id: "vision",
    display_name: "Vision",
    category: ComponentCategory::Gameplay,
    icon: EYE,
    priority: 22,
    add_fn: add_vision,
    remove_fn: remove_vision,
    has_fn: has_vision,
    serialize_fn: serialize_vision,
    deserialize_fn: deserialize_vision,
    inspector_fn: inspect_vision,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Follow Target Component - Follow/chase behavior
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum FollowMode {
    #[default]
    Direct,
    Orbit,
    Formation,
    Flank,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct FollowTargetData {
    pub follow_mode: FollowMode,
    pub target_tag: String,
    pub target_entity_name: String,
    pub follow_distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub orbit_radius: f32,
    pub orbit_speed: f32,
    pub formation_offset: [f32; 3],
    pub predict_movement: bool,
    pub lose_target_distance: f32,
}

impl Default for FollowTargetData {
    fn default() -> Self {
        Self {
            follow_mode: FollowMode::Direct,
            target_tag: "Player".to_string(),
            target_entity_name: String::new(),
            follow_distance: 2.0,
            min_distance: 1.5,
            max_distance: 20.0,
            orbit_radius: 3.0,
            orbit_speed: 1.0,
            formation_offset: [0.0, 0.0, -2.0],
            predict_movement: false,
            lose_target_distance: 30.0,
        }
    }
}

pub static FOLLOW_TARGET: ComponentDefinition = ComponentDefinition {
    type_id: "follow_target",
    display_name: "Follow Target",
    category: ComponentCategory::Gameplay,
    icon: COMPASS,
    priority: 23,
    add_fn: add_follow_target,
    remove_fn: remove_follow_target,
    has_fn: has_follow_target,
    serialize_fn: serialize_follow_target,
    deserialize_fn: deserialize_follow_target,
    inspector_fn: inspect_follow_target,
    conflicts_with: &[],
    requires: &["ai_controller"],
};

// ============================================================================
// Flee Component - Run away behavior
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct FleeData {
    pub flee_from_tags: String,
    pub trigger_distance: f32,
    pub safe_distance: f32,
    pub flee_speed_multiplier: f32,
    pub panic_mode: bool,
    pub hide_when_safe: bool,
}

impl Default for FleeData {
    fn default() -> Self {
        Self {
            flee_from_tags: "Player".to_string(),
            trigger_distance: 5.0,
            safe_distance: 15.0,
            flee_speed_multiplier: 1.5,
            panic_mode: false,
            hide_when_safe: false,
        }
    }
}

pub static FLEE: ComponentDefinition = ComponentDefinition {
    type_id: "flee",
    display_name: "Flee",
    category: ComponentCategory::Gameplay,
    icon: ARROWS_OUT_CARDINAL,
    priority: 24,
    add_fn: add_flee,
    remove_fn: remove_flee,
    has_fn: has_flee,
    serialize_fn: serialize_flee,
    deserialize_fn: deserialize_flee,
    inspector_fn: inspect_flee,
    conflicts_with: &[],
    requires: &["ai_controller"],
};

// ============================================================================
// State Machine Component - Custom AI states
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct StateMachineData {
    pub initial_state: String,
    pub current_state: String,
    pub states: Vec<String>,
    pub debug_mode: bool,
}

impl Default for StateMachineData {
    fn default() -> Self {
        Self {
            initial_state: "idle".to_string(),
            current_state: "idle".to_string(),
            states: vec!["idle".to_string(), "active".to_string()],
            debug_mode: false,
        }
    }
}

pub static STATE_MACHINE: ComponentDefinition = ComponentDefinition {
    type_id: "state_machine",
    display_name: "State Machine",
    category: ComponentCategory::Gameplay,
    icon: ROBOT,
    priority: 25,
    add_fn: add_state_machine,
    remove_fn: remove_state_machine,
    has_fn: has_state_machine,
    serialize_fn: serialize_state_machine,
    deserialize_fn: deserialize_state_machine,
    inspector_fn: inspect_state_machine,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Awareness Component - Hearing and sensing
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct AwarenessData {
    pub hearing_range: f32,
    pub hearing_threshold: f32,
    pub alert_duration: f32,
    pub investigate_sounds: bool,
    pub alert_allies: bool,
    pub alert_radius: f32,
}

impl Default for AwarenessData {
    fn default() -> Self {
        Self {
            hearing_range: 20.0,
            hearing_threshold: 0.3,
            alert_duration: 10.0,
            investigate_sounds: true,
            alert_allies: true,
            alert_radius: 15.0,
        }
    }
}

pub static AWARENESS: ComponentDefinition = ComponentDefinition {
    type_id: "awareness",
    display_name: "Awareness",
    category: ComponentCategory::Gameplay,
    icon: DETECTIVE,
    priority: 26,
    add_fn: add_awareness,
    remove_fn: remove_awareness,
    has_fn: has_awareness,
    serialize_fn: serialize_awareness,
    deserialize_fn: deserialize_awareness,
    inspector_fn: inspect_awareness,
    conflicts_with: &[],
    requires: &[],
};

/// Register all AI components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&AI_CONTROLLER);
    registry.register(&PATROL);
    registry.register(&VISION);
    registry.register(&FOLLOW_TARGET);
    registry.register(&FLEE);
    registry.register(&STATE_MACHINE);
    registry.register(&AWARENESS);
}

// ============================================================================
// AI Controller Implementation
// ============================================================================

fn add_ai_controller(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(AIControllerData::default());
}

fn remove_ai_controller(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AIControllerData>();
}

fn has_ai_controller(world: &World, entity: Entity) -> bool {
    world.get::<AIControllerData>(entity).is_some()
}

fn serialize_ai_controller(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<AIControllerData>(entity)?;
    Some(json!({
        "behavior": data.behavior,
        "current_state": data.current_state,
        "move_speed": data.move_speed,
        "run_speed": data.run_speed,
        "turn_speed": data.turn_speed,
        "acceleration": data.acceleration,
        "stop_distance": data.stop_distance,
        "enabled": data.enabled,
    }))
}

fn deserialize_ai_controller(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let ai_data = AIControllerData {
        behavior: data.get("behavior").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        current_state: data.get("current_state").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        move_speed: data.get("move_speed").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
        run_speed: data.get("run_speed").and_then(|v| v.as_f64()).unwrap_or(6.0) as f32,
        turn_speed: data.get("turn_speed").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        acceleration: data.get("acceleration").and_then(|v| v.as_f64()).unwrap_or(8.0) as f32,
        stop_distance: data.get("stop_distance").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
        enabled: data.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
    };
    entity_commands.insert(ai_data);
}

fn inspect_ai_controller(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<AIControllerData>(entity) {
        if ui.checkbox(&mut data.enabled, "Enabled").changed() { changed = true; }

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Behavior:");
            egui::ComboBox::from_id_salt("ai_behavior")
                .selected_text(match data.behavior {
                    AIBehavior::Idle => "Idle",
                    AIBehavior::Patrol => "Patrol",
                    AIBehavior::Follow => "Follow",
                    AIBehavior::Flee => "Flee",
                    AIBehavior::Wander => "Wander",
                    AIBehavior::Guard => "Guard",
                    AIBehavior::Custom => "Custom",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.behavior == AIBehavior::Idle, "Idle").clicked() { data.behavior = AIBehavior::Idle; changed = true; }
                    if ui.selectable_label(data.behavior == AIBehavior::Patrol, "Patrol").clicked() { data.behavior = AIBehavior::Patrol; changed = true; }
                    if ui.selectable_label(data.behavior == AIBehavior::Follow, "Follow").clicked() { data.behavior = AIBehavior::Follow; changed = true; }
                    if ui.selectable_label(data.behavior == AIBehavior::Flee, "Flee").clicked() { data.behavior = AIBehavior::Flee; changed = true; }
                    if ui.selectable_label(data.behavior == AIBehavior::Wander, "Wander").clicked() { data.behavior = AIBehavior::Wander; changed = true; }
                    if ui.selectable_label(data.behavior == AIBehavior::Guard, "Guard").clicked() { data.behavior = AIBehavior::Guard; changed = true; }
                    if ui.selectable_label(data.behavior == AIBehavior::Custom, "Custom").clicked() { data.behavior = AIBehavior::Custom; changed = true; }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Current State:");
            ui.label(match data.current_state {
                AIState::Idle => "Idle",
                AIState::Moving => "Moving",
                AIState::Attacking => "Attacking",
                AIState::Chasing => "Chasing",
                AIState::Fleeing => "Fleeing",
                AIState::Searching => "Searching",
                AIState::Dead => "Dead",
            });
        });

        ui.separator();
        ui.label("Movement");

        ui.horizontal(|ui| {
            ui.label("Walk Speed:");
            if ui.add(egui::DragValue::new(&mut data.move_speed).speed(0.1).range(0.1..=20.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Run Speed:");
            if ui.add(egui::DragValue::new(&mut data.run_speed).speed(0.1).range(0.1..=30.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Turn Speed:");
            if ui.add(egui::DragValue::new(&mut data.turn_speed).speed(0.1).range(0.1..=20.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Acceleration:");
            if ui.add(egui::DragValue::new(&mut data.acceleration).speed(0.1).range(0.1..=50.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Stop Distance:");
            if ui.add(egui::DragValue::new(&mut data.stop_distance).speed(0.05).range(0.1..=5.0)).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Patrol Implementation
// ============================================================================

fn add_patrol(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(PatrolData::default());
}

fn remove_patrol(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<PatrolData>();
}

fn has_patrol(world: &World, entity: Entity) -> bool {
    world.get::<PatrolData>(entity).is_some()
}

fn serialize_patrol(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<PatrolData>(entity)?;
    Some(json!({
        "patrol_mode": data.patrol_mode,
        "waypoint_ids": data.waypoint_ids,
        "wait_time_min": data.wait_time_min,
        "wait_time_max": data.wait_time_max,
        "patrol_speed": data.patrol_speed,
        "start_on_spawn": data.start_on_spawn,
    }))
}

fn deserialize_patrol(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let patrol_data = PatrolData {
        patrol_mode: data.get("patrol_mode").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        waypoint_ids: data.get("waypoint_ids").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        wait_time_min: data.get("wait_time_min").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        wait_time_max: data.get("wait_time_max").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
        patrol_speed: data.get("patrol_speed").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        start_on_spawn: data.get("start_on_spawn").and_then(|v| v.as_bool()).unwrap_or(true),
        current_waypoint: 0,
        reverse: false,
    };
    entity_commands.insert(patrol_data);
}

fn inspect_patrol(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<PatrolData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            egui::ComboBox::from_id_salt("patrol_mode")
                .selected_text(match data.patrol_mode {
                    PatrolMode::Loop => "Loop",
                    PatrolMode::PingPong => "Ping-Pong",
                    PatrolMode::Random => "Random",
                    PatrolMode::Once => "Once",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.patrol_mode == PatrolMode::Loop, "Loop").clicked() { data.patrol_mode = PatrolMode::Loop; changed = true; }
                    if ui.selectable_label(data.patrol_mode == PatrolMode::PingPong, "Ping-Pong").clicked() { data.patrol_mode = PatrolMode::PingPong; changed = true; }
                    if ui.selectable_label(data.patrol_mode == PatrolMode::Random, "Random").clicked() { data.patrol_mode = PatrolMode::Random; changed = true; }
                    if ui.selectable_label(data.patrol_mode == PatrolMode::Once, "Once").clicked() { data.patrol_mode = PatrolMode::Once; changed = true; }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Patrol Speed:");
            if ui.add(egui::DragValue::new(&mut data.patrol_speed).speed(0.1).range(0.1..=10.0)).changed() { changed = true; }
        });

        ui.separator();
        ui.label("Wait Time");

        ui.horizontal(|ui| {
            ui.label("Min:");
            if ui.add(egui::DragValue::new(&mut data.wait_time_min).speed(0.1).range(0.0..=60.0).suffix("s")).changed() { changed = true; }
            ui.label("Max:");
            if ui.add(egui::DragValue::new(&mut data.wait_time_max).speed(0.1).range(0.0..=60.0).suffix("s")).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.start_on_spawn, "Start on Spawn").changed() { changed = true; }

        ui.separator();
        ui.label(format!("Waypoints: {} (edit via Waypoint IDs)", data.waypoint_ids.len()));
    }
    changed
}

// ============================================================================
// Vision Implementation
// ============================================================================

fn add_vision(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(VisionData::default());
}

fn remove_vision(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VisionData>();
}

fn has_vision(world: &World, entity: Entity) -> bool {
    world.get::<VisionData>(entity).is_some()
}

fn serialize_vision(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<VisionData>(entity)?;
    Some(json!({
        "view_distance": data.view_distance,
        "view_angle": data.view_angle,
        "height_offset": data.height_offset,
        "detection_layers": data.detection_layers,
        "target_tags": data.target_tags,
        "requires_line_of_sight": data.requires_line_of_sight,
        "remember_time": data.remember_time,
        "peripheral_distance": data.peripheral_distance,
    }))
}

fn deserialize_vision(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let vision_data = VisionData {
        view_distance: data.get("view_distance").and_then(|v| v.as_f64()).unwrap_or(15.0) as f32,
        view_angle: data.get("view_angle").and_then(|v| v.as_f64()).unwrap_or(120.0) as f32,
        height_offset: data.get("height_offset").and_then(|v| v.as_f64()).unwrap_or(1.6) as f32,
        detection_layers: data.get("detection_layers").and_then(|v| v.as_u64()).unwrap_or(0xFFFFFFFF) as u32,
        target_tags: data.get("target_tags").and_then(|v| v.as_str()).unwrap_or("Player").to_string(),
        requires_line_of_sight: data.get("requires_line_of_sight").and_then(|v| v.as_bool()).unwrap_or(true),
        remember_time: data.get("remember_time").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        peripheral_distance: data.get("peripheral_distance").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
    };
    entity_commands.insert(vision_data);
}

fn inspect_vision(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<VisionData>(entity) {
        ui.horizontal(|ui| {
            ui.label("View Distance:");
            if ui.add(egui::DragValue::new(&mut data.view_distance).speed(0.5).range(1.0..=100.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("View Angle:");
            if ui.add(egui::DragValue::new(&mut data.view_angle).speed(1.0).range(10.0..=360.0).suffix("°")).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Eye Height:");
            if ui.add(egui::DragValue::new(&mut data.height_offset).speed(0.1).range(0.0..=5.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Peripheral:");
            if ui.add(egui::DragValue::new(&mut data.peripheral_distance).speed(0.1).range(0.0..=20.0)).changed() { changed = true; }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Target Tags:");
            if ui.text_edit_singleline(&mut data.target_tags).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.requires_line_of_sight, "Requires Line of Sight").changed() { changed = true; }

        ui.horizontal(|ui| {
            ui.label("Remember Time:");
            if ui.add(egui::DragValue::new(&mut data.remember_time).speed(0.1).range(0.0..=60.0).suffix("s")).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Follow Target Implementation
// ============================================================================

fn add_follow_target(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(FollowTargetData::default());
}

fn remove_follow_target(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<FollowTargetData>();
}

fn has_follow_target(world: &World, entity: Entity) -> bool {
    world.get::<FollowTargetData>(entity).is_some()
}

fn serialize_follow_target(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<FollowTargetData>(entity)?;
    Some(json!({
        "follow_mode": data.follow_mode,
        "target_tag": data.target_tag,
        "target_entity_name": data.target_entity_name,
        "follow_distance": data.follow_distance,
        "min_distance": data.min_distance,
        "max_distance": data.max_distance,
        "orbit_radius": data.orbit_radius,
        "orbit_speed": data.orbit_speed,
        "formation_offset": data.formation_offset,
        "predict_movement": data.predict_movement,
        "lose_target_distance": data.lose_target_distance,
    }))
}

fn deserialize_follow_target(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let follow_data = FollowTargetData {
        follow_mode: data.get("follow_mode").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        target_tag: data.get("target_tag").and_then(|v| v.as_str()).unwrap_or("Player").to_string(),
        target_entity_name: data.get("target_entity_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        follow_distance: data.get("follow_distance").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        min_distance: data.get("min_distance").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32,
        max_distance: data.get("max_distance").and_then(|v| v.as_f64()).unwrap_or(20.0) as f32,
        orbit_radius: data.get("orbit_radius").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
        orbit_speed: data.get("orbit_speed").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        formation_offset: data.get("formation_offset").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 0.0, -2.0]),
        predict_movement: data.get("predict_movement").and_then(|v| v.as_bool()).unwrap_or(false),
        lose_target_distance: data.get("lose_target_distance").and_then(|v| v.as_f64()).unwrap_or(30.0) as f32,
    };
    entity_commands.insert(follow_data);
}

fn inspect_follow_target(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<FollowTargetData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            egui::ComboBox::from_id_salt("follow_mode")
                .selected_text(match data.follow_mode {
                    FollowMode::Direct => "Direct",
                    FollowMode::Orbit => "Orbit",
                    FollowMode::Formation => "Formation",
                    FollowMode::Flank => "Flank",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.follow_mode == FollowMode::Direct, "Direct").clicked() { data.follow_mode = FollowMode::Direct; changed = true; }
                    if ui.selectable_label(data.follow_mode == FollowMode::Orbit, "Orbit").clicked() { data.follow_mode = FollowMode::Orbit; changed = true; }
                    if ui.selectable_label(data.follow_mode == FollowMode::Formation, "Formation").clicked() { data.follow_mode = FollowMode::Formation; changed = true; }
                    if ui.selectable_label(data.follow_mode == FollowMode::Flank, "Flank").clicked() { data.follow_mode = FollowMode::Flank; changed = true; }
                });
        });

        ui.separator();
        ui.label("Target");

        ui.horizontal(|ui| {
            ui.label("Tag:");
            if ui.text_edit_singleline(&mut data.target_tag).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Or Name:");
            if ui.text_edit_singleline(&mut data.target_entity_name).changed() { changed = true; }
        });

        ui.separator();
        ui.label("Distance");

        ui.horizontal(|ui| {
            ui.label("Follow:");
            if ui.add(egui::DragValue::new(&mut data.follow_distance).speed(0.1).range(0.5..=20.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Min:");
            if ui.add(egui::DragValue::new(&mut data.min_distance).speed(0.1).range(0.0..=10.0)).changed() { changed = true; }
            ui.label("Max:");
            if ui.add(egui::DragValue::new(&mut data.max_distance).speed(0.5).range(5.0..=100.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Lose Target:");
            if ui.add(egui::DragValue::new(&mut data.lose_target_distance).speed(0.5).range(10.0..=200.0)).changed() { changed = true; }
        });

        match data.follow_mode {
            FollowMode::Orbit => {
                ui.separator();
                ui.label("Orbit");
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(&mut data.orbit_radius).speed(0.1).range(1.0..=20.0)).changed() { changed = true; }
                });
                ui.horizontal(|ui| {
                    ui.label("Speed:");
                    if ui.add(egui::DragValue::new(&mut data.orbit_speed).speed(0.1).range(0.1..=5.0)).changed() { changed = true; }
                });
            }
            FollowMode::Formation => {
                ui.separator();
                ui.label("Formation Offset");
                ui.horizontal(|ui| {
                    if ui.add(egui::DragValue::new(&mut data.formation_offset[0]).speed(0.1).prefix("X: ")).changed() { changed = true; }
                    if ui.add(egui::DragValue::new(&mut data.formation_offset[1]).speed(0.1).prefix("Y: ")).changed() { changed = true; }
                    if ui.add(egui::DragValue::new(&mut data.formation_offset[2]).speed(0.1).prefix("Z: ")).changed() { changed = true; }
                });
            }
            _ => {}
        }

        ui.separator();
        if ui.checkbox(&mut data.predict_movement, "Predict Movement").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Flee Implementation
// ============================================================================

fn add_flee(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(FleeData::default());
}

fn remove_flee(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<FleeData>();
}

fn has_flee(world: &World, entity: Entity) -> bool {
    world.get::<FleeData>(entity).is_some()
}

fn serialize_flee(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<FleeData>(entity)?;
    Some(json!({
        "flee_from_tags": data.flee_from_tags,
        "trigger_distance": data.trigger_distance,
        "safe_distance": data.safe_distance,
        "flee_speed_multiplier": data.flee_speed_multiplier,
        "panic_mode": data.panic_mode,
        "hide_when_safe": data.hide_when_safe,
    }))
}

fn deserialize_flee(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let flee_data = FleeData {
        flee_from_tags: data.get("flee_from_tags").and_then(|v| v.as_str()).unwrap_or("Player").to_string(),
        trigger_distance: data.get("trigger_distance").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        safe_distance: data.get("safe_distance").and_then(|v| v.as_f64()).unwrap_or(15.0) as f32,
        flee_speed_multiplier: data.get("flee_speed_multiplier").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32,
        panic_mode: data.get("panic_mode").and_then(|v| v.as_bool()).unwrap_or(false),
        hide_when_safe: data.get("hide_when_safe").and_then(|v| v.as_bool()).unwrap_or(false),
    };
    entity_commands.insert(flee_data);
}

fn inspect_flee(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<FleeData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Flee From Tags:");
            if ui.text_edit_singleline(&mut data.flee_from_tags).changed() { changed = true; }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Trigger Distance:");
            if ui.add(egui::DragValue::new(&mut data.trigger_distance).speed(0.1).range(1.0..=50.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Safe Distance:");
            if ui.add(egui::DragValue::new(&mut data.safe_distance).speed(0.1).range(5.0..=100.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Speed Multiplier:");
            if ui.add(egui::DragValue::new(&mut data.flee_speed_multiplier).speed(0.1).range(1.0..=3.0)).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.panic_mode, "Panic Mode (erratic)").changed() { changed = true; }
        if ui.checkbox(&mut data.hide_when_safe, "Hide When Safe").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// State Machine Implementation
// ============================================================================

fn add_state_machine(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(StateMachineData::default());
}

fn remove_state_machine(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<StateMachineData>();
}

fn has_state_machine(world: &World, entity: Entity) -> bool {
    world.get::<StateMachineData>(entity).is_some()
}

fn serialize_state_machine(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<StateMachineData>(entity)?;
    Some(json!({
        "initial_state": data.initial_state,
        "states": data.states,
        "debug_mode": data.debug_mode,
    }))
}

fn deserialize_state_machine(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let sm_data = StateMachineData {
        initial_state: data.get("initial_state").and_then(|v| v.as_str()).unwrap_or("idle").to_string(),
        current_state: data.get("initial_state").and_then(|v| v.as_str()).unwrap_or("idle").to_string(),
        states: data.get("states").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_else(|| vec!["idle".to_string(), "active".to_string()]),
        debug_mode: data.get("debug_mode").and_then(|v| v.as_bool()).unwrap_or(false),
    };
    entity_commands.insert(sm_data);
}

fn inspect_state_machine(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<StateMachineData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Initial State:");
            if ui.text_edit_singleline(&mut data.initial_state).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Current:");
            ui.label(&data.current_state);
        });

        ui.separator();
        ui.label(format!("States: {}", data.states.join(", ")));

        ui.separator();
        if ui.checkbox(&mut data.debug_mode, "Debug Mode").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Awareness Implementation
// ============================================================================

fn add_awareness(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(AwarenessData::default());
}

fn remove_awareness(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AwarenessData>();
}

fn has_awareness(world: &World, entity: Entity) -> bool {
    world.get::<AwarenessData>(entity).is_some()
}

fn serialize_awareness(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<AwarenessData>(entity)?;
    Some(json!({
        "hearing_range": data.hearing_range,
        "hearing_threshold": data.hearing_threshold,
        "alert_duration": data.alert_duration,
        "investigate_sounds": data.investigate_sounds,
        "alert_allies": data.alert_allies,
        "alert_radius": data.alert_radius,
    }))
}

fn deserialize_awareness(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let awareness_data = AwarenessData {
        hearing_range: data.get("hearing_range").and_then(|v| v.as_f64()).unwrap_or(20.0) as f32,
        hearing_threshold: data.get("hearing_threshold").and_then(|v| v.as_f64()).unwrap_or(0.3) as f32,
        alert_duration: data.get("alert_duration").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        investigate_sounds: data.get("investigate_sounds").and_then(|v| v.as_bool()).unwrap_or(true),
        alert_allies: data.get("alert_allies").and_then(|v| v.as_bool()).unwrap_or(true),
        alert_radius: data.get("alert_radius").and_then(|v| v.as_f64()).unwrap_or(15.0) as f32,
    };
    entity_commands.insert(awareness_data);
}

fn inspect_awareness(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<AwarenessData>(entity) {
        ui.label("Hearing");

        ui.horizontal(|ui| {
            ui.label("Range:");
            if ui.add(egui::DragValue::new(&mut data.hearing_range).speed(0.5).range(1.0..=100.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Threshold:");
            if ui.add(egui::Slider::new(&mut data.hearing_threshold, 0.0..=1.0)).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.investigate_sounds, "Investigate Sounds").changed() { changed = true; }

        ui.separator();
        ui.label("Alertness");

        ui.horizontal(|ui| {
            ui.label("Alert Duration:");
            if ui.add(egui::DragValue::new(&mut data.alert_duration).speed(0.5).range(1.0..=60.0).suffix("s")).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.alert_allies, "Alert Allies").changed() { changed = true; }

        if data.alert_allies {
            ui.horizontal(|ui| {
                ui.label("Alert Radius:");
                if ui.add(egui::DragValue::new(&mut data.alert_radius).speed(0.5).range(1.0..=50.0)).changed() { changed = true; }
            });
        }
    }
    changed
}
