//! An orrery: a sun with orbiting planets, built with `bsn!`, that responds to
//! what you have selected in the editor.
//!
//! This is the demo that needs the native mechanism. The other two do not —
//! `hello-native` only proves the boundary works, and `native-grayscale` reaches
//! a framework the C-ABI path reaches more cheaply. Everything here is out of
//! reach of a `#[repr(C)]` descriptor:
//!
//! * a nested entity hierarchy declared with `bsn!`
//! * mesh and material assets created at run time
//! * reading *and writing* [`EditorSelection`] — the plugin participates in the
//!   editor's own state
//! * gizmos drawn from arbitrary queries over the world
//!
//! # What to try
//!
//! Select a planet in the hierarchy and its orbit is drawn as a ring, with a
//! line home to the sun. Select the sun and every orbit is drawn at once. Nothing
//! here is wired into the editor by the engine — the plugin reads the same
//! selection resource the inspector does, because that type lives in the contract
//! crate precisely so both sides can share it.

use bevy::color::palettes::tailwind;
use bevy::prelude::*;
// The editor contract is glob-re-exported at the crate root (`pub use
// editor_contract::*`), so the module path is not importable — `renzora::X`,
// never `renzora::editor_contract::X`.
use renzora::{EditorSelection, SplashState};

/// One orbiting body. `distance`/`speed` drive the animation; `Reflect` puts them
/// in the inspector, so you can retune the system while it runs.
#[derive(Component, Clone, Reflect, Debug)]
#[reflect(Component, Default)]
pub struct Orbit {
    pub distance: f32,
    pub speed: f32,
    /// Where it started, so the ring is drawn from the same origin the motion
    /// uses rather than from wherever it happens to be this frame.
    pub phase: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self { distance: 3.0, speed: 0.5, phase: 0.0 }
    }
}

/// The body everything orbits. Selecting it draws every ring at once.
#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component, Default)]
pub struct Sun;

pub struct OrreryPlugin;

impl Plugin for OrreryPlugin {
    fn build(&self, app: &mut App) {
        // Registered so the components show up in the inspector and survive a
        // scene save — an ordinary Bevy plugin's job, done from a library.
        app.register_type::<Orbit>().register_type::<Sun>();

        // NOT `Startup`: that runs before the project-load teardown, which
        // despawns everything carrying a `Name`. See `plugins/spinning-cube`.
        app.add_systems(OnEnter(SplashState::Editor), spawn_orrery)
            .add_systems(Update, (orbit, draw_selected_orbits));

        info!("[orrery] registered");
    }
}

/// Build the whole system as one BSN scene.
///
/// The nesting is the point: `Children [...]` gives real parent/child
/// relationships, so the planets appear indented under the sun in the hierarchy
/// and inherit its transform. Writing this with `commands.spawn` and
/// `add_child` would be three times the code and would not read as a *shape*.
fn spawn_orrery(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sun_mesh = meshes.add(Sphere::new(0.8).mesh().uv(24, 12));
    // Cloned into named bindings up front because `bsn!` parses its component
    // arguments as literal values, not arbitrary Rust: a `planet_mesh.clone()`
    // inside the macro fails with "Unexpected input after function name".
    let planet_mesh = meshes.add(Sphere::new(0.25).mesh().uv(16, 8));
    let mesh_a = planet_mesh.clone();
    let mesh_b = planet_mesh.clone();
    let mesh_c = planet_mesh;

    // Emissive so the orrery is visible in an empty scene with no lights. A demo
    // that only appears once you add a light looks broken.
    let mut glowing = |c: Srgba, e: f32| {
        materials.add(StandardMaterial {
            base_color: c.into(),
            emissive: LinearRgba::from(c) * e,
            ..default()
        })
    };
    let sun_mat = glowing(tailwind::AMBER_400, 3.0);
    let a = glowing(tailwind::SKY_400, 1.0);
    let b = glowing(tailwind::ROSE_400, 1.0);
    let c = glowing(tailwind::EMERALD_400, 1.0);

    commands.spawn_scene(bsn! {
        Name::new("Orrery Sun")
        Sun
        Mesh3d(sun_mesh)
        MeshMaterial3d::<StandardMaterial>(sun_mat)
        Transform::from_xyz(0.0, 2.0, 0.0)
        Children [
            (
                Name::new("Planet I")
                Orbit { distance: 2.0, speed: 0.9, phase: 0.0 }
                Mesh3d(mesh_a)
                MeshMaterial3d::<StandardMaterial>(a)
                Transform::default()
            ),
            (
                Name::new("Planet II")
                Orbit { distance: 3.2, speed: 0.55, phase: 2.1 }
                Mesh3d(mesh_b)
                MeshMaterial3d::<StandardMaterial>(b)
                Transform::default()
            ),
            (
                Name::new("Planet III")
                Orbit { distance: 4.6, speed: 0.3, phase: 4.0 }
                Mesh3d(mesh_c)
                MeshMaterial3d::<StandardMaterial>(c)
                Transform::default()
            )
        ]
    });

    info!("[orrery] spawned a sun and 3 planets from one bsn! scene");
}

/// Move the planets. Local transforms, so the parent's position carries them —
/// drag the sun in the editor and the whole system follows.
fn orbit(time: Res<Time>, mut q: Query<(&Orbit, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (o, mut tr) in &mut q {
        let angle = o.phase + t * o.speed;
        tr.translation = Vec3::new(angle.cos() * o.distance, 0.0, angle.sin() * o.distance);
    }
}

/// Draw a ring for whatever is selected — the plugin reacting to editor state.
///
/// `EditorSelection` lives in the contract crate, so a plugin reads the same
/// resource the inspector and the hierarchy panel do. Nothing special was wired
/// up for this; it is simply a shared type, which is the whole point of §6.3.
fn draw_selected_orbits(
    selection: Option<Res<EditorSelection>>,
    mut gizmos: Gizmos,
    planets: Query<(Entity, &Orbit, &ChildOf)>,
    suns: Query<Entity, With<Sun>>,
    transforms: Query<&GlobalTransform>,
) {
    let Some(selection) = selection else {
        // No selection resource means this is the runtime, not the editor. The
        // orrery still turns; it just has nobody to draw gizmos for.
        return;
    };

    // Selecting the sun lights up every orbit at once; selecting one planet
    // draws only its own.
    let sun_selected = suns.iter().any(|e| selection.is_selected(e));

    for (entity, orbit, parent) in &planets {
        if !sun_selected && !selection.is_selected(entity) {
            continue;
        }
        let Ok(centre) = transforms.get(parent.parent()) else {
            continue;
        };
        let centre = centre.translation();

        gizmos
            .circle(
                Isometry3d::new(centre, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                orbit.distance,
                tailwind::SLATE_400,
            )
            .resolution(64);

        // A line home, so which body owns the ring is unambiguous when several
        // are drawn at once.
        if let Ok(here) = transforms.get(entity) {
            gizmos.line(centre, here.translation(), tailwind::SLATE_600);
        }
    }
}

renzora::plugin!(OrreryPlugin);
