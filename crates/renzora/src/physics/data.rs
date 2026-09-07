//! What a body and a collider are, as the scene stores them.
//!
//! Backend-agnostic by design. These are what a `.scene` round-trips and what
//! the Inspector edits; a backend system turns them into avian components at
//! runtime and never the other way round. That indirection is what lets the
//! simulation be swapped without rewriting every scene, and it is why the
//! authored types are here while the avian ones are not.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Type of physics body
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum PhysicsBodyType {
    #[default]
    RigidBody,
    StaticBody,
    KinematicBody,
}

/// Serializable physics body data — backend-agnostic.
///
/// This component stores physics properties in a format that can be saved to scenes.
/// At runtime (or in play mode), a backend system converts this into the actual
/// physics engine components (Avian).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct PhysicsBodyData {
    pub body_type: PhysicsBodyType,
    pub mass: f32,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub lock_rotation_x: bool,
    pub lock_rotation_y: bool,
    pub lock_rotation_z: bool,
    pub lock_translation_x: bool,
    pub lock_translation_y: bool,
    pub lock_translation_z: bool,
}

impl Default for PhysicsBodyData {
    fn default() -> Self {
        Self {
            body_type: PhysicsBodyType::RigidBody,
            mass: 1.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.05,
            lock_rotation_x: false,
            lock_rotation_y: false,
            lock_rotation_z: false,
            lock_translation_x: false,
            lock_translation_y: false,
            lock_translation_z: false,
        }
    }
}

impl PhysicsBodyData {
    pub fn static_body() -> Self {
        Self {
            body_type: PhysicsBodyType::StaticBody,
            ..Default::default()
        }
    }

    pub fn kinematic_body() -> Self {
        Self {
            body_type: PhysicsBodyType::KinematicBody,
            ..Default::default()
        }
    }
}

/// Type of collision shape
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum CollisionShapeType {
    #[default]
    Box,
    Sphere,
    Capsule,
    Cylinder,
    /// Trimesh generated from the entity's `Mesh3d`. No size fields apply.
    Mesh,
}

/// Serializable collision shape data — backend-agnostic.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct CollisionShapeData {
    pub shape_type: CollisionShapeType,
    #[serde(default)]
    #[reflect(default)]
    pub offset: Vec3,
    pub half_extents: Vec3,
    pub radius: f32,
    pub half_height: f32,
    pub friction: f32,
    pub restitution: f32,
    pub is_sensor: bool,
}

impl Default for CollisionShapeData {
    fn default() -> Self {
        Self {
            shape_type: CollisionShapeType::Box,
            offset: Vec3::ZERO,
            half_extents: Vec3::splat(0.5),
            radius: 0.5,
            half_height: 0.5,
            friction: 0.5,
            restitution: 0.0,
            is_sensor: false,
        }
    }
}

impl CollisionShapeData {
    pub fn sphere(radius: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Sphere,
            radius,
            ..Default::default()
        }
    }

    pub fn capsule(radius: f32, half_height: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Capsule,
            radius,
            half_height,
            ..Default::default()
        }
    }

    pub fn cylinder(radius: f32, half_height: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Cylinder,
            radius,
            half_height,
            ..Default::default()
        }
    }

    pub fn mesh() -> Self {
        Self {
            shape_type: CollisionShapeType::Mesh,
            ..Default::default()
        }
    }

    pub fn cuboid(half_extents: Vec3) -> Self {
        Self {
            shape_type: CollisionShapeType::Box,
            half_extents,
            ..Default::default()
        }
    }

    pub fn world_center(&self, global_transform: &GlobalTransform) -> Vec3 {
        global_transform.transform_point(self.offset)
    }
}

/// Marker component to track entities that have runtime physics components spawned.
#[derive(Component)]
pub struct RuntimePhysics;

/// Runtime marker: this entity's physics components were spawned by the
/// **avian2d** backend. `sync_physics_data`, the script-action observer and the
/// read-state updaters use it to keep talking to the right simulation — the
/// ancestry walk that decided the dimension only happens once, at init.
#[derive(Component)]
pub struct RuntimePhysics2d;

/// Serializable opt-in: force this entity onto the **2D** physics backend.
///
/// `auto_init_physics` already routes sprites and anything under a `Node2d` to
/// avian2d automatically; this marker covers 2D physics entities with no visual
/// of their own — e.g. the merged static colliders a tilemap layer generates,
/// or an invisible trigger area in a 2D scene.
#[derive(Component, Clone, Copy, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct Physics2d;

/// Opt-out: spawn this alongside a [`CollisionShapeData`] whose values are
/// already exact (e.g. the tilemap's merged tile colliders). Without it the
/// shape would be tagged for auto-fit and — having no render AABB to fit to —
/// sit in the retry query forever.
///
/// Here rather than beside the auto-fit pass that reads it because the crates
/// that need to *say* this are the ones generating colliders in bulk, and one
/// of them is now a plugin. The `PendingAutoFit` tag it suppresses stays in
/// `renzora_physics`: that one is set and cleared inside a single pass and
/// nothing outside could act on it.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct SkipAutoFit;
