//! The runtime systems in a headless app: an effect builds its renderers, fills
//! its meshes, obeys playback and script calls, reloads from a saved path, and
//! a one-shot cleans up after itself.
//!
//! Time is stepped manually at 60 Hz so the results do not depend on how fast
//! the test machine runs frames.

use std::collections::HashMap;
use std::time::Duration;

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use renzora::{ScriptAction, ScriptActionValue};
use renzora_particles::asset::parse_vfx;
use renzora_particles::presets::PRESETS;
use renzora_particles::render::ParticleRenderer;
use renzora_particles::{
    OneShot, ParticlePlayback, ParticleRuntime, ParticleStats, ParticleSystem, ParticlesPlugin, PlayState,
    VfxAsset,
};

fn app() -> App {
    let mut app = renzora_test_harness::headless_app_with(|app| {
        app.add_plugins(ParticlesPlugin);
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
    app
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn spawn_preset(app: &mut App, index: usize) -> Entity {
    let asset = parse_vfx(PRESETS[index].source).expect("preset parses");
    let handle = app.world_mut().resource_mut::<Assets<VfxAsset>>().add(asset);
    app.world_mut().spawn(ParticleSystem::from_handle(handle)).id()
}

fn alive(app: &App, e: Entity) -> u32 {
    app.world().get::<ParticleStats>(e).map(|s| s.alive).unwrap_or(0)
}

#[test]
fn an_effect_builds_renderers_and_fills_their_meshes() {
    let mut app = app();
    let e = spawn_preset(&mut app, 0);
    frames(&mut app, 30);

    let runtime = app.world().get::<ParticleRuntime>(e).expect("runtime built");
    assert_eq!(runtime.renderers.len(), 2, "one renderer per emitter");
    let renderer = runtime.renderers[0].expect("3D renderer in this build");
    assert!(alive(&app, e) > 0);

    let world = app.world();
    let r = world.get::<ParticleRenderer>(renderer).expect("renderer component");
    let mesh = world.resource::<Assets<Mesh>>().get(&r.mesh).expect("mesh");
    assert!(mesh.count_vertices() > 4, "particles were written");
    assert_eq!(world.get::<Visibility>(renderer), Some(&Visibility::Inherited));
    let aabb = world.get::<Aabb>(renderer).expect("aabb");
    assert!(aabb.half_extents.length() > 0.0);
    assert_eq!(world.get::<ChildOf>(renderer).map(|c| c.parent()), Some(e));
}

#[test]
fn playback_controls_work() {
    let mut app = app();
    let e = spawn_preset(&mut app, 0);
    frames(&mut app, 30);
    assert!(alive(&app, e) > 0);

    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().stop();
    frames(&mut app, 2);
    assert_eq!(alive(&app, e), 0, "stop clears");

    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().restart();
    frames(&mut app, 30);
    assert!(alive(&app, e) > 0, "restart plays again");

    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().pause();
    frames(&mut app, 1);
    let paused_at = app.world().get::<ParticleStats>(e).unwrap().time;
    frames(&mut app, 10);
    assert_eq!(app.world().get::<ParticleStats>(e).unwrap().time, paused_at, "pause freezes time");

    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().play();
    let before = alive(&app, e);
    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().burst(20);
    frames(&mut app, 1);
    assert!(alive(&app, e) >= before + 20);
}

#[test]
fn script_actions_drive_playback() {
    let mut app = app();
    let e = spawn_preset(&mut app, 0);
    frames(&mut app, 10);

    let action = |name: &str, args: HashMap<String, ScriptActionValue>| ScriptAction {
        name: name.to_string(),
        entity: e,
        target_entity: None,
        args,
    };
    app.world_mut().trigger(action("vfx_stop", HashMap::new()));
    frames(&mut app, 2);
    assert_eq!(app.world().get::<ParticlePlayback>(e).unwrap().state, PlayState::Stopped);
    assert_eq!(alive(&app, e), 0);

    app.world_mut().trigger(action("vfx_play", HashMap::new()));
    let mut args = HashMap::new();
    args.insert("scale".to_string(), ScriptActionValue::Float(0.5));
    app.world_mut().trigger(action("vfx_set_time_scale", args));
    let mut args = HashMap::new();
    args.insert("count".to_string(), ScriptActionValue::Int(7));
    app.world_mut().trigger(action("vfx_burst", args));
    frames(&mut app, 1);
    let playback = app.world().get::<ParticlePlayback>(e).unwrap();
    assert_eq!(playback.state, PlayState::Playing);
    assert_eq!(playback.time_scale, 0.5);
    assert!(alive(&app, e) >= 7);

    let extensions = app
        .world()
        .resource::<renzora::script_extension::ScriptExtensions>();
    for name in ["vfx_play", "vfx_pause", "vfx_stop", "vfx_restart", "vfx_burst", "vfx_set_time_scale"] {
        assert!(
            extensions.bindings().iter().any(|b| b.name == name),
            "{name} is not declared to the script backends"
        );
    }
}

#[test]
fn a_saved_path_loads_and_plays() {
    let mut app = app();
    // The default effect is an embedded path, exactly what a scene stores.
    let e = app.world_mut().spawn(ParticleSystem::default()).id();
    for _ in 0..400 {
        app.update();
        if alive(&app, e) > 0 {
            break;
        }
    }
    assert!(app.world().get::<ParticleRuntime>(e).is_some(), "the path never resolved to a runtime");
    assert!(alive(&app, e) > 0);
}

#[test]
fn the_component_persists_its_path_not_its_handle() {
    let system = ParticleSystem::from_path("effects/fire.vfx");
    let text = ron::to_string(&system).expect("serialize");
    assert!(text.contains("effects/fire.vfx"));
    assert!(!text.contains("asset"), "the handle is not saved: {text}");
    let back: ParticleSystem = ron::from_str(&text).expect("deserialize");
    assert_eq!(back.path, "effects/fire.vfx");

    let app = app();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    for type_id in [
        std::any::TypeId::of::<ParticleSystem>(),
        std::any::TypeId::of::<ParticlePlayback>(),
    ] {
        let registration = registry.get(type_id).expect("registered");
        assert!(registration.data::<ReflectComponent>().is_some());
        assert!(registration.data::<bevy::reflect::ReflectSerialize>().is_some());
    }
}

#[test]
fn a_finished_one_shot_despawns_when_asked() {
    let mut app = app();
    let e = spawn_preset(&mut app, 1);
    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().one_shot = OneShot::Despawn;
    frames(&mut app, 10);
    assert!(alive(&app, e) > 0);
    frames(&mut app, 180);
    assert!(app.world().get_entity(e).is_err(), "the one-shot entity should be gone");
}

/// An effect of five particles burst at once, drawn as `render`.
fn spawn_burst(app: &mut App, render: &str) -> Entity {
    let source = format!(
        r#"(emitters: [(capacity: 16, backend: Cpu, graph: (nodes: [(id: 1, node_type: "output/particle", input_values: {{
            "spawn_rate": Float(0.0), "burst_count": Float(5.0), "lifetime": Float(10.0)
        }})]), render: {render})])"#
    );
    let asset = parse_vfx(&source).expect("effect parses");
    let handle = app.world_mut().resource_mut::<Assets<VfxAsset>>().add(asset);
    app.world_mut().spawn(ParticleSystem::from_handle(handle)).id()
}

