//! Physics metrics state — energy, velocity, momentum, and performance tracking

use bevy::prelude::*;
use std::collections::VecDeque;

const MAX_HISTORY: usize = 300;

/// Real-time physics metrics for monitoring simulation health
#[derive(Resource)]
pub struct PhysicsMetricsState {
    /// Total kinetic energy (0.5 * m * v^2)
    pub total_kinetic_energy: f64,
    /// Total potential energy (m * g * h)
    pub total_potential_energy: f64,
    /// Total energy (KE + PE)
    pub total_energy: f64,
    /// Energy history for graphing
    pub energy_history: VecDeque<f64>,
    /// Average velocity magnitude across all dynamic bodies
    pub avg_velocity: f32,
    /// Maximum velocity magnitude
    pub max_velocity: f32,
    /// Number of active (non-sleeping) bodies
    pub active_bodies: usize,
    /// Number of sleeping bodies
    pub sleeping_bodies: usize,
    /// Total momentum vector
    pub total_momentum: Vec3,
    /// Physics step time in ms (estimated from frame time)
    pub frame_physics_time_ms: f32,
    /// Physics time history for graphing
    pub physics_time_history: VecDeque<f32>,
    /// Active collision pair count
    pub collision_count: usize,
    /// Collision count history
    pub collision_pairs_history: VecDeque<usize>,
    /// Whether tracking is enabled
    pub tracking_enabled: bool,
    /// Update interval
    pub update_interval: f32,
    /// Time since last update
    pub time_since_update: f32,
}

impl Default for PhysicsMetricsState {
    fn default() -> Self {
        Self {
            total_kinetic_energy: 0.0,
            total_potential_energy: 0.0,
            total_energy: 0.0,
            energy_history: VecDeque::with_capacity(MAX_HISTORY),
            avg_velocity: 0.0,
            max_velocity: 0.0,
            active_bodies: 0,
            sleeping_bodies: 0,
            total_momentum: Vec3::ZERO,
            frame_physics_time_ms: 0.0,
            physics_time_history: VecDeque::with_capacity(MAX_HISTORY),
            collision_count: 0,
            collision_pairs_history: VecDeque::with_capacity(MAX_HISTORY),
            tracking_enabled: true,
            update_interval: 0.05, // 20 Hz
            time_since_update: 0.0,
        }
    }
}

impl PhysicsMetricsState {
    fn push_energy(&mut self, value: f64) {
        if self.energy_history.len() >= MAX_HISTORY {
            self.energy_history.pop_front();
        }
        self.energy_history.push_back(value);
    }

    fn push_physics_time(&mut self, value: f32) {
        if self.physics_time_history.len() >= MAX_HISTORY {
            self.physics_time_history.pop_front();
        }
        self.physics_time_history.push_back(value);
    }

    fn push_collisions(&mut self, value: usize) {
        if self.collision_pairs_history.len() >= MAX_HISTORY {
            self.collision_pairs_history.pop_front();
        }
        self.collision_pairs_history.push_back(value);
    }
}

/// System that computes physics metrics from the simulation state
pub fn update_physics_metrics(
    mut state: ResMut<PhysicsMetricsState>,
    time: Res<Time>,
    bodies: Query<(
        &avian3d::prelude::RigidBody,
        &avian3d::prelude::LinearVelocity,
        &avian3d::prelude::Mass,
        &Transform,
    )>,
    sleeping: Query<Entity, With<avian3d::prelude::Sleeping>>,
    colliding: Query<&avian3d::prelude::CollidingEntities>,
    gravity: Option<Res<avian3d::prelude::Gravity>>,
) {
    if !state.tracking_enabled {
        return;
    }

    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    let g = gravity.map(|g| g.0.y.abs() as f64).unwrap_or(9.81);

    let mut total_ke: f64 = 0.0;
    let mut total_pe: f64 = 0.0;
    let mut total_momentum = Vec3::ZERO;
    let mut max_vel: f32 = 0.0;
    let mut vel_sum: f32 = 0.0;
    let mut dynamic_count: usize = 0;

    for (body, lin_vel, mass, transform) in bodies.iter() {
        if *body != avian3d::prelude::RigidBody::Dynamic {
            continue;
        }
        dynamic_count += 1;
        let m = mass.0 as f64;
        let v = lin_vel.0;
        let speed = v.length();

        // KE = 0.5 * m * v^2
        total_ke += 0.5 * m * (speed as f64) * (speed as f64);

        // PE = m * g * h (height above y=0)
        let h = transform.translation.y.max(0.0) as f64;
        total_pe += m * g * h;

        // Momentum
        total_momentum += v * mass.0;

        vel_sum += speed;
        if speed > max_vel {
            max_vel = speed;
        }
    }

    let sleeping_count = sleeping.iter().count();

    // Collision count
    let mut collision_set = std::collections::HashSet::new();
    for colliding_ents in colliding.iter() {
        for &other in colliding_ents.iter() {
            // Use sorted pair to avoid double counting
            let pair = if colliding_ents.iter().next().copied().unwrap_or(other) < other {
                (colliding_ents.iter().next().copied().unwrap_or(other), other)
            } else {
                (other, colliding_ents.iter().next().copied().unwrap_or(other))
            };
            collision_set.insert(pair);
        }
    }

    state.total_kinetic_energy = total_ke;
    state.total_potential_energy = total_pe;
    state.total_energy = total_ke + total_pe;
    let energy = state.total_energy;
    state.push_energy(energy);

    state.avg_velocity = if dynamic_count > 0 { vel_sum / dynamic_count as f32 } else { 0.0 };
    state.max_velocity = max_vel;
    state.active_bodies = dynamic_count.saturating_sub(sleeping_count);
    state.sleeping_bodies = sleeping_count;
    state.total_momentum = total_momentum;

    // Estimated physics time (rough)
    let physics_ms = (dynamic_count as f32 * 0.01 + collision_set.len() as f32 * 0.005).max(0.01);
    state.frame_physics_time_ms = physics_ms;
    state.push_physics_time(physics_ms);

    let collision_count = collision_set.len();
    state.collision_count = collision_count;
    state.push_collisions(collision_count);
}
