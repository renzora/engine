//! `popular/`: the effects most games reach for first (an explosion,
//! fireworks, confetti, a healing aura, sparks, coins, hearts, petals, snow, a
//! shield, a teleport), built from the showcase art: SVG sprites, the fire and
//! smoke flipbooks, and the particle materials. Like the showcase set, a
//! missing texture or material draws the plain particle look instead of
//! failing, so these validate without the project's art present.

use super::*;

/// Every emitter of a one-shot effect bursts together: `count` at once, again
/// every `interval` seconds, forever (the effect loops).
fn bursts(b: &mut B, count: f32, interval: f32) {
    b.output("spawn_rate", f(0.0));
    b.output("burst_count", f(count));
    b.output("burst_interval", f(interval));
    b.output("burst_cycles", f(0.0));
}

/// A flat textured quad lying on the ground (normal +Y), for rings and decals.
fn ground_decal(texture: &str) -> RenderSettings {
    RenderSettings {
        texture: tex(texture),
        blend: BlendMode::Additive,
        mesh: Some(ParticleMesh::Quad),
        orient: MeshOrient::Axis([0.0, 1.0, 0.0]),
        ..Default::default()
    }
}

/// A camera-facing SVG or PNG sprite.
fn sprite(texture: &str, blend: BlendMode) -> RenderSettings {
    RenderSettings { texture: tex(texture), blend, ..Default::default() }
}

fn light(color: [f32; 3], intensity: f32, range: f32, flicker: f32, height: f32) -> ParticleLight {
    ParticleLight {
        mode: LightMode::PerEmitter,
        color,
        intensity,
        range,
        radius: 0.2,
        smoothing: 0.08,
        flicker,
        flicker_rate: 9.0,
        offset: [0.0, height, 0.0],
        ..Default::default()
    }
}

