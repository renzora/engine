//! Interaction component definitions
//!
//! Components for player interaction, collectibles, and interactive objects.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};

use egui_phosphor::regular::{
    HAND_POINTING, COIN, DOOR as DOOR_ICON, CHAT_CIRCLE, PACKAGE, KEY as KEY_ICON, TREASURE_CHEST,
};

// ============================================================================
// Interactable Component - Objects that can be interacted with
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum InteractionType {
    #[default]
    Use,
    Talk,
    Pickup,
    Open,
    Examine,
    Custom,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct InteractableData {
    pub interaction_type: InteractionType,
    pub interaction_range: f32,
    pub prompt_text: String,
    pub requires_key: bool,
    pub key_item: String,
    pub one_shot: bool,
    pub cooldown: f32,
    pub highlight_on_hover: bool,
}

impl Default for InteractableData {
    fn default() -> Self {
        Self {
            interaction_type: InteractionType::Use,
            interaction_range: 2.0,
            prompt_text: "Press E to interact".to_string(),
            requires_key: false,
            key_item: String::new(),
            one_shot: false,
            cooldown: 0.0,
            highlight_on_hover: true,
        }
    }
}

pub static INTERACTABLE: ComponentDefinition = ComponentDefinition {
    type_id: "interactable",
    display_name: "Interactable",
    category: ComponentCategory::Gameplay,
    icon: HAND_POINTING,
    priority: 10,
    add_fn: add_interactable,
    remove_fn: remove_interactable,
    has_fn: has_interactable,
    serialize_fn: serialize_interactable,
    deserialize_fn: deserialize_interactable,
    inspector_fn: inspect_interactable,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Collectible Component - Pickups and items
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum CollectibleType {
    #[default]
    Item,
    Currency,
    Health,
    Ammo,
    Key,
    PowerUp,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct CollectibleData {
    pub collectible_type: CollectibleType,
    pub item_id: String,
    pub amount: i32,
    pub auto_collect: bool,
    pub collect_range: f32,
    pub respawn: bool,
    pub respawn_time: f32,
    pub destroy_on_collect: bool,
    pub play_sound: bool,
    pub sound_path: String,
}

impl Default for CollectibleData {
    fn default() -> Self {
        Self {
            collectible_type: CollectibleType::Item,
            item_id: String::new(),
            amount: 1,
            auto_collect: true,
            collect_range: 1.0,
            respawn: false,
            respawn_time: 10.0,
            destroy_on_collect: true,
            play_sound: true,
            sound_path: String::new(),
        }
    }
}

pub static COLLECTIBLE: ComponentDefinition = ComponentDefinition {
    type_id: "collectible",
    display_name: "Collectible",
    category: ComponentCategory::Gameplay,
    icon: COIN,
    priority: 11,
    add_fn: add_collectible,
    remove_fn: remove_collectible,
    has_fn: has_collectible,
    serialize_fn: serialize_collectible,
    deserialize_fn: deserialize_collectible,
    inspector_fn: inspect_collectible,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Door Component - Openable doors/gates
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum DoorState {
    #[default]
    Closed,
    Open,
    Opening,
    Closing,
    Locked,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum DoorType {
    #[default]
    Swing,
    Slide,
    Rotate,
    Lift,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct DoorData {
    pub door_type: DoorType,
    pub initial_state: DoorState,
    pub locked: bool,
    pub key_required: String,
    pub open_angle: f32,
    pub slide_distance: f32,
    pub open_speed: f32,
    pub auto_close: bool,
    pub auto_close_delay: f32,
    pub play_sound: bool,
    pub open_sound: String,
    pub close_sound: String,
}

impl Default for DoorData {
    fn default() -> Self {
        Self {
            door_type: DoorType::Swing,
            initial_state: DoorState::Closed,
            locked: false,
            key_required: String::new(),
            open_angle: 90.0,
            slide_distance: 2.0,
            open_speed: 2.0,
            auto_close: false,
            auto_close_delay: 3.0,
            play_sound: true,
            open_sound: String::new(),
            close_sound: String::new(),
        }
    }
}

pub static DOOR: ComponentDefinition = ComponentDefinition {
    type_id: "door",
    display_name: "Door",
    category: ComponentCategory::Gameplay,
    icon: DOOR_ICON,
    priority: 12,
    add_fn: add_door,
    remove_fn: remove_door,
    has_fn: has_door,
    serialize_fn: serialize_door,
    deserialize_fn: deserialize_door,
    inspector_fn: inspect_door,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Dialogue Component - NPCs with dialogue
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct DialogueData {
    pub dialogue_id: String,
    pub speaker_name: String,
    pub interaction_range: f32,
    pub look_at_player: bool,
    pub can_repeat: bool,
    pub greeting_line: String,
}

impl Default for DialogueData {
    fn default() -> Self {
        Self {
            dialogue_id: String::new(),
            speaker_name: "NPC".to_string(),
            interaction_range: 3.0,
            look_at_player: true,
            can_repeat: true,
            greeting_line: "Hello!".to_string(),
        }
    }
}

pub static DIALOGUE: ComponentDefinition = ComponentDefinition {
    type_id: "dialogue",
    display_name: "Dialogue",
    category: ComponentCategory::Gameplay,
    icon: CHAT_CIRCLE,
    priority: 13,
    add_fn: add_dialogue,
    remove_fn: remove_dialogue,
    has_fn: has_dialogue,
    serialize_fn: serialize_dialogue,
    deserialize_fn: deserialize_dialogue,
    inspector_fn: inspect_dialogue,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Inventory Component - Item storage
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct InventoryData {
    pub max_slots: u32,
    pub max_stack_size: u32,
    pub drop_items_on_death: bool,
    pub persist_items: bool,
}

impl Default for InventoryData {
    fn default() -> Self {
        Self {
            max_slots: 20,
            max_stack_size: 99,
            drop_items_on_death: false,
            persist_items: true,
        }
    }
}

pub static INVENTORY: ComponentDefinition = ComponentDefinition {
    type_id: "inventory",
    display_name: "Inventory",
    category: ComponentCategory::Gameplay,
    icon: PACKAGE,
    priority: 14,
    add_fn: add_inventory,
    remove_fn: remove_inventory,
    has_fn: has_inventory,
    serialize_fn: serialize_inventory,
    deserialize_fn: deserialize_inventory,
    inspector_fn: inspect_inventory,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Loot Container Component - Chests, crates, etc.
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct LootContainerData {
    pub loot_table_id: String,
    pub is_locked: bool,
    pub key_required: String,
    pub open_once: bool,
    pub respawn_loot: bool,
    pub respawn_time: f32,
    pub drop_on_ground: bool,
    pub interaction_range: f32,
}

impl Default for LootContainerData {
    fn default() -> Self {
        Self {
            loot_table_id: String::new(),
            is_locked: false,
            key_required: String::new(),
            open_once: true,
            respawn_loot: false,
            respawn_time: 60.0,
            drop_on_ground: false,
            interaction_range: 2.0,
        }
    }
}

pub static LOOT_CONTAINER: ComponentDefinition = ComponentDefinition {
    type_id: "loot_container",
    display_name: "Loot Container",
    category: ComponentCategory::Gameplay,
    icon: TREASURE_CHEST,
    priority: 15,
    add_fn: add_loot_container,
    remove_fn: remove_loot_container,
    has_fn: has_loot_container,
    serialize_fn: serialize_loot_container,
    deserialize_fn: deserialize_loot_container,
    inspector_fn: inspect_loot_container,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Key Item Component - Keys and unlock items
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct KeyItemData {
    pub key_id: String,
    pub uses: i32,
    pub consumed_on_use: bool,
    pub display_name: String,
}

impl Default for KeyItemData {
    fn default() -> Self {
        Self {
            key_id: "default_key".to_string(),
            uses: -1, // -1 = infinite
            consumed_on_use: true,
            display_name: "Key".to_string(),
        }
    }
}

pub static KEY_ITEM: ComponentDefinition = ComponentDefinition {
    type_id: "key_item",
    display_name: "Key Item",
    category: ComponentCategory::Gameplay,
    icon: KEY_ICON,
    priority: 16,
    add_fn: add_key_item,
    remove_fn: remove_key_item,
    has_fn: has_key_item,
    serialize_fn: serialize_key_item,
    deserialize_fn: deserialize_key_item,
    inspector_fn: inspect_key_item,
    conflicts_with: &[],
    requires: &[],
};

/// Register all interaction components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&INTERACTABLE);
    registry.register(&COLLECTIBLE);
    registry.register(&DOOR);
    registry.register(&DIALOGUE);
    registry.register(&INVENTORY);
    registry.register(&LOOT_CONTAINER);
    registry.register(&KEY_ITEM);
}

// ============================================================================
// Interactable Implementation
// ============================================================================

fn add_interactable(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(InteractableData::default());
}

fn remove_interactable(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<InteractableData>();
}

fn has_interactable(world: &World, entity: Entity) -> bool {
    world.get::<InteractableData>(entity).is_some()
}

fn serialize_interactable(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<InteractableData>(entity)?;
    Some(json!({
        "interaction_type": data.interaction_type,
        "interaction_range": data.interaction_range,
        "prompt_text": data.prompt_text,
        "requires_key": data.requires_key,
        "key_item": data.key_item,
        "one_shot": data.one_shot,
        "cooldown": data.cooldown,
        "highlight_on_hover": data.highlight_on_hover,
    }))
}

fn deserialize_interactable(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let interactable_data = InteractableData {
        interaction_type: data.get("interaction_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        interaction_range: data.get("interaction_range").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        prompt_text: data.get("prompt_text").and_then(|v| v.as_str()).unwrap_or("Press E to interact").to_string(),
        requires_key: data.get("requires_key").and_then(|v| v.as_bool()).unwrap_or(false),
        key_item: data.get("key_item").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        one_shot: data.get("one_shot").and_then(|v| v.as_bool()).unwrap_or(false),
        cooldown: data.get("cooldown").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        highlight_on_hover: data.get("highlight_on_hover").and_then(|v| v.as_bool()).unwrap_or(true),
    };
    entity_commands.insert(interactable_data);
}

fn inspect_interactable(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<InteractableData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Type:");
            egui::ComboBox::from_id_salt("interaction_type")
                .selected_text(match data.interaction_type {
                    InteractionType::Use => "Use",
                    InteractionType::Talk => "Talk",
                    InteractionType::Pickup => "Pickup",
                    InteractionType::Open => "Open",
                    InteractionType::Examine => "Examine",
                    InteractionType::Custom => "Custom",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.interaction_type == InteractionType::Use, "Use").clicked() { data.interaction_type = InteractionType::Use; changed = true; }
                    if ui.selectable_label(data.interaction_type == InteractionType::Talk, "Talk").clicked() { data.interaction_type = InteractionType::Talk; changed = true; }
                    if ui.selectable_label(data.interaction_type == InteractionType::Pickup, "Pickup").clicked() { data.interaction_type = InteractionType::Pickup; changed = true; }
                    if ui.selectable_label(data.interaction_type == InteractionType::Open, "Open").clicked() { data.interaction_type = InteractionType::Open; changed = true; }
                    if ui.selectable_label(data.interaction_type == InteractionType::Examine, "Examine").clicked() { data.interaction_type = InteractionType::Examine; changed = true; }
                    if ui.selectable_label(data.interaction_type == InteractionType::Custom, "Custom").clicked() { data.interaction_type = InteractionType::Custom; changed = true; }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Range:");
            if ui.add(egui::DragValue::new(&mut data.interaction_range).speed(0.1).range(0.5..=20.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Prompt:");
            if ui.text_edit_singleline(&mut data.prompt_text).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.requires_key, "Requires Key").changed() { changed = true; }
        if data.requires_key {
            ui.horizontal(|ui| {
                ui.label("Key ID:");
                if ui.text_edit_singleline(&mut data.key_item).changed() { changed = true; }
            });
        }

        if ui.checkbox(&mut data.one_shot, "One Shot").changed() { changed = true; }

        ui.horizontal(|ui| {
            ui.label("Cooldown:");
            if ui.add(egui::DragValue::new(&mut data.cooldown).speed(0.1).range(0.0..=60.0).suffix("s")).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.highlight_on_hover, "Highlight on Hover").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Collectible Implementation
// ============================================================================

fn add_collectible(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(CollectibleData::default());
}

fn remove_collectible(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<CollectibleData>();
}

fn has_collectible(world: &World, entity: Entity) -> bool {
    world.get::<CollectibleData>(entity).is_some()
}

fn serialize_collectible(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<CollectibleData>(entity)?;
    Some(json!({
        "collectible_type": data.collectible_type,
        "item_id": data.item_id,
        "amount": data.amount,
        "auto_collect": data.auto_collect,
        "collect_range": data.collect_range,
        "respawn": data.respawn,
        "respawn_time": data.respawn_time,
        "destroy_on_collect": data.destroy_on_collect,
        "play_sound": data.play_sound,
        "sound_path": data.sound_path,
    }))
}

fn deserialize_collectible(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let collectible_data = CollectibleData {
        collectible_type: data.get("collectible_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        item_id: data.get("item_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        amount: data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32,
        auto_collect: data.get("auto_collect").and_then(|v| v.as_bool()).unwrap_or(true),
        collect_range: data.get("collect_range").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        respawn: data.get("respawn").and_then(|v| v.as_bool()).unwrap_or(false),
        respawn_time: data.get("respawn_time").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        destroy_on_collect: data.get("destroy_on_collect").and_then(|v| v.as_bool()).unwrap_or(true),
        play_sound: data.get("play_sound").and_then(|v| v.as_bool()).unwrap_or(true),
        sound_path: data.get("sound_path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    };
    entity_commands.insert(collectible_data);
}

fn inspect_collectible(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<CollectibleData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Type:");
            egui::ComboBox::from_id_salt("collectible_type")
                .selected_text(match data.collectible_type {
                    CollectibleType::Item => "Item",
                    CollectibleType::Currency => "Currency",
                    CollectibleType::Health => "Health",
                    CollectibleType::Ammo => "Ammo",
                    CollectibleType::Key => "Key",
                    CollectibleType::PowerUp => "Power-Up",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.collectible_type == CollectibleType::Item, "Item").clicked() { data.collectible_type = CollectibleType::Item; changed = true; }
                    if ui.selectable_label(data.collectible_type == CollectibleType::Currency, "Currency").clicked() { data.collectible_type = CollectibleType::Currency; changed = true; }
                    if ui.selectable_label(data.collectible_type == CollectibleType::Health, "Health").clicked() { data.collectible_type = CollectibleType::Health; changed = true; }
                    if ui.selectable_label(data.collectible_type == CollectibleType::Ammo, "Ammo").clicked() { data.collectible_type = CollectibleType::Ammo; changed = true; }
                    if ui.selectable_label(data.collectible_type == CollectibleType::Key, "Key").clicked() { data.collectible_type = CollectibleType::Key; changed = true; }
                    if ui.selectable_label(data.collectible_type == CollectibleType::PowerUp, "Power-Up").clicked() { data.collectible_type = CollectibleType::PowerUp; changed = true; }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Item ID:");
            if ui.text_edit_singleline(&mut data.item_id).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Amount:");
            if ui.add(egui::DragValue::new(&mut data.amount).speed(1.0).range(1..=9999)).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.auto_collect, "Auto Collect on Touch").changed() { changed = true; }

        ui.horizontal(|ui| {
            ui.label("Collect Range:");
            if ui.add(egui::DragValue::new(&mut data.collect_range).speed(0.1).range(0.1..=10.0)).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.destroy_on_collect, "Destroy on Collect").changed() { changed = true; }

        ui.separator();

        if ui.checkbox(&mut data.respawn, "Respawn").changed() { changed = true; }
        if data.respawn {
            ui.horizontal(|ui| {
                ui.label("Respawn Time:");
                if ui.add(egui::DragValue::new(&mut data.respawn_time).speed(0.5).range(1.0..=300.0).suffix("s")).changed() { changed = true; }
            });
        }

        ui.separator();

        if ui.checkbox(&mut data.play_sound, "Play Sound").changed() { changed = true; }
        if data.play_sound {
            ui.horizontal(|ui| {
                ui.label("Sound:");
                if ui.text_edit_singleline(&mut data.sound_path).changed() { changed = true; }
            });
        }
    }
    changed
}

// ============================================================================
// Door Implementation
// ============================================================================

fn add_door(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(DoorData::default());
}

fn remove_door(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<DoorData>();
}

fn has_door(world: &World, entity: Entity) -> bool {
    world.get::<DoorData>(entity).is_some()
}

fn serialize_door(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<DoorData>(entity)?;
    Some(json!({
        "door_type": data.door_type,
        "initial_state": data.initial_state,
        "locked": data.locked,
        "key_required": data.key_required,
        "open_angle": data.open_angle,
        "slide_distance": data.slide_distance,
        "open_speed": data.open_speed,
        "auto_close": data.auto_close,
        "auto_close_delay": data.auto_close_delay,
        "play_sound": data.play_sound,
        "open_sound": data.open_sound,
        "close_sound": data.close_sound,
    }))
}

fn deserialize_door(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let door_data = DoorData {
        door_type: data.get("door_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        initial_state: data.get("initial_state").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        locked: data.get("locked").and_then(|v| v.as_bool()).unwrap_or(false),
        key_required: data.get("key_required").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        open_angle: data.get("open_angle").and_then(|v| v.as_f64()).unwrap_or(90.0) as f32,
        slide_distance: data.get("slide_distance").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        open_speed: data.get("open_speed").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        auto_close: data.get("auto_close").and_then(|v| v.as_bool()).unwrap_or(false),
        auto_close_delay: data.get("auto_close_delay").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
        play_sound: data.get("play_sound").and_then(|v| v.as_bool()).unwrap_or(true),
        open_sound: data.get("open_sound").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        close_sound: data.get("close_sound").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    };
    entity_commands.insert(door_data);
}

fn inspect_door(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<DoorData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Door Type:");
            egui::ComboBox::from_id_salt("door_type")
                .selected_text(match data.door_type {
                    DoorType::Swing => "Swing",
                    DoorType::Slide => "Slide",
                    DoorType::Rotate => "Rotate",
                    DoorType::Lift => "Lift",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.door_type == DoorType::Swing, "Swing").clicked() { data.door_type = DoorType::Swing; changed = true; }
                    if ui.selectable_label(data.door_type == DoorType::Slide, "Slide").clicked() { data.door_type = DoorType::Slide; changed = true; }
                    if ui.selectable_label(data.door_type == DoorType::Rotate, "Rotate").clicked() { data.door_type = DoorType::Rotate; changed = true; }
                    if ui.selectable_label(data.door_type == DoorType::Lift, "Lift").clicked() { data.door_type = DoorType::Lift; changed = true; }
                });
        });

        match data.door_type {
            DoorType::Swing | DoorType::Rotate => {
                ui.horizontal(|ui| {
                    ui.label("Open Angle:");
                    if ui.add(egui::DragValue::new(&mut data.open_angle).speed(1.0).range(0.0..=180.0).suffix("°")).changed() { changed = true; }
                });
            }
            DoorType::Slide | DoorType::Lift => {
                ui.horizontal(|ui| {
                    ui.label("Slide Distance:");
                    if ui.add(egui::DragValue::new(&mut data.slide_distance).speed(0.1).range(0.1..=20.0)).changed() { changed = true; }
                });
            }
        }

        ui.horizontal(|ui| {
            ui.label("Open Speed:");
            if ui.add(egui::DragValue::new(&mut data.open_speed).speed(0.1).range(0.1..=10.0)).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.locked, "Locked").changed() { changed = true; }
        if data.locked {
            ui.horizontal(|ui| {
                ui.label("Key Required:");
                if ui.text_edit_singleline(&mut data.key_required).changed() { changed = true; }
            });
        }

        ui.separator();

        if ui.checkbox(&mut data.auto_close, "Auto Close").changed() { changed = true; }
        if data.auto_close {
            ui.horizontal(|ui| {
                ui.label("Delay:");
                if ui.add(egui::DragValue::new(&mut data.auto_close_delay).speed(0.1).range(0.5..=30.0).suffix("s")).changed() { changed = true; }
            });
        }

        ui.separator();

        if ui.checkbox(&mut data.play_sound, "Play Sounds").changed() { changed = true; }
        if data.play_sound {
            ui.horizontal(|ui| {
                ui.label("Open:");
                if ui.text_edit_singleline(&mut data.open_sound).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Close:");
                if ui.text_edit_singleline(&mut data.close_sound).changed() { changed = true; }
            });
        }
    }
    changed
}

// ============================================================================
// Dialogue Implementation
// ============================================================================

fn add_dialogue(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(DialogueData::default());
}

fn remove_dialogue(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<DialogueData>();
}

fn has_dialogue(world: &World, entity: Entity) -> bool {
    world.get::<DialogueData>(entity).is_some()
}

fn serialize_dialogue(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<DialogueData>(entity)?;
    Some(json!({
        "dialogue_id": data.dialogue_id,
        "speaker_name": data.speaker_name,
        "interaction_range": data.interaction_range,
        "look_at_player": data.look_at_player,
        "can_repeat": data.can_repeat,
        "greeting_line": data.greeting_line,
    }))
}

fn deserialize_dialogue(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let dialogue_data = DialogueData {
        dialogue_id: data.get("dialogue_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        speaker_name: data.get("speaker_name").and_then(|v| v.as_str()).unwrap_or("NPC").to_string(),
        interaction_range: data.get("interaction_range").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
        look_at_player: data.get("look_at_player").and_then(|v| v.as_bool()).unwrap_or(true),
        can_repeat: data.get("can_repeat").and_then(|v| v.as_bool()).unwrap_or(true),
        greeting_line: data.get("greeting_line").and_then(|v| v.as_str()).unwrap_or("Hello!").to_string(),
    };
    entity_commands.insert(dialogue_data);
}

fn inspect_dialogue(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<DialogueData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Speaker Name:");
            if ui.text_edit_singleline(&mut data.speaker_name).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Dialogue ID:");
            if ui.text_edit_singleline(&mut data.dialogue_id).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Greeting:");
            if ui.text_edit_singleline(&mut data.greeting_line).changed() { changed = true; }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Interaction Range:");
            if ui.add(egui::DragValue::new(&mut data.interaction_range).speed(0.1).range(1.0..=20.0)).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.look_at_player, "Look at Player").changed() { changed = true; }
        if ui.checkbox(&mut data.can_repeat, "Can Repeat Dialogue").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Inventory Implementation
// ============================================================================

fn add_inventory(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(InventoryData::default());
}

fn remove_inventory(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<InventoryData>();
}

fn has_inventory(world: &World, entity: Entity) -> bool {
    world.get::<InventoryData>(entity).is_some()
}

fn serialize_inventory(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<InventoryData>(entity)?;
    Some(json!({
        "max_slots": data.max_slots,
        "max_stack_size": data.max_stack_size,
        "drop_items_on_death": data.drop_items_on_death,
        "persist_items": data.persist_items,
    }))
}

fn deserialize_inventory(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let inventory_data = InventoryData {
        max_slots: data.get("max_slots").and_then(|v| v.as_u64()).unwrap_or(20) as u32,
        max_stack_size: data.get("max_stack_size").and_then(|v| v.as_u64()).unwrap_or(99) as u32,
        drop_items_on_death: data.get("drop_items_on_death").and_then(|v| v.as_bool()).unwrap_or(false),
        persist_items: data.get("persist_items").and_then(|v| v.as_bool()).unwrap_or(true),
    };
    entity_commands.insert(inventory_data);
}

fn inspect_inventory(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<InventoryData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Max Slots:");
            if ui.add(egui::DragValue::new(&mut data.max_slots).speed(1.0).range(1..=100)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Max Stack Size:");
            if ui.add(egui::DragValue::new(&mut data.max_stack_size).speed(1.0).range(1..=999)).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.drop_items_on_death, "Drop Items on Death").changed() { changed = true; }
        if ui.checkbox(&mut data.persist_items, "Persist Items (Save)").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Loot Container Implementation
// ============================================================================

fn add_loot_container(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(LootContainerData::default());
}

fn remove_loot_container(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<LootContainerData>();
}

fn has_loot_container(world: &World, entity: Entity) -> bool {
    world.get::<LootContainerData>(entity).is_some()
}

fn serialize_loot_container(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<LootContainerData>(entity)?;
    Some(json!({
        "loot_table_id": data.loot_table_id,
        "is_locked": data.is_locked,
        "key_required": data.key_required,
        "open_once": data.open_once,
        "respawn_loot": data.respawn_loot,
        "respawn_time": data.respawn_time,
        "drop_on_ground": data.drop_on_ground,
        "interaction_range": data.interaction_range,
    }))
}

fn deserialize_loot_container(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let loot_data = LootContainerData {
        loot_table_id: data.get("loot_table_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        is_locked: data.get("is_locked").and_then(|v| v.as_bool()).unwrap_or(false),
        key_required: data.get("key_required").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        open_once: data.get("open_once").and_then(|v| v.as_bool()).unwrap_or(true),
        respawn_loot: data.get("respawn_loot").and_then(|v| v.as_bool()).unwrap_or(false),
        respawn_time: data.get("respawn_time").and_then(|v| v.as_f64()).unwrap_or(60.0) as f32,
        drop_on_ground: data.get("drop_on_ground").and_then(|v| v.as_bool()).unwrap_or(false),
        interaction_range: data.get("interaction_range").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
    };
    entity_commands.insert(loot_data);
}

fn inspect_loot_container(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<LootContainerData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Loot Table:");
            if ui.text_edit_singleline(&mut data.loot_table_id).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Interact Range:");
            if ui.add(egui::DragValue::new(&mut data.interaction_range).speed(0.1).range(0.5..=10.0)).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.is_locked, "Locked").changed() { changed = true; }
        if data.is_locked {
            ui.horizontal(|ui| {
                ui.label("Key Required:");
                if ui.text_edit_singleline(&mut data.key_required).changed() { changed = true; }
            });
        }

        if ui.checkbox(&mut data.open_once, "Open Once").changed() { changed = true; }
        if ui.checkbox(&mut data.drop_on_ground, "Drop on Ground").changed() { changed = true; }

        ui.separator();

        if ui.checkbox(&mut data.respawn_loot, "Respawn Loot").changed() { changed = true; }
        if data.respawn_loot {
            ui.horizontal(|ui| {
                ui.label("Respawn Time:");
                if ui.add(egui::DragValue::new(&mut data.respawn_time).speed(1.0).range(1.0..=3600.0).suffix("s")).changed() { changed = true; }
            });
        }
    }
    changed
}

// ============================================================================
// Key Item Implementation
// ============================================================================

fn add_key_item(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(KeyItemData::default());
}

fn remove_key_item(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<KeyItemData>();
}

fn has_key_item(world: &World, entity: Entity) -> bool {
    world.get::<KeyItemData>(entity).is_some()
}

fn serialize_key_item(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<KeyItemData>(entity)?;
    Some(json!({
        "key_id": data.key_id,
        "uses": data.uses,
        "consumed_on_use": data.consumed_on_use,
        "display_name": data.display_name,
    }))
}

fn deserialize_key_item(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let key_data = KeyItemData {
        key_id: data.get("key_id").and_then(|v| v.as_str()).unwrap_or("default_key").to_string(),
        uses: data.get("uses").and_then(|v| v.as_i64()).unwrap_or(-1) as i32,
        consumed_on_use: data.get("consumed_on_use").and_then(|v| v.as_bool()).unwrap_or(true),
        display_name: data.get("display_name").and_then(|v| v.as_str()).unwrap_or("Key").to_string(),
    };
    entity_commands.insert(key_data);
}

fn inspect_key_item(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<KeyItemData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Key ID:");
            if ui.text_edit_singleline(&mut data.key_id).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Display Name:");
            if ui.text_edit_singleline(&mut data.display_name).changed() { changed = true; }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Uses:");
            if ui.add(egui::DragValue::new(&mut data.uses).speed(1.0).range(-1..=100)).changed() { changed = true; }
        });
        ui.label("(-1 = infinite uses)");

        if ui.checkbox(&mut data.consumed_on_use, "Consumed on Use").changed() { changed = true; }
    }
    changed
}
