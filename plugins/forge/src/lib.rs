#![no_std]
extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use core::sync::atomic::{AtomicU64, Ordering};
use renzora_plugin::prelude::*;
use renzora_plugin::sys::{AssetHandle, Primitive};

/// `x` is the major radius and `y` the minor — the ABI's documented meaning for
/// a torus, which the host used to hand to bevy's `(inner, outer)` constructor
/// in the wrong order.
const MAJOR: f32 = 0.45;
const MINOR: f32 = 0.18;

static MESH: AtomicU64 = AtomicU64::new(u64::MAX);
static MATERIAL: AtomicU64 = AtomicU64::new(u64::MAX);

#[derive(Component)]
#[repr(C)]
pub struct Forge {
    pub count: i32,
    pub radius: f32,
    pub height: f32,
}

impl Default for Forge {
    fn default() -> Self {
        Self {
            count: 8,
            radius: 3.0,
            height: 1.0,
        }
    }
}

#[derive(Component, Default)]
#[repr(C)]
pub struct Forged {
    pub _v: f32,
}

fn forge(mut q: Query<(Entity, &Forge), Without<Forged>>, mut cmds: Commands) {
    let mesh = AssetHandle(MESH.load(Ordering::Relaxed));
    let material = AssetHandle(MATERIAL.load(Ordering::Relaxed));
    if !mesh.is_valid() || !material.is_valid() {
        return;
    }
    for (e, f) in &mut q {
        cmds.entity(e).insert(Forged::default());

        let n = f.count.max(1);
        for i in 0..n {
            let a = (i as f32 / n as f32) * core::f32::consts::TAU;
            cmds.spawn_mesh(
                mesh,
                material,
                Transform::from_xyz(a.cos() * f.radius, f.height, a.sin() * f.radius),
            );
        }
    }
}

pub struct ForgePlugin;

impl Plugin for ForgePlugin {
    fn build(&self, app: &mut App) {
        let mesh = app.add_mesh(Primitive::Torus, Vec3::new(MAJOR, MINOR, 0.0));
        let material = app.add_material([0.9, 0.6, 0.2, 1.0]);
        MESH.store(mesh.0, Ordering::Relaxed);
        MATERIAL.store(material.0, Ordering::Relaxed);

        app.register_component::<Forge>()
            .register_component::<Forged>()
            .add_systems(Update, forge);
    }
}

renzora_plugin::add!(ForgePlugin);
