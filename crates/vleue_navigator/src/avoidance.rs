//! ORCA (Optimal Reciprocal Collision Avoidance) for agent-to-agent collision avoidance.
//!
//! Feature-gated behind the `avoidance` feature flag.
//!
//! Users set [`NavigationAgent::preferred_velocity`] (toward next waypoint) each frame,
//! then read [`NavigationAgent::avoidance_velocity`] to move.

use bevy::{
    math::{Vec2, Vec3},
    prelude::*,
    transform::TransformSystems,
};

/// Plugin enabling ORCA agent-to-agent collision avoidance.
///
/// Add this plugin alongside [`NavmeshUpdaterPlugin`](crate::updater::NavmeshUpdaterPlugin).
/// Each agent needs a [`NavigationAgent`] component.
#[derive(Debug, Clone, Copy, Default)]
pub struct AvoidancePlugin;

impl Plugin for AvoidancePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvoidanceConfig>().add_systems(
            PostUpdate,
            compute_avoidance.after(TransformSystems::Propagate),
        );
    }
}

/// Per-agent navigation component for ORCA avoidance.
///
/// Set `preferred_velocity` each frame (direction toward next waypoint, magnitude = desired speed).
/// After the avoidance system runs, read `avoidance_velocity` to apply movement.
#[derive(Component, Debug, Clone)]
pub struct NavigationAgent {
    /// Collision radius of this agent.
    pub radius: f32,
    /// Maximum speed this agent can move.
    pub max_speed: f32,
    /// Desired velocity toward the next waypoint. Set this each frame.
    pub preferred_velocity: Vec3,
    /// Velocity adjusted by ORCA to avoid other agents. Read this each frame.
    pub avoidance_velocity: Vec3,
}

impl Default for NavigationAgent {
    fn default() -> Self {
        Self {
            radius: 0.5,
            max_speed: 5.0,
            preferred_velocity: Vec3::ZERO,
            avoidance_velocity: Vec3::ZERO,
        }
    }
}

/// Global configuration for the ORCA avoidance system.
#[derive(Resource, Debug, Clone)]
pub struct AvoidanceConfig {
    /// Maximum distance at which other agents are considered neighbors.
    pub neighbor_distance: f32,
    /// Maximum number of nearest neighbors to consider per agent.
    pub max_neighbors: usize,
    /// Time horizon for agent-agent avoidance (seconds). Larger values make agents
    /// avoid each other earlier but may cause unnecessary detours.
    pub time_horizon_agents: f32,
}

impl Default for AvoidanceConfig {
    fn default() -> Self {
        Self {
            neighbor_distance: 10.0,
            max_neighbors: 10,
            time_horizon_agents: 2.0,
        }
    }
}

/// An ORCA half-plane constraint: velocity must satisfy `dot(normal, v) >= d`.
#[derive(Debug, Clone, Copy)]
struct OrcaLine {
    point: Vec2,
    direction: Vec2,
}

