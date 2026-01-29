//! Combat component definitions
//!
//! Components for weapons, projectiles, damage zones, and combat mechanics.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};

use egui_phosphor::regular::{
    SWORD, CROSSHAIR, FIRE, SHIELD as SHIELD_ICON, LIGHTNING, ATOM, SKULL, FIRST_AID,
};

// ============================================================================
// Weapon Component - Melee and ranged weapons
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum WeaponType {
    #[default]
    Melee,
    Ranged,
    Throwable,
    Beam,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct WeaponData {
    pub weapon_type: WeaponType,
    pub damage: f32,
    pub attack_rate: f32,
    pub range: f32,
    pub knockback: f32,
    pub critical_chance: f32,
    pub critical_multiplier: f32,
    pub ammo_type: String,
    pub ammo_per_shot: i32,
    pub magazine_size: i32,
    pub reload_time: f32,
    pub spread: f32,
    pub projectile_prefab: String,
    pub projectile_speed: f32,
    pub muzzle_offset: [f32; 3],
    pub attack_sound: String,
}

impl Default for WeaponData {
    fn default() -> Self {
        Self {
            weapon_type: WeaponType::Melee,
            damage: 10.0,
            attack_rate: 1.0,
            range: 2.0,
            knockback: 0.0,
            critical_chance: 0.05,
            critical_multiplier: 2.0,
            ammo_type: String::new(),
            ammo_per_shot: 1,
            magazine_size: 0,
            reload_time: 1.0,
            spread: 0.0,
            projectile_prefab: String::new(),
            projectile_speed: 50.0,
            muzzle_offset: [0.0, 0.0, 1.0],
            attack_sound: String::new(),
        }
    }
}

pub static WEAPON: ComponentDefinition = ComponentDefinition {
    type_id: "weapon",
    display_name: "Weapon",
    category: ComponentCategory::Gameplay,
    icon: SWORD,
    priority: 30,
    add_fn: add_weapon,
    remove_fn: remove_weapon,
    has_fn: has_weapon,
    serialize_fn: serialize_weapon,
    deserialize_fn: deserialize_weapon,
    inspector_fn: inspect_weapon,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Projectile Component - Bullets, arrows, etc.
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum ProjectileType {
    #[default]
    Bullet,
    Arrow,
    Rocket,
    Grenade,
    Laser,
    Custom,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct ProjectileData {
    pub projectile_type: ProjectileType,
    pub damage: f32,
    pub speed: f32,
    pub lifetime: f32,
    pub gravity_scale: f32,
    pub pierce_count: i32,
    pub homing: bool,
    pub homing_strength: f32,
    pub explosion_radius: f32,
    pub explosion_damage: f32,
    pub destroy_on_hit: bool,
    pub hit_effect_prefab: String,
    pub trail_effect: bool,
}

impl Default for ProjectileData {
    fn default() -> Self {
        Self {
            projectile_type: ProjectileType::Bullet,
            damage: 10.0,
            speed: 50.0,
            lifetime: 5.0,
            gravity_scale: 0.0,
            pierce_count: 0,
            homing: false,
            homing_strength: 5.0,
            explosion_radius: 0.0,
            explosion_damage: 0.0,
            destroy_on_hit: true,
            hit_effect_prefab: String::new(),
            trail_effect: false,
        }
    }
}

pub static PROJECTILE: ComponentDefinition = ComponentDefinition {
    type_id: "projectile",
    display_name: "Projectile",
    category: ComponentCategory::Gameplay,
    icon: CROSSHAIR,
    priority: 31,
    add_fn: add_projectile,
    remove_fn: remove_projectile,
    has_fn: has_projectile,
    serialize_fn: serialize_projectile,
    deserialize_fn: deserialize_projectile,
    inspector_fn: inspect_projectile,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Damage Zone Component - Area damage
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum DamageZoneShape {
    #[default]
    Box,
    Sphere,
    Capsule,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum DamageType {
    #[default]
    Physical,
    Fire,
    Ice,
    Electric,
    Poison,
    Magic,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct DamageZoneData {
    pub shape: DamageZoneShape,
    pub size: [f32; 3],
    pub radius: f32,
    pub damage: f32,
    pub damage_type: DamageType,
    pub damage_interval: f32,
    pub knockback: f32,
    pub knockback_origin_center: bool,
    pub one_shot: bool,
    pub affects_tags: String,
    pub ignore_tags: String,
    pub apply_status_effect: String,
    pub status_duration: f32,
    pub active: bool,
}

impl Default for DamageZoneData {
    fn default() -> Self {
        Self {
            shape: DamageZoneShape::Box,
            size: [2.0, 2.0, 2.0],
            radius: 1.0,
            damage: 10.0,
            damage_type: DamageType::Physical,
            damage_interval: 1.0,
            knockback: 0.0,
            knockback_origin_center: true,
            one_shot: false,
            affects_tags: String::new(),
            ignore_tags: String::new(),
            apply_status_effect: String::new(),
            status_duration: 0.0,
            active: true,
        }
    }
}

pub static DAMAGE_ZONE: ComponentDefinition = ComponentDefinition {
    type_id: "damage_zone",
    display_name: "Damage Zone",
    category: ComponentCategory::Gameplay,
    icon: FIRE,
    priority: 32,
    add_fn: add_damage_zone,
    remove_fn: remove_damage_zone,
    has_fn: has_damage_zone,
    serialize_fn: serialize_damage_zone,
    deserialize_fn: deserialize_damage_zone,
    inspector_fn: inspect_damage_zone,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Shield Component - Damage absorption
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct ShieldData {
    pub max_shield: f32,
    pub current_shield: f32,
    pub recharge_rate: f32,
    pub recharge_delay: f32,
    pub absorb_percent: f32,
    pub blocks_damage_types: Vec<DamageType>,
    pub break_threshold: f32,
    pub break_cooldown: f32,
}

impl Default for ShieldData {
    fn default() -> Self {
        Self {
            max_shield: 50.0,
            current_shield: 50.0,
            recharge_rate: 5.0,
            recharge_delay: 3.0,
            absorb_percent: 1.0,
            blocks_damage_types: vec![DamageType::Physical],
            break_threshold: 0.0,
            break_cooldown: 5.0,
        }
    }
}

pub static SHIELD_COMPONENT: ComponentDefinition = ComponentDefinition {
    type_id: "shield",
    display_name: "Shield",
    category: ComponentCategory::Gameplay,
    icon: SHIELD_ICON,
    priority: 33,
    add_fn: add_shield,
    remove_fn: remove_shield,
    has_fn: has_shield,
    serialize_fn: serialize_shield,
    deserialize_fn: deserialize_shield,
    inspector_fn: inspect_shield,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Status Effect Component - Buffs/debuffs
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum StatusEffectType {
    #[default]
    Buff,
    Debuff,
    DOT,
    HOT,
    Stun,
    Slow,
    Haste,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct StatusEffectData {
    pub effect_id: String,
    pub effect_type: StatusEffectType,
    pub duration: f32,
    pub tick_rate: f32,
    pub value: f32,
    pub stacks: i32,
    pub max_stacks: i32,
    pub refresh_on_apply: bool,
    pub remove_on_death: bool,
    pub visual_effect: String,
}

impl Default for StatusEffectData {
    fn default() -> Self {
        Self {
            effect_id: "burning".to_string(),
            effect_type: StatusEffectType::DOT,
            duration: 5.0,
            tick_rate: 1.0,
            value: 5.0,
            stacks: 1,
            max_stacks: 5,
            refresh_on_apply: true,
            remove_on_death: true,
            visual_effect: String::new(),
        }
    }
}

pub static STATUS_EFFECT: ComponentDefinition = ComponentDefinition {
    type_id: "status_effect",
    display_name: "Status Effect",
    category: ComponentCategory::Gameplay,
    icon: LIGHTNING,
    priority: 34,
    add_fn: add_status_effect,
    remove_fn: remove_status_effect,
    has_fn: has_status_effect,
    serialize_fn: serialize_status_effect,
    deserialize_fn: deserialize_status_effect,
    inspector_fn: inspect_status_effect,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Explosion Component - Explosive objects
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct ExplosionData {
    pub radius: f32,
    pub damage: f32,
    pub damage_falloff: bool,
    pub knockback_force: f32,
    pub destroy_self: bool,
    pub chain_explode: bool,
    pub chain_radius: f32,
    pub trigger_on_damage: bool,
    pub trigger_threshold: f32,
    pub fuse_time: f32,
    pub explosion_effect: String,
    pub explosion_sound: String,
}

impl Default for ExplosionData {
    fn default() -> Self {
        Self {
            radius: 5.0,
            damage: 50.0,
            damage_falloff: true,
            knockback_force: 10.0,
            destroy_self: true,
            chain_explode: false,
            chain_radius: 3.0,
            trigger_on_damage: false,
            trigger_threshold: 0.0,
            fuse_time: 0.0,
            explosion_effect: String::new(),
            explosion_sound: String::new(),
        }
    }
}

pub static EXPLOSION: ComponentDefinition = ComponentDefinition {
    type_id: "explosion",
    display_name: "Explosion",
    category: ComponentCategory::Gameplay,
    icon: ATOM,
    priority: 35,
    add_fn: add_explosion,
    remove_fn: remove_explosion,
    has_fn: has_explosion,
    serialize_fn: serialize_explosion,
    deserialize_fn: deserialize_explosion,
    inspector_fn: inspect_explosion,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Combat Stats Component - RPG-style stats
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct CombatStatsData {
    pub attack_power: f32,
    pub defense: f32,
    pub armor: f32,
    pub armor_penetration: f32,
    pub critical_chance: f32,
    pub critical_damage: f32,
    pub attack_speed: f32,
    pub movement_speed_bonus: f32,
    pub damage_reduction: f32,
    pub lifesteal: f32,
    pub level: i32,
    pub experience: f32,
}

impl Default for CombatStatsData {
    fn default() -> Self {
        Self {
            attack_power: 10.0,
            defense: 5.0,
            armor: 0.0,
            armor_penetration: 0.0,
            critical_chance: 0.05,
            critical_damage: 1.5,
            attack_speed: 1.0,
            movement_speed_bonus: 0.0,
            damage_reduction: 0.0,
            lifesteal: 0.0,
            level: 1,
            experience: 0.0,
        }
    }
}

pub static COMBAT_STATS: ComponentDefinition = ComponentDefinition {
    type_id: "combat_stats",
    display_name: "Combat Stats",
    category: ComponentCategory::Gameplay,
    icon: SKULL,
    priority: 36,
    add_fn: add_combat_stats,
    remove_fn: remove_combat_stats,
    has_fn: has_combat_stats,
    serialize_fn: serialize_combat_stats,
    deserialize_fn: deserialize_combat_stats,
    inspector_fn: inspect_combat_stats,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Healing Zone Component - Area healing
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct HealingZoneData {
    pub heal_amount: f32,
    pub heal_interval: f32,
    pub radius: f32,
    pub affects_tags: String,
    pub max_targets: i32,
    pub duration: f32,
    pub heal_effect: String,
}

impl Default for HealingZoneData {
    fn default() -> Self {
        Self {
            heal_amount: 10.0,
            heal_interval: 1.0,
            radius: 3.0,
            affects_tags: "Player,Ally".to_string(),
            max_targets: 0,
            duration: 0.0,
            heal_effect: String::new(),
        }
    }
}

pub static HEALING_ZONE: ComponentDefinition = ComponentDefinition {
    type_id: "healing_zone",
    display_name: "Healing Zone",
    category: ComponentCategory::Gameplay,
    icon: FIRST_AID,
    priority: 37,
    add_fn: add_healing_zone,
    remove_fn: remove_healing_zone,
    has_fn: has_healing_zone,
    serialize_fn: serialize_healing_zone,
    deserialize_fn: deserialize_healing_zone,
    inspector_fn: inspect_healing_zone,
    conflicts_with: &[],
    requires: &[],
};

/// Register all combat components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&WEAPON);
    registry.register(&PROJECTILE);
    registry.register(&DAMAGE_ZONE);
    registry.register(&SHIELD_COMPONENT);
    registry.register(&STATUS_EFFECT);
    registry.register(&EXPLOSION);
    registry.register(&COMBAT_STATS);
    registry.register(&HEALING_ZONE);
}

// ============================================================================
// Weapon Implementation
// ============================================================================

fn add_weapon(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(WeaponData::default());
}

fn remove_weapon(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<WeaponData>();
}

fn has_weapon(world: &World, entity: Entity) -> bool {
    world.get::<WeaponData>(entity).is_some()
}

fn serialize_weapon(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<WeaponData>(entity)?;
    Some(json!({
        "weapon_type": data.weapon_type,
        "damage": data.damage,
        "attack_rate": data.attack_rate,
        "range": data.range,
        "knockback": data.knockback,
        "critical_chance": data.critical_chance,
        "critical_multiplier": data.critical_multiplier,
        "ammo_type": data.ammo_type,
        "ammo_per_shot": data.ammo_per_shot,
        "magazine_size": data.magazine_size,
        "reload_time": data.reload_time,
        "spread": data.spread,
        "projectile_prefab": data.projectile_prefab,
        "projectile_speed": data.projectile_speed,
        "muzzle_offset": data.muzzle_offset,
        "attack_sound": data.attack_sound,
    }))
}

fn deserialize_weapon(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let weapon_data = WeaponData {
        weapon_type: data.get("weapon_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        damage: data.get("damage").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        attack_rate: data.get("attack_rate").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        range: data.get("range").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        knockback: data.get("knockback").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        critical_chance: data.get("critical_chance").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32,
        critical_multiplier: data.get("critical_multiplier").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        ammo_type: data.get("ammo_type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        ammo_per_shot: data.get("ammo_per_shot").and_then(|v| v.as_i64()).unwrap_or(1) as i32,
        magazine_size: data.get("magazine_size").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        reload_time: data.get("reload_time").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        spread: data.get("spread").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        projectile_prefab: data.get("projectile_prefab").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        projectile_speed: data.get("projectile_speed").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32,
        muzzle_offset: data.get("muzzle_offset").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 0.0, 1.0]),
        attack_sound: data.get("attack_sound").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    };
    entity_commands.insert(weapon_data);
}

fn inspect_weapon(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<WeaponData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Type:");
            egui::ComboBox::from_id_salt("weapon_type")
                .selected_text(match data.weapon_type {
                    WeaponType::Melee => "Melee",
                    WeaponType::Ranged => "Ranged",
                    WeaponType::Throwable => "Throwable",
                    WeaponType::Beam => "Beam",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.weapon_type == WeaponType::Melee, "Melee").clicked() { data.weapon_type = WeaponType::Melee; changed = true; }
                    if ui.selectable_label(data.weapon_type == WeaponType::Ranged, "Ranged").clicked() { data.weapon_type = WeaponType::Ranged; changed = true; }
                    if ui.selectable_label(data.weapon_type == WeaponType::Throwable, "Throwable").clicked() { data.weapon_type = WeaponType::Throwable; changed = true; }
                    if ui.selectable_label(data.weapon_type == WeaponType::Beam, "Beam").clicked() { data.weapon_type = WeaponType::Beam; changed = true; }
                });
        });

        ui.separator();
        ui.label("Damage");

        ui.horizontal(|ui| {
            ui.label("Base Damage:");
            if ui.add(egui::DragValue::new(&mut data.damage).speed(0.5).range(0.0..=1000.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Attack Rate:");
            if ui.add(egui::DragValue::new(&mut data.attack_rate).speed(0.1).range(0.1..=20.0).suffix("/s")).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Range:");
            if ui.add(egui::DragValue::new(&mut data.range).speed(0.1).range(0.5..=100.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Knockback:");
            if ui.add(egui::DragValue::new(&mut data.knockback).speed(0.1).range(0.0..=50.0)).changed() { changed = true; }
        });

        ui.separator();
        ui.label("Critical");

        ui.horizontal(|ui| {
            ui.label("Crit Chance:");
            if ui.add(egui::Slider::new(&mut data.critical_chance, 0.0..=1.0).show_value(true)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Crit Multiplier:");
            if ui.add(egui::DragValue::new(&mut data.critical_multiplier).speed(0.1).range(1.0..=10.0).suffix("x")).changed() { changed = true; }
        });

        if matches!(data.weapon_type, WeaponType::Ranged | WeaponType::Throwable) {
            ui.separator();
            ui.label("Ammo");

            ui.horizontal(|ui| {
                ui.label("Ammo Type:");
                if ui.text_edit_singleline(&mut data.ammo_type).changed() { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Per Shot:");
                if ui.add(egui::DragValue::new(&mut data.ammo_per_shot).speed(1.0).range(1..=10)).changed() { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Magazine:");
                if ui.add(egui::DragValue::new(&mut data.magazine_size).speed(1.0).range(0..=999)).changed() { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Reload Time:");
                if ui.add(egui::DragValue::new(&mut data.reload_time).speed(0.1).range(0.1..=10.0).suffix("s")).changed() { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Spread:");
                if ui.add(egui::DragValue::new(&mut data.spread).speed(0.1).range(0.0..=45.0).suffix("°")).changed() { changed = true; }
            });

            ui.separator();
            ui.label("Projectile");

            ui.horizontal(|ui| {
                ui.label("Prefab:");
                if ui.text_edit_singleline(&mut data.projectile_prefab).changed() { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Speed:");
                if ui.add(egui::DragValue::new(&mut data.projectile_speed).speed(1.0).range(1.0..=500.0)).changed() { changed = true; }
            });
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Sound:");
            if ui.text_edit_singleline(&mut data.attack_sound).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Projectile Implementation
// ============================================================================

fn add_projectile(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(ProjectileData::default());
}

fn remove_projectile(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<ProjectileData>();
}

fn has_projectile(world: &World, entity: Entity) -> bool {
    world.get::<ProjectileData>(entity).is_some()
}

fn serialize_projectile(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<ProjectileData>(entity)?;
    Some(json!({
        "projectile_type": data.projectile_type,
        "damage": data.damage,
        "speed": data.speed,
        "lifetime": data.lifetime,
        "gravity_scale": data.gravity_scale,
        "pierce_count": data.pierce_count,
        "homing": data.homing,
        "homing_strength": data.homing_strength,
        "explosion_radius": data.explosion_radius,
        "explosion_damage": data.explosion_damage,
        "destroy_on_hit": data.destroy_on_hit,
        "hit_effect_prefab": data.hit_effect_prefab,
        "trail_effect": data.trail_effect,
    }))
}

fn deserialize_projectile(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let proj_data = ProjectileData {
        projectile_type: data.get("projectile_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        damage: data.get("damage").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        speed: data.get("speed").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32,
        lifetime: data.get("lifetime").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        gravity_scale: data.get("gravity_scale").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        pierce_count: data.get("pierce_count").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        homing: data.get("homing").and_then(|v| v.as_bool()).unwrap_or(false),
        homing_strength: data.get("homing_strength").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        explosion_radius: data.get("explosion_radius").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        explosion_damage: data.get("explosion_damage").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        destroy_on_hit: data.get("destroy_on_hit").and_then(|v| v.as_bool()).unwrap_or(true),
        hit_effect_prefab: data.get("hit_effect_prefab").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        trail_effect: data.get("trail_effect").and_then(|v| v.as_bool()).unwrap_or(false),
    };
    entity_commands.insert(proj_data);
}

fn inspect_projectile(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<ProjectileData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Type:");
            egui::ComboBox::from_id_salt("projectile_type")
                .selected_text(match data.projectile_type {
                    ProjectileType::Bullet => "Bullet",
                    ProjectileType::Arrow => "Arrow",
                    ProjectileType::Rocket => "Rocket",
                    ProjectileType::Grenade => "Grenade",
                    ProjectileType::Laser => "Laser",
                    ProjectileType::Custom => "Custom",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.projectile_type == ProjectileType::Bullet, "Bullet").clicked() { data.projectile_type = ProjectileType::Bullet; changed = true; }
                    if ui.selectable_label(data.projectile_type == ProjectileType::Arrow, "Arrow").clicked() { data.projectile_type = ProjectileType::Arrow; changed = true; }
                    if ui.selectable_label(data.projectile_type == ProjectileType::Rocket, "Rocket").clicked() { data.projectile_type = ProjectileType::Rocket; changed = true; }
                    if ui.selectable_label(data.projectile_type == ProjectileType::Grenade, "Grenade").clicked() { data.projectile_type = ProjectileType::Grenade; changed = true; }
                    if ui.selectable_label(data.projectile_type == ProjectileType::Laser, "Laser").clicked() { data.projectile_type = ProjectileType::Laser; changed = true; }
                    if ui.selectable_label(data.projectile_type == ProjectileType::Custom, "Custom").clicked() { data.projectile_type = ProjectileType::Custom; changed = true; }
                });
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Damage:");
            if ui.add(egui::DragValue::new(&mut data.damage).speed(0.5).range(0.0..=1000.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Speed:");
            if ui.add(egui::DragValue::new(&mut data.speed).speed(1.0).range(1.0..=500.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Lifetime:");
            if ui.add(egui::DragValue::new(&mut data.lifetime).speed(0.1).range(0.1..=60.0).suffix("s")).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Gravity:");
            if ui.add(egui::DragValue::new(&mut data.gravity_scale).speed(0.1).range(0.0..=5.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Pierce Count:");
            if ui.add(egui::DragValue::new(&mut data.pierce_count).speed(1.0).range(0..=10)).changed() { changed = true; }
        });

        ui.separator();

        if ui.checkbox(&mut data.homing, "Homing").changed() { changed = true; }
        if data.homing {
            ui.horizontal(|ui| {
                ui.label("Homing Strength:");
                if ui.add(egui::DragValue::new(&mut data.homing_strength).speed(0.1).range(0.1..=20.0)).changed() { changed = true; }
            });
        }

        ui.separator();
        ui.label("Explosion");

        ui.horizontal(|ui| {
            ui.label("Radius:");
            if ui.add(egui::DragValue::new(&mut data.explosion_radius).speed(0.1).range(0.0..=50.0)).changed() { changed = true; }
        });

        if data.explosion_radius > 0.0 {
            ui.horizontal(|ui| {
                ui.label("Damage:");
                if ui.add(egui::DragValue::new(&mut data.explosion_damage).speed(0.5).range(0.0..=500.0)).changed() { changed = true; }
            });
        }

        ui.separator();

        if ui.checkbox(&mut data.destroy_on_hit, "Destroy on Hit").changed() { changed = true; }
        if ui.checkbox(&mut data.trail_effect, "Trail Effect").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Damage Zone Implementation
// ============================================================================

fn add_damage_zone(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(DamageZoneData::default());
}

fn remove_damage_zone(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<DamageZoneData>();
}

fn has_damage_zone(world: &World, entity: Entity) -> bool {
    world.get::<DamageZoneData>(entity).is_some()
}

fn serialize_damage_zone(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<DamageZoneData>(entity)?;
    Some(json!({
        "shape": data.shape,
        "size": data.size,
        "radius": data.radius,
        "damage": data.damage,
        "damage_type": data.damage_type,
        "damage_interval": data.damage_interval,
        "knockback": data.knockback,
        "knockback_origin_center": data.knockback_origin_center,
        "one_shot": data.one_shot,
        "affects_tags": data.affects_tags,
        "ignore_tags": data.ignore_tags,
        "apply_status_effect": data.apply_status_effect,
        "status_duration": data.status_duration,
        "active": data.active,
    }))
}

fn deserialize_damage_zone(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let zone_data = DamageZoneData {
        shape: data.get("shape").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        size: data.get("size").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([2.0, 2.0, 2.0]),
        radius: data.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        damage: data.get("damage").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        damage_type: data.get("damage_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        damage_interval: data.get("damage_interval").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        knockback: data.get("knockback").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        knockback_origin_center: data.get("knockback_origin_center").and_then(|v| v.as_bool()).unwrap_or(true),
        one_shot: data.get("one_shot").and_then(|v| v.as_bool()).unwrap_or(false),
        affects_tags: data.get("affects_tags").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        ignore_tags: data.get("ignore_tags").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        apply_status_effect: data.get("apply_status_effect").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        status_duration: data.get("status_duration").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        active: data.get("active").and_then(|v| v.as_bool()).unwrap_or(true),
    };
    entity_commands.insert(zone_data);
}

fn inspect_damage_zone(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<DamageZoneData>(entity) {
        if ui.checkbox(&mut data.active, "Active").changed() { changed = true; }

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Shape:");
            egui::ComboBox::from_id_salt("damage_zone_shape")
                .selected_text(match data.shape {
                    DamageZoneShape::Box => "Box",
                    DamageZoneShape::Sphere => "Sphere",
                    DamageZoneShape::Capsule => "Capsule",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.shape == DamageZoneShape::Box, "Box").clicked() { data.shape = DamageZoneShape::Box; changed = true; }
                    if ui.selectable_label(data.shape == DamageZoneShape::Sphere, "Sphere").clicked() { data.shape = DamageZoneShape::Sphere; changed = true; }
                    if ui.selectable_label(data.shape == DamageZoneShape::Capsule, "Capsule").clicked() { data.shape = DamageZoneShape::Capsule; changed = true; }
                });
        });

        match data.shape {
            DamageZoneShape::Box => {
                ui.label("Size:");
                ui.horizontal(|ui| {
                    if ui.add(egui::DragValue::new(&mut data.size[0]).speed(0.1).prefix("X: ")).changed() { changed = true; }
                    if ui.add(egui::DragValue::new(&mut data.size[1]).speed(0.1).prefix("Y: ")).changed() { changed = true; }
                    if ui.add(egui::DragValue::new(&mut data.size[2]).speed(0.1).prefix("Z: ")).changed() { changed = true; }
                });
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(&mut data.radius).speed(0.1).range(0.1..=50.0)).changed() { changed = true; }
                });
            }
        }

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Damage:");
            if ui.add(egui::DragValue::new(&mut data.damage).speed(0.5).range(0.0..=1000.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Type:");
            egui::ComboBox::from_id_salt("damage_type")
                .selected_text(match data.damage_type {
                    DamageType::Physical => "Physical",
                    DamageType::Fire => "Fire",
                    DamageType::Ice => "Ice",
                    DamageType::Electric => "Electric",
                    DamageType::Poison => "Poison",
                    DamageType::Magic => "Magic",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(data.damage_type == DamageType::Physical, "Physical").clicked() { data.damage_type = DamageType::Physical; changed = true; }
                    if ui.selectable_label(data.damage_type == DamageType::Fire, "Fire").clicked() { data.damage_type = DamageType::Fire; changed = true; }
                    if ui.selectable_label(data.damage_type == DamageType::Ice, "Ice").clicked() { data.damage_type = DamageType::Ice; changed = true; }
                    if ui.selectable_label(data.damage_type == DamageType::Electric, "Electric").clicked() { data.damage_type = DamageType::Electric; changed = true; }
                    if ui.selectable_label(data.damage_type == DamageType::Poison, "Poison").clicked() { data.damage_type = DamageType::Poison; changed = true; }
                    if ui.selectable_label(data.damage_type == DamageType::Magic, "Magic").clicked() { data.damage_type = DamageType::Magic; changed = true; }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Interval:");
            if ui.add(egui::DragValue::new(&mut data.damage_interval).speed(0.1).range(0.1..=10.0).suffix("s")).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.one_shot, "One Shot (single hit)").changed() { changed = true; }

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Knockback:");
            if ui.add(egui::DragValue::new(&mut data.knockback).speed(0.1).range(0.0..=50.0)).changed() { changed = true; }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Affects Tags:");
            if ui.text_edit_singleline(&mut data.affects_tags).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Ignore Tags:");
            if ui.text_edit_singleline(&mut data.ignore_tags).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Shield Implementation
// ============================================================================

fn add_shield(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(ShieldData::default());
}

fn remove_shield(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<ShieldData>();
}

fn has_shield(world: &World, entity: Entity) -> bool {
    world.get::<ShieldData>(entity).is_some()
}

fn serialize_shield(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<ShieldData>(entity)?;
    Some(json!({
        "max_shield": data.max_shield,
        "current_shield": data.current_shield,
        "recharge_rate": data.recharge_rate,
        "recharge_delay": data.recharge_delay,
        "absorb_percent": data.absorb_percent,
        "break_threshold": data.break_threshold,
        "break_cooldown": data.break_cooldown,
    }))
}

fn deserialize_shield(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let shield_data = ShieldData {
        max_shield: data.get("max_shield").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32,
        current_shield: data.get("current_shield").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32,
        recharge_rate: data.get("recharge_rate").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        recharge_delay: data.get("recharge_delay").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
        absorb_percent: data.get("absorb_percent").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        blocks_damage_types: vec![DamageType::Physical],
        break_threshold: data.get("break_threshold").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        break_cooldown: data.get("break_cooldown").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
    };
    entity_commands.insert(shield_data);
}

fn inspect_shield(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<ShieldData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Max Shield:");
            if ui.add(egui::DragValue::new(&mut data.max_shield).speed(1.0).range(1.0..=1000.0)).changed() { changed = true; }
        });

        let max = data.max_shield;
        ui.horizontal(|ui| {
            ui.label("Current:");
            if ui.add(egui::DragValue::new(&mut data.current_shield).speed(1.0).range(0.0..=max)).changed() { changed = true; }
        });

        let pct = data.current_shield / data.max_shield;
        ui.add(egui::ProgressBar::new(pct).fill(egui::Color32::from_rgb(100, 150, 255)));

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Absorb %:");
            if ui.add(egui::Slider::new(&mut data.absorb_percent, 0.0..=1.0).show_value(true)).changed() { changed = true; }
        });

        ui.separator();
        ui.label("Recharge");

        ui.horizontal(|ui| {
            ui.label("Rate:");
            if ui.add(egui::DragValue::new(&mut data.recharge_rate).speed(0.1).range(0.0..=100.0).suffix("/s")).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Delay:");
            if ui.add(egui::DragValue::new(&mut data.recharge_delay).speed(0.1).range(0.0..=30.0).suffix("s")).changed() { changed = true; }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Break Threshold:");
            if ui.add(egui::DragValue::new(&mut data.break_threshold).speed(1.0).range(0.0..=500.0)).changed() { changed = true; }
        });

        ui.horizontal(|ui| {
            ui.label("Break Cooldown:");
            if ui.add(egui::DragValue::new(&mut data.break_cooldown).speed(0.1).range(0.0..=60.0).suffix("s")).changed() { changed = true; }
        });
    }
    changed
}

// ============================================================================
// Status Effect, Explosion, Combat Stats, Healing Zone - Simplified implementations
// ============================================================================

fn add_status_effect(commands: &mut Commands, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) { commands.entity(entity).insert(StatusEffectData::default()); }
fn remove_status_effect(commands: &mut Commands, entity: Entity) { commands.entity(entity).remove::<StatusEffectData>(); }
fn has_status_effect(world: &World, entity: Entity) -> bool { world.get::<StatusEffectData>(entity).is_some() }
fn serialize_status_effect(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let d = world.get::<StatusEffectData>(entity)?;
    Some(json!({"effect_id": d.effect_id, "effect_type": d.effect_type, "duration": d.duration, "tick_rate": d.tick_rate, "value": d.value, "stacks": d.stacks, "max_stacks": d.max_stacks}))
}
fn deserialize_status_effect(ec: &mut EntityCommands, data: &serde_json::Value, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) {
    ec.insert(StatusEffectData {
        effect_id: data.get("effect_id").and_then(|v| v.as_str()).unwrap_or("burning").to_string(),
        effect_type: data.get("effect_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        duration: data.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        tick_rate: data.get("tick_rate").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        value: data.get("value").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        stacks: data.get("stacks").and_then(|v| v.as_i64()).unwrap_or(1) as i32,
        max_stacks: data.get("max_stacks").and_then(|v| v.as_i64()).unwrap_or(5) as i32,
        ..Default::default()
    });
}
fn inspect_status_effect(ui: &mut egui::Ui, world: &mut World, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut d) = world.get_mut::<StatusEffectData>(entity) {
        ui.horizontal(|ui| { ui.label("Effect ID:"); if ui.text_edit_singleline(&mut d.effect_id).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Duration:"); if ui.add(egui::DragValue::new(&mut d.duration).speed(0.1)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Value:"); if ui.add(egui::DragValue::new(&mut d.value).speed(0.1)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Tick Rate:"); if ui.add(egui::DragValue::new(&mut d.tick_rate).speed(0.1)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Max Stacks:"); if ui.add(egui::DragValue::new(&mut d.max_stacks).speed(1.0)).changed() { changed = true; } });
    }
    changed
}

fn add_explosion(commands: &mut Commands, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) { commands.entity(entity).insert(ExplosionData::default()); }
fn remove_explosion(commands: &mut Commands, entity: Entity) { commands.entity(entity).remove::<ExplosionData>(); }
fn has_explosion(world: &World, entity: Entity) -> bool { world.get::<ExplosionData>(entity).is_some() }
fn serialize_explosion(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let d = world.get::<ExplosionData>(entity)?;
    Some(json!({"radius": d.radius, "damage": d.damage, "knockback_force": d.knockback_force, "destroy_self": d.destroy_self, "fuse_time": d.fuse_time}))
}
fn deserialize_explosion(ec: &mut EntityCommands, data: &serde_json::Value, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) {
    ec.insert(ExplosionData {
        radius: data.get("radius").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        damage: data.get("damage").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32,
        knockback_force: data.get("knockback_force").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        destroy_self: data.get("destroy_self").and_then(|v| v.as_bool()).unwrap_or(true),
        fuse_time: data.get("fuse_time").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        ..Default::default()
    });
}
fn inspect_explosion(ui: &mut egui::Ui, world: &mut World, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut d) = world.get_mut::<ExplosionData>(entity) {
        ui.horizontal(|ui| { ui.label("Radius:"); if ui.add(egui::DragValue::new(&mut d.radius).speed(0.1)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Damage:"); if ui.add(egui::DragValue::new(&mut d.damage).speed(0.5)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Knockback:"); if ui.add(egui::DragValue::new(&mut d.knockback_force).speed(0.1)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Fuse Time:"); if ui.add(egui::DragValue::new(&mut d.fuse_time).speed(0.1).suffix("s")).changed() { changed = true; } });
        if ui.checkbox(&mut d.destroy_self, "Destroy Self").changed() { changed = true; }
        if ui.checkbox(&mut d.damage_falloff, "Damage Falloff").changed() { changed = true; }
        if ui.checkbox(&mut d.chain_explode, "Chain Explode").changed() { changed = true; }
    }
    changed
}

fn add_combat_stats(commands: &mut Commands, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) { commands.entity(entity).insert(CombatStatsData::default()); }
fn remove_combat_stats(commands: &mut Commands, entity: Entity) { commands.entity(entity).remove::<CombatStatsData>(); }
fn has_combat_stats(world: &World, entity: Entity) -> bool { world.get::<CombatStatsData>(entity).is_some() }
fn serialize_combat_stats(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let d = world.get::<CombatStatsData>(entity)?;
    Some(json!({"attack_power": d.attack_power, "defense": d.defense, "armor": d.armor, "critical_chance": d.critical_chance, "critical_damage": d.critical_damage, "level": d.level}))
}
fn deserialize_combat_stats(ec: &mut EntityCommands, data: &serde_json::Value, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) {
    ec.insert(CombatStatsData {
        attack_power: data.get("attack_power").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        defense: data.get("defense").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
        armor: data.get("armor").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        critical_chance: data.get("critical_chance").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32,
        critical_damage: data.get("critical_damage").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32,
        level: data.get("level").and_then(|v| v.as_i64()).unwrap_or(1) as i32,
        ..Default::default()
    });
}
fn inspect_combat_stats(ui: &mut egui::Ui, world: &mut World, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut d) = world.get_mut::<CombatStatsData>(entity) {
        ui.horizontal(|ui| { ui.label("Level:"); if ui.add(egui::DragValue::new(&mut d.level).speed(1.0).range(1..=100)).changed() { changed = true; } });
        ui.separator();
        ui.horizontal(|ui| { ui.label("Attack Power:"); if ui.add(egui::DragValue::new(&mut d.attack_power).speed(0.5)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Defense:"); if ui.add(egui::DragValue::new(&mut d.defense).speed(0.5)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Armor:"); if ui.add(egui::DragValue::new(&mut d.armor).speed(0.5)).changed() { changed = true; } });
        ui.separator();
        ui.horizontal(|ui| { ui.label("Crit Chance:"); if ui.add(egui::Slider::new(&mut d.critical_chance, 0.0..=1.0)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Crit Damage:"); if ui.add(egui::DragValue::new(&mut d.critical_damage).speed(0.1).suffix("x")).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Attack Speed:"); if ui.add(egui::DragValue::new(&mut d.attack_speed).speed(0.1)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Lifesteal:"); if ui.add(egui::Slider::new(&mut d.lifesteal, 0.0..=1.0)).changed() { changed = true; } });
    }
    changed
}

fn add_healing_zone(commands: &mut Commands, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) { commands.entity(entity).insert(HealingZoneData::default()); }
fn remove_healing_zone(commands: &mut Commands, entity: Entity) { commands.entity(entity).remove::<HealingZoneData>(); }
fn has_healing_zone(world: &World, entity: Entity) -> bool { world.get::<HealingZoneData>(entity).is_some() }
fn serialize_healing_zone(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let d = world.get::<HealingZoneData>(entity)?;
    Some(json!({"heal_amount": d.heal_amount, "heal_interval": d.heal_interval, "radius": d.radius, "affects_tags": d.affects_tags}))
}
fn deserialize_healing_zone(ec: &mut EntityCommands, data: &serde_json::Value, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) {
    ec.insert(HealingZoneData {
        heal_amount: data.get("heal_amount").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32,
        heal_interval: data.get("heal_interval").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        radius: data.get("radius").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
        affects_tags: data.get("affects_tags").and_then(|v| v.as_str()).unwrap_or("Player,Ally").to_string(),
        ..Default::default()
    });
}
fn inspect_healing_zone(ui: &mut egui::Ui, world: &mut World, entity: Entity, _: &mut Assets<Mesh>, _: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;
    if let Some(mut d) = world.get_mut::<HealingZoneData>(entity) {
        ui.horizontal(|ui| { ui.label("Heal Amount:"); if ui.add(egui::DragValue::new(&mut d.heal_amount).speed(0.5)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Interval:"); if ui.add(egui::DragValue::new(&mut d.heal_interval).speed(0.1).suffix("s")).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Radius:"); if ui.add(egui::DragValue::new(&mut d.radius).speed(0.1)).changed() { changed = true; } });
        ui.horizontal(|ui| { ui.label("Affects Tags:"); if ui.text_edit_singleline(&mut d.affects_tags).changed() { changed = true; } });
    }
    changed
}
