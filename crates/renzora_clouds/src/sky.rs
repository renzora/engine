//! Couples the cloud lighting to the scene's atmosphere.
//!
//! Bevy's atmosphere lives entirely in the render world: its transmittance,
//! sky-view and aerial-perspective LUTs are bound in the atmosphere node's own
//! group 0, and a `Material` — which builds its bind group from main-world data
//! — has no way to reach them. So instead of sampling those LUTs, this evaluates
//! the same physics on the CPU, from the same `ScatteringMedium` asset the sky
//! is rendered from, using Bevy's own `Falloff::sample` and
//! `PhaseFunction::sample`. Change the medium (or swap Earth for Mars) and the
//! clouds follow, because there is no second copy of the model here.
//!
//! That trade is affordable because of what actually varies. The two terms that
//! matter most to a cloud — how much sunlight survives down to the deck, and
//! what colour the sky filling its shadows is — depend on the sun's elevation
//! and the deck's altitude, not on the pixel. They are one value for the whole
//! sky, so a per-pixel LUT fetch would return the same answer a few hundred
//! thousand times. Only the haze genuinely varies with view direction, and that
//! is handled by evaluating the horizon *twice*, toward the sun and away from
//! it, and letting the shader blend between them by azimuth.
//!
//! Everything is returned **relative to the same quantity with the sun at
//! zenith**, so a noon sky multiplies the authored colours by exactly 1 and only
//! the deviation from noon — the reddening, the dimming, the shift of the
//! horizon — reaches the clouds. That keeps every inspector colour meaning what
//! it says it means.

use bevy::light::atmosphere::{ScatteringMedium, ScatteringTerm};
use bevy::math::{DVec3, Vec3};

/// Samples along a view ray from the deck to the top of the atmosphere.
const VIEW_STEPS: usize = 24;
/// Samples along a ray to the sun, for the transmittance at one point.
const SUN_STEPS: usize = 8;

/// Below this the reference sky is effectively black — a zero-density medium,
/// which is how `renzora_atmosphere` represents "sky off". Dividing by it would
/// produce infinities, so the coupling reports itself unavailable instead and
/// the authored colours are used as-is.
const MIN_REFERENCE: f32 = 1e-9;

/// How the atmosphere is currently treating the light that reaches the clouds,
/// as a multiplier on the authored colours. All-ones is "exactly noon".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyTransfer {
    /// Sunlight surviving from space down to the deck, per channel.
    pub sun: Vec3,
    /// Sky the top of the deck sees.
    pub zenith: Vec3,
    /// Horizon sky in the sun's half of the sky.
    pub horizon_sunward: Vec3,
    /// Horizon sky opposite the sun.
    pub horizon_away: Vec3,
}

impl SkyTransfer {
    /// The identity: no atmosphere, or the coupling switched off.
    pub const NONE: Self = Self {
        sun: Vec3::ONE,
        zenith: Vec3::ONE,
        horizon_sunward: Vec3::ONE,
        horizon_away: Vec3::ONE,
    };
}

/// The noon values everything is divided by. Depends only on the medium and the
/// planet radii, so it is cached rather than recomputed per frame.
#[derive(Clone, Copy, Debug)]
pub struct SkyReference {
    sun: Vec3,
    zenith: Vec3,
    horizon: Vec3,
}

impl SkyReference {
    /// Whether this medium scatters enough to divide by.
    ///
    /// A zero-density one does not — which is exactly how `renzora_atmosphere`
    /// represents a switched-off sky, so this is the signal that a scene is
    /// getting its sky from somewhere else and its clouds need a stand-in.
    pub fn is_usable(&self) -> bool {
        self.sun.max_element() >= MIN_REFERENCE
            && self.zenith.max_element() >= MIN_REFERENCE
            && self.horizon.max_element() >= MIN_REFERENCE
    }
}

/// The atmosphere as this module needs to see it.
pub struct Sky<'a> {
    pub terms: &'a [ScatteringTerm],
    /// Planet surface radius, metres.
    pub inner_radius: f64,
    /// Top of the atmosphere, metres.
    pub outer_radius: f64,
}

impl<'a> Sky<'a> {
    pub fn new(medium: &'a ScatteringMedium, inner_radius: f32, outer_radius: f32) -> Self {
        Self {
            terms: &medium.terms,
            inner_radius: inner_radius as f64,
            outer_radius: outer_radius.max(inner_radius + 1.0) as f64,
        }
    }

    /// Bevy's falloff parameter: 1 at the surface, 0 at the edge of space.
    fn falloff_param(&self, radius: f64) -> f32 {
        let p = 1.0 - (radius - self.inner_radius) / (self.outer_radius - self.inner_radius);
        p.clamp(0.0, 1.0) as f32
    }

