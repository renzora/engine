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

/// The nearest magnet whose `reach` covers `at`, or `fallback` when none does.
///
/// Split out of [`attract`] so it can be tested: a plugin's `Query` is backed by
/// the host's interface table, so the system only runs inside a real engine,
/// while the selection — which is where an off-by-one on `reach` or a wrong
/// comparison would live — is ordinary arithmetic over a slice.
fn nearest_pole(poles: &[(Vec3, f32)], at: Vec3, fallback: Vec3) -> Vec3 {
    poles
        .iter()
        .filter(|(p, reach)| p.distance(at) <= *reach)
        .min_by(|a, b| a.0.distance(at).total_cmp(&b.0.distance(at)))
        .map(|(p, _)| *p)
        .unwrap_or(fallback)
}

/// Pull applied per second, for a field strength and a piece's mass.
///
/// The `max(0.1)` is load-bearing rather than tidy: `mass` is an inspector field
/// whose range starts at 0, and a mass of exactly 0 would divide by zero and
/// teleport the piece to infinity — or to NaN, which sticks.
fn pull_strength(strength: f32, mass: f32) -> f32 {
    strength / mass.max(0.1)
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
        let target = nearest_pole(
            &poles,
            t.translation,
            Vec3::new(0.0, field.hover_height + field.pulse, 0.0),
        );

        let to_target = target - t.translation;
        let d = to_target.length();
        if d < 0.001 {
            continue;
        }

        let mut pull = pull_strength(field.strength, m.mass);
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

#[cfg(test)]
mod tests {
    use super::*;

    const FALLBACK: Vec3 = Vec3::new(0.0, 3.0, 0.0);

    fn at(x: f32) -> Vec3 {
        Vec3::new(x, 0.0, 0.0)
    }

    #[test]
    fn with_no_magnets_a_piece_falls_back_to_the_hover_point() {
        assert_eq!(nearest_pole(&[], at(5.0), FALLBACK), FALLBACK);
    }

    /// `reach` is what makes a magnet local. A pole outside it must be ignored
    /// entirely, not merely ranked lower — otherwise every magnet in the scene
    /// pulls everything, however far away.
    #[test]
    fn a_pole_out_of_reach_is_ignored() {
        let poles = [(at(100.0), 12.0)];
        assert_eq!(nearest_pole(&poles, Vec3::ZERO, FALLBACK), FALLBACK);
    }

    #[test]
    fn the_nearest_pole_in_reach_wins() {
        let poles = [(at(10.0), 50.0), (at(2.0), 50.0), (at(-30.0), 50.0)];
        assert_eq!(nearest_pole(&poles, Vec3::ZERO, FALLBACK), at(2.0));
    }

    /// A piece between two magnets must pick one and hold it. Returning
    /// whichever the iteration happened to reach first would make it jitter
    /// between them as the scene changed.
    #[test]
    fn selection_is_by_distance_not_by_order() {
        let near = at(1.0);
        let far = at(9.0);
        assert_eq!(nearest_pole(&[(far, 50.0), (near, 50.0)], Vec3::ZERO, FALLBACK), near);
        assert_eq!(nearest_pole(&[(near, 50.0), (far, 50.0)], Vec3::ZERO, FALLBACK), near);
    }

    /// `<=` rather than `<`: a piece exactly at the edge of a magnet's reach is
    /// inside it. With `<` it would flicker in and out as it drifted.
    #[test]
    fn a_piece_exactly_at_the_edge_of_reach_is_caught() {
        let poles = [(at(12.0), 12.0)];
        assert_eq!(nearest_pole(&poles, Vec3::ZERO, FALLBACK), at(12.0));
    }

    #[test]
    fn a_nearer_pole_out_of_reach_loses_to_a_further_one_in_reach() {
        // The close magnet has a tiny reach that does not cover the piece.
        let poles = [(at(3.0), 0.5), (at(20.0), 100.0)];
        assert_eq!(nearest_pole(&poles, Vec3::ZERO, FALLBACK), at(20.0));
    }

    // ── pull strength ────────────────────────────────────────────────────────

    #[test]
    fn a_heavier_piece_is_pulled_more_slowly() {
        assert!(pull_strength(10.0, 4.0) < pull_strength(10.0, 1.0));
    }

    /// `mass` is an inspector field whose range starts at 0. Without the clamp
    /// this divides by zero, and the piece leaves the scene — or goes NaN, which
    /// never recovers.
    #[test]
    fn a_zero_or_negative_mass_cannot_divide_by_zero() {
        for mass in [0.0f32, -1.0, -1e9] {
            let pull = pull_strength(10.0, mass);
            assert!(pull.is_finite(), "mass {mass} gave {pull}");
            assert_eq!(pull, 100.0, "mass {mass} should clamp to 0.1");
        }
    }

    #[test]
    fn pull_scales_with_field_strength() {
        assert_eq!(pull_strength(20.0, 2.0), 2.0 * pull_strength(10.0, 2.0));
    }

    #[test]
    fn defaults_are_usable() {
        assert!(Magnet::default().reach > 0.0);
        let m = Magnetic::default();
        assert!(m.mass > 0.0, "a default mass of 0 would rely on the clamp");
        assert!(pull_strength(MagnetField::default().strength, m.mass).is_finite());
    }
}
