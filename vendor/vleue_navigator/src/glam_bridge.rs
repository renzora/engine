//! RENZORA: conversion between Bevy's glam and the one polyanya is built on.
//!
//! Bevy moved to glam 0.34 while polyanya 0.18 (the latest release) is still on
//! 0.33, so a Bevy `Vec2` and a polyanya `Vec2` are different types with the same
//! layout. Every value crossing into or out of polyanya goes through these.
//! Remove this module once polyanya moves to Bevy's glam.

use bevy::math::{Vec2, Vec3};

/// A polyanya 2D vector.
pub type PVec2 = polyanya_glam::Vec2;
/// A polyanya 3D vector.
pub type PVec3 = polyanya_glam::Vec3;

/// Bevy's vector as polyanya's.
pub fn to_p(v: Vec2) -> PVec2 {
    PVec2::new(v.x, v.y)
}

/// Polyanya's vector as Bevy's.
pub fn from_p(v: PVec2) -> Vec2 {
    Vec2::new(v.x, v.y)
}

/// Polyanya's 3D vector as Bevy's.
pub fn from_p3(v: PVec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Bevy's 3D vector as polyanya's.
pub fn to_p3(v: Vec3) -> PVec3 {
    PVec3::new(v.x, v.y, v.z)
}