/// A ground explosion every 2.5 s, origin on the ground. A white flash that
/// lights the street, a fireball of flipbook flame billowing up and out, a
/// shockwave ring racing across the ground, molten debris (the cooling-lava
/// material) bouncing out, spark ribbons, and dark lit smoke that lingers and
/// drifts off on the wind. The 2.5 s repeat keeps a burst inside the
/// validator's last second, where it checks the light.
fn explosion() -> VfxAsset {
    const EVERY: f32 = 2.5;

    let mut b = B::default();
    bursts(&mut b, 1.0, EVERY);
    b.output("position", v3(0.0, 1.0, 0.0));
    b.output("lifetime", f(0.22));
    b.output("size", f(5.0));
    b.output("color", col([3.0, 2.0, 1.0, 1.0]));
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 0.6, 0.3, 0.0])]);
    b.output("color_over_life", fade);
    let e = glow(&mut b, 10.0, 0.0);
    b.output("emission", e);
    let mut flash = emitter("flash", 4, World, 901, b.finish(), dot_render(BlendMode::Additive));
    // One particle, weighted by its HDR brightness (about 16 at the peak of
    // the emission curve), so 8000 is a brief flash of roughly 130000 lm.
    flash.light = light([1.0, 0.6, 0.3], 8000.0, 25.0, 0.0, 1.0);

    let mut b = B::default();
    bursts(&mut b, 18.0, EVERY);
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(0.4))]);
    let pos = lift(&mut b, pos, 0.8);
    b.output("position", pos);
    let vel = burst_velocity(&mut b, (1.5, 4.5), [0.0, 1.5, 0.0]);
    b.output("velocity", vel);
    b.output("drag", f(3.0));
    b.output("gravity", v3(0.0, 1.2, 0.0));
    let life = rand(&mut b, 0.9, 1.4);
    b.output("lifetime", life);
    let size = rand(&mut b, 1.2, 1.9);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.5), ease(1.0, 1.8)]);
    b.output("size_over_life", grow);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (0.6, [1.0, 0.85, 0.7, 0.85]), (1.0, [0.8, 0.5, 0.4, 0.0])]);
    b.output("color_over_life", fade);
    let e = glow(&mut b, 3.5, 0.6);
    b.output("emission", e);
    random_spin(&mut b, 1.2);
    let fireball_look = RenderSettings {
        texture: tex("fire_flipbook.png"),
        sprite_sheet: SpriteSheet { columns: 8, rows: 8, mode: SheetMode::OverLifetime },
        frame_blend: true,
        blend: BlendMode::Premultiplied,
        soft_distance: 0.4,
        ..Default::default()
    };
    let fireball = emitter("fireball", 48, World, 902, b.finish(), fireball_look);

    let mut b = B::default();
    bursts(&mut b, 1.0, EVERY);
    b.output("position", v3(0.0, 0.05, 0.0));
    b.output("lifetime", f(0.7));
    b.output("size", f(1.0));
    let grow = curve(&mut b, &[ease_out(0.0, 0.5), ease(1.0, 14.0)]);
    b.output("size_over_life", grow);
    b.output("color", col([2.0, 1.4, 0.9, 1.0]));
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    b.output("color_over_life", fade);
    let shockwave = emitter("shockwave", 4, World, 903, b.finish(), ground_decal("shockwave_ring.svg"));

    let mut b = B::default();
    bursts(&mut b, 30.0, EVERY);
    b.output("position", v3(0.0, 0.4, 0.0));
    let vel = burst_velocity(&mut b, (4.0, 9.0), [0.0, 4.0, 0.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 2.5, 3.5);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.08, 0.2);
    b.output("size", size);
    random_spin(&mut b, 10.0);
    let air = drag(&mut b, 0.0, 0.04);
    b.output("acceleration", air);
    ground(&mut b, 0.3, 0.6, false);
    let rock = RenderSettings {
        blend: BlendMode::Opaque,
        mesh: Some(ParticleMesh::Tetrahedron),
        orient: MeshOrient::Tumble,
        material: mat("fx_molten_debris.material"),
        ..Default::default()
    };
    let debris = emitter("debris", 80, World, 904, b.finish(), rock);

    let mut b = B::default();
    bursts(&mut b, 70.0, EVERY);
    b.output("position", v3(0.0, 0.6, 0.0));
    let vel = burst_velocity(&mut b, (7.0, 15.0), [0.0, 3.0, 0.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("drag", f(0.6));
    let life = rand(&mut b, 0.6, 1.4);
    b.output("lifetime", life);
    b.output("size", f(0.035));
    b.output("color", col([3.0, 1.6, 0.6, 1.0]));
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (0.7, [1.0, 0.7, 0.4, 1.0]), (1.0, [1.0, 0.4, 0.2, 0.0])]);
    b.output("color_over_life", fade);
    let e = glow(&mut b, 7.0, 1.0);
    b.output("emission", e);
    ground(&mut b, 0.4, 0.3, false);
    let streaks = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Trail, points: 10, min_distance: 0.05, ..Default::default() },
        ..Default::default()
    };
    let sparks = emitter("sparks", 160, World, 905, b.finish(), streaks);

    let mut b = B::default();
    bursts(&mut b, 16.0, EVERY);
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(0.7))]);
    let pos = lift(&mut b, pos, 1.0);
    b.output("position", pos);
    let vel = burst_velocity(&mut b, (0.6, 1.6), [0.0, 0.9, 0.0]);
    b.output("velocity", vel);
    b.output("drag", f(1.2));
    b.output("gravity", v3(0.0, 0.3, 0.0));
    let life = rand(&mut b, 3.5, 5.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 1.2, 1.8);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.6), ease(1.0, 2.6)]);
    b.output("size_over_life", grow);
    b.output("color", col([0.16, 0.14, 0.12, 0.85]));
    let fade = fade_in_out(&mut b, 0.12, 0.45);
    b.output("color_over_life", fade);
    random_spin(&mut b, 0.4);
    let swirl = curl(&mut b, 0.4, 0.5, [0.0, 0.3, 0.0]);
    let w = wind(&mut b, 0.5, 0.0);
    accelerate(&mut b, vec![swirl, w]);
    let smoke_look = RenderSettings {
        texture: tex("smoke_wisp_sheet.png"),
        sprite_sheet: SpriteSheet { columns: 4, rows: 4, mode: SheetMode::OverLifetime },
        frame_blend: true,
        blend: BlendMode::Alpha,
        soft_distance: 0.5,
        lighting: Lighting { lit: true, roughness: 1.0, metallic: 0.0 },
        draw_order: DrawOrder::OldestFirst,
        ..Default::default()
    };
    let smoke = emitter("smoke", 48, World, 906, b.finish(), smoke_look);

    effect("Explosion", EVERY, true, vec![flash, fireball, shockwave, debris, sparks, smoke])
}