fn compute_avoidance(
    mut agents: Query<(Entity, &GlobalTransform, &mut NavigationAgent)>,
    config: Res<AvoidanceConfig>,
) {
    // Snapshot all agent data for immutable access during computation
    let snapshots: Vec<(Entity, Vec3, f32, f32, Vec3)> = agents
        .iter()
        .map(|(e, t, a)| {
            (
                e,
                t.translation(),
                a.radius,
                a.max_speed,
                a.preferred_velocity,
            )
        })
        .collect();

    // For each agent, compute ORCA velocity
    for (entity, transform, mut agent) in agents.iter_mut() {
        let pos = transform.translation();
        let pref_vel = agent.preferred_velocity;
        // Project to XZ plane for 2D avoidance
        let pos_2d = Vec2::new(pos.x, pos.z);
        let pref_vel_2d = Vec2::new(pref_vel.x, pref_vel.z);

        // Find nearest neighbors
        let mut neighbors: Vec<(f32, usize)> = snapshots
            .iter()
            .enumerate()
            .filter(|(_, (e, ..))| *e != entity)
            .map(|(i, (_, other_pos, ..))| {
                let dist_sq = pos.distance_squared(*other_pos);
                (dist_sq, i)
            })
            .filter(|(dist_sq, _)| *dist_sq < config.neighbor_distance * config.neighbor_distance)
            .collect();

        neighbors.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        neighbors.truncate(config.max_neighbors);

        // Build ORCA lines
        let mut orca_lines: Vec<OrcaLine> = Vec::with_capacity(neighbors.len());

        for &(_, idx) in &neighbors {
            let (_, other_pos, other_radius, _, other_pref_vel) = &snapshots[idx];
            let other_pos_2d = Vec2::new(other_pos.x, other_pos.z);
            let other_pref_vel_2d = Vec2::new(other_pref_vel.x, other_pref_vel.z);

            let relative_pos = other_pos_2d - pos_2d;
            let relative_vel = pref_vel_2d - other_pref_vel_2d;
            let dist_sq = relative_pos.length_squared();
            let combined_radius = agent.radius + other_radius;
            let combined_radius_sq = combined_radius * combined_radius;

            let tau = config.time_horizon_agents;

            if dist_sq > combined_radius_sq {
                // No collision at current positions
                let w = relative_vel - relative_pos / tau;
                let w_length_sq = w.length_squared();
                let dot_product = w.dot(relative_pos);

                if dot_product < 0.0
                    && dot_product * dot_product > combined_radius_sq * w_length_sq
                {
                    // Project on circle
                    let w_length = w_length_sq.sqrt();
                    if w_length < f32::EPSILON {
                        continue;
                    }
                    let unit_w = w / w_length;
                    let line_direction = Vec2::new(unit_w.y, -unit_w.x);
                    let u = (combined_radius / tau - w_length) * unit_w;
                    orca_lines.push(OrcaLine {
                        point: pref_vel_2d + 0.5 * u,
                        direction: line_direction,
                    });
                } else {
                    // Project on legs
                    let leg = (dist_sq - combined_radius_sq).max(0.0).sqrt();

                    if det(relative_pos, w) > 0.0 {
                        // Left leg
                        let line_direction = Vec2::new(
                            relative_pos.x * leg - relative_pos.y * combined_radius,
                            relative_pos.x * combined_radius + relative_pos.y * leg,
                        ) / dist_sq;
                        let u_dot = relative_vel.dot(line_direction) ;
                        let u = u_dot * line_direction - relative_vel;
                        orca_lines.push(OrcaLine {
                            point: pref_vel_2d + 0.5 * u,
                            direction: line_direction,
                        });
                    } else {
                        // Right leg
                        let line_direction = -Vec2::new(
                            relative_pos.x * leg + relative_pos.y * combined_radius,
                            -relative_pos.x * combined_radius + relative_pos.y * leg,
                        ) / dist_sq;
                        let u_dot = relative_vel.dot(line_direction);
                        let u = u_dot * line_direction - relative_vel;
                        orca_lines.push(OrcaLine {
                            point: pref_vel_2d + 0.5 * u,
                            direction: line_direction,
                        });
                    }
                }
            } else {
                // Already colliding, push apart
                let inv_dt = 1.0 / 0.016; // Assume ~60fps timestep for collision resolution
                let w = relative_vel - relative_pos * inv_dt;
                let w_length = w.length();
                if w_length < f32::EPSILON {
                    continue;
                }
                let unit_w = w / w_length;
                let line_direction = Vec2::new(unit_w.y, -unit_w.x);
                let u = (combined_radius * inv_dt - w_length) * unit_w;
                orca_lines.push(OrcaLine {
                    point: pref_vel_2d + 0.5 * u,
                    direction: line_direction,
                });
            }
        }

        // Solve the linear program to find the best velocity satisfying all ORCA constraints
        let result = solve_linear_program(&orca_lines, agent.max_speed, pref_vel_2d);
        agent.avoidance_velocity = Vec3::new(result.x, pref_vel.y, result.y);
    }
}

