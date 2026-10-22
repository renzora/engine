//! `stress/`: heavy effects for profiling, each built to load one part of the
//! system hard so a frame-time regression points at its cause: raw GPU
//! simulation, ribbon geometry, chained GPU sub-emitters, lit shadowed mesh
//! particles, many particle lights, sorted lit alpha, and the CPU collision
//! path. Counts are steady-state (rate times lifetime), sized so none drops a
//! spawn. Not for shipping scenes.

use super::*;

/// A two-armed spiral galaxy of about 230000 GPU particles, 12 m across: a
/// flat disc spun by Orbit and pulled in by a softened Gravity Well, so the
/// inner stars lap the outer ones and wind the arms. Raw simulation and
/// additive fill rate; nothing else.
fn galaxy() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(80000.0));
    let (pos, _) = bed(&mut b, [12.0, 0.25, 12.0]);
    let pos = lift(&mut b, pos, 3.0);
    b.output("position", pos);
    let life = rand(&mut b, 2.4, 2.9);
    b.output("lifetime", life);
    b.output("size", f(0.025));
    let hue = rand_color(&mut b, [0.5, 0.7, 2.0, 1.0], [2.0, 0.6, 1.4, 1.0]);
    b.output("color", hue);
    let fade = fade_in_out(&mut b, 0.15, 0.75);
    b.output("color_over_life", fade);
    b.output("emission", f(1.5));
    let spin = b.op(
        "force/orbit",
        &[("center", v3(0.0, 3.0, 0.0)), ("axis", v3(0.0, 1.0, 0.0)), ("speed", f(1.4)), ("grip", f(2.0))],
        "force",
    );
    let well = b.op(
        "force/gravity_well",
        &[("point", v3(0.0, 3.0, 0.0)), ("strength", f(3.0)), ("softening", f(1.0)), ("max_force", f(6.0))],
        "force",
    );
    accelerate(&mut b, vec![spin, well]);
    let mut stars = emitter("stars", 240000, World, 2001, b.finish(), dot_render(BlendMode::Additive));
    stars.backend = Backend::Gpu;
    effect("Stress: Galaxy 230k", 0.0, true, vec![stars])
}

/// About 3900 particles each dragging a 24-point tube trail (6 sides) wearing
/// the energy ribbon material: over half a million triangles rebuilt every
/// frame, swarming on strong curl noise held round a point by an attractor.
fn ribbon_swarm() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(1300.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(3.0))]);
    let pos = lift(&mut b, pos, 3.0);
    b.output("position", pos);
    let life = rand(&mut b, 2.5, 3.0);
    b.output("lifetime", life);
    b.output("size", f(0.03));
    let hue = rand_color(&mut b, [0.3, 1.2, 2.5, 1.0], [2.5, 0.5, 1.5, 1.0]);
    b.output("color", hue);
    let fade = fade_in_out(&mut b, 0.1, 0.8);
    b.output("color_over_life", fade);
    let swarm = curl(&mut b, 6.0, 0.5, [0.0, 0.5, 0.0]);
    let pull = b.op("force/attractor", &[("point", v3(0.0, 3.0, 0.0)), ("strength", f(2.0))], "force");
    accelerate(&mut b, vec![swarm, pull]);
    let tubes = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon {
            kind: RibbonKind::Trail,
            points: 24,
            min_distance: 0.04,
            tail_width: 0.2,
            shape: RibbonShape::Tube,
            radial_segments: 6,
            ..Default::default()
        },
        material: mat("fx_energy_ribbon.material"),
        ..Default::default()
    };
    let mut swarm = emitter("swarm", 4200, World, 2011, b.finish(), tubes);
    swarm.backend = Backend::Gpu;
    effect("Stress: Ribbon Swarm", 0.0, true, vec![swarm])
}

