//! Editor-only half of `renzora_physics`.
//!
//! `renzora_physics` compiles lean (no `editor` feature, no egui-phosphor /
//! renzora_ember). This crate holds everything that only matters inside the
//! editor:
//!
//! - the **Physics Body** + **Collision Shape** inspector entries and their
//!   native (ember) drawers (body type / mass / damping / axis locks; shape
//!   type + per-shape params; offset / friction / restitution / sensor);
//! - the **Edit Collider** toggle button + tool (drives the lean-crate
//!   `renzora_physics::ColliderEditMode` resource the gizmo crate reads);
//! - the **Stamp / Strip** mesh-collider bulk actions, the
//!   [`ColliderStampQueue`] resource, and the frame-batched
//!   [`drain_collider_stamp_queue`] system that drains it;
//! - the entity presets (Rigid / Static / Kinematic body, Box / Sphere /
//!   Capsule / Cylinder collider);
//! - the auto-insert-collider-on-spawn observer for `MeshPrimitive` entities.
//!
//! Registered via `renzora::add!(PhysicsEditorPlugin, Editor)` and linked only
//! by the editor bundle.

use bevy::prelude::*;

pub mod inspector;

/// Background queue for bulk-stamping mesh colliders on a hierarchy, chunked
/// across frames so the UI can show progress. Populated by the inspector's
/// "Stamp Mesh Colliders" button; drained by [`drain_collider_stamp_queue`].
#[derive(Resource, Default)]
pub struct ColliderStampQueue {
    pub root: Option<Entity>,
    pub remaining: Vec<Entity>,
    pub total: usize,
}

impl ColliderStampQueue {
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        (self.total - self.remaining.len()) as f32 / self.total as f32
    }
    pub fn is_active(&self) -> bool {
        !self.remaining.is_empty()
    }
}

/// Stamps up to `BATCH` entities per frame from the queue. Keeps the UI
/// responsive on huge scenes (thousands of meshes) and lets the hierarchy
/// panel draw a live progress bar.
fn drain_collider_stamp_queue(
    mut commands: Commands,
    mut queue: ResMut<ColliderStampQueue>,
    existing_shapes: Query<(), With<renzora_physics::CollisionShapeData>>,
) {
    use renzora_physics::{CollisionShapeData, PhysicsBodyData};
    const BATCH: usize = 24;
    if queue.remaining.is_empty() {
        return;
    }
    for _ in 0..BATCH {
        let Some(e) = queue.remaining.pop() else {
            break;
        };
        // Skip if the entity has gained a collision shape since we queued it.
        if existing_shapes.get(e).is_ok() {
            continue;
        }
        commands
            .entity(e)
            .insert((PhysicsBodyData::static_body(), CollisionShapeData::mesh()));
    }
    if queue.remaining.is_empty() {
        renzora::console_log::console_success(
            "Physics",
            format!("Stamped Mesh Colliders on {} entities", queue.total),
        );
        queue.root = None;
        queue.total = 0;
    }
}

/// Editor-scope companion to `renzora_physics::PhysicsPlugin`.
#[derive(Default)]
pub struct PhysicsEditorPlugin;

impl Plugin for PhysicsEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] PhysicsEditorPlugin");
        // In the editor the simulation must not run until the user hits play.
        // The lean `PhysicsPlugin` always starts the backend running; mirror
        // the old `start_paused = cfg!(feature = "editor")` behaviour by pausing
        // at startup here (this plugin is only ever installed in the editor).
        app.add_systems(Startup, |world: &mut World| renzora_physics::pause(world));
        app.init_resource::<renzora_physics::ColliderEditMode>();
        app.init_resource::<ColliderStampQueue>();
        app.add_systems(Update, drain_collider_stamp_queue);
        inspector::register_physics_inspectors(app);
    }
}

renzora::add!(PhysicsEditorPlugin, Editor);