/// A fireworks display over a 6 x 6 m launch area. Rockets climb on a
/// sparkling trail (Interval sub-emitter) and pop near their apex into a
/// shell of 140 ribbon sparks that take the rocket's colour (inherit colour),
/// sag under a light gravity and crackle into white star glints as they die
/// (a second OnDeath level). Each shell lights the scene while it burns.
fn fireworks() -> VfxAsset {
    let mut b = B::default();
    bursts(&mut b, 1.0, 0.7);
    let (pos, _) = bed(&mut b, [6.0, 0.0, 6.0]);
    b.output("position", pos);
    let vel = rand3(&mut b, [-1.5, 14.0, -1.5], [1.5, 18.0, 1.5]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 1.1, 1.4);
    b.output("lifetime", life);
    b.output("size", f(0.09));
    let hue = rand_color(&mut b, [3.0, 0.5, 0.35, 1.0], [0.45, 1.3, 3.0, 1.0]);
    b.output("color", hue);
    b.output("emission", f(3.0));
    let mut rockets = emitter("rockets", 8, World, 911, b.finish(), dot_render(BlendMode::Additive));
    rockets.sub_emitters = vec![
        sub(SubEmitterTrigger::Interval(0.03), 1, 2, 0.1, false),
        sub(SubEmitterTrigger::OnDeath, 2, 140, 0.15, true),
    ];

    let mut b = B::default();
    let vel = rand3(&mut b, [-0.4, -0.6, -0.4], [0.4, 0.2, 0.4]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -2.0, 0.0));
    let life = rand(&mut b, 0.35, 0.7);
    b.output("lifetime", life);
    b.output("size", f(0.05));
    b.output("color", col([2.4, 1.6, 0.8, 1.0]));
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 0.5, 0.2, 0.0])]);
    b.output("color_over_life", fade);
    let f_ = flicker(&mut b, (25.0, 40.0), 0.6, 2.5);
    b.output("emission", f_);
    let trail = emitter("rocket trail", 200, World, 912, b.finish(), dot_render(BlendMode::Additive));

    let mut b = B::default();
    let vel = burst_velocity(&mut b, (7.0, 10.0), [0.0, 0.0, 0.0]);
    b.output("velocity", vel);
    b.output("drag", f(1.4));
    b.output("gravity", v3(0.0, -3.5, 0.0));
    let life = rand(&mut b, 1.2, 2.2);
    b.output("lifetime", life);
    b.output("size", f(0.06));
    b.output("color", col([1.0, 1.0, 1.0, 1.0]));
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (0.7, [1.0, 0.9, 0.8, 1.0]), (1.0, [1.0, 0.7, 0.5, 0.0])]);
    b.output("color_over_life", fade);
    let e = glow(&mut b, 5.0, 1.5);
    b.output("emission", e);
    let ribbons = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Trail, points: 8, min_distance: 0.06, ..Default::default() },
        ..Default::default()
    };
    let mut shells = emitter("shell sparks", 600, World, 913, b.finish(), ribbons);
    shells.sub_emitters = vec![sub(SubEmitterTrigger::OnDeath, 3, 1, 0.0, false)];
    // Per spark, weighted by its HDR brightness (about 8 when fresh): two
    // shells in the air give about 50000 lm, fading as they cool.
    shells.light = ParticleLight {
        mode: LightMode::PerEmitter,
        color: [1.0, 0.85, 0.7],
        intensity: 12.0,
        range: 30.0,
        radius: 1.0,
        smoothing: 0.1,
        offset: [0.0, 15.0, 0.0],
        ..Default::default()
    };

    let mut b = B::default();
    let life = rand(&mut b, 0.12, 0.35);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.1, 0.22);
    b.output("size", size);
    b.output("color", col([3.0, 3.0, 2.6, 1.0]));
    let f_ = flicker(&mut b, (40.0, 70.0), 0.0, 3.0);
    b.output("emission", f_);
    test_fade(&mut b);
    let crackle = emitter("crackle", 360, World, 914, b.finish(), sprite("star.svg", BlendMode::Additive));

    effect("Fireworks", 0.0, true, vec![rockets, trail, shells, crackle])
}

