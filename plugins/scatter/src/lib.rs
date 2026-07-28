use core::sync::atomic::{AtomicU64, Ordering};
use renzora_plugin::prelude::*;
use renzora_plugin::sys::{AssetHandle, Primitive};

/// Marks a source entity whose grid has been built, so `scatter` runs once.
#[derive(Component, Default)]
pub struct Scattered;

/// Put one on any entity to build a grid of cubes around the origin.
#[derive(Component)]
pub struct Scatter {
    pub count: i32,
    pub spacing: f32,
    pub height: f32,
}

impl Default for Scatter {
    fn default() -> Self {
        Self {
            count: 5,
            spacing: 1.5,
            height: 0.0,
        }
    }
}

/// Created once at init and reused by every spawn — one mesh asset shared by
/// however many cubes appear.
static MESH: AtomicU64 = AtomicU64::new(u64::MAX);
static MATERIAL: AtomicU64 = AtomicU64::new(u64::MAX);

fn scatter(mut q: Query<(Entity, &Scatter), Without<Scattered>>, mut cmds: Commands) {
    let mesh = AssetHandle(MESH.load(Ordering::Relaxed));
    let material = AssetHandle(MATERIAL.load(Ordering::Relaxed));
    if !mesh.is_valid() || !material.is_valid() {
        return;
    }
    for (e, s) in &mut q {
        // Mark the source done, so the grid is built once rather than every
        // frame. Needs `Entity` in the query — without it a system cannot act on
        // the entity it is looking at.
        cmds.entity(e).insert(Scattered);

        let half = s.count / 2;
        for x in -half..=half {
            for z in -half..=half {
                cmds.spawn_mesh(
                    mesh,
                    material,
                    Transform::from_xyz(x as f32 * s.spacing, s.height, z as f32 * s.spacing),
                );
            }
        }
    }
}

pub struct ScatterPlugin;

impl Plugin for ScatterPlugin {
    fn build(&self, app: &mut App) {
        let mesh = app.add_mesh(Primitive::Cuboid, Vec3::splat(0.6));
        let material = app.add_material([0.3, 0.6, 0.9, 1.0]);
        MESH.store(mesh.0, Ordering::Relaxed);
        MATERIAL.store(material.0, Ordering::Relaxed);

        app.register_component::<Scatter>()
            .register_component::<Scattered>()
            .add_systems(Update, scatter);
    }
}

renzora_plugin::add!(ScatterPlugin);
