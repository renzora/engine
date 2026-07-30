use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use renzora_plugin::prelude::*;
use renzora_plugin::sys::{AssetHandle, Primitive};

#[derive(Resource)]
#[repr(C)]
pub struct Bench {
    pub count: i32,
    pub spacing: f32,
    pub height: f32,
    pub spin_speed: f32,
    pub spin: bool,
    pub stagger: bool,
}

impl Default for Bench {
    fn default() -> Self {
        Self {
            count: 8,
            spacing: 1.5,
            height: 1.0,
            spin_speed: 1.0,
            spin: true,
            stagger: false,
        }
    }
}

/// Marks a cube this plugin spawned, so `Clear` knows what it owns and does not
/// despawn something the user placed by hand.
#[derive(Component, Default)]
#[repr(C)]
pub struct Made {
    pub _v: f32,
}

/// Created once at init and shared by every cube. A handle created during
/// `build` cannot be captured by a system — the host has nowhere to put a
/// capture — so it is parked somewhere the system can read it.
static MESH: AtomicU64 = AtomicU64::new(u64::MAX);
static MATERIAL: AtomicU64 = AtomicU64::new(u64::MAX);

// ── The panel ────────────────────────────────────────────────────────────────

const NONE: u32 = 0;
const SPAWN: u32 = 1;
const CLEAR: u32 = 2;
const LABEL: u32 = 3;

/// What the last click asked for, waiting for a system to service it.
///
/// The handler runs inside the editor's own UI systems, where it is given a
/// command queue but no queries and no resources — the world is borrowed while
/// the click is being handled, so there is nothing safe to hand it. Recording
/// the intent and letting a real system act on it next frame is the way round
/// that, and it is the same shape a Bevy UI callback would take.
static REQUEST: AtomicU32 = AtomicU32::new(NONE);

fn on_action(action: Action) {
    // The name is the `action` number the button's `PanelActionId` carried.
    match action.name() {
        "1" => REQUEST.store(SPAWN, Ordering::Relaxed),
        "2" => REQUEST.store(CLEAR, Ordering::Relaxed),
        "3" => REQUEST.store(LABEL, Ordering::Relaxed),
        other => warn(&format!("bench: unknown action {other}")),
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Services whatever the last click asked for.
///
/// Two queries, over disjoint sets: the existing cubes to remove, and the
/// entities carrying a `Bench` marker to place new ones around. They are
/// separate parameters rather than one merged query, which a system could not
/// have expressed before — a second `Query` used to AND into the first.
fn service(existing: Query<Entity, With<Made>>, bench: Res<Bench>, mut cmds: Commands) {
    let mesh = AssetHandle(MESH.load(Ordering::Relaxed));
    let material = AssetHandle(MATERIAL.load(Ordering::Relaxed));

    match REQUEST.swap(NONE, Ordering::Relaxed) {
        SPAWN => {
            if !mesh.is_valid() || !material.is_valid() {
                return;
            }
            let n = bench.count.clamp(1, 64);
            let span = (n - 1) as f32 * bench.spacing * 0.5;
            for i in 0..n {
                let x = i as f32 * bench.spacing - span;
                // A visible difference the toggle drives, so `stagger` is worth
                // having on the panel rather than being a field nobody reads.
                let y = if bench.stagger && i % 2 == 1 {
                    bench.height + bench.spacing * 0.5
                } else {
                    bench.height
                };
                cmds.spawn_mesh(mesh, material, Transform::from_xyz(x, y, 0.0))
                    .insert(Made::default());
            }
        }
        LABEL => {
            // One tree, engine and plugin components side by side, and the
            // author does not have to know which is which — `PointLight` resolves
            // through the type registry, `Made` through this plugin's schema.
            cmds.spawn_scene(bsn! {
                #BenchLight
                Transform { translation: Vec3(0.0, 6.0, 0.0) }
                PointLight { intensity: 400000.0, shadows_enabled: true }
                Made { _v: 1.0 }
            });
        }
        CLEAR => {
            for e in &existing {
                cmds.entity(e).despawn();
            }
        }
        _ => {}
    }
}

fn spin(mut q: Query<&mut Transform, With<Made>>, bench: Res<Bench>, time: Res<Time>) {
    if !bench.spin {
        return;
    }
    for t in &mut q {
        t.rotate_y(bench.spin_speed * time.delta_secs());
    }
}

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        let mesh = app.add_mesh(Primitive::Cuboid, Vec3::splat(0.7));
        let material = app.add_material([0.35, 0.75, 0.55, 1.0]);
        MESH.store(mesh.0, Ordering::Relaxed);
        MATERIAL.store(material.0, Ordering::Relaxed);

        app.init_resource::<Bench>()
            .register_component::<Made>()
            // Everything named below is a real component — `Node` and `Text`
            // from `bevy_ui`, `EmberButtonWidget` from `renzora_ember` — so this
            // plugin never learned a panel-specific vocabulary. `PanelActionId`
            // is the editor's marker for "clicks here reach my handler".
            .add_panel(
                Panel::new(
                    "bench",
                    "Workbench",
                    bsn! {
                        Node {
                            flex_direction: Column,
                            row_gap: Px(6.0),
                            padding: { left: Px(4.0), right: Px(4.0), top: Px(4.0), bottom: Px(4.0) },
                        }
                        Children [
                            Text("Workbench"),
                            ( Node { flex_direction: Row, column_gap: Px(6.0) }
                              Children [
                                ( EmberButtonWidget { label: "Spawn" }
                                  PanelActionId { panel: 0, action: 1 } ),
                                ( EmberButtonWidget { label: "Clear" }
                                  PanelActionId { panel: 0, action: 2 } ),
                              ] ),
                            ( EmberButtonWidget { label: "Add light" }
                              PanelActionId { panel: 0, action: 3 } ),
                        ]
                    },
                )
                .icon("stack")
                .on_action(on_action),
            )
            .add_systems(Update, service)
            .add_systems(Update, spin);
    }
}

renzora_plugin::add!(BenchPlugin);