/// The confetti a party cannon fires every 3 s: foil cards (a two-sided
/// material taking each card's colour) shot 10 m up, then fluttering down on
/// heavy drag and curl noise, drifting with the wind, and settling on a
/// ground plane. Two emitters give two colour families, since a random
/// colour between two ends only ever mixes those two.
fn confetti_cannon() -> VfxAsset {
    let layer = |name: &str, seed: u64, a: [f32; 4], c: [f32; 4]| {
        let mut b = B::default();
        bursts(&mut b, 150.0, 3.0);
        let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(22.0)), ("radius", PinValue::Float(0.15))]);
        b.output("position", pos);
        let speed = rand(&mut b, 9.0, 14.0);
        let vel = b.mul(dir, speed);
        b.output("velocity", vel);
        b.output("gravity", v3(0.0, -4.0, 0.0));
        let life = rand(&mut b, 5.0, 6.0);
        b.output("lifetime", life);
        b.output("size", v2(0.06, 0.035));
        let hue = rand_color(&mut b, a, c);
        b.output("color", hue);
        random_spin(&mut b, 14.0);
        let air = drag(&mut b, 2.2, 0.0);
        let flutter = curl(&mut b, 1.6, 1.2, [0.0, -0.5, 0.0]);
        let w = wind(&mut b, 1.0, 0.0);
        accelerate(&mut b, vec![air, flutter, w]);
        ground(&mut b, 0.05, 0.95, false);
        let look = RenderSettings {
            blend: BlendMode::Opaque,
            mesh: Some(ParticleMesh::Quad),
            orient: MeshOrient::Tumble,
            material: mat("fx_confetti.material"),
            ..Default::default()
        };
        let mut e = emitter(name, 360, World, seed, b.finish(), look);
        e.backend = Backend::Gpu;
        e
    };
    let warm = layer("warm confetti", 921, [1.0, 0.15, 0.3, 1.0], [1.0, 0.85, 0.1, 1.0]);
    let cool = layer("cool confetti", 922, [0.1, 0.9, 0.5, 1.0], [0.45, 0.25, 1.0, 1.0]);
    effect("Confetti Cannon", 3.0, true, vec![warm, cool])
}

/// A healing aura around a character standing at the origin: SVG crosses
/// rising and fading, motes spiralling up on an Orbit as glowing ribbons, a
/// pulsing ring on the ground, and a soft green light.
fn healing_aura() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(6.0));
    let (pos, _) = bed(&mut b, [1.4, 0.1, 1.4]);
    let pos = lift(&mut b, pos, 0.2);
    b.output("position", pos);
    let rise = rand3(&mut b, [0.0, 0.5, 0.0], [0.0, 0.9, 0.0]);
    b.output("velocity", rise);
    let life = rand(&mut b, 1.6, 2.2);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.18, 0.3);
    b.output("size", size);
    b.output("color", col([0.45, 2.0, 0.6, 1.0]));
    let fade = fade_in_out(&mut b, 0.2, 0.6);
    b.output("color_over_life", fade);
    let crosses = emitter("crosses", 24, World, 931, b.finish(), sprite("plus.svg", BlendMode::Additive));

    let mut b = B::default();
    b.output("spawn_rate", f(45.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(0.9)), ("surface", PinValue::Bool(true))]);
    let pos = lift(&mut b, pos, 0.3);
    b.output("position", pos);
    b.output("gravity", v3(0.0, 0.7, 0.0));
    let life = rand(&mut b, 1.8, 2.6);
    b.output("lifetime", life);
    b.output("size", f(0.03));
    let tint = rand_color(&mut b, [0.4, 2.2, 0.6, 1.0], [1.8, 2.0, 0.6, 1.0]);
    b.output("color", tint);
    let fade = fade_in_out(&mut b, 0.15, 0.7);
    b.output("color_over_life", fade);
    b.output("emission", f(2.0));
    let spiral = b.op(
        "force/orbit",
        &[("center", v3(0.0, 0.0, 0.0)), ("axis", v3(0.0, 1.0, 0.0)), ("speed", f(2.6)), ("grip", f(5.0)), ("radius", f(0.7)), ("radial_pull", f(5.0))],
        "force",
    );
    b.output("acceleration", spiral);
    let ribbons = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Trail, points: 12, min_distance: 0.03, ..Default::default() },
        ..Default::default()
    };
    let mut motes = emitter("motes", 140, World, 932, b.finish(), ribbons);
    // Per mote: about 100 alive give roughly 6000 lm of green.
    motes.light = light([0.4, 1.0, 0.5], 60.0, 6.0, 0.1, 1.0);

    let mut b = B::default();
    b.output("spawn_rate", f(1.2));
    b.output("position", v3(0.0, 0.03, 0.0));
    b.output("lifetime", f(1.4));
    b.output("size", f(1.0));
    let grow = curve(&mut b, &[ease_out(0.0, 1.0), ease(1.0, 2.8)]);
    b.output("size_over_life", grow);
    b.output("color", col([0.3, 1.6, 0.5, 0.9]));
    let fade = fade_in_out(&mut b, 0.15, 0.5);
    b.output("color_over_life", fade);
    let ring = emitter("ground ring", 4, World, 933, b.finish(), ground_decal("shockwave_ring.svg"));

    effect("Healing Aura", 0.0, true, vec![crosses, motes, ring])
}

