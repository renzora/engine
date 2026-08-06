#![no_std]
extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

#[derive(Resource)]
#[repr(C)]
pub struct MagnetField {
    pub strength: f32,
    pub hover_height: f32,
    pub pulse: f32,
}

impl Default for MagnetField {
    fn default() -> Self {
        Self {
            strength: 2.0,
            hover_height: 1.5,
            pulse: 0.0,
        }
    }
}

/// `enabled` and `invert` sit next to each other on purpose: a `bool` is one
/// byte, so writing four would put `enabled`'s value straight through `invert`.
/// Toggling one in the inspector and watching the other stay put is the test.
#[derive(Component)]
#[repr(C)]
pub struct Magnetic {
    pub mass: f32,
    pub enabled: bool,
    pub invert: bool,
}

impl Default for Magnetic {
    fn default() -> Self {
        Self {
            mass: 1.0,
            enabled: true,
            invert: false,
        }
    }
}

/// Optional. An entity carrying one is pulled harder until it runs out, and
/// entities without one still match the query — `Option<&mut T>` is the
/// difference between "some of these have a battery" and two separate systems.
#[derive(Component)]
#[repr(C)]
pub struct Charge {
    pub remaining: f32,
    pub drain: f32,
    pub boost: f32,
}

impl Default for Charge {
    fn default() -> Self {
        Self {
            remaining: 4.0,
            drain: 1.0,
            boost: 3.0,
        }
    }
}

#[derive(Component, Default)]
#[repr(C)]
pub struct Iron {
    pub _v: f32,
}

#[derive(Component, Default)]
#[repr(C)]
pub struct Nickel {
    pub _v: f32,
}

#[derive(Component, Default)]
#[repr(C)]
pub struct Cobalt {
    pub _v: f32,
}

fn oscillate(mut field: ResMut<MagnetField>, time: Res<Time>) {
    field.pulse = (time.elapsed_secs() * 0.8).sin();
}

/// Put one of these on an entity and metal is pulled toward *it*.
#[derive(Component)]
#[repr(C)]
pub struct Magnet {
    pub reach: f32,
}

impl Default for Magnet {
    fn default() -> Self {
        Self { reach: 12.0 }
    }
}

/// Two queries, over provably disjoint sets.
///
/// This is the shape a system like this wants and could not have before: one
/// flat term list per system meant both queries merged into a single builder and
/// AND-ed, so this matched only entities that were somehow both the magnet and
/// the metal, and each parameter read the other's cells.
///
/// The `Without<Magnet>` is what makes it legal rather than a conflict. Both
/// queries touch `Transform`, one of them mutably, so Bevy has to be able to
/// *prove* they never see the same entity — which is exactly what an explicit
/// disjointness filter is for, in a plugin as in ordinary Bevy.
///
/// The metal filter is nested: `Or<T>` is itself a `QueryFilter`, so nesting one
/// is ordinary code. A flat walk over the bracketed term run drops the inner
/// brackets while still emitting the inner terms, which quietly turns the inner
/// `Or` into an `AND` — nickel-only and cobalt-only pieces stop moving and only
/// something carrying both does.
fn attract(
    magnets: Query<(&Transform, &Magnet)>,
    mut metal: Query<
        (&mut Transform, &Magnetic, Option<&mut Charge>),
        (
            Or<(With<Iron>, Or<(With<Nickel>, With<Cobalt>)>)>,
            Without<Magnet>,
        ),
    >,
    field: Res<MagnetField>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);

    // Collected up front so the second query can borrow mutably.
    let poles: Vec<(Vec3, f32)> = magnets
        .iter()
        .map(|(t, m)| (t.translation, m.reach))
        .collect();

    for (t, m, charge) in &mut metal {
        if !m.enabled {
            continue;
        }

        // Fall back to the world origin when no magnet exists, so the plugin
        // still does something visible on a scene that has none.
        let target = poles
            .iter()
            .filter(|(p, reach)| p.distance(t.translation) <= *reach)
            .min_by(|a, b| {
                a.0.distance(t.translation)
                    .total_cmp(&b.0.distance(t.translation))
            })
            .map(|(p, _)| *p)
            .unwrap_or(Vec3::new(0.0, field.hover_height + field.pulse, 0.0));

        let to_target = target - t.translation;
        let d = to_target.length();
        if d < 0.001 {
            continue;
        }

        let mut pull = field.strength / m.mass.max(0.1);
        if let Some(c) = charge {
            if c.remaining > 0.0 {
                pull *= c.boost;
                c.remaining = (c.remaining - c.drain * dt).max(0.0);
            }
        }

        let dir = if m.invert { -to_target } else { to_target } / d;
        t.translation += dir * pull * dt;
    }
}

pub struct MagnetPlugin;

impl Plugin for MagnetPlugin {
    fn build(&self, app: &mut App) {
        // `insert_resource` rather than `init_resource`: the default is a
        // sensible starting point, but a plugin that wants its own tuning
        // shipped should not have to write it twice.
        app.insert_resource(MagnetField {
            strength: 2.5,
            hover_height: 1.5,
            pulse: 0.0,
        })
        .register_component::<Magnet>()
        .register_component::<Magnetic>()
        .register_component::<Charge>()
        .register_component::<Iron>()
        .register_component::<Nickel>()
        .register_component::<Cobalt>()
        .add_systems(Update, oscillate)
        .add_systems(Update, attract);
    }
}

renzora_plugin::add!(MagnetPlugin);
