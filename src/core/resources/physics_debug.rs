//! Physics debug state resource for Avian3D diagnostics

use bevy::prelude::*;
use std::collections::VecDeque;

/// Maximum number of samples to keep for graphs
const MAX_SAMPLES: usize = 120;

/// Physics body type classification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PhysicsBodyType {
    #[default]
    Dynamic,
    Kinematic,
    Static,
}

/// Collider shape type for statistics
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColliderShapeType {
    Sphere,
    Box,
    Capsule,
    Cylinder,
    Cone,
    ConvexHull,
    TriMesh,
    HeightField,
    Compound,
    Unknown,
}

impl std::fmt::Display for ColliderShapeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColliderShapeType::Sphere => write!(f, "Sphere"),
            ColliderShapeType::Box => write!(f, "Box"),
            ColliderShapeType::Capsule => write!(f, "Capsule"),
            ColliderShapeType::Cylinder => write!(f, "Cylinder"),
            ColliderShapeType::Cone => write!(f, "Cone"),
            ColliderShapeType::ConvexHull => write!(f, "Convex Hull"),
            ColliderShapeType::TriMesh => write!(f, "Trimesh"),
            ColliderShapeType::HeightField => write!(f, "Heightfield"),
            ColliderShapeType::Compound => write!(f, "Compound"),
            ColliderShapeType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Debug rendering toggle options
#[derive(Clone, Debug, Default)]
pub struct PhysicsDebugToggles {
    /// Show collider wireframes
    pub show_colliders: bool,
    /// Show contact points
    pub show_contacts: bool,
    /// Show AABB bounding boxes
    pub show_aabbs: bool,
    /// Show velocity vectors
    pub show_velocities: bool,
    /// Show center of mass
    pub show_center_of_mass: bool,
    /// Show joint connections
    pub show_joints: bool,
}

/// Information about a collision pair
#[derive(Clone, Debug)]
pub struct CollisionPairInfo {
    /// First entity in the pair
    pub entity_a: Entity,
    /// Second entity in the pair
    pub entity_b: Entity,
    /// Number of contact points
    pub contact_count: usize,
}

/// Physics debug state for monitoring Avian3D simulation
#[derive(Resource)]
pub struct PhysicsDebugState {
    /// Whether physics simulation is running
    pub simulation_running: bool,
    /// Number of dynamic rigid bodies
    pub dynamic_body_count: usize,
    /// Number of kinematic rigid bodies
    pub kinematic_body_count: usize,
    /// Number of static rigid bodies
    pub static_body_count: usize,
    /// Total collider count
    pub collider_count: usize,
    /// Collider counts by shape type
    pub colliders_by_type: std::collections::HashMap<ColliderShapeType, usize>,
    /// Number of active collision pairs
    pub collision_pair_count: usize,
    /// Recent collision pairs (limited)
    pub collision_pairs: Vec<CollisionPairInfo>,
    /// Physics step time history (ms)
    pub step_time_history: VecDeque<f32>,
    /// Current physics step time (ms)
    pub step_time_ms: f32,
    /// Average step time
    pub avg_step_time_ms: f32,
    /// Debug visualization toggles
    pub debug_toggles: PhysicsDebugToggles,
    /// Whether to show collision pairs list (collapsible)
    pub show_collision_pairs: bool,
    /// Update interval in seconds
    pub update_interval: f32,
    /// Time since last update
    pub time_since_update: f32,
    /// Whether physics feature is enabled
    pub physics_available: bool,
}

impl Default for PhysicsDebugState {
    fn default() -> Self {
        Self {
            simulation_running: false,
            dynamic_body_count: 0,
            kinematic_body_count: 0,
            static_body_count: 0,
            collider_count: 0,
            colliders_by_type: std::collections::HashMap::new(),
            collision_pair_count: 0,
            collision_pairs: Vec::new(),
            step_time_history: VecDeque::with_capacity(MAX_SAMPLES),
            step_time_ms: 0.0,
            avg_step_time_ms: 0.0,
            debug_toggles: PhysicsDebugToggles::default(),
            show_collision_pairs: false,
            update_interval: 0.1,
            time_since_update: 0.0,
            #[cfg(feature = "physics")]
            physics_available: true,
            #[cfg(not(feature = "physics"))]
            physics_available: false,
        }
    }
}

