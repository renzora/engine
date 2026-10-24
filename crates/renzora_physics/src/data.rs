//! The serializable physics components.
//!
//! They live in the contract crate (`renzora::physics`) so a backend plugin can
//! name them without linking this crate. Re-exported here so every existing
//! `renzora_physics::PhysicsBodyData` and `renzora_physics::data::…` path still
//! resolves; the reflected type paths, which scenes store, are unchanged too.

pub use renzora::physics::{
    CollisionShapeData, CollisionShapeType, Physics2d, PhysicsBodyData, PhysicsBodyType,
    RuntimeOnlyPhysics, RuntimePhysics, RuntimePhysics2d,
};