/// A shorting cable 1.2 m up: every 0.35 s a crack of blue-white sparks spray
/// out as streaks, bounce off the ground and die, with an SVG star flash and a
/// harsh flickering light at the break.
fn electric_sparks() -> VfxAsset {
    const EVERY: f32 = 0.35;
    let mut b = B::default();
    bursts(&mut b, 25.0, EVERY);
    b.output("position", v3(0.0, 1.2, 0.0));
    let vel = burst_velocity(&mut b, (2.0, 6.0), [0.0, 1.0, 0.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 0.4, 0.9);
    b.output("lifetime", life);
    b.output("size", f(0.02));
    b.output("color", col([1.6, 2.0, 3.0, 1.0]));
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (1.0, [0.6, 0.7, 1.0, 0.0])]);
    b.output("color_over_life", fade);
    let e = glow(&mut b, 6.0, 1.0);
    b.output("emission", e);
    ground(&mut b, 0.35, 0.4, false);
    let sparks = emitter("sparks", 100, World, 941, b.finish(), streak_render(BlendMode::Additive, 0.06));

    let mut b = B::default();
    bursts(&mut b, 1.0, EVERY);
    b.output("position", v3(0.0, 1.2, 0.0));
    let life = rand(&mut b, 0.08, 0.14);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.5, 0.9);
    b.output("size", size);
    b.output("color", col([2.0, 2.4, 3.0, 1.0]));
    b.output("emission", f(4.0));
    let r = rand(&mut b, 0.0, TAU);
    b.output("rotation", r);
    let mut flash = emitter("flash", 4, World, 942, b.finish(), sprite("star.svg", BlendMode::Additive));
    // One flash weighted by its HDR brightness (about 10): roughly 20000 lm.
    flash.light = ParticleLight { flicker_rate: 30.0, ..light([0.6, 0.75, 1.0], 2000.0, 7.0, 0.7, 1.2) };

    effect("Electric Sparks", 0.0, true, vec![sparks, flash])
}

/// A fountain of gold coins: flat cylinders wearing the gold material, flung
/// up, tumbling, bouncing and skidding on the ground, with SVG glints
/// twinkling over the pile.
fn coin_fountain() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(25.0));
    let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(16.0)), ("radius", PinValue::Float(0.1))]);
    b.output("position", pos);
    let speed = rand(&mut b, 5.5, 7.5);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 3.0, 4.0);
    b.output("lifetime", life);
    b.output("size", v2(0.12, 0.014));
    random_spin(&mut b, 14.0);
    ground(&mut b, 0.35, 0.5, false);
    let coin_look = RenderSettings {
        blend: BlendMode::Opaque,
        mesh: Some(ParticleMesh::Cylinder),
        orient: MeshOrient::Tumble,
        material: mat("fx_gold_coin.material"),
        ..Default::default()
    };
    let coins = emitter("coins", 128, World, 951, b.finish(), coin_look);

    let mut b = B::default();
    b.output("spawn_rate", f(14.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(1.2))]);
    let pos = lift(&mut b, pos, 0.6);
    b.output("position", pos);
    let life = rand(&mut b, 0.3, 0.6);
    b.output("lifetime", life);
    let px = rand(&mut b, 14.0, 26.0);
    b.output("size", px);
    b.output("color", col([2.0, 1.6, 0.8, 1.0]));
    let twinkle = fade_in_out(&mut b, 0.4, 0.5);
    b.output("color_over_life", twinkle);
    let glints_look = RenderSettings { screen_space_size: true, ..sprite("star.svg", BlendMode::Additive) };
    let glints = emitter("glints", 24, World, 952, b.finish(), glints_look);

    effect("Coin Fountain", 0.0, true, vec![coins, glints])
}