/// Chained GPU sub-emitters: 20 rockets a second each pop into 150 ribbon
/// sparks, and every spark splits into two crackles as it dies. About 4500
/// sparks and 1800 crackles alive, all three levels on the GPU (a sub-emitter
/// rule keeps its emitters on one backend).
fn firework_cascade() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(20.0));
    let (pos, _) = bed(&mut b, [20.0, 0.0, 20.0]);
    b.output("position", pos);
    let vel = rand3(&mut b, [-2.0, 12.0, -2.0], [2.0, 17.0, 2.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 1.0, 1.3);
    b.output("lifetime", life);
    b.output("size", f(0.08));
    let hue = rand_color(&mut b, [3.0, 0.5, 0.35, 1.0], [0.45, 1.3, 3.0, 1.0]);
    b.output("color", hue);
    b.output("emission", f(3.0));
    let mut rockets = emitter("rockets", 40, World, 2021, b.finish(), dot_render(BlendMode::Additive));
    rockets.backend = Backend::Gpu;
    rockets.sub_emitters = vec![sub(SubEmitterTrigger::OnDeath, 1, 150, 0.15, true)];

    let mut b = B::default();
    let vel = burst_velocity(&mut b, (6.0, 9.0), [0.0, 0.0, 0.0]);
    b.output("velocity", vel);
    b.output("drag", f(1.4));
    b.output("gravity", v3(0.0, -3.5, 0.0));
    let life = rand(&mut b, 1.0, 1.5);
    b.output("lifetime", life);
    b.output("size", f(0.05));
    b.output("color", col([1.0, 1.0, 1.0, 1.0]));
    test_fade(&mut b);
    let e = glow(&mut b, 5.0, 1.5);
    b.output("emission", e);
    let ribbons = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Trail, points: 8, min_distance: 0.06, ..Default::default() },
        ..Default::default()
    };
    let mut sparks = emitter("sparks", 6000, World, 2022, b.finish(), ribbons);
    sparks.backend = Backend::Gpu;
    sparks.sub_emitters = vec![sub(SubEmitterTrigger::OnDeath, 2, 2, 0.3, true)];

    let mut b = B::default();
    let vel = burst_velocity(&mut b, (0.5, 1.5), [0.0, 0.0, 0.0]);
    b.output("velocity", vel);
    let life = rand(&mut b, 0.15, 0.35);
    b.output("lifetime", life);
    b.output("size", f(0.12));
    b.output("color", col([1.0, 1.0, 1.0, 1.0]));
    let f_ = flicker(&mut b, (40.0, 70.0), 0.0, 3.0);
    b.output("emission", f_);
    test_fade(&mut b);
    let star = RenderSettings { texture: tex("star.svg"), blend: BlendMode::Additive, ..Default::default() };
    let mut crackle = emitter("crackle", 3600, World, 2023, b.finish(), star);
    crackle.backend = Backend::Gpu;

    effect("Stress: Firework Cascade", 0.0, true, vec![rockets, sparks, crackle])
}

/// A tornado of about 39000 lit, shadow-casting crystal shards (tetrahedra
/// wearing the crystal material) on the GPU: a vortex spinning them up a
/// 12 m funnel. Mesh instancing, the material pass and shadow maps together.
fn mesh_storm() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(13000.0));
    let (pos, _) = bed(&mut b, [8.0, 0.2, 8.0]);
    b.output("position", pos);
    b.output("gravity", v3(0.0, 2.5, 0.0));
    b.output("drag", f(0.6));
    let life = rand(&mut b, 2.6, 3.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.05, 0.12);
    b.output("size", size);
    random_spin(&mut b, 12.0);
    let twist = b.op("force/vortex", &[("center", v3(0.0, 0.0, 0.0)), ("axis", v3(0.0, 1.0, 0.0)), ("strength", f(4.0))], "force");
    let funnel = b.op(
        "force/orbit",
        &[("center", v3(0.0, 0.0, 0.0)), ("axis", v3(0.0, 1.0, 0.0)), ("speed", f(2.0)), ("grip", f(1.5)), ("radius", f(2.0)), ("radial_pull", f(2.0))],
        "force",
    );
    accelerate(&mut b, vec![twist, funnel]);
    let look = RenderSettings {
        blend: BlendMode::Opaque,
        mesh: Some(ParticleMesh::Tetrahedron),
        orient: MeshOrient::Tumble,
        material: mat("fx_crystal_shard.material"),
        shadows: ParticleShadows::On,
        ..Default::default()
    };
    let mut shards = emitter("shards", 42000, World, 2031, b.finish(), look);
    shards.backend = Backend::Gpu;
    effect("Stress: Mesh Storm 39k", 0.0, true, vec![shards])
}

/// About 2000 fireflies over 20 x 20 m with the 64 brightest casting point
/// lights at once: the particle-light gather and the scene's clustered
/// lighting under load.
fn light_swarm() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(700.0));
    let (pos, _) = bed(&mut b, [20.0, 3.0, 20.0]);
    let pos = lift(&mut b, pos, 1.8);
    b.output("position", pos);
    let life = rand(&mut b, 2.6, 3.0);
    b.output("lifetime", life);
    b.output("size", f(0.05));
    let hue = rand_color(&mut b, [1.6, 2.2, 0.4, 1.0], [0.4, 1.6, 2.4, 1.0]);
    b.output("color", hue);
    let blink = flicker(&mut b, (2.0, 6.0), 0.0, 3.0);
    b.output("emission", blink);
    test_fade(&mut b);
    let wander = curl(&mut b, 1.2, 0.4, [0.1, 0.0, 0.1]);
    b.output("acceleration", wander);
    let mut flies = emitter("fireflies", 2200, World, 2041, b.finish(), dot_render(BlendMode::Additive));
    flies.light = ParticleLight {
        mode: LightMode::BrightestN(64),
        color: [0.8, 1.0, 0.5],
        intensity: 600.0,
        range: 4.0,
        radius: 0.05,
        ..Default::default()
    };
    effect("Stress: Light Swarm", 0.0, true, vec![flies])
}