    /// Extinction (absorption + out-scattering) at a radius, per channel.
    fn extinction(&self, radius: f64) -> DVec3 {
        let p = self.falloff_param(radius);
        let mut total = DVec3::ZERO;
        for term in self.terms {
            let density = term.falloff.sample(p) as f64;
            let coefficient = (term.absorption + term.scattering).as_dvec3();
            total += coefficient * density;
        }
        total
    }

    /// Distance to the top of the atmosphere along `dir`, or `None` if the ray
    /// hits the ground first — a sun below the local horizon, in other words.
    fn distance_to_space(&self, origin: DVec3, dir: DVec3) -> Option<f64> {
        let b = origin.dot(dir);
        let r2 = origin.length_squared();

        // The ground blocks first if the ray both aims down and reaches it.
        let ground_disc = b * b - (r2 - self.inner_radius * self.inner_radius);
        if ground_disc > 0.0 && -b - ground_disc.sqrt() > 0.0 {
            return None;
        }

        let disc = b * b - (r2 - self.outer_radius * self.outer_radius);
        if disc <= 0.0 {
            return None;
        }
        let t = -b + disc.sqrt();
        (t > 0.0).then_some(t)
    }

    /// Transmittance from a point out to space along `dir`. Zero if the planet
    /// is in the way.
    fn transmittance_to_space(&self, origin: DVec3, dir: DVec3) -> DVec3 {
        let Some(distance) = self.distance_to_space(origin, dir) else {
            return DVec3::ZERO;
        };
        let step = distance / SUN_STEPS as f64;
        let mut optical_depth = DVec3::ZERO;
        for i in 0..SUN_STEPS {
            let t = (i as f64 + 0.5) * step;
            optical_depth += self.extinction((origin + dir * t).length()) * step;
        }
        DVec3::new(
            (-optical_depth.x).exp(),
            (-optical_depth.y).exp(),
            (-optical_depth.z).exp(),
        )
    }

    /// Single-scattered sky radiance looking along `view` from `origin`, lit by a
    /// sun at `sun`.
    ///
    /// Single scattering only: multiple scattering is what fills a real twilight
    /// sky in and this will read a little darker than Bevy's own sky just after
    /// sunset. It is a *ratio* against the noon sky that leaves here, though, and
    /// the missing term is the one that varies least between the two, so the
    /// error largely divides out.
    fn radiance(&self, origin: DVec3, view: DVec3, sun: DVec3) -> DVec3 {
        let Some(distance) = self.distance_to_space(origin, view) else {
            return DVec3::ZERO;
        };
        // Bevy's phase convention: the cosine between the direction *to* the
        // light and the view direction.
        let cos_theta = sun.dot(view).clamp(-1.0, 1.0) as f32;

        let step = distance / VIEW_STEPS as f64;
        let mut radiance = DVec3::ZERO;
        let mut optical_depth = DVec3::ZERO;

        for i in 0..VIEW_STEPS {
            let t = (i as f64 + 0.5) * step;
            let position = origin + view * t;
            let radius = position.length();
            let p = self.falloff_param(radius);

            // Transmittance from the eye to here, carried forward rather than
            // re-integrated per step.
            optical_depth += self.extinction(radius) * step;
            let view_transmittance = DVec3::new(
                (-optical_depth.x).exp(),
                (-optical_depth.y).exp(),
                (-optical_depth.z).exp(),
            );
            let sun_transmittance = self.transmittance_to_space(position, sun);

            let mut in_scatter = DVec3::ZERO;
            for term in self.terms {
                let Some(phase) = term.phase.sample(cos_theta) else {
                    // A chromatic phase texture that has not loaded yet.
                    continue;
                };
                let density = term.falloff.sample(p) as f64;
                let phase = DVec3::new(phase.red as f64, phase.green as f64, phase.blue as f64);
                in_scatter += term.scattering.as_dvec3() * density * phase;
            }

            radiance += view_transmittance * sun_transmittance * in_scatter * step;
        }

        radiance
    }

    /// The noon values [`transfer`] divides by.
    pub fn reference(&self, deck_altitude: f32) -> SkyReference {
        let origin = DVec3::new(0.0, self.inner_radius + deck_altitude as f64, 0.0);
        let up = DVec3::Y;
        // Not exactly horizontal: a ray along the tangent grazes the surface for
        // thousands of km and the integration is needlessly stiff. A couple of
        // degrees up is the sky an observer calls "the horizon" anyway.
        let horizon = DVec3::new(0.966, 0.259, 0.0);

        SkyReference {
            sun: self.transmittance_to_space(origin, up).as_vec3(),
            zenith: self.radiance(origin, up, up).as_vec3(),
            horizon: self.radiance(origin, horizon, up).as_vec3(),
        }
    }