/// Hearts floating up from a point 1 m up: SVG hearts in pinks and reds,
/// swaying side to side as they rise, swelling then fading.
fn love_hearts() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(5.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(0.25))]);
    let pos = lift(&mut b, pos, 1.0);
    b.output("position", pos);
    let rise = rand3(&mut b, [-0.15, 0.5, -0.15], [0.15, 0.8, 0.15]);
    b.output("velocity", rise);
    let life = rand(&mut b, 2.0, 2.8);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.16, 0.28);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.3), ease(0.25, 1.0), ease(1.0, 1.1)]);
    b.output("size_over_life", grow);
    let tint = rand_color(&mut b, [1.0, 0.25, 0.45, 1.0], [1.0, 0.55, 0.75, 1.0]);
    b.output("color", tint);
    let fade = fade_in_out(&mut b, 0.12, 0.65);
    b.output("color_over_life", fade);
    // The sway: a sideways acceleration from a per-heart sine wave.
    let s = wave(&mut b, (2.5, 4.0));
    let sway = b.op("math/compose_vec3", &[("x", s)], "value");
    let sway = b.mul(sway, f(0.6));
    b.output("acceleration", sway);
    let r = rand(&mut b, -0.3, 0.3);
    b.output("rotation", r);
    let hearts = emitter("hearts", 24, World, 961, b.finish(), sprite("heart.svg", BlendMode::Alpha));
    effect("Love Hearts", 0.0, true, vec![hearts])
}

/// Cherry blossom petals drifting down from a 6 x 6 m canopy 5 m up: SVG
/// petals as lit, tumbling alpha-masked quads, fluttering on curl noise and
/// carried by the wind, coming to rest on the ground.
fn cherry_blossoms() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(30.0));
    let (pos, _) = bed(&mut b, [6.0, 0.5, 6.0]);
    let pos = lift(&mut b, pos, 5.0);
    b.output("position", pos);
    b.output("gravity", v3(0.0, -1.2, 0.0));
    b.output("drag", f(1.5));
    let life = rand(&mut b, 7.0, 9.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.05, 0.08);
    b.output("size", size);
    let tint = rand_color(&mut b, [1.0, 1.0, 1.0, 1.0], [1.0, 0.8, 0.88, 1.0]);
    b.output("color", tint);
    let fade = fade_in_out(&mut b, 0.05, 0.9);
    b.output("color_over_life", fade);
    random_spin(&mut b, 5.0);
    let flutter = curl(&mut b, 1.4, 0.9, [0.0, -0.3, 0.0]);
    let w = wind(&mut b, 0.9, 0.0);
    accelerate(&mut b, vec![flutter, w]);
    ground(&mut b, 0.0, 0.95, false);
    let look = RenderSettings {
        texture: tex("petal.svg"),
        blend: BlendMode::AlphaMask,
        mesh: Some(ParticleMesh::Quad),
        orient: MeshOrient::Tumble,
        lighting: Lighting { lit: true, roughness: 0.7, metallic: 0.0 },
        ..Default::default()
    };
    let mut petals = emitter("petals", 330, World, 971, b.finish(), look);
    petals.backend = Backend::Gpu;
    effect("Cherry Blossoms", 0.0, true, vec![petals])
}

