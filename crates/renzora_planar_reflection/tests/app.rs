//! The ECS side in a headless app: rigs appear for surfaces a camera can see,
//! are posed as the camera's reflection, respect the cap and go away with
//! their surface.
//!
//! Headless means no GPU, so nothing is drawn; what is checked is every
//! decision the plugin makes before Bevy renders.

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use renzora::{EditorLocked, HideInHierarchy, IsolatedCamera, ReflectionCamera, SceneCamera};
use renzora_planar_reflection::systems::{MirrorCamera, MirrorOverlay, ReflectionRigs};
use renzora_planar_reflection::{PlanarReflection, PlanarReflectionPlugin, PlanarReflectionSettings};

fn app() -> App {
    renzora_test_harness::headless_app_with(|app| {
        app.add_plugins(PlanarReflectionPlugin);
    })
}

fn run(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// A scene camera rendering into a 320x180 image, looking at the origin.
fn spawn_viewer(app: &mut App, position: Vec3) -> Entity {
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(320, 180, TextureFormat::Rgba8UnormSrgb, None));
    app.world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            RenderTarget::Image(image.into()),
            Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
            SceneCamera,
        ))
        .id()
}

/// A 4x4 floor at `position` reflecting upward.
fn spawn_floor(app: &mut App, position: Vec3) -> Entity {
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Plane3d::default().mesh().size(4.0, 4.0));
    app.world_mut()
        .spawn((
            Name::new("Floor Mirror"),
            Mesh3d(mesh),
            Transform::from_translation(position),
            PlanarReflection::default(),
        ))
        .id()
}

fn mirror_cameras(app: &mut App) -> Vec<(Entity, MirrorCamera, bool, Transform)> {
    let mut q = app
        .world_mut()
        .query::<(Entity, &MirrorCamera, &Camera, &Transform)>();
    q.iter(app.world())
        .map(|(e, m, c, t)| (e, *m, c.is_active, *t))
        .collect()
}

#[test]
fn a_visible_floor_gets_a_mirrored_capture_camera() {
    let mut app = app();
    let viewer = spawn_viewer(&mut app, Vec3::new(0.0, 2.0, 6.0));
    let floor = spawn_floor(&mut app, Vec3::ZERO);
    run(&mut app, 4);

    let cameras = mirror_cameras(&mut app);
    assert_eq!(cameras.len(), 1, "one rig for one (surface, camera) pair");
    let (camera, link, active, pose) = cameras[0];
    assert_eq!((link.surface, link.source), (floor, viewer));
    assert!(active, "a visible surface renders every frame by default");
    assert!(
        pose.translation.distance(Vec3::new(0.0, -2.0, 6.0)) < 1e-4,
        "posed as the viewer reflected under the floor: {:?}",
        pose.translation
    );

    // Out of the hierarchy, out of scene saves, out of routed effects, and
    // recognisable as a capture rather than a scene camera.
    let entity = app.world().entity(camera);
    assert!(entity.contains::<HideInHierarchy>());
    assert!(entity.contains::<EditorLocked>());
    assert!(entity.contains::<IsolatedCamera>());
    assert!(entity.contains::<ReflectionCamera>());
    assert!(!entity.contains::<SceneCamera>());
    assert!(matches!(entity.get::<Projection>(), Some(Projection::Custom(_))));

    // The overlay is a hidden-from-hierarchy child of the surface, shown once
    // the capture has had a frame to render.
    let mut q = app
        .world_mut()
        .query::<(&MirrorOverlay, &ChildOf, &Visibility, Has<HideInHierarchy>)>();
    let overlays: Vec<_> = q
        .iter(app.world())
        .map(|(o, c, v, h)| (o.surface, c.parent(), *v, h))
        .collect();
    assert_eq!(overlays, vec![(floor, floor, Visibility::Inherited, true)]);
}

#[test]
fn a_camera_behind_the_surface_renders_nothing() {
    let mut app = app();
    spawn_viewer(&mut app, Vec3::new(0.0, -2.0, 6.0));
    spawn_floor(&mut app, Vec3::ZERO);
    run(&mut app, 4);
    assert!(mirror_cameras(&mut app).is_empty());
}

#[test]
fn the_cap_keeps_the_nearest_surfaces() {
    let mut app = app();
    app.insert_resource(PlanarReflectionSettings { max_active: 2 });
    spawn_viewer(&mut app, Vec3::new(0.0, 3.0, 8.0));
    let near = spawn_floor(&mut app, Vec3::new(0.0, 0.0, 4.0));
    let middle = spawn_floor(&mut app, Vec3::new(0.0, 0.0, 0.0));
    let far = spawn_floor(&mut app, Vec3::new(0.0, 0.0, -30.0));
    run(&mut app, 4);

    let mut active: Vec<Entity> = mirror_cameras(&mut app)
        .into_iter()
        .filter(|(_, _, active, _)| *active)
        .map(|(_, link, _, _)| link.surface)
        .collect();
    active.sort();
    let mut expected = vec![near, middle];
    expected.sort();
    assert_eq!(active, expected, "far floor {far:?} should wait its turn");
}

#[test]
fn removing_the_component_tears_the_rig_down() {
    let mut app = app();
    spawn_viewer(&mut app, Vec3::new(0.0, 2.0, 6.0));
    let floor = spawn_floor(&mut app, Vec3::ZERO);
    run(&mut app, 4);
    assert_eq!(mirror_cameras(&mut app).len(), 1);

    app.world_mut().entity_mut(floor).remove::<PlanarReflection>();
    run(&mut app, 2);
    assert!(mirror_cameras(&mut app).is_empty());
    let mut q = app.world_mut().query::<&MirrorOverlay>();
    assert_eq!(q.iter(app.world()).count(), 0);
    assert!(app.world().resource::<ReflectionRigs>().is_empty());
}

#[test]
fn despawning_the_surface_tears_the_rig_down() {
    let mut app = app();
    spawn_viewer(&mut app, Vec3::new(0.0, 2.0, 6.0));
    let floor = spawn_floor(&mut app, Vec3::ZERO);
    run(&mut app, 4);
    app.world_mut().entity_mut(floor).despawn();
    run(&mut app, 2);
    assert!(mirror_cameras(&mut app).is_empty());
}

#[test]
fn a_disabled_surface_keeps_its_rig_but_stops_rendering() {
    let mut app = app();
    spawn_viewer(&mut app, Vec3::new(0.0, 2.0, 6.0));
    let floor = spawn_floor(&mut app, Vec3::ZERO);
    run(&mut app, 4);
    app.world_mut()
        .get_mut::<PlanarReflection>(floor)
        .unwrap()
        .enabled = false;
    run(&mut app, 2);
    let cameras = mirror_cameras(&mut app);
    assert_eq!(cameras.len(), 1);
    assert!(!cameras[0].2, "a disabled surface costs no render");
}