impl PhysicsDebugState {
    /// Push a step time sample to history
    pub fn push_step_time(&mut self, time_ms: f32) {
        if self.step_time_history.len() >= MAX_SAMPLES {
            self.step_time_history.pop_front();
        }
        self.step_time_history.push_back(time_ms);

        // Update average
        if !self.step_time_history.is_empty() {
            self.avg_step_time_ms = self.step_time_history.iter().sum::<f32>()
                / self.step_time_history.len() as f32;
        }
    }

    /// Get total body count
    pub fn total_body_count(&self) -> usize {
        self.dynamic_body_count + self.kinematic_body_count + self.static_body_count
    }
}

/// System to update physics debug state
#[cfg(feature = "physics")]
pub fn update_physics_debug_state(
    mut state: ResMut<PhysicsDebugState>,
    time: Res<Time>,
    rigid_bodies: Query<&avian3d::prelude::RigidBody>,
    colliders: Query<&avian3d::prelude::Collider>,
) {
    state.time_since_update += time.delta_secs();

    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    // Check if physics is running - assume running if we have rigid bodies
    state.simulation_running = rigid_bodies.iter().next().is_some();

    // Count rigid bodies by type
    state.dynamic_body_count = 0;
    state.kinematic_body_count = 0;
    state.static_body_count = 0;

    for body in rigid_bodies.iter() {
        match body {
            avian3d::prelude::RigidBody::Dynamic => state.dynamic_body_count += 1,
            avian3d::prelude::RigidBody::Kinematic => state.kinematic_body_count += 1,
            avian3d::prelude::RigidBody::Static => state.static_body_count += 1,
        }
    }

    // Count colliders by type
    state.collider_count = 0;
    state.colliders_by_type.clear();

    for collider in colliders.iter() {
        state.collider_count += 1;

        // Determine collider shape type
        let shape_type = classify_collider_shape(collider);
        *state.colliders_by_type.entry(shape_type).or_insert(0) += 1;
    }

    // Count collision pairs
    // Note: Detailed collision info requires Avian API introspection
    state.collision_pair_count = 0;
    state.collision_pairs.clear();

    // Estimate physics step time (rough estimation based on body count and frame time)
    // Real timing would require internal profiling
    let estimated_step_time = (state.total_body_count() as f32 * 0.01
        + state.collision_pair_count as f32 * 0.005)
        .max(0.1);
    state.step_time_ms = estimated_step_time;
    state.push_step_time(estimated_step_time);
}

/// Classify a collider's shape type
#[cfg(feature = "physics")]
fn classify_collider_shape(collider: &avian3d::prelude::Collider) -> ColliderShapeType {
    // Try to determine shape type based on collider properties
    // This is a simplified classification - Avian's API may vary
    let shape = collider.shape_scaled();

    if shape.as_ball().is_some() {
        ColliderShapeType::Sphere
    } else if shape.as_cuboid().is_some() {
        ColliderShapeType::Box
    } else if shape.as_capsule().is_some() {
        ColliderShapeType::Capsule
    } else if shape.as_cylinder().is_some() {
        ColliderShapeType::Cylinder
    } else if shape.as_cone().is_some() {
        ColliderShapeType::Cone
    } else if shape.as_convex_polyhedron().is_some() {
        ColliderShapeType::ConvexHull
    } else if shape.as_trimesh().is_some() {
        ColliderShapeType::TriMesh
    } else if shape.as_heightfield().is_some() {
        ColliderShapeType::HeightField
    } else if shape.as_compound().is_some() {
        ColliderShapeType::Compound
    } else {
        ColliderShapeType::Unknown
    }
}

/// Stub system when physics feature is disabled
#[cfg(not(feature = "physics"))]
pub fn update_physics_debug_state(
    mut state: ResMut<PhysicsDebugState>,
    time: Res<Time>,
) {
    state.time_since_update += time.delta_secs();

    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    // Reset all counts when physics is disabled
    state.simulation_running = false;
    state.dynamic_body_count = 0;
    state.kinematic_body_count = 0;
    state.static_body_count = 0;
    state.collider_count = 0;
    state.colliders_by_type.clear();
    state.collision_pair_count = 0;
    state.collision_pairs.clear();
}