/// Big storybook snowflakes over 10 x 10 m: SVG flakes that turn slowly as
/// they fall, flutter and drift with the wind. Fewer and larger than the
/// weather snow, for a close-up or a menu scene.
fn storybook_snow() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(40.0));
    let (pos, _) = bed(&mut b, [10.0, 0.5, 10.0]);
    let pos = lift(&mut b, pos, 6.0);
    b.output("position", pos);
    b.output("gravity", v3(0.0, -0.6, 0.0));
    b.output("drag", f(0.9));
    let life = rand(&mut b, 8.0, 10.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.06, 0.14);
    b.output("size", size);
    b.output("color", col([0.95, 0.97, 1.0, 0.95]));
    let fade = fade_in_out(&mut b, 0.05, 0.9);
    b.output("color_over_life", fade);
    random_spin(&mut b, 0.8);
    let flutter = curl(&mut b, 0.5, 0.6, [0.0, -0.2, 0.0]);
    let w = wind(&mut b, 0.8, 0.0);
    accelerate(&mut b, vec![flutter, w]);
    let flakes = emitter("flakes", 440, World, 981, b.finish(), sprite("snowflake.svg", BlendMode::Alpha));
    effect("Storybook Snow", 0.0, true, vec![flakes])
}

/// An energy shield 1.6 m across around the origin, centre 1 m up: SVG hex
/// cells drawn onto a sphere by Conform To Sphere, each shimmering on its
/// own beat, with sparks crawling over the surface and a cool light.
fn energy_shield() -> VfxAsset {
    const CENTER: [f32; 3] = [0.0, 1.0, 0.0];
    let mut b = B::default();
    b.output("spawn_rate", f(80.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(1.6)), ("surface", PinValue::Bool(true))]);
    let pos = b.add(pos, Src::vec3(CENTER));
    b.output("position", pos);
    let life = rand(&mut b, 2.0, 3.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.2, 0.28);
    b.output("size", size);
    b.output("color", col([0.35, 0.9, 2.0, 0.9]));
    let fade = fade_in_out(&mut b, 0.25, 0.7);
    b.output("color_over_life", fade);
    let shimmer = flicker(&mut b, (3.0, 7.0), 0.4, 1.8);
    b.output("emission", shimmer);
    let hold = b.op(
        "force/conform_sphere",
        &[("center", Src::vec3(CENTER)), ("radius", f(1.6)), ("influence", f(4.0)), ("attraction", f(10.0)), ("max_speed", f(1.0))],
        "force",
    );
    b.output("acceleration", hold);
    let cells = emitter("hex cells", 280, Local, 991, b.finish(), sprite("hex_cell.svg", BlendMode::Additive));

    let mut b = B::default();
    b.output("spawn_rate", f(60.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(1.6)), ("surface", PinValue::Bool(true))]);
    let pos = b.add(pos, Src::vec3(CENTER));
    b.output("position", pos);
    let life = rand(&mut b, 0.4, 0.8);
    b.output("lifetime", life);
    b.output("size", f(0.025));
    b.output("color", col([1.2, 2.0, 3.0, 1.0]));
    test_fade(&mut b);
    b.output("emission", f(3.0));
    let crawl = curl(&mut b, 3.0, 2.0, [0.0, 0.0, 0.0]);
    let hold = b.op(
        "force/conform_sphere",
        &[("center", Src::vec3(CENTER)), ("radius", f(1.62)), ("influence", f(4.0)), ("attraction", f(14.0)), ("max_speed", f(3.0))],
        "force",
    );
    accelerate(&mut b, vec![crawl, hold]);
    let mut crawlers = emitter("surface sparks", 64, Local, 992, b.finish(), streak_render(BlendMode::Additive, 0.05));
    // Per spark: about 40 alive give roughly 4000 lm of cold blue.
    crawlers.light = light([0.45, 0.75, 1.0], 100.0, 6.0, 0.15, 1.0);

    effect("Energy Shield", 0.0, true, vec![cells, crawlers])
}