    /// Multipliers for the current sun position, relative to `reference`.
    ///
    /// `sun_direction` points *toward* the sun and is in world space, where +Y
    /// is up, which is also the deck's local up — over any scene the engine can
    /// hold, the surface under it is flat.
    pub fn transfer(&self, deck_altitude: f32, sun_direction: Vec3, reference: &SkyReference) -> SkyTransfer {
        if !reference.is_usable() {
            return SkyTransfer::NONE;
        }

        let radius = self.inner_radius + deck_altitude as f64;
        let origin = DVec3::new(0.0, radius, 0.0);
        let sun = sun_direction.normalize_or(Vec3::Y).as_dvec3();

        // The deck sits above the surface, so it keeps its own sunset about a
        // degree and a half after the ground below has lost the sun. Past that
        // the sun ray is blocked by the planet and the transmittance drops from
        // "very red" to exactly zero within a fraction of a degree of sun
        // motion, which pops. Hold the ray at the grazing direction instead: the
        // light reaches its reddest and stays there while the caller's own
        // day/night fade carries it down to nothing.
        let grazing = -(1.0 - (self.inner_radius / radius).powi(2)).max(0.0).sqrt() + 1e-4;
        let sun_ray = lift_to(sun, grazing);

        // The two horizon rays that bracket the haze: toward the sun's compass
        // bearing and away from it, both raised the same couple of degrees as the
        // reference so the ratio is like-for-like.
        let bearing = DVec3::new(sun.x, 0.0, sun.z)
            .try_normalize()
            .unwrap_or(DVec3::X);
        let sunward = (bearing * 0.966 + DVec3::Y * 0.259).normalize();
        let away = (-bearing * 0.966 + DVec3::Y * 0.259).normalize();

        SkyTransfer {
            sun: ratio(
                self.transmittance_to_space(origin, sun_ray).as_vec3(),
                reference.sun,
            ),
            zenith: ratio(
                self.radiance(origin, DVec3::Y, sun).as_vec3(),
                reference.zenith,
            ),
            horizon_sunward: ratio(
                self.radiance(origin, sunward, sun).as_vec3(),
                reference.horizon,
            ),
            horizon_away: ratio(self.radiance(origin, away, sun).as_vec3(), reference.horizon),
        }
    }
}

/// Raise a direction to at least `min_y` while keeping its compass bearing.
fn lift_to(dir: DVec3, min_y: f64) -> DVec3 {
    if dir.y >= min_y {
        return dir;
    }
    let bearing = DVec3::new(dir.x, 0.0, dir.z)
        .try_normalize()
        .unwrap_or(DVec3::X);
    (bearing * (1.0 - min_y * min_y).max(0.0).sqrt() + DVec3::Y * min_y).normalize()
}