/// 2D cross product (determinant).
#[inline]
fn det(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Incrementally solve the 2D linear program:
/// Find the velocity closest to `preferred` that satisfies all ORCA half-plane constraints
/// and has magnitude <= `max_speed`.
fn solve_linear_program(lines: &[OrcaLine], max_speed: f32, preferred: Vec2) -> Vec2 {
    let mut result = preferred;

    // Clamp to max speed disc
    if result.length_squared() > max_speed * max_speed {
        result = result.normalize_or_zero() * max_speed;
    }

    for i in 0..lines.len() {
        if det(lines[i].direction, lines[i].point - result) > 0.0 {
            // Result does not satisfy constraint i; project onto line i
            let temp = result;
            if !linear_program_1(&lines[..=i], i, max_speed, preferred, &mut result) {
                result = temp;
                // Fall back to safest velocity for remaining constraints
                linear_program_3(lines, i, max_speed, &mut result);
                break;
            }
        }
    }

    result
}

/// Solve 1D optimization on line `line_no` with constraints from lines[0..line_no].
fn linear_program_1(
    lines: &[OrcaLine],
    line_no: usize,
    max_speed: f32,
    preferred: Vec2,
    result: &mut Vec2,
) -> bool {
    let line = &lines[line_no];
    let dot_product = line.point.dot(line.direction);
    let discriminant = dot_product * dot_product + max_speed * max_speed - line.point.length_squared();

    if discriminant < 0.0 {
        return false;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let mut t_left = -dot_product - sqrt_discriminant;
    let mut t_right = -dot_product + sqrt_discriminant;

    for j in 0..line_no {
        let denominator = det(line.direction, lines[j].direction);
        let numerator = det(lines[j].direction, line.point - lines[j].point);

        if denominator.abs() <= f32::EPSILON {
            // Lines are nearly parallel
            if numerator < 0.0 {
                return false;
            }
            continue;
        }

        let t = numerator / denominator;
        if denominator > 0.0 {
            t_right = t_right.min(t);
        } else {
            t_left = t_left.max(t);
        }

        if t_left > t_right {
            return false;
        }
    }

    // Optimize closest point on the valid segment to preferred velocity
    let t = line.direction.dot(preferred - line.point);
    let t = t.clamp(t_left, t_right);
    *result = line.point + t * line.direction;
    true
}

/// Fallback: when LP1 fails for constraint `begin_line`, find the safest velocity
/// that satisfies as many constraints as possible.
fn linear_program_3(lines: &[OrcaLine], begin_line: usize, max_speed: f32, result: &mut Vec2) {
    let mut distance = 0.0_f32;

    for i in begin_line..lines.len() {
        if det(lines[i].direction, lines[i].point - *result) > distance {
            // Construct projected lines for constraints 0..i
            let mut proj_lines: Vec<OrcaLine> = Vec::with_capacity(i);

            for j in 0..i {
                let mut line = OrcaLine {
                    point: Vec2::ZERO,
                    direction: Vec2::ZERO,
                };

                let determinant = det(lines[i].direction, lines[j].direction);

                if determinant.abs() <= f32::EPSILON {
                    // Parallel lines
                    if lines[i].direction.dot(lines[j].direction) > 0.0 {
                        // Same direction - constraint j is redundant
                        continue;
                    }
                    // Opposite direction
                    line.point = 0.5 * (lines[i].point + lines[j].point);
                } else {
                    line.point = lines[i].point
                        + (det(lines[j].direction, lines[i].point - lines[j].point) / determinant)
                            * lines[i].direction;
                }

                line.direction = (lines[j].direction - lines[i].direction).normalize_or_zero();
                proj_lines.push(line);
            }

            let temp = *result;
            // Optimize direction perpendicular to line i (pointing away from constraint)
            let opt_direction = Vec2::new(-lines[i].direction.y, lines[i].direction.x);

            if linear_program_1(&proj_lines, proj_lines.len().saturating_sub(1), max_speed, opt_direction, result) {
                // Successfully found a feasible point; no action needed
            } else {
                *result = temp;
            }

            distance = det(lines[i].direction, lines[i].point - *result);
        }
    }
}