/// A mesh emitter's renderer gets its built-in template and writes one copy
/// of the mesh per particle, with the mesh-particle vertex columns.
#[test]
fn mesh_particles_draw_a_copy_of_their_mesh_per_particle() {
    use renzora_particles::render::{MeshParticles, ATTRIBUTE_PARTICLE_LOCAL};
    let mut app = app();
    let e = spawn_burst(&mut app, "(mesh: Some(Cube), orient: Velocity, lighting: (lit: true))");
    frames(&mut app, 10);
    assert_eq!(alive(&app, e), 5);
    let renderer = app.world().get::<ParticleRuntime>(e).unwrap().renderers[0].expect("renderer");
    let world = app.world();
    let template = world.get::<MeshParticles>(renderer).and_then(|m| m.template.clone()).expect("template resolved");
    let r = world.get::<ParticleRenderer>(renderer).unwrap();
    let mesh = world.resource::<Assets<Mesh>>().get(&r.mesh).unwrap();
    assert_eq!(mesh.count_vertices(), 5 * template.vertex_count());
    assert!(mesh.attribute(ATTRIBUTE_PARTICLE_LOCAL).is_some());
    assert_eq!(world.get::<Visibility>(renderer), Some(&Visibility::Inherited));
}

/// Particles wearing a project material, mesh copies and quads alike, are
/// worn through a `MaterialRef` for the engine's resolver and a surface slot
/// that holds their particle side until the resolver hands the material
/// over; the mesh is the particle shader's own, velocity column included. A
/// size in pixels turns frustum culling off.
#[test]
fn a_project_material_is_worn_through_a_material_ref() {
    use renzora_particles::render::surface::ParticleSurfaceSlot;
    use renzora_particles::render::{MeshParticles, ATTRIBUTE_PARTICLE_LOCAL, ATTRIBUTE_PARTICLE_VELOCITY};
    let mut app = app();
    let mesh_e = spawn_burst(&mut app, r#"(mesh: Some(Sphere), material: Some("materials/rock.material"), screen_space_size: true)"#);
    let quad_e = spawn_burst(&mut app, r#"(material: Some("materials/spark.material"), blend: Additive)"#);
    frames(&mut app, 10);
    for (e, path) in [(mesh_e, "materials/rock.material"), (quad_e, "materials/spark.material")] {
        let renderer = app.world().get::<ParticleRuntime>(e).unwrap().renderers[0].expect("renderer");
        let world = app.world();
        assert_eq!(world.get::<renzora::MaterialRef>(renderer).map(|m| m.0.as_str()), Some(path));
        assert!(world.get::<MeshMaterial3d<renzora_particles::render::ParticleMaterial3d>>(renderer).is_none());
        let slot = world.get::<ParticleSurfaceSlot>(renderer).expect("surface slot");
        assert!(!slot.adopted(), "nothing resolved it in a test app");
        let r = world.get::<ParticleRenderer>(renderer).unwrap();
        let mesh = world.resource::<Assets<Mesh>>().get(&r.mesh).unwrap();
        assert!(mesh.attribute(ATTRIBUTE_PARTICLE_VELOCITY).is_some(), "{path}");
        if e == mesh_e {
            assert!(world.get::<bevy::camera::visibility::NoFrustumCulling>(renderer).is_some());
            let m = world.get::<MeshParticles>(renderer).expect("mesh particles");
            let template = m.template.clone().expect("template resolved");
            assert_eq!(mesh.count_vertices(), 5 * template.vertex_count());
            assert!(mesh.attribute(ATTRIBUTE_PARTICLE_LOCAL).is_some());
        } else {
            assert_eq!(mesh.count_vertices(), 5 * 4, "five quads");
        }
    }
}

/// Solid blends cast shadows by default and blended ones do not; `On` and
/// `Off` override either way.
#[test]
fn shadow_casting_follows_the_blend_and_the_setting() {
    use bevy::light::NotShadowCaster;
    let mut app = app();
    let cases = [
        ("(blend: Opaque)", true),
        ("(blend: AlphaMask)", true),
        ("(blend: AlphaToCoverage)", true),
        ("(blend: Alpha)", false),
        ("(blend: Multiply)", false),
        ("(blend: Alpha, shadows: On)", true),
        ("(blend: Opaque, shadows: Off)", false),
    ];
    let entities: Vec<Entity> = cases.iter().map(|(render, _)| spawn_burst(&mut app, render)).collect();
    frames(&mut app, 5);
    for ((render, casts), e) in cases.iter().zip(entities) {
        let renderer = app.world().get::<ParticleRuntime>(e).unwrap().renderers[0].expect("renderer");
        assert_eq!(app.world().get::<NotShadowCaster>(renderer).is_none(), *casts, "{render}");
    }
}

/// Where Bevy draws a material on a camera, as its queue systems decide it
/// (`queue_material_meshes` and `queue_prepass_material_meshes`): a
/// transparent alpha mode in the forward transparent pass whatever its
/// method, an opaque or masked one in the G-buffer when its method, `Auto`
/// resolved through `DefaultOpaqueRendererMethod`, is deferred and the camera
/// has a deferred prepass, and in the forward opaque pass otherwise.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Pass {
    GBuffer,
    ForwardOpaque,
    ForwardTransparent,
}

fn pass(alpha: AlphaMode, method: bevy::material::OpaqueRendererMethod, default_deferred: bool, camera_deferred: bool) -> Pass {
    use bevy::material::OpaqueRendererMethod;
    if matches!(alpha, AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add | AlphaMode::Multiply) {
        return Pass::ForwardTransparent;
    }
    let deferred = match method {
        OpaqueRendererMethod::Auto => default_deferred,
        OpaqueRendererMethod::Deferred => true,
        OpaqueRendererMethod::Forward => false,
    };
    if deferred && camera_deferred {
        Pass::GBuffer
    } else {
        Pass::ForwardOpaque
    }
}

/// On a deferred camera the solid blends go to the G-buffer and the
/// transparent ones stay forward, on the built-in material and on a project
/// material's slot; `Forward` pins a solid particle forward; and an explicit
/// `Deferred` is honoured only while every 3D camera has a deferred prepass,
/// so a forward camera arriving demotes it (Bevy cannot draw a deferred
/// material there) and leaves it to draw forward.
///
/// Headless, so the render world's phases are not there to inspect: the test
/// reads the material each renderer hands Bevy (its alpha mode and render
/// method) and routes it the way Bevy's queue systems do ([`pass`]).
#[test]
fn a_deferred_camera_routes_solid_particles_to_the_gbuffer() {
    use bevy::core_pipeline::prepass::DeferredPrepass;
    use bevy::pbr::Material;
    use renzora_particles::render::surface::ParticleSurfaceSlot;
    use renzora_particles::render::ParticleMaterial3d;
    use renzora_particles::RenderMethod;

    let mut app = app();
    let camera = app.world_mut().spawn((Camera3d::default(), DeferredPrepass)).id();
    let cases = [
        ("(blend: Opaque)", Pass::GBuffer),
        ("(blend: AlphaMask)", Pass::GBuffer),
        ("(blend: AlphaToCoverage, mesh: Some(Cube), lighting: (lit: true))", Pass::GBuffer),
        ("(blend: Opaque, shading: ProceduralFlame)", Pass::GBuffer),
        ("(blend: Opaque, render_method: Deferred)", Pass::GBuffer),
        ("(blend: Opaque, render_method: Forward)", Pass::ForwardOpaque),
        ("(blend: Alpha)", Pass::ForwardTransparent),
        ("(blend: Additive, render_method: Deferred)", Pass::ForwardTransparent),
        ("(blend: Premultiplied)", Pass::ForwardTransparent),
        ("(blend: Multiply)", Pass::ForwardTransparent),
    ];
    let entities: Vec<Entity> = cases.iter().map(|(render, _)| spawn_burst(&mut app, render)).collect();
    let solid_material = spawn_burst(&mut app, r#"(blend: Opaque, material: Some("materials/rock.material"))"#);
    let blended_material = spawn_burst(&mut app, r#"(blend: Alpha, material: Some("materials/smoke.material"))"#);
    frames(&mut app, 5);

    let renderer = |app: &App, e: Entity| app.world().get::<ParticleRuntime>(e).unwrap().renderers[0].expect("renderer");
    let material = |app: &App, e: Entity| {
        let handle = app.world().get::<MeshMaterial3d<ParticleMaterial3d>>(renderer(app, e)).expect("built-in material").0.clone();
        app.world().resource::<Assets<ParticleMaterial3d>>().get(&handle).expect("material").clone()
    };
    // A deferred project: what the engine sets whenever its cameras carry a
    // deferred prepass, so `Auto` resolves deferred.
    for ((render, expected), e) in cases.iter().zip(&entities) {
        let m = material(&app, *e);
        assert_eq!(pass(m.alpha_mode(), m.opaque_render_method(), true, true), *expected, "{render}");
        // A forward project's camera sees every one of them exactly as before.
        let forward = pass(m.alpha_mode(), m.opaque_render_method(), false, false);
        let before = if *expected == Pass::ForwardTransparent { Pass::ForwardTransparent } else { Pass::ForwardOpaque };
        assert_eq!(forward, before, "{render} on a forward camera");
    }
    for (e, expected) in [(solid_material, Pass::GBuffer), (blended_material, Pass::ForwardTransparent)] {
        let slot = app.world().get::<ParticleSurfaceSlot>(renderer(&app, e)).expect("surface slot").clone();
        let alpha = renzora_particles::render::material::alpha_mode_3d(slot.blend, slot.cutoff);
        assert_eq!(pass(alpha, slot.route.opaque(), true, true), expected);
    }

    // A forward camera joins: the explicit Deferred request can no longer be
    // drawn by every camera, so it draws forward; Auto still follows the
    // project and Forward stays forward.
    let explicit = entities[4];
    app.world_mut().spawn(Camera3d::default());
    frames(&mut app, 1);
    let m = material(&app, explicit);
    assert_eq!(m.route.requested, RenderMethod::Deferred);
    assert_eq!(m.route.method, RenderMethod::Forward);
    assert_eq!(pass(m.alpha_mode(), m.opaque_render_method(), true, true), Pass::ForwardOpaque);
    assert_eq!(material(&app, entities[0]).route.method, RenderMethod::Auto);
    // And it is deferred again once every camera has the prepass.
    let forward_camera = app.world_mut().query_filtered::<Entity, (With<Camera3d>, Without<DeferredPrepass>)>().single(app.world()).unwrap();
    app.world_mut().entity_mut(forward_camera).insert(DeferredPrepass);
    frames(&mut app, 1);
    assert_eq!(material(&app, explicit).route.method, RenderMethod::Deferred);
    let _ = camera;
}

/// Pausing an effect stops its motion vectors: the uniform's playback rate
/// follows the playback, so a frozen effect does not smear under TAA.
#[test]
fn the_motion_vector_rate_follows_playback() {
    use renzora_particles::render::{ParticleMaterial3d, ParticleMotion};
    let mut app = app();
    let e = spawn_burst(&mut app, "(blend: Opaque)");
    frames(&mut app, 5);
    let renderer = app.world().get::<ParticleRuntime>(e).unwrap().renderers[0].expect("renderer");
    let rate = |app: &App| {
        let handle = app.world().get::<MeshMaterial3d<ParticleMaterial3d>>(renderer).unwrap().0.clone();
        app.world().resource::<Assets<ParticleMaterial3d>>().get(&handle).unwrap().params.motion.x
    };
    assert_eq!(rate(&app), 1.0);
    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().pause();
    frames(&mut app, 2);
    assert_eq!(rate(&app), 0.0, "paused");
    assert_eq!(app.world().get::<ParticleMotion>(renderer).map(|m| m.rate), Some(0.0));
    let mut playback = app.world_mut().get_mut::<ParticlePlayback>(e).unwrap();
    playback.play();
    playback.time_scale = 0.5;
    frames(&mut app, 2);
    assert_eq!(rate(&app), 0.5, "half speed");
}

/// The editor's preview puts its effect on a private layer; a renderer child
/// left on layer 0 drew the preview's particles into the scene viewport.
#[test]
fn renderers_inherit_the_effects_render_layers() {
    use bevy::camera::visibility::RenderLayers;
    let mut app = app();
    let e = spawn_preset(&mut app, 0);
    app.world_mut().entity_mut(e).insert(RenderLayers::layer(19));
    frames(&mut app, 5);
    let runtime = app.world().get::<ParticleRuntime>(e).expect("runtime");
    for r in runtime.renderers.iter().flatten() {
        assert_eq!(app.world().get::<RenderLayers>(*r), Some(&RenderLayers::layer(19)));
    }
}

/// The editor applies an edit by overwriting the asset in `Assets<VfxAsset>`;
/// every instance must rebuild from it without being touched itself.
#[test]
fn editing_the_asset_in_place_rebuilds_its_instances() {
    let mut app = app();
    let e = spawn_preset(&mut app, 0);
    frames(&mut app, 10);
    let handle = app.world().get::<ParticleSystem>(e).unwrap().asset.clone();
    let emitters_before = app.world().get::<ParticleRuntime>(e).unwrap().renderers.len();
    {
        let mut assets = app.world_mut().resource_mut::<Assets<VfxAsset>>();
        let mut asset = assets.get_mut(&handle).expect("asset");
        asset.emitters.truncate(1);
    }
    frames(&mut app, 2);
    let after = app.world().get::<ParticleRuntime>(e).unwrap().renderers.len();
    assert_eq!(emitters_before, 2);
    assert_eq!(after, 1, "the instance rebuilt from the edited asset");
}

#[test]
fn removing_the_component_removes_the_renderers() {
    let mut app = app();
    let e = spawn_preset(&mut app, 2);
    frames(&mut app, 5);
    let renderers: Vec<Entity> = app
        .world()
        .get::<ParticleRuntime>(e)
        .expect("runtime")
        .renderers
        .iter()
        .flatten()
        .copied()
        .collect();
    assert!(!renderers.is_empty());
    app.world_mut().entity_mut(e).remove::<ParticleSystem>();
    frames(&mut app, 2);
    for r in renderers {
        assert!(app.world().get_entity(r).is_err());
    }
    assert!(app.world().get::<ParticleRuntime>(e).is_none());
}

/// The campfire's light is a hidden, unsaved, unselectable `PointLight` child
/// that comes on once its flames burn, follows the effect's layers, and goes
/// with the effect.
#[test]
fn a_fire_lights_the_scene_with_a_hidden_point_light_child() {
    use bevy::camera::visibility::RenderLayers;
    let mut app = app();
    let e = spawn_preset(&mut app, 0);
    app.world_mut().entity_mut(e).insert(RenderLayers::layer(3));
    frames(&mut app, 90);
    let runtime = app.world().get::<ParticleRuntime>(e).expect("runtime");
    let lights: Vec<Entity> = runtime.lights.iter().flatten().flat_map(|l| l.entities.iter().copied()).collect();
    assert_eq!(lights.len(), 1, "the campfire has one light");
    let light = lights[0];
    let world = app.world();
    assert_eq!(world.get::<ChildOf>(light).map(|c| c.parent()), Some(e));
    assert!(world.get::<renzora::HideInHierarchy>(light).is_some(), "hidden, so never saved");
    assert!(world.get::<renzora::NoSelectionBounds>(light).is_some());
    assert_eq!(world.get::<RenderLayers>(light), Some(&RenderLayers::layer(3)));
    let point = world.get::<PointLight>(light).expect("a point light");
    assert!(point.intensity > 10_000.0, "lit by the flames: {}", point.intensity);
    assert!(!point.shadow_maps_enabled, "no shadow maps unless asked for");
    assert_eq!(world.get::<Visibility>(light), Some(&Visibility::Inherited));
    let at = world.get::<Transform>(light).expect("transform").translation;
    assert!((at.y - 0.4).abs() < 1e-4, "at flame height: {at}");

    // Stopped, the fire goes out and its light with it.
    app.world_mut().get_mut::<ParticlePlayback>(e).unwrap().stop();
    frames(&mut app, 30);
    assert_eq!(app.world().get::<Visibility>(light), Some(&Visibility::Hidden));

    app.world_mut().entity_mut(e).remove::<ParticleSystem>();
    frames(&mut app, 2);
    assert!(app.world().get_entity(light).is_err(), "the light goes with the effect");
}
