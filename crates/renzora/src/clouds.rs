//! `CloudsData` — the authored volumetric-cloud settings.
//!
//! The renderer lives in the `clouds` **native plugin**; only the settings type
//! is here. That split is the rule this crate exists for: `renzora_level_presets`
//! builds a sky by inserting `CloudsData`, and it is compiled into the editor
//! binary while the cloud renderer is a library loaded at runtime. A binary
//! cannot name a type that lives in a plugin, so the type has to sit where both
//! can reach it — and it must be *one* definition, or the `TypeId` the preset
//! inserts is not the one the plugin queries for and the sky silently never
//! appears.
//!
//! Same reasoning as [`crate::gi`] and [`crate::wind`], and the same as the note
//! at the top of [`crate::world_environment`].

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Volumetric cloud settings.
///
/// Heights are metres above the world's ground plane; the shader works in km
/// and converts on the way in.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[serde(default)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct CloudsData {
    pub enabled: bool,

    // ── Deck geometry ──
    /// Altitude of the base of the cloud deck, in metres.
    pub bottom_height: f32,
    /// Altitude of the top of the cloud deck, in metres.
    pub top_height: f32,
    /// Radius of the virtual planet the deck wraps, in metres. This is what sets
    /// how sharply clouds compress toward the horizon: shrink it for a small,
    /// storybook world, leave it at Earth's radius for a realistic sky.
    pub planet_radius: f32,

    // ── Shape ──
    /// 0 = clear sky, 1 = solid overcast.
    pub coverage: f32,
    /// How opaque the cloud material is, 0..1.
    pub density: f32,
    /// Size of the cloud formations — smaller means larger, broader systems.
    pub scale: f32,
    /// Frequency of the erosion detail relative to the base shape.
    pub detail_scale: f32,
    /// How much the detail volume eats into the base silhouette.
    pub detail_strength: f32,
    /// Width of the coverage threshold. Low values give crisp cauliflower edges,
    /// high values give soft haze.
    pub edge_softness: f32,
    /// Fraction of the deck's depth over which density fades in from the base.
    pub base_softness: f32,

    // ── Wind ──
    /// Wind speed in metres per second. Cloud features are kilometres across, so
    /// this has to be weather-system fast — a literal 2 m/s breeze moves the
    /// deck by a thousandth of a cloud per second and reads as a still image.
    pub speed: f32,
    /// Wind direction in degrees (0–360). Ignored while
    /// [`follow_world_wind`](Self::follow_world_wind) is set.
    pub wind_direction: f32,
    /// Take heading and drift from the world wind (`crate::WindState`) instead
    /// of the two fields above.
    ///
    /// On by default, because a deck sliding one way over grass leaning the
    /// other is the single most obvious way a sky reads as fake. `speed` still
    /// matters when this is on — it is the drift the deck reaches at reference
    /// wind, so the authored value keeps its meaning and the world wind scales
    /// it. Turn this off for a deliberately decoupled sky (a stylised level, or
    /// a cutscene where the ground wind is scripted and the sky must not be).
    ///
    /// Note the deck ignores gusts entirely: cloud features are kilometres
    /// across and a two-second gust does not move them.
    pub follow_world_wind: bool,
    /// How fast cloud shapes evolve, in metres per second, independently of the
    /// wind that carries them. Wind alone only translates the deck, and a cloud
    /// whose silhouette never changes reads as a cutout sliding across the sky
    /// however fast it goes. 0 freezes the shapes and leaves pure drift.
    pub morph_speed: f32,

    // ── Lighting ──
    /// Tint of the sunlight scattering out of the cloud.
    pub color: (f32, f32, f32),
    /// Overall brightness of the deck.
    ///
    /// This is a *trim* on the scene sun, not a substitute for it: the sunlight
    /// and the skylight both scale with the `Sun` component's illuminance
    /// first, so dimming the scene's sun dims the clouds with everything else,
    /// and this only says how bright the deck sits within that.
    pub brightness: f32,
    /// Skylight filling the top of the deck.
    pub ambient_color: (f32, f32, f32),
    /// Skylight filling the base of the deck. This is what actually lights a
    /// real cloud's underside — scattered blue sky, not darkness.
    ///
    /// Keep it *saturated*, not merely dark. Grey cloud is almost never a
    /// brightness problem: a warm direct term summed with a near-neutral fill
    /// lands on neutral across most of a cloud whatever the levels are, and the
    /// only way out is for the two to disagree about hue.
    pub shadow_color: (f32, f32, f32),
    /// Multiplier on both ambient colours.
    pub ambient_brightness: f32,
    /// Extinction multiplier: how fast light is absorbed inside the cloud, and
    /// therefore how hard the sun-facing/shadowed contrast is.
    pub absorption: f32,
    /// Eccentricity of the forward scattering lobe — the silver lining.
    pub forward_scattering: f32,
    /// Eccentricity of the backward lobe, which lights clouds seen against the
    /// sun. Negative by convention.
    pub backward_scattering: f32,
    /// Mix between the two lobes, 0 = all forward, 1 = all backward.
    pub scattering_blend: f32,
    /// Darkening of thin sunlit edges, which have scattered little light back
    /// toward the eye yet. Without it, rims look like cut paper.
    pub powder_strength: f32,

    // ── March ──
    /// Samples along each view ray. The dominant cost knob.
    pub raymarch_steps: u32,
    /// Samples along each sun-shadow ray.
    pub shadow_steps: u32,

    // ── Atmosphere ──
    /// Drive the cloud lighting from the scene's atmosphere: sunlight reddens
    /// and dims as it does in the sky, and the haze follows the real horizon.
    /// Every authored colour below is then a *noon* value that the atmosphere
    /// modulates. Turn it off for a stylised sky that ignores the sun's height.
    pub atmosphere_lighting: bool,
    /// Colour distant cloud fades into.
    pub horizon_color: (f32, f32, f32),
    /// How strongly that haze takes over toward the horizon.
    pub atmosphere_strength: f32,
}

impl Default for CloudsData {
    fn default() -> Self {
        Self {
            enabled: true,

            bottom_height: 1690.0,
            top_height: 2960.0,
            planet_radius: 6_371_000.0,

            coverage: 0.5,
            density: 0.2,
            scale: 1.5,
            detail_scale: 30.5,
            detail_strength: 0.27,
            edge_softness: 0.2,
            base_softness: 0.3,

            speed: 40.0,
            wind_direction: 220.0,
            follow_world_wind: true,
            morph_speed: 50.0,

            color: (1.0, 0.93, 0.80),
            brightness: 3.2,
            ambient_color: (0.58, 0.72, 0.96),
            shadow_color: (0.28, 0.44, 0.74),
            ambient_brightness: 1.1,
            absorption: 1.0,
            forward_scattering: 0.85,
            backward_scattering: -0.2,
            scattering_blend: 0.3,
            powder_strength: 0.3,

            raymarch_steps: 42,
            shadow_steps: 6,

            atmosphere_lighting: true,
            horizon_color: (0.72, 0.83, 0.96),
            atmosphere_strength: 0.8,
        }
    }
}