/// Component-wise ratio, guarding the channels a reference can legitimately be
/// zero in (a medium that does not scatter red at all, say).
///
/// Capped at 4 so an exotic medium cannot hand the shader a multiplier that
/// blows the cloud colours out — the coupling is meant to shade the authored
/// look, not to take it over.
fn ratio(value: Vec3, reference: Vec3) -> Vec3 {
    let channel = |value: f32, reference: f32| {
        if reference > MIN_REFERENCE {
            (value / reference).clamp(0.0, 4.0)
        } else {
            1.0
        }
    };
    Vec3::new(
        channel(value.x, reference.x),
        channel(value.y, reference.y),
        channel(value.z, reference.z),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth() -> ScatteringMedium {
        ScatteringMedium::earth(256, 256)
    }

    fn sky(medium: &ScatteringMedium) -> Sky<'_> {
        Sky::new(medium, 6_360_000.0, 6_460_000.0)
    }

    /// A sun at zenith is the reference, so every multiplier must be 1 there. If
    /// this drifts, every authored cloud colour silently changes meaning.
    #[test]
    fn noon_is_the_identity() {
        let medium = earth();
        let sky = sky(&medium);
        let reference = sky.reference(1800.0);
        let transfer = sky.transfer(1800.0, Vec3::Y, &reference);

        for value in [
            transfer.sun,
            transfer.zenith,
            transfer.horizon_sunward,
            transfer.horizon_away,
        ] {
            assert!(
                (value - Vec3::ONE).abs().max_element() < 1e-4,
                "expected 1, got {value:?}",
            );
        }
    }

    /// The whole point: a low sun must reach the clouds redder and dimmer than a
    /// high one.
    #[test]
    fn low_sun_reddens_and_dims() {
        let medium = earth();
        let sky = sky(&medium);
        let reference = sky.reference(1800.0);

        let elevation = 4f32.to_radians();
        let sun = Vec3::new(elevation.cos(), elevation.sin(), 0.0);
        let transfer = sky.transfer(1800.0, sun, &reference);

        assert!(
            transfer.sun.x < 1.0 && transfer.sun.z < transfer.sun.x,
            "sunset light should dim and redden, got {:?}",
            transfer.sun,
        );
        assert!(
            transfer.horizon_sunward.x > transfer.horizon_away.x,
            "the sun's half of the horizon should be the warmer one: {:?} vs {:?}",
            transfer.horizon_sunward,
            transfer.horizon_away,
        );
    }

    /// Sunlight must redden and dim *monotonically* as the sun drops, and the
    /// sky must dim with it. A non-monotonic curve here would show up as the
    /// clouds brightening again partway through a sunset.
    #[test]
    fn the_sunset_runs_one_way() {
        let medium = earth();
        let sky = sky(&medium);
        let reference = sky.reference(1800.0);

        let mut previous: Option<(Vec3, Vec3)> = None;
        for degrees in [90.0f32, 45.0, 20.0, 10.0, 5.0, 2.0, 0.0] {
            let elevation = degrees.to_radians();
            let sun = Vec3::new(elevation.cos(), elevation.sin(), 0.0);
            let transfer = sky.transfer(1800.0, sun, &reference);

            assert!(
                transfer.sun.cmple(Vec3::ONE + 1e-3).all() && transfer.sun.cmpge(Vec3::ZERO).all(),
                "{degrees}deg: transmittance out of range: {:?}",
                transfer.sun,
            );
            if let Some((sun_before, zenith_before)) = previous {
                assert!(
                    transfer.sun.cmple(sun_before + 1e-4).all(),
                    "{degrees}deg: sunlight brightened on the way down: {:?} then {sun_before:?}",
                    transfer.sun,
                );
                assert!(
                    transfer.zenith.cmple(zenith_before + 1e-4).all(),
                    "{degrees}deg: the zenith sky brightened on the way down",
                );
            }
            previous = Some((transfer.sun, transfer.zenith));
        }

        // ...and the last of it is red.
        let elevation = 2f32.to_radians();
        let sun = Vec3::new(elevation.cos(), elevation.sin(), 0.0);
        let transfer = sky.transfer(1800.0, sun, &reference);
        assert!(
            transfer.sun.x > 4.0 * transfer.sun.z,
            "a 2deg sun should be strongly red: {:?}",
            transfer.sun,
        );
    }

    /// A sun below the deck's own horizon must not step straight to black — the
    /// ray is held at grazing so the colour settles instead of popping.
    #[test]
    fn sunlight_does_not_pop_at_the_terminator() {
        let medium = earth();
        let sky = sky(&medium);
        let reference = sky.reference(1800.0);

        let at = |degrees: f32| {
            let elevation = degrees.to_radians();
            sky.transfer(
                1800.0,
                Vec3::new(elevation.cos(), elevation.sin(), 0.0),
                &reference,
            )
            .sun
        };

        // -1.44deg is where a 1.8 km deck loses the sun behind an Earth-sized
        // planet; straddle it.
        let above = at(-1.0);
        let below = at(-2.0);
        assert!(
            below.x > 0.0 && (above.x - below.x).abs() < 0.05,
            "the terminator should be smooth, got {above:?} then {below:?}",
        );
    }

    /// Scenes lit by a skybox or an HDRI never spawn an `Atmosphere`, and their
    /// clouds fall back to `ScatteringMedium::default()` measured against
    /// Earth's radii. If that default ever stopped being a real atmosphere the
    /// fallback would go quietly inert, and those skies would keep noon-white
    /// clouds hanging over a scene that had gone to dusk.
    #[test]
    fn the_fallback_medium_still_responds_to_a_low_sun() {
        let medium = ScatteringMedium::default();
        let sky = Sky::new(&medium, 6_360_000.0, 6_460_000.0);
        let reference = sky.reference(3200.0);

        let elevation = 5f32.to_radians();
        let transfer = sky.transfer(
            3200.0,
            Vec3::new(elevation.cos(), elevation.sin(), 0.0),
            &reference,
        );

        assert!(
            transfer.sun.x < 0.8,
            "a 5deg sun should be well down on noon: {:?}",
            transfer.sun,
        );
        assert!(
            transfer.sun.z < 0.25 * transfer.sun.x,
            "...and strongly red by then: {:?}",
            transfer.sun,
        );
    }

    /// `renzora_atmosphere` turns the sky off with a zero-density medium. That
    /// must report itself unmeasurable rather than divide by zero — it is the
    /// signal `atmosphere_transfer` reads to swap in the fallback medium, so a
    /// scene using a skybox instead of the procedural sky still gets clouds that
    /// follow the sun down.
    #[test]
    fn zero_density_medium_is_reported_unmeasurable() {
        let medium = earth().with_density_multiplier(0.0);
        let sky = sky(&medium);
        let reference = sky.reference(1800.0);
        let transfer = sky.transfer(1800.0, Vec3::new(0.0, 0.5, 0.5).normalize(), &reference);
        assert_eq!(transfer, SkyTransfer::NONE);
    }
}
