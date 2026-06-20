/*
* Inspiration taken with great thanks from: https://github.com/dgreenheck/ez-tree
*/

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::enums::{LeafBillboard, TreeType};

// Fork note: upstream gated an extra `InspectorOptions` derive behind the
// `inspector` feature (bevy-inspector-egui). renzora has no egui, so the
// feature + that variant are dropped and we keep a single definition. serde
// derives are added so `Tree` (which embeds this) survives renzora's
// reflect-based scene (de)serialization via the `ReflectSerialize` path.
// Bevy 0.19: `Resource: Component`, so `#[derive(Resource)]` now also provides
// the `Component` impl — deriving `Component` too is a conflicting-impl error.
// We keep `Resource` (it gives us `Component` for free, so this stays usable as
// both a resource and a per-entity component) and still reflect both.
#[derive(Resource, Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[reflect(Resource, Component, Default, Serialize, Deserialize)]
pub struct TreeMeshSettings {
    pub tree_type: TreeType,
    pub branch: BranchParams,
    pub leaves: LeafParams,
}


impl Default for TreeMeshSettings {
    fn default() -> Self {
        Self {
            tree_type: TreeType::Deciduous,
            branch: BranchParams::default(),
            leaves: LeafParams::default(),
        }
    }
}


/**
 * All branches have a random angle to their parent branch/trunk.
 * This branch force controls a direction vector and an amount to lerp between the random direction and this vector by the given strength.
 * This can be used i.e. for trees that generally have branches that point in a specific direction (i.e. up:Aspen or down:Willow).
 */
#[derive(Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchForce {
    /// in which direction should all branches be pointed based on their radius (larger radius = smaller influence of this force)
    /// value will be normalized internally; no need to do it beforehand
    pub direction: Vec3,
    /// how strong this force is (1.0 -> the branch points in this direction); sensible values are below 1.0
    /// must be positive; negative values will be ignored
    pub strength: f32,
    /// starting at which branch radius should the force not have any effect
    /// default is 0.1 (a branch of a thickness of 20cm should not be bothered by outside forces)
    /// must be positive
    pub radius_cutoff: f32
}

impl Default for BranchForce {
    fn default() -> Self {
        Self {
            direction: Vec3 { x: 0.0, y: 1.0, z: 0.0 },
            strength: 0.05,
            radius_cutoff: 0.1,
        }
    }
}

/**
 * amount of recursion for branches (0 = only trunk, no branches)
 */
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BranchRecursionLevel {
    Zero = 0,
    One  = 1,
    Two  = 2,
    Three= 3,
    //Four = 4, // four recursion levels create way to small branches (polygons in the subpixel range)
}

impl TryFrom<u8> for BranchRecursionLevel {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(BranchRecursionLevel::Zero),
            1 => Ok(BranchRecursionLevel::One),
            2 => Ok(BranchRecursionLevel::Two),
            3 => Ok(BranchRecursionLevel::Three),
            //4 => Ok(BranchRecursionLevel::Four), // four recursion levels create way to small branches (polygons in the subpixel range)
            _ => Err(()),
        }
    }
}

impl From<BranchRecursionLevel> for u8 {
    fn from(z: BranchRecursionLevel) -> u8 { z as u8 }
}

impl From<BranchRecursionLevel> for usize {
    fn from(z: BranchRecursionLevel) -> usize { z as usize }
}

impl From<BranchRecursionLevel> for f32 {
    fn from(z: BranchRecursionLevel) -> f32 { z as usize as f32 }
}


#[derive(Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchParams {
    /// amount of recursion for branches (0 = only trunk, no branches)
    pub levels: BranchRecursionLevel,

    /// angle of child branch(es) to parent branch/trunk per level 0..3
    /// The first value is ignored (the trunk is always perpendicular to the ground)
    pub angle: [f32; 4],

    /// amount of children per level 0..3 (0 = children of trunk)
    pub children: [u8; 3],

    /// Control the general direction of branches
    pub force: BranchForce,

    /// curling/twisting per level (0=straight; 1=very crooked; values higher than 1 can work, but may create unrealistic branches)
    pub gnarliness: [f32; 4],

    /// length per level
    pub length: [f32; 4],

    /// radius of the trunk (at the base; taper reduces the trunk's radius at the top)
    pub trunk_base_radius: f32,

    /// radius factor per level (how much smaller/larger the radius of a branch in relation to the parent branch)
    /// The first value is ignored (the trunk's radius is given by trunk_base_radius)
    pub radius_factor: [f32; 4],

    /// how many sections each brach has per level (along its length; more sections = more polygons)
    ///
    /// hint: as textures are repeated (one full uv-range per section), it can be beneficial to play around with this value to influence how often the given texture repeats on this branch to better fit the texture size
    ///
    /// Additionnaly take a look at ['bevy::pbr::StandardMaterial::uv_transform']
    pub sections: [u8; 4],

    /// how many segments each branch has per section per level (how 'round' the mesh is; more segments = more polygons)
    pub segments: [u8; 4],

    /// when to start adding child branches along the length of the branch (0..1) per level
    /// The first value is ignored (the trunk is always starting at the ground level)
    pub start: [f32; 4],

    /// taper per level (how fast the branch gets thinner until the end; clamped between 0.0 and 0.9999)
    pub taper: [f32; 4],

    /// twist per Level
    pub twist: [f32; 4],
}

impl Default for BranchParams {
    fn default() -> Self {
        Self {
            levels: BranchRecursionLevel::Three,
            angle: [0.0, 39.0, 39.0, 59.0],
            children: [7, 4, 10],
            force: BranchForce::default(),
            gnarliness: [-0.05, 0.20, 0.16, 0.05],
            length: [4.5, 2.9, 1.5, 0.45],
            trunk_base_radius: 0.2,
            radius_factor: [1.0, 0.5, 0.5, 0.5],
            sections: [12, 8, 6, 4],
            segments: [8, 6, 4, 3],
            start: [0.0, 0.32, 0.4, 0.0],
            taper: [0.95, 0.8, 0.85, 0.8],
            twist: [0.09, -0.07, 0.0, 0.0],
        }
    }
}

/**
 * Leaves are only added to the last level of branches.
 * Control how they look like and how they are positioned relative to the last level of branches (or on the trunk if levels = 0).
 */
#[derive(Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeafParams {
    /// single or double/perpendicular
    pub leaf_billboard: LeafBillboard,
    /// angle of leaves relative to parent branch/trunk in degrees
    pub angle: f32,
    /// amount of leaves
    pub count: u32,
    /// when leaves start relative to the length of the branch (0..1); will be clamped between 0.0 and 1.0
    pub start: f32,
    /// average size of leaves
    pub size: f32,
    /// variance of leaf sizes (negative values are ignored)
    ///
    /// internal formula for a single leaf is: (rng(-1.0..1.0) * size_variance + 1.0) * size
    pub size_variance: f32
}

impl Default for LeafParams {
    fn default() -> Self {
        Self {
            leaf_billboard: LeafBillboard::Double,
            angle: 35.0,
            count: 3,
            start: 0.25,
            size: 0.25,
            size_variance: 0.2,
        }
    }
}
