//! Gameplay component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};

use egui_phosphor::regular::{
    HEART, PERSON_SIMPLE_RUN, FLAG_BANNER, TIMER, SHAPES, CROSSHAIR, MAP_PIN,
};

// ============================================================================
// Health Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct HealthData {
    pub max_health: f32,
    pub current_health: f32,
    pub regeneration_rate: f32,
    pub invincible: bool,
    pub destroy_on_death: bool,
}

impl Default for HealthData {
    fn default() -> Self {
        Self {
            max_health: 100.0,
            current_health: 100.0,
            regeneration_rate: 0.0,
            invincible: false,
            destroy_on_death: true,
        }
    }
}

pub static HEALTH: ComponentDefinition = ComponentDefinition {
    type_id: "health",
    display_name: "Health",
    category: ComponentCategory::Gameplay,
    icon: HEART,
    priority: 0,
    add_fn: add_health,
    remove_fn: remove_health,
    has_fn: has_health,
    serialize_fn: serialize_health,
    deserialize_fn: deserialize_health,
    inspector_fn: inspect_health,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Character Controller Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct CharacterControllerData {
    pub move_speed: f32,
    pub sprint_multiplier: f32,
    pub jump_force: f32,
    pub gravity_scale: f32,
    pub ground_friction: f32,
    pub air_control: f32,
    pub height: f32,
    pub radius: f32,
    pub step_height: f32,
    pub slope_limit: f32,
}

impl Default for CharacterControllerData {
    fn default() -> Self {
        Self {
            move_speed: 5.0,
            sprint_multiplier: 1.5,
            jump_force: 8.0,
            gravity_scale: 1.0,
            ground_friction: 6.0,
            air_control: 0.3,
            height: 1.8,
            radius: 0.3,
            step_height: 0.3,
            slope_limit: 45.0,
        }
    }
}

pub static CHARACTER_CONTROLLER: ComponentDefinition = ComponentDefinition {
    type_id: "character_controller",
    display_name: "Character Controller",
    category: ComponentCategory::Gameplay,
    icon: PERSON_SIMPLE_RUN,
    priority: 1,
    add_fn: add_character_controller,
    remove_fn: remove_character_controller,
    has_fn: has_character_controller,
    serialize_fn: serialize_character_controller,
    deserialize_fn: deserialize_character_controller,
    inspector_fn: inspect_character_controller,
    conflicts_with: &["rigid_body"],
    requires: &[],
};

// ============================================================================
// Trigger Zone Component
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum TriggerShape {
    #[default]
    Box,
    Sphere,
    Capsule,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct TriggerZoneData {
    pub shape: TriggerShape,
    pub size: [f32; 3],
    pub radius: f32,
    pub one_shot: bool,
    pub trigger_layers: u32,
    pub tag_filter: String,
}

impl Default for TriggerZoneData {
    fn default() -> Self {
        Self {
            shape: TriggerShape::Box,
            size: [2.0, 2.0, 2.0],
            radius: 1.0,
            one_shot: false,
            trigger_layers: 0xFFFFFFFF,
            tag_filter: String::new(),
        }
    }
}

pub static TRIGGER_ZONE: ComponentDefinition = ComponentDefinition {
    type_id: "trigger_zone",
    display_name: "Trigger Zone",
    category: ComponentCategory::Gameplay,
    icon: FLAG_BANNER,
    priority: 2,
    add_fn: add_trigger_zone,
    remove_fn: remove_trigger_zone,
    has_fn: has_trigger_zone,
    serialize_fn: serialize_trigger_zone,
    deserialize_fn: deserialize_trigger_zone,
    inspector_fn: inspect_trigger_zone,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Spawner Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct SpawnerData {
    pub prefab_path: String,
    pub spawn_interval: f32,
    pub max_spawned: u32,
    pub spawn_on_start: bool,
    pub spawn_count: u32,
    pub random_offset: [f32; 3],
    pub inherit_rotation: bool,
}

impl Default for SpawnerData {
    fn default() -> Self {
        Self {
            prefab_path: String::new(),
            spawn_interval: 1.0,
            max_spawned: 10,
            spawn_on_start: false,
            spawn_count: 1,
            random_offset: [0.0, 0.0, 0.0],
            inherit_rotation: true,
        }
    }
}

pub static SPAWNER: ComponentDefinition = ComponentDefinition {
    type_id: "spawner",
    display_name: "Spawner",
    category: ComponentCategory::Gameplay,
    icon: SHAPES,
    priority: 3,
    add_fn: add_spawner,
    remove_fn: remove_spawner,
    has_fn: has_spawner,
    serialize_fn: serialize_spawner,
    deserialize_fn: deserialize_spawner,
    inspector_fn: inspect_spawner,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Timer Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct TimerData {
    pub duration: f32,
    pub elapsed: f32,
    pub looping: bool,
    pub auto_start: bool,
    pub paused: bool,
}

impl Default for TimerData {
    fn default() -> Self {
        Self {
            duration: 1.0,
            elapsed: 0.0,
            looping: false,
            auto_start: false,
            paused: true,
        }
    }
}

pub static TIMER_COMPONENT: ComponentDefinition = ComponentDefinition {
    type_id: "timer",
    display_name: "Timer",
    category: ComponentCategory::Gameplay,
    icon: TIMER,
    priority: 4,
    add_fn: add_timer,
    remove_fn: remove_timer,
    has_fn: has_timer,
    serialize_fn: serialize_timer,
    deserialize_fn: deserialize_timer,
    inspector_fn: inspect_timer,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Target Component - For AI/gameplay targeting
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct TargetData {
    pub team: u32,
    pub priority: i32,
    pub targetable: bool,
    pub auto_target_range: f32,
}

impl Default for TargetData {
    fn default() -> Self {
        Self {
            team: 0,
            priority: 0,
            targetable: true,
            auto_target_range: 0.0,
        }
    }
}

pub static TARGET_COMPONENT: ComponentDefinition = ComponentDefinition {
    type_id: "target",
    display_name: "Target",
    category: ComponentCategory::Gameplay,
    icon: CROSSHAIR,
    priority: 5,
    add_fn: add_target,
    remove_fn: remove_target,
    has_fn: has_target,
    serialize_fn: serialize_target,
    deserialize_fn: deserialize_target,
    inspector_fn: inspect_target,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Waypoint Component - For navigation/paths
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct WaypointData {
    pub waypoint_id: String,
    pub connected_waypoints: Vec<String>,
    pub wait_time: f32,
    pub radius: f32,
}

impl Default for WaypointData {
    fn default() -> Self {
        Self {
            waypoint_id: String::new(),
            connected_waypoints: Vec::new(),
            wait_time: 0.0,
            radius: 0.5,
        }
    }
}

pub static WAYPOINT: ComponentDefinition = ComponentDefinition {
    type_id: "waypoint",
    display_name: "Waypoint",
    category: ComponentCategory::Gameplay,
    icon: MAP_PIN,
    priority: 6,
    add_fn: add_waypoint,
    remove_fn: remove_waypoint,
    has_fn: has_waypoint,
    serialize_fn: serialize_waypoint,
    deserialize_fn: deserialize_waypoint,
    inspector_fn: inspect_waypoint,
    conflicts_with: &[],
    requires: &[],
};

/// Register all gameplay components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&HEALTH);
    registry.register(&CHARACTER_CONTROLLER);
    registry.register(&TRIGGER_ZONE);
    registry.register(&SPAWNER);
    registry.register(&TIMER_COMPONENT);
    registry.register(&TARGET_COMPONENT);
    registry.register(&WAYPOINT);
}

// ============================================================================
// Health Implementation
// ============================================================================

fn add_health(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(HealthData::default());
}

fn remove_health(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<HealthData>();
}

fn has_health(world: &World, entity: Entity) -> bool {
    world.get::<HealthData>(entity).is_some()
}

fn serialize_health(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<HealthData>(entity)?;
    Some(json!({
        "max_health": data.max_health,
        "current_health": data.current_health,
        "regeneration_rate": data.regeneration_rate,
        "invincible": data.invincible,
        "destroy_on_death": data.destroy_on_death,
    }))
}

fn deserialize_health(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let health_data = HealthData {
        max_health: data.get("max_health").and_then(|v| v.as_f64()).unwrap_or(100.0) as f32,
        current_health: data.get("current_health").and_then(|v| v.as_f64()).unwrap_or(100.0) as f32,
        regeneration_rate: data.get("regeneration_rate").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        invincible: data.get("invincible").and_then(|v| v.as_bool()).unwrap_or(false),
        destroy_on_death: data.get("destroy_on_death").and_then(|v| v.as_bool()).unwrap_or(true),
    };
    entity_commands.insert(health_data);
}

fn inspect_health(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<HealthData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Max Health:");
            if ui.add(egui::DragValue::new(&mut data.max_health).speed(1.0).range(1.0..=10000.0)).changed() {
                changed = true;
            }
        });

        let max_health = data.max_health;
        ui.horizontal(|ui| {
            ui.label("Current:");
            if ui.add(egui::DragValue::new(&mut data.current_health).speed(1.0).range(0.0..=max_health)).changed() {
                changed = true;
            }
        });

        // Health bar
        let health_pct = data.current_health / data.max_health;
        let bar_color = if health_pct > 0.5 {
            egui::Color32::from_rgb(100, 200, 100)
        } else if health_pct > 0.25 {
            egui::Color32::from_rgb(200, 200, 100)
        } else {
            egui::Color32::from_rgb(200, 100, 100)
        };
        ui.add(egui::ProgressBar::new(health_pct).fill(bar_color));

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Regen/sec:");
            if ui.add(egui::DragValue::new(&mut data.regeneration_rate).speed(0.1).range(0.0..=100.0)).changed() {
                changed = true;
            }
        });

        if ui.checkbox(&mut data.invincible, "Invincible").changed() { changed = true; }
        if ui.checkbox(&mut data.destroy_on_death, "Destroy on Death").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Character Controller Implementation
// ============================================================================

fn add_character_controller(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(CharacterControllerData::default());
}

fn remove_character_controller(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<CharacterControllerData>();
}

fn has_character_controller(world: &World, entity: Entity) -> bool {
    world.get::<CharacterControllerData>(entity).is_some()
}

fn serialize_character_controller(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<CharacterControllerData>(entity)?;
    Some(json!({
        "move_speed": data.move_speed,
        "sprint_multiplier": data.sprint_multiplier,
        "jump_force": data.jump_force,
        "gravity_scale": data.gravity_scale,
        "ground_friction": data.ground_friction,
        "air_control": data.air_control,
        "height": data.height,
        "radius": data.radius,
        "step_height": data.step_height,
        "slope_limit": data.slope_limit,
    }))
}

fn deserialize_character_controller(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let cc_data = CharacterControllerData {
        move_speed: data.get("move_speed").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        sprint_multiplier: data.get("sprint_multiplier").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32,
        jump_force: data.get("jump_force").and_then(|v| v.as_f64()).unwrap_or(8.0) as f32,
        gravity_scale: data.get("gravity_scale").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        ground_friction: data.get("ground_friction").and_then(|v| v.as_f64()).unwrap_or(6.0) as f32,
        air_control: data.get("air_control").and_then(|v| v.as_f64()).unwrap_or(0.3) as f32,
        height: data.get("height").and_then(|v| v.as_f64()).unwrap_or(1.8) as f32,
        radius: data.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.3) as f32,
        step_height: data.get("step_height").and_then(|v| v.as_f64()).unwrap_or(0.3) as f32,
        slope_limit: data.get("slope_limit").and_then(|v| v.as_f64()).unwrap_or(45.0) as f32,
    };
    entity_commands.insert(cc_data);
}

fn inspect_character_controller(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<CharacterControllerData>(entity) {
        ui.collapsing("Movement", |ui| {
            ui.horizontal(|ui| {
                ui.label("Speed:");
                if ui.add(egui::DragValue::new(&mut data.move_speed).speed(0.1).range(0.1..=50.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Sprint Mult:");
                if ui.add(egui::DragValue::new(&mut data.sprint_multiplier).speed(0.1).range(1.0..=5.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Jump Force:");
                if ui.add(egui::DragValue::new(&mut data.jump_force).speed(0.1).range(0.0..=30.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Gravity Scale:");
                if ui.add(egui::DragValue::new(&mut data.gravity_scale).speed(0.1).range(0.0..=5.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Ground Friction:");
                if ui.add(egui::DragValue::new(&mut data.ground_friction).speed(0.1).range(0.0..=20.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Air Control:");
                if ui.add(egui::DragValue::new(&mut data.air_control).speed(0.05).range(0.0..=1.0)).changed() { changed = true; }
            });
        });

        ui.collapsing("Capsule", |ui| {
            ui.horizontal(|ui| {
                ui.label("Height:");
                if ui.add(egui::DragValue::new(&mut data.height).speed(0.1).range(0.5..=5.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Radius:");
                if ui.add(egui::DragValue::new(&mut data.radius).speed(0.05).range(0.1..=2.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Step Height:");
                if ui.add(egui::DragValue::new(&mut data.step_height).speed(0.05).range(0.0..=1.0)).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Slope Limit:");
                if ui.add(egui::DragValue::new(&mut data.slope_limit).speed(1.0).range(0.0..=90.0).suffix("°")).changed() { changed = true; }
            });
        });
    }
    changed
}

// ============================================================================
// Trigger Zone Implementation
// ============================================================================

fn add_trigger_zone(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(TriggerZoneData::default());
}

fn remove_trigger_zone(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<TriggerZoneData>();
}

fn has_trigger_zone(world: &World, entity: Entity) -> bool {
    world.get::<TriggerZoneData>(entity).is_some()
}

fn serialize_trigger_zone(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<TriggerZoneData>(entity)?;
    Some(json!({
        "shape": data.shape,
        "size": data.size,
        "radius": data.radius,
        "one_shot": data.one_shot,
        "trigger_layers": data.trigger_layers,
        "tag_filter": data.tag_filter,
    }))
}

fn deserialize_trigger_zone(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let trigger_data = TriggerZoneData {
        shape: data.get("shape").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        size: data.get("size").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([2.0, 2.0, 2.0]),
        radius: data.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        one_shot: data.get("one_shot").and_then(|v| v.as_bool()).unwrap_or(false),
        trigger_layers: data.get("trigger_layers").and_then(|v| v.as_u64()).unwrap_or(0xFFFFFFFF) as u32,
        tag_filter: data.get("tag_filter").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    };
    entity_commands.insert(trigger_data);
}

fn inspect_trigger_zone(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<TriggerZoneData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Shape:");
            egui::ComboBox::from_id_salt("trigger_shape")
                .selected_text(match data.shape {
                    TriggerShape::Box => "Box",
                    TriggerShape::Sphere => "Sphere",
                    TriggerShape::Capsule => "Capsule",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.shape == TriggerShape::Box, "Box").clicked() { data.shape = TriggerShape::Box; changed = true; }
                    if ui.selectable_label(data.shape == TriggerShape::Sphere, "Sphere").clicked() { data.shape = TriggerShape::Sphere; changed = true; }
                    if ui.selectable_label(data.shape == TriggerShape::Capsule, "Capsule").clicked() { data.shape = TriggerShape::Capsule; changed = true; }
                });
        });

        match data.shape {
            TriggerShape::Box => {
                ui.horizontal(|ui| {
                    ui.label("Size:");
                    if ui.add(egui::DragValue::new(&mut data.size[0]).speed(0.1).prefix("X: ")).changed() { changed = true; }
                    if ui.add(egui::DragValue::new(&mut data.size[1]).speed(0.1).prefix("Y: ")).changed() { changed = true; }
                    if ui.add(egui::DragValue::new(&mut data.size[2]).speed(0.1).prefix("Z: ")).changed() { changed = true; }
                });
            }
            TriggerShape::Sphere | TriggerShape::Capsule => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(&mut data.radius).speed(0.1).range(0.1..=100.0)).changed() { changed = true; }
                });
                if data.shape == TriggerShape::Capsule {
                    ui.horizontal(|ui| {
                        ui.label("Height:");
                        if ui.add(egui::DragValue::new(&mut data.size[1]).speed(0.1).range(0.1..=100.0)).changed() { changed = true; }
                    });
                }
            }
        }

        ui.add_space(4.0);
        if ui.checkbox(&mut data.one_shot, "One Shot").changed() { changed = true; }

        ui.horizontal(|ui| {
            ui.label("Tag Filter:");
            if ui.text_edit_singleline(&mut data.tag_filter).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Spawner Implementation
// ============================================================================

fn add_spawner(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(SpawnerData::default());
}

fn remove_spawner(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<SpawnerData>();
}

fn has_spawner(world: &World, entity: Entity) -> bool {
    world.get::<SpawnerData>(entity).is_some()
}

fn serialize_spawner(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<SpawnerData>(entity)?;
    Some(json!({
        "prefab_path": data.prefab_path,
        "spawn_interval": data.spawn_interval,
        "max_spawned": data.max_spawned,
        "spawn_on_start": data.spawn_on_start,
        "spawn_count": data.spawn_count,
        "random_offset": data.random_offset,
        "inherit_rotation": data.inherit_rotation,
    }))
}

fn deserialize_spawner(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let spawner_data = SpawnerData {
        prefab_path: data.get("prefab_path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        spawn_interval: data.get("spawn_interval").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        max_spawned: data.get("max_spawned").and_then(|v| v.as_u64()).unwrap_or(10) as u32,
        spawn_on_start: data.get("spawn_on_start").and_then(|v| v.as_bool()).unwrap_or(false),
        spawn_count: data.get("spawn_count").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
        random_offset: data.get("random_offset").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 0.0, 0.0]),
        inherit_rotation: data.get("inherit_rotation").and_then(|v| v.as_bool()).unwrap_or(true),
    };
    entity_commands.insert(spawner_data);
}

fn inspect_spawner(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<SpawnerData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Prefab:");
            if ui.text_edit_singleline(&mut data.prefab_path).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Interval:");
            if ui.add(egui::DragValue::new(&mut data.spawn_interval).speed(0.1).range(0.1..=60.0).suffix("s")).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Max Spawned:");
            if ui.add(egui::DragValue::new(&mut data.max_spawned).speed(1.0).range(1..=1000)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Spawn Count:");
            if ui.add(egui::DragValue::new(&mut data.spawn_count).speed(1.0).range(1..=100)).changed() { changed = true; }
        });

        ui.label("Random Offset:");
        ui.horizontal(|ui| {
            if ui.add(egui::DragValue::new(&mut data.random_offset[0]).speed(0.1).prefix("X: ")).changed() { changed = true; }
            if ui.add(egui::DragValue::new(&mut data.random_offset[1]).speed(0.1).prefix("Y: ")).changed() { changed = true; }
            if ui.add(egui::DragValue::new(&mut data.random_offset[2]).speed(0.1).prefix("Z: ")).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.spawn_on_start, "Spawn on Start").changed() { changed = true; }
        if ui.checkbox(&mut data.inherit_rotation, "Inherit Rotation").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Timer Implementation
// ============================================================================

fn add_timer(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(TimerData::default());
}

fn remove_timer(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<TimerData>();
}

fn has_timer(world: &World, entity: Entity) -> bool {
    world.get::<TimerData>(entity).is_some()
}

fn serialize_timer(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<TimerData>(entity)?;
    Some(json!({
        "duration": data.duration,
        "looping": data.looping,
        "auto_start": data.auto_start,
    }))
}

fn deserialize_timer(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let timer_data = TimerData {
        duration: data.get("duration").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        elapsed: 0.0,
        looping: data.get("looping").and_then(|v| v.as_bool()).unwrap_or(false),
        auto_start: data.get("auto_start").and_then(|v| v.as_bool()).unwrap_or(false),
        paused: true,
    };
    entity_commands.insert(timer_data);
}

fn inspect_timer(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<TimerData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Duration:");
            if ui.add(egui::DragValue::new(&mut data.duration).speed(0.1).range(0.01..=3600.0).suffix("s")).changed() { changed = true; }
        });

        // Progress bar
        let progress = if data.duration > 0.0 { data.elapsed / data.duration } else { 0.0 };
        ui.add(egui::ProgressBar::new(progress).text(format!("{:.1}s / {:.1}s", data.elapsed, data.duration)));

        if ui.checkbox(&mut data.looping, "Loop").changed() { changed = true; }
        if ui.checkbox(&mut data.auto_start, "Auto Start").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Target Implementation
// ============================================================================

fn add_target(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(TargetData::default());
}

fn remove_target(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<TargetData>();
}

fn has_target(world: &World, entity: Entity) -> bool {
    world.get::<TargetData>(entity).is_some()
}

fn serialize_target(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<TargetData>(entity)?;
    Some(json!({
        "team": data.team,
        "priority": data.priority,
        "targetable": data.targetable,
        "auto_target_range": data.auto_target_range,
    }))
}

fn deserialize_target(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let target_data = TargetData {
        team: data.get("team").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        priority: data.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        targetable: data.get("targetable").and_then(|v| v.as_bool()).unwrap_or(true),
        auto_target_range: data.get("auto_target_range").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
    };
    entity_commands.insert(target_data);
}

fn inspect_target(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<TargetData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Team:");
            if ui.add(egui::DragValue::new(&mut data.team).speed(1.0).range(0..=255)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Priority:");
            if ui.add(egui::DragValue::new(&mut data.priority).speed(1.0).range(-100..=100)).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.targetable, "Targetable").changed() { changed = true; }

        ui.horizontal(|ui| {
            ui.label("Auto-Target Range:");
            if ui.add(egui::DragValue::new(&mut data.auto_target_range).speed(0.5).range(0.0..=1000.0)).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Waypoint Implementation
// ============================================================================

fn add_waypoint(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(WaypointData::default());
}

fn remove_waypoint(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<WaypointData>();
}

fn has_waypoint(world: &World, entity: Entity) -> bool {
    world.get::<WaypointData>(entity).is_some()
}

fn serialize_waypoint(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<WaypointData>(entity)?;
    Some(json!({
        "waypoint_id": data.waypoint_id,
        "connected_waypoints": data.connected_waypoints,
        "wait_time": data.wait_time,
        "radius": data.radius,
    }))
}

fn deserialize_waypoint(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let waypoint_data = WaypointData {
        waypoint_id: data.get("waypoint_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        connected_waypoints: data.get("connected_waypoints").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        wait_time: data.get("wait_time").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        radius: data.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
    };
    entity_commands.insert(waypoint_data);
}

fn inspect_waypoint(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<WaypointData>(entity) {
        ui.horizontal(|ui| {
            ui.label("ID:");
            if ui.text_edit_singleline(&mut data.waypoint_id).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Radius:");
            if ui.add(egui::DragValue::new(&mut data.radius).speed(0.1).range(0.1..=10.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Wait Time:");
            if ui.add(egui::DragValue::new(&mut data.wait_time).speed(0.1).range(0.0..=60.0).suffix("s")).changed() { changed = true; }
        });

        ui.label(format!("Connected: {}", data.connected_waypoints.len()));
    }
    changed
}