/// A 20 m wall of about 12000 lit, shadow-casting smoke sprites sorted back to
/// front every frame: the sort, overdraw of large alpha sprites, soft
/// particles and lit flipbook shading at once.
fn smoke_wall() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(3000.0));
    let (pos, _) = bed(&mut b, [20.0, 0.5, 4.0]);
    b.output("position", pos);
    let rise = rand3(&mut b, [-0.2, 0.6, -0.2], [0.2, 1.4, 0.2]);
    b.output("velocity", rise);
    b.output("drag", f(0.4));
    let life = rand(&mut b, 3.6, 4.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.8, 1.4);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.6), ease(1.0, 2.4)]);
    b.output("size_over_life", grow);
    b.output("color", col([0.5, 0.48, 0.45, 0.5]));
    let fade = fade_in_out(&mut b, 0.1, 0.5);
    b.output("color_over_life", fade);
    random_spin(&mut b, 0.4);
    let swirl = curl(&mut b, 0.5, 0.3, [0.0, 0.3, 0.0]);
    let w = wind(&mut b, 0.6, 0.0);
    accelerate(&mut b, vec![swirl, w]);
    let look = RenderSettings {
        texture: tex("smoke_wisp_sheet.png"),
        sprite_sheet: SpriteSheet { columns: 4, rows: 4, mode: SheetMode::OverLifetime },
        frame_blend: true,
        blend: BlendMode::Alpha,
        soft_distance: 0.5,
        lighting: Lighting { lit: true, roughness: 1.0, metallic: 0.0 },
        shadows: ParticleShadows::On,
        alpha_cutoff: 0.35,
        draw_order: DrawOrder::ViewDepth,
        ..Default::default()
    };
    let smoke = emitter("smoke", 13000, World, 2051, b.finish(), look);
    effect("Stress: Sorted Smoke Wall", 0.0, true, vec![smoke])
}

/// About 20000 balls forced onto the CPU backend, raining onto a ground
/// plane, a sphere and a box and bouncing between them, drawn as lit sphere
/// meshes: the CPU interpreter, the collision nodes and CPU mesh upload.
fn cpu_collisions() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(6000.0));
    let (pos, _) = bed(&mut b, [8.0, 0.5, 8.0]);
    let pos = lift(&mut b, pos, 6.0);
    b.output("position", pos);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 3.0, 3.3);
    b.output("lifetime", life);
    b.output("size", f(0.07));
    let hue = rand_color(&mut b, [0.9, 0.2, 0.15, 1.0], [0.15, 0.4, 0.95, 1.0]);
    b.output("color", hue);
    ground(&mut b, 0.6, 0.1, false);
    let ball = b.node("collide/sphere");
    b.set(ball, "center", PinValue::Vec3([0.0, 0.0, 0.0]));
    b.set(ball, "radius", PinValue::Float(1.8));
    b.set(ball, "bounce", PinValue::Float(0.6));
    let block = b.node("collide/box");
    b.set(block, "center", PinValue::Vec3([2.5, 0.5, 2.5]));
    b.set(block, "size", PinValue::Vec3([1.5, 1.0, 1.5]));
    b.set(block, "bounce", PinValue::Float(0.6));
    let look = RenderSettings {
        blend: BlendMode::Opaque,
        mesh: Some(ParticleMesh::Sphere),
        lighting: Lighting { lit: true, roughness: 0.4, metallic: 0.0 },
        ..Default::default()
    };
    let mut balls = emitter("balls", 21000, World, 2061, b.finish(), look);
    balls.backend = Backend::Cpu;
    effect("Stress: CPU Collisions 20k", 0.0, true, vec![balls])
}

pub(super) fn entries() -> Vec<Entry> {
    vec![
        entry("stress", "galaxy", "About 230000 GPU particles in a spinning two-armed galaxy, 12 m across: raw simulation and additive fill.", galaxy),
        entry("stress", "ribbon_swarm", "About 3900 tube trails (24 points, 6 sides, material): over half a million triangles rebuilt each frame. GPU.", ribbon_swarm),
        entry("stress", "firework_cascade", "Three levels of GPU sub-emitters: 20 rockets/s into 150 ribbon sparks each, every spark splitting into crackles.", firework_cascade),
        entry("stress", "mesh_storm", "About 39000 lit, shadow-casting crystal shard meshes (material) in a 12 m tornado. GPU.", mesh_storm),
        entry("stress", "light_swarm", "About 2000 fireflies over 20 x 20 m, the 64 brightest casting point lights.", light_swarm),
        entry("stress", "smoke_wall", "A 20 m wall of about 12000 lit, shadowed flipbook smoke sprites sorted back to front.", smoke_wall),
        entry("stress", "cpu_collisions", "About 20000 lit sphere meshes on the CPU backend bouncing off a ground plane, a sphere and a box.", cpu_collisions),
    ]
}