/// A teleport beam at the origin: streaks racing up a 0.5 m column, rings
/// (the shockwave SVG laid flat) climbing it and widening, a bright core
/// column of soft glow, and a cyan light.
fn teleport_beam() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(90.0));
    let (pos, _) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(0.0)), ("radius", PinValue::Float(0.5))]);
    b.output("position", pos);
    let up = rand3(&mut b, [0.0, 3.0, 0.0], [0.0, 6.0, 0.0]);
    b.output("velocity", up);
    let life = rand(&mut b, 0.5, 0.9);
    b.output("lifetime", life);
    b.output("size", f(0.03));
    b.output("color", col([0.6, 1.8, 3.0, 1.0]));
    let fade = fade_in_out(&mut b, 0.1, 0.5);
    b.output("color_over_life", fade);
    b.output("emission", f(3.0));
    let mut streaks = emitter("streaks", 100, World, 1001, b.finish(), streak_render(BlendMode::Additive, 0.12));
    // Per streak: about 65 alive give roughly 13000 lm of cyan.
    streaks.light = light([0.5, 0.85, 1.0], 200.0, 8.0, 0.1, 1.2);

    let mut b = B::default();
    b.output("spawn_rate", f(2.5));
    b.output("position", v3(0.0, 0.05, 0.0));
    b.output("velocity", v3(0.0, 1.6, 0.0));
    b.output("lifetime", f(1.6));
    b.output("size", f(1.2));
    let grow = curve(&mut b, &[ease_out(0.0, 0.8), ease(1.0, 1.6)]);
    b.output("size_over_life", grow);
    b.output("color", col([0.5, 1.6, 2.6, 1.0]));
    let fade = fade_in_out(&mut b, 0.1, 0.6);
    b.output("color_over_life", fade);
    let rings = emitter("rings", 8, World, 1002, b.finish(), ground_decal("shockwave_ring.svg"));

    let mut b = B::default();
    b.output("spawn_rate", f(20.0));
    let (pos, _) = bed(&mut b, [0.15, 2.5, 0.15]);
    let pos = lift(&mut b, pos, 1.25);
    b.output("position", pos);
    let life = rand(&mut b, 0.4, 0.7);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.6, 0.9);
    b.output("size", size);
    b.output("color", col([0.4, 1.0, 1.8, 0.35]));
    let fade = fade_in_out(&mut b, 0.3, 0.6);
    b.output("color_over_life", fade);
    let core = emitter("core glow", 24, World, 1003, b.finish(), dot_render(BlendMode::Additive));

    effect("Teleport Beam", 0.0, true, vec![streaks, rings, core])
}

pub(super) fn entries() -> Vec<Entry> {
    vec![
        entry("popular", "explosion", "A ground explosion every 2.5 s: light flash, flipbook fireball, SVG shockwave ring, molten debris (material), spark ribbons, lit smoke.", explosion),
        entry("popular", "fireworks", "Rockets on sparkling trails bursting into 140-spark ribbon shells in the rocket's colour, crackling into SVG stars. Lights the sky.", fireworks),
        entry("popular", "confetti_cannon", "A party cannon every 3 s: two-sided foil confetti (material) fluttering down on the wind and settling. GPU.", confetti_cannon),
        entry("popular", "healing_aura", "Rising SVG crosses, spiralling ribbon motes, a pulsing ground ring and a green light around a character at the origin.", healing_aura),
        entry("popular", "electric_sparks", "A shorting cable 1.2 m up: blue-white spark streaks that bounce, an SVG star flash and a harsh flickering light.", electric_sparks),
        entry("popular", "coin_fountain", "Gold coins (flat cylinders, gold material) flung up, tumbling and bouncing, with pixel-sized SVG glints.", coin_fountain),
        entry("popular", "love_hearts", "SVG hearts in pinks and reds floating up from 1 m, swaying as they rise.", love_hearts),
        entry("popular", "cherry_blossoms", "SVG petals as lit, tumbling alpha-mask quads drifting down from a 6 x 6 m canopy on the wind. GPU.", cherry_blossoms),
        entry("popular", "storybook_snow", "Big SVG snowflakes over 10 x 10 m, turning slowly, fluttering and drifting with the wind.", storybook_snow),
        entry("popular", "energy_shield", "A 1.6 m sphere of shimmering SVG hex cells (Conform To Sphere) with sparks crawling over it and a cool light.", energy_shield),
        entry("popular", "teleport_beam", "Streaks racing up a column, SVG rings climbing it, a glowing core and a cyan light.", teleport_beam),
    ]
}
