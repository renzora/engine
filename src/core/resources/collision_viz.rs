//! Collision visualization state — contact points, normals, penetration depth

use bevy::prelude::*;
use std::collections::VecDeque;

/// How to color contact visualizations
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ContactColorMode {
    #[default]
    Fixed,
    ByImpulse,
    ByPenetration,
}

impl ContactColorMode {
    pub fn label(&self) -> &'static str {
        match self {
            ContactColorMode::Fixed => "Fixed Color",
            ContactColorMode::ByImpulse => "By Impulse",
            ContactColorMode::ByPenetration => "By Penetration",
        }
    }

    pub const ALL: &'static [ContactColorMode] = &[
        ContactColorMode::Fixed,
        ContactColorMode::ByImpulse,
        ContactColorMode::ByPenetration,
    ];
}

/// Visualization data for a single contact point
#[derive(Clone, Debug)]
pub struct ContactVizData {
    pub point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
    pub impulse: f32,
    pub entity_a: Entity,
    pub entity_b: Entity,
}

/// A logged collision event
#[derive(Clone, Debug)]
pub struct CollisionEvent {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub event_type: CollisionEventType,
    pub timestamp: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionEventType {
    Start,
    End,
}

/// State resource for the Collision Visualizer panel
#[derive(Resource)]
pub struct CollisionVizState {
    /// Show contact points as dots
    pub show_contact_points: bool,
    /// Show contact normals as arrows
    pub show_normals: bool,
    /// Show penetration depth
    pub show_penetration: bool,
    /// Flash on impulse
    pub show_impulse_flash: bool,
    /// Contact point marker size
    pub contact_point_size: f32,
    /// Normal arrow length multiplier
    pub normal_length: f32,
    /// Color mode
    pub color_by: ContactColorMode,
    /// Current frame contact data (populated by update system)
    pub contacts: Vec<ContactVizData>,
    /// Collision event log
    pub collision_log: VecDeque<CollisionEvent>,
    /// Max log entries
    pub log_max: usize,
    /// Live stats
    pub active_contacts: usize,
    pub deepest_penetration: f32,
    pub max_impulse: f32,
}

impl Default for CollisionVizState {
    fn default() -> Self {
        Self {
            show_contact_points: true,
            show_normals: true,
            show_penetration: false,
            show_impulse_flash: false,
            contact_point_size: 0.08,
            normal_length: 1.0,
            color_by: ContactColorMode::Fixed,
            contacts: Vec::new(),
            collision_log: VecDeque::with_capacity(100),
            log_max: 100,
            active_contacts: 0,
            deepest_penetration: 0.0,
            max_impulse: 0.0,
        }
    }
}

/// System that collects collision contact data for visualization
pub fn update_collision_viz(
    mut state: ResMut<CollisionVizState>,
    colliding_entities: Query<(Entity, &avian3d::prelude::CollidingEntities)>,
    transforms: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    state.contacts.clear();
    state.active_contacts = 0;
    state.deepest_penetration = 0.0;
    state.max_impulse = 0.0;

    // Collect contact approximations from CollidingEntities
    let mut seen_pairs = std::collections::HashSet::new();
    for (entity_a, colliding) in colliding_entities.iter() {
        for &entity_b in colliding.iter() {
            // Avoid duplicate pairs
            let pair = if entity_a < entity_b { (entity_a, entity_b) } else { (entity_b, entity_a) };
            if !seen_pairs.insert(pair) {
                continue;
            }

            state.active_contacts += 1;

            // Approximate contact point as midpoint between entities
            if let (Ok(ta), Ok(tb)) = (transforms.get(entity_a), transforms.get(entity_b)) {
                let pos_a = ta.translation();
                let pos_b = tb.translation();
                let midpoint = (pos_a + pos_b) * 0.5;
                let diff = pos_b - pos_a;
                let dist = diff.length();
                let normal = if dist > 0.001 { diff / dist } else { Vec3::Y };

                // Estimate penetration (very rough)
                let penetration = (1.0 - dist).max(0.0).min(1.0);

                state.contacts.push(ContactVizData {
                    point: midpoint,
                    normal,
                    penetration,
                    impulse: 0.0, // Would need deeper Avian API access
                    entity_a,
                    entity_b,
                });

                if penetration > state.deepest_penetration {
                    state.deepest_penetration = penetration;
                }
            }
        }
    }

    // Trim collision log
    while state.collision_log.len() > state.log_max {
        state.collision_log.pop_front();
    }
}

/// System that renders collision visualization gizmos
pub fn render_collision_viz_gizmos(
    state: Res<CollisionVizState>,
    mut gizmos: Gizmos<crate::gizmo::physics_viz::PhysicsVizGizmoGroup>,
) {
    let base_color = Color::srgba(1.0, 0.3, 0.3, 0.9);

    for contact in &state.contacts {
        let color = match state.color_by {
            ContactColorMode::Fixed => base_color,
            ContactColorMode::ByPenetration => {
                let t = (contact.penetration * 5.0).min(1.0);
                Color::srgba(1.0, 1.0 - t, 0.0, 0.9)
            }
            ContactColorMode::ByImpulse => {
                let t = (contact.impulse * 0.1).min(1.0);
                Color::srgba(t, 0.3, 1.0 - t, 0.9)
            }
        };

        // Draw contact point
        if state.show_contact_points {
            let s = state.contact_point_size;
            gizmos.line(contact.point - Vec3::X * s, contact.point + Vec3::X * s, color);
            gizmos.line(contact.point - Vec3::Y * s, contact.point + Vec3::Y * s, color);
            gizmos.line(contact.point - Vec3::Z * s, contact.point + Vec3::Z * s, color);
        }

        // Draw normal arrow
        if state.show_normals {
            let end = contact.point + contact.normal * state.normal_length;
            gizmos.arrow(contact.point, end, color);
        }
    }
}
