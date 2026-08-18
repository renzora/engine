//! Water surface configuration — the authored half of the FFT ocean.
//!
//! The surface is described by a small number of **wave cascades**. Each
//! cascade is an independent ocean simulation over its own square tile
//! (`tile_length`), driven by a JONSWAP/TMA spectrum with Hasselmann
//! directional spreading. Layering cascades with different tile lengths is what
//! hides the tiling: a 400 m swell and a 16 m chop repeat at different
//! intervals, so the eye never locks onto a period.
//!
//! Every parameter here is a *sea state* parameter (wind speed, fetch, swell,
//! whitecap threshold), not a wave shape — the shape falls out of the spectrum.
//! That is the whole point of the FFT approach and why there is no longer a
//! list of hand-authored waves.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Hard cap on cascades. Matches `MAX_CASCADES` in `water.wgsl` and the uniform
/// array in `sim.rs`; raising it means editing all three.
pub const MAX_CASCADES: usize = 8;

/// Water preset types for quick configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum WaterPreset {
    CalmLake,
    River,
    Ocean,
    StormyOcean,
    Tropical,
    Arctic,
    Swamp,
}

/// Clipmap density preset. The reference project ships two baked clipmap
/// meshes ("low" and "high") and picks between them with a single dropdown;
/// these presets are the same idea, expressed as the three numbers
/// [`generate_clipmap_mesh`](crate::mesh::generate_clipmap_mesh) actually
/// takes.
///
/// Density matters as much as simulation resolution: a metre-scale clipmap
/// renders a choppy sea as smooth hills no matter how good the spectrum is,
/// and coarse far rings that still displace are what makes the surface crawl
/// as the camera moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum WaterMeshQuality {
    /// 0.5 m centre quads, 4 rings, reaching ±512 m.
    Low,
    /// 0.35 m centre quads, 5 rings, reaching ±896 m.
    Medium,
    /// 0.25 m centre quads, 5 rings, reaching ±1024 m — the density the
    /// reference project's high-quality clipmap uses, and the previous
    /// hard-coded default.
    #[default]
    High,
    /// Use the raw `clipmap_rings` / `clipmap_resolution` / `clipmap_quad_size`
    /// fields instead of a preset.
    Custom,
}

impl WaterMeshQuality {
    /// `(rings, resolution, quad_size)` for this preset, or `None` for
    /// [`Custom`](WaterMeshQuality::Custom).
    pub fn clipmap_params(self) -> Option<(u32, u32, f32)> {
        match self {
            WaterMeshQuality::Low => Some((4, 128, 0.5)),
            WaterMeshQuality::Medium => Some((5, 160, 0.35)),
            WaterMeshQuality::High => Some((5, 256, 0.25)),
            WaterMeshQuality::Custom => None,
        }
    }
}

/// How the water's mesh is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum WaterMeshMode {
    /// Fixed subdivided plane of `mesh_size` metres. For lakes, ponds and any
    /// bounded body of water where the shoreline hides the mesh edge.
    #[default]
    Grid,
    /// Camera-centred clipmap: a dense middle block ringed by progressively
    /// coarser quads, snapped to the camera each frame. For open ocean — the
    /// surface reaches the horizon with a near-constant triangle density on
    /// screen.
    Clipmap,
}

/// One wave cascade. Defaults are the middle cascade of the reference project.
#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WaveCascade {
    /// World-space size of the cascade's repeating tile, in metres. The
    /// dominant knob: small tiles carry chop, large tiles carry swell.
    pub tile_length: Vec2,
    /// Multiplier on this cascade's contribution to vertex displacement.
    /// Reduce as cascades are added — several cascades at full scale stack into
    /// an implausibly violent sea.
    pub displacement_scale: f32,
    /// Multiplier on this cascade's contribution to the shading normal.
    pub normal_scale: f32,
    /// Rate this cascade's clock advances, relative to real time. 1 is real
    /// time, 0 freezes the cascade, 2 doubles its speed.
    ///
    /// This is *not* a wind knob. The spectrum is unchanged — only the
    /// `exp(i·ω·t)` propagation is rescaled, so the waves keep their shape and
    /// travel faster or slower. Slowing a long-swell cascade below its wind
    /// sea is the usual way to sell scale: real ocean swell moves far slower
    /// than its wavelength suggests to the eye.
    #[serde(default = "default_time_scale")]
    #[reflect(default = "default_time_scale")]
    pub time_scale: f32,
    /// Average wind speed over the water, m/s. Higher = steeper, choppier.
    pub wind_speed: f32,
    /// Wind heading in radians.
    pub wind_direction: f32,
    /// Distance from shore the wind has blown over, in kilometres. Longer fetch
    /// gives steeper but less chaotic waves.
    pub fetch_length: f32,
    /// Swell (0–2): biases the spectrum toward long, ordered waves travelling
    /// with the wind.
    pub swell: f32,
    /// Directional spread (0–1): 0 follows the wind tightly, 1 is isotropic.
    pub spread: f32,
    /// Detail (0–1): attenuates high-frequency waves. Lower it on cascades
    /// whose tile is small relative to `map_size` to avoid aliasing.
    pub detail: f32,
    /// How steep a wave must get before foam accumulates (Jacobian threshold).
    /// Lower = foam appears sooner.
    pub whitecap: f32,
    /// Foam quantity (0–10). Wispier foam comes from a high `foam_amount` with
    /// a low `whitecap`.
    pub foam_amount: f32,
}

/// Serde/reflect default for [`WaveCascade::time_scale`], so cascades saved
/// before the field existed load as real-time rather than frozen.
fn default_time_scale() -> f32 {
    1.0
}

/// Serde/reflect default for [`WaterSurface::enable_sea_spray`]. Scenes saved
/// before the field existed keep spray on, which is what they looked like.
fn default_one() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

impl Default for WaveCascade {
    fn default() -> Self {
        Self {
            tile_length: Vec2::splat(57.0),
            displacement_scale: 0.75,
            normal_scale: 1.0,
            time_scale: 1.0,
            wind_speed: 5.0,
            wind_direction: 15.0_f32.to_radians(),
            fetch_length: 150.0,
            swell: 0.8,
            spread: 0.4,
            detail: 1.0,
            whitecap: 0.5,
            foam_amount: 0.0,
        }
    }
}

impl WaveCascade {
    /// JONSWAP scale parameter α for this cascade's wind/fetch.
    /// Source: <https://wikiwaves.org/Ocean-Wave_Spectra#JONSWAP_Spectrum>
    pub fn jonswap_alpha(&self) -> f32 {
        const G: f32 = 9.81;
        let fetch_m = (self.fetch_length * 1e3).max(1e-4);
        0.076 * (self.wind_speed * self.wind_speed / (fetch_m * G)).powf(0.22)
    }

    /// JONSWAP peak angular frequency ω_p for this cascade's wind/fetch.
    pub fn jonswap_peak_frequency(&self) -> f32 {
        const G: f32 = 9.81;
        let fetch_m = (self.fetch_length * 1e3).max(1e-4);
        22.0 * (G * G / (self.wind_speed.max(1e-4) * fetch_m)).powf(1.0 / 3.0)
    }

    /// Per-frame foam growth rate. The constants normalise `foam_amount` (0–10)
    /// into something frame-rate independent; taken from the reference.
    pub fn foam_grow_rate(&self, delta: f32) -> f32 {
        delta * self.foam_amount * 7.5
    }

    /// Per-frame foam decay rate. Note the inversion: more foam means it also
    /// lingers longer, so the decay term shrinks as `foam_amount` grows.
    pub fn foam_decay_rate(&self, delta: f32) -> f32 {
        delta * (10.0 - self.foam_amount).max(0.5) * 1.15
    }
}

/// Tag component for water surface entities.
///
/// Only the **first** `WaterSurface` in the scene drives the GPU simulation —
/// the displacement/normal cascade maps are global textures, not per-entity
/// ones. Additional water entities render the same sea state (they may still
/// use their own colours and mesh). A scene wanting two genuinely different
/// oceans would need one texture set per entity, which is not worth the memory.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct WaterSurface {
    /// Base water colour (linear). Deep-water body colour under the foam.
    pub water_color: [f32; 3],
    /// Foam colour (linear).
    pub foam_color: [f32; 3],
    /// Surface roughness feeding the microfacet specular term.
    pub roughness: f32,
    /// Global multiplier on the cascade normals. 0 flattens the shading normal
    /// without touching the geometry.
    pub normal_strength: f32,
    /// The wave cascades, at most [`MAX_CASCADES`].
    pub cascades: Vec<WaveCascade>,
    /// Sea depth in metres, used by the dispersion relation and the
    /// Kitaigorodskii depth attenuation. Shallow water shortens and steepens
    /// waves.
    pub sea_depth: f32,
    /// Seed for the spectrum's Gaussian noise. Changing it re-rolls the sea.
    pub seed: u32,
    /// Scale the whole sea state with the world wind (`renzora::WindState`).
    ///
    /// The authored per-cascade `wind_speed`/`wind_direction` become the sea's
    /// *shape* — the swell-to-wind-sea balance and the relative bearings — and
    /// the world wind then scales and rotates the set as a whole. That keeps a
    /// hand-tuned ocean recognisable at every wind strength instead of
    /// flattening every cascade to one number.
    ///
    /// It follows [`WindState::sea_state_speed`](renzora::WindState), which lags
    /// the wind by tens of seconds. That is not a shortcut: `wind_speed` is a
    /// JONSWAP spectrum input, so every change rebuilds the cascade textures,
    /// and a real sea takes hours to build to a new wind. The lag is both
    /// cheaper and more correct than tracking gusts would be.
    #[serde(default = "default_true")]
    #[reflect(default = "default_true")]
    pub follow_world_wind: bool,
    /// How much of the world wind reaches this surface, while
    /// [`follow_world_wind`](Self::follow_world_wind) is on. A sheltered bay is
    /// ~0.4; open ocean is 1.0.
    #[serde(default = "default_one")]
    #[reflect(default = "default_one")]
    pub wind_response: f32,
    /// Simulation resolution per cascade — 128, 256, 512 or 1024. Cost scales
    /// with the square, so this is the main performance dial.
    pub map_size: u32,
    /// How many times per second the simulation advances. 0 means every frame.
    /// Lowering it cuts GPU time without changing how the waves look in motion.
    pub updates_per_second: f32,
    /// Emit sea-spray particles where waves break. Costs nothing when the
    /// spray plugin is absent; the flag still round-trips through a scene so
    /// authoring does not depend on which plugins are loaded.
    #[serde(default = "default_true")]
    #[reflect(default = "default_true")]
    pub enable_sea_spray: bool,
    /// Grid (bounded plane) or clipmap (camera-following, horizon-reaching).
    pub mesh_mode: WaterMeshMode,
    /// `Clipmap` mode: density preset. Anything but
    /// [`Custom`](WaterMeshQuality::Custom) overrides the three `clipmap_*`
    /// fields below — see [`WaterSurface::clipmap_params`].
    #[serde(default)]
    #[reflect(default)]
    pub mesh_quality: WaterMeshQuality,
    /// `Grid` mode: plane size in metres.
    pub mesh_size: f32,
    /// `Grid` mode: quads per edge.
    pub subdivisions: u32,
    /// `Clipmap` mode: number of rings around the centre block. Each ring
    /// doubles the quad size, so extent grows as `2^rings`.
    pub clipmap_rings: u32,
    /// `Clipmap` mode: quads per edge of the centre block, and of each ring.
    /// Must be even.
    pub clipmap_resolution: u32,
    /// `Clipmap` mode: size in metres of the smallest (centre) quad. This sets
    /// how fine the geometry is right under the camera.
    pub clipmap_quad_size: f32,
}

impl Default for WaterSurface {
    fn default() -> Self {
        Self {
            water_color: [0.010, 0.020, 0.027],
            foam_color: [0.492, 0.406, 0.342],
            roughness: 0.65,
            normal_strength: 1.0,
            cascades: default_ocean_cascades(),
            sea_depth: 20.0,
            seed: 1234,
            follow_world_wind: true,
            wind_response: 1.0,
            map_size: 512,
            updates_per_second: 50.0,
            enable_sea_spray: true,
            mesh_mode: WaterMeshMode::Clipmap,
            mesh_quality: WaterMeshQuality::High,
            mesh_size: 200.0,
            subdivisions: 256,
            clipmap_rings: 5,
            clipmap_resolution: 256,
            clipmap_quad_size: 0.25,
        }
    }
}

/// The reference project's shipped three-cascade ocean: a long swell, a
/// mid-scale wind sea, and a normals-only detail cascade whose displacement is
/// zeroed (it would only add aliasing at that tile size).
fn default_ocean_cascades() -> Vec<WaveCascade> {
    vec![
        WaveCascade {
            tile_length: Vec2::splat(88.0),
            displacement_scale: 1.0,
            normal_scale: 1.0,
            time_scale: 1.0,
            wind_speed: 10.0,
            wind_direction: 20.0_f32.to_radians(),
            fetch_length: 150.0,
            swell: 0.8,
            spread: 0.2,
            detail: 1.0,
            whitecap: 0.5,
            foam_amount: 8.0,
        },
        WaveCascade::default(),
        WaveCascade {
            tile_length: Vec2::splat(16.0),
            displacement_scale: 0.0,
            normal_scale: 0.25,
            time_scale: 1.0,
            wind_speed: 20.0,
            wind_direction: 20.0_f32.to_radians(),
            fetch_length: 550.0,
            swell: 0.8,
            spread: 0.4,
            detail: 1.0,
            whitecap: 0.25,
            foam_amount: 3.0,
        },
    ]
}

impl WaterSurface {
    /// Apply a preset, overwriting all parameters.
    pub fn from_preset(preset: WaterPreset) -> Self {
        match preset {
            WaterPreset::Ocean => Self::default(),

            // Small enclosed water: short fetch and near-zero wind leave only a
            // fine ripple, and a bounded grid mesh suits a lake's shoreline.
            WaterPreset::CalmLake => Self {
                water_color: [0.003, 0.012, 0.017],
                foam_color: [0.692, 0.787, 0.848],
                roughness: 0.35,
                mesh_mode: WaterMeshMode::Grid,
                cascades: vec![
                    WaveCascade {
                        tile_length: Vec2::splat(40.0),
                        displacement_scale: 0.35,
                        normal_scale: 1.0,
                        time_scale: 1.0,
                        wind_speed: 2.5,
                        wind_direction: 0.0,
                        fetch_length: 2.0,
                        swell: 0.4,
                        spread: 0.5,
                        detail: 1.0,
                        whitecap: 1.6,
                        foam_amount: 0.0,
                    },
                    WaveCascade {
                        tile_length: Vec2::splat(9.0),
                        displacement_scale: 0.15,
                        normal_scale: 0.6,
                        time_scale: 1.0,
                        wind_speed: 3.0,
                        wind_direction: 0.4,
                        fetch_length: 1.0,
                        swell: 0.2,
                        spread: 0.7,
                        detail: 1.0,
                        whitecap: 1.6,
                        foam_amount: 0.0,
                    },
                ],
                ..Self::default()
            },

            // A river reads as directional chop: one tightly-spread cascade
            // travelling downstream plus a fine surface texture.
            WaterPreset::River => Self {
                water_color: [0.004, 0.013, 0.010],
                foam_color: [0.604, 0.692, 0.748],
                roughness: 0.5,
                mesh_mode: WaterMeshMode::Grid,
                cascades: vec![
                    WaveCascade {
                        tile_length: Vec2::splat(24.0),
                        displacement_scale: 0.4,
                        normal_scale: 1.0,
                        time_scale: 1.0,
                        wind_speed: 5.0,
                        wind_direction: 0.0,
                        fetch_length: 6.0,
                        swell: 0.1,
                        spread: 0.05,
                        detail: 1.0,
                        whitecap: 0.8,
                        foam_amount: 2.0,
                    },
                    WaveCascade {
                        tile_length: Vec2::splat(7.0),
                        displacement_scale: 0.2,
                        normal_scale: 0.5,
                        time_scale: 1.0,
                        wind_speed: 8.0,
                        wind_direction: 0.0,
                        fetch_length: 4.0,
                        swell: 0.1,
                        spread: 0.2,
                        detail: 1.0,
                        whitecap: 0.6,
                        foam_amount: 3.0,
                    },
                ],
                ..Self::default()
            },

            // Storm: high wind, long fetch, low whitecap threshold so crests
            // break into foam early and often.
            WaterPreset::StormyOcean => Self {
                water_color: [0.004, 0.007, 0.010],
                foam_color: [0.571, 0.604, 0.638],
                roughness: 0.75,
                cascades: vec![
                    WaveCascade {
                        tile_length: Vec2::splat(220.0),
                        displacement_scale: 1.0,
                        normal_scale: 1.0,
                        time_scale: 1.0,
                        wind_speed: 26.0,
                        wind_direction: 0.5,
                        fetch_length: 800.0,
                        swell: 1.2,
                        spread: 0.15,
                        detail: 1.0,
                        whitecap: 0.3,
                        foam_amount: 9.0,
                    },
                    WaveCascade {
                        tile_length: Vec2::splat(64.0),
                        displacement_scale: 0.8,
                        normal_scale: 1.0,
                        time_scale: 1.0,
                        wind_speed: 20.0,
                        wind_direction: 0.6,
                        fetch_length: 400.0,
                        swell: 0.6,
                        spread: 0.35,
                        detail: 1.0,
                        whitecap: 0.3,
                        foam_amount: 6.0,
                    },
                    WaveCascade {
                        tile_length: Vec2::splat(16.0),
                        displacement_scale: 0.0,
                        normal_scale: 0.35,
                        time_scale: 1.0,
                        wind_speed: 24.0,
                        wind_direction: 0.4,
                        fetch_length: 550.0,
                        swell: 0.8,
                        spread: 0.5,
                        detail: 1.0,
                        whitecap: 0.2,
                        foam_amount: 5.0,
                    },
                ],
                ..Self::default()
            },

            WaterPreset::Tropical => Self {
                water_color: [0.005, 0.064, 0.073],
                foam_color: [0.828, 0.890, 0.933],
                roughness: 0.45,
                cascades: vec![
                    WaveCascade {
                        tile_length: Vec2::splat(110.0),
                        displacement_scale: 0.7,
                        normal_scale: 1.0,
                        time_scale: 1.0,
                        wind_speed: 6.0,
                        wind_direction: 0.2,
                        fetch_length: 300.0,
                        swell: 1.4,
                        spread: 0.1,
                        detail: 1.0,
                        whitecap: 1.0,
                        foam_amount: 3.0,
                    },
                    WaveCascade {
                        tile_length: Vec2::splat(20.0),
                        displacement_scale: 0.0,
                        normal_scale: 0.4,
                        time_scale: 1.0,
                        wind_speed: 9.0,
                        wind_direction: 0.3,
                        fetch_length: 200.0,
                        swell: 0.6,
                        spread: 0.4,
                        detail: 1.0,
                        whitecap: 0.8,
                        foam_amount: 1.5,
                    },
                ],
                ..Self::default()
            },

            WaterPreset::Arctic => Self {
                water_color: [0.005, 0.010, 0.015],
                foam_color: [0.692, 0.748, 0.828],
                roughness: 0.6,
                cascades: vec![
                    WaveCascade {
                        tile_length: Vec2::splat(120.0),
                        displacement_scale: 0.9,
                        normal_scale: 1.0,
                        time_scale: 1.0,
                        wind_speed: 14.0,
                        wind_direction: 1.2,
                        fetch_length: 300.0,
                        swell: 1.0,
                        spread: 0.25,
                        detail: 1.0,
                        whitecap: 0.45,
                        foam_amount: 6.0,
                    },
                    WaveCascade {
                        tile_length: Vec2::splat(45.0),
                        displacement_scale: 0.6,
                        normal_scale: 1.0,
                        time_scale: 1.0,
                        wind_speed: 9.0,
                        wind_direction: 1.0,
                        fetch_length: 150.0,
                        swell: 0.7,
                        spread: 0.4,
                        detail: 1.0,
                        whitecap: 0.5,
                        foam_amount: 2.0,
                    },
                ],
                ..Self::default()
            },

            // Stagnant water: almost no wind, no foam at all, and a single
            // large-tile cascade so the surface barely breathes.
            WaterPreset::Swamp => Self {
                water_color: [0.002, 0.004, 0.002],
                foam_color: [0.073, 0.101, 0.033],
                roughness: 0.8,
                normal_strength: 0.5,
                mesh_mode: WaterMeshMode::Grid,
                map_size: 128,
                updates_per_second: 20.0,
                cascades: vec![WaveCascade {
                    tile_length: Vec2::splat(30.0),
                    displacement_scale: 0.1,
                    normal_scale: 0.6,
                    time_scale: 1.0,
                    wind_speed: 1.5,
                    wind_direction: 0.0,
                    fetch_length: 0.5,
                    swell: 0.2,
                    spread: 0.8,
                    detail: 0.7,
                    whitecap: 2.0,
                    foam_amount: 0.0,
                }],
                ..Self::default()
            },
        }
    }

    /// The clipmap's `(rings, resolution, quad_size)`, resolving
    /// [`mesh_quality`](WaterSurface::mesh_quality) against the raw fields.
    ///
    /// Every consumer goes through here rather than reading the three fields
    /// directly, so a preset and its expansion can never disagree — the mesh,
    /// its culling bounds and the shader's distance falloffs are all derived
    /// from one answer.
    pub fn clipmap_params(&self) -> (u32, u32, f32) {
        self.mesh_quality.clipmap_params().unwrap_or((
            self.clipmap_rings,
            self.clipmap_resolution,
            self.clipmap_quad_size,
        ))
    }

    /// Half the mesh's width in metres — how far the water reaches from its
    /// centre. Drives the shader's distance falloffs.
    pub fn mesh_half_extent(&self) -> f32 {
        match self.mesh_mode {
            WaterMeshMode::Grid => self.mesh_size * 0.5,
            WaterMeshMode::Clipmap => {
                let (rings, resolution, quad_size) = self.clipmap_params();
                resolution as f32 * quad_size * 0.5 * (1u32 << rings.min(16)) as f32
            }
        }
    }

    /// Cascades clamped to what the GPU uniform and shader can address.
    pub fn active_cascades(&self) -> &[WaveCascade] {
        let n = self.cascades.len().min(MAX_CASCADES);
        &self.cascades[..n]
    }

    /// `map_size` snapped to a supported power of two. Anything the inspector
    /// or a hand-edited scene can produce has to land on one of these, since
    /// the FFT kernels are specialised per size.
    pub fn clamped_map_size(&self) -> u32 {
        match self.map_size {
            0..=191 => 128,
            192..=383 => 256,
            384..=767 => 512,
            _ => 1024,
        }
    }
}

// Inspector entries for `WaterSurface` are editor-only and live in the
// `renzora_water_editor` crate (crates/renzora_water/editor). The data types
// above stay `pub` so that crate can read/write them.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_quality_presets_resolve() {
        let mut surface = WaterSurface::default();

        // The High preset must reproduce what the hard-coded defaults were, or
        // every existing scene silently changes density on load.
        surface.mesh_quality = WaterMeshQuality::High;
        assert_eq!(surface.clipmap_params(), (5, 256, 0.25));
        assert_eq!(surface.mesh_half_extent(), 1024.0);

        surface.mesh_quality = WaterMeshQuality::Low;
        assert_eq!(surface.clipmap_params(), (4, 128, 0.5));
        assert_eq!(surface.mesh_half_extent(), 512.0);

        surface.mesh_quality = WaterMeshQuality::Medium;
        assert_eq!(surface.mesh_half_extent(), 896.0);
    }

    #[test]
    fn custom_quality_uses_the_raw_fields() {
        let mut surface = WaterSurface {
            mesh_quality: WaterMeshQuality::Custom,
            clipmap_rings: 3,
            clipmap_resolution: 64,
            clipmap_quad_size: 1.0,
            ..default()
        };
        assert_eq!(surface.clipmap_params(), (3, 64, 1.0));

        // ...and a preset must override them rather than merge with them.
        surface.mesh_quality = WaterMeshQuality::High;
        assert_eq!(surface.clipmap_params(), (5, 256, 0.25));
    }

    #[test]
    fn grid_mode_extent_ignores_clipmap_quality() {
        let surface = WaterSurface {
            mesh_mode: WaterMeshMode::Grid,
            mesh_size: 300.0,
            mesh_quality: WaterMeshQuality::Low,
            ..default()
        };
        assert_eq!(surface.mesh_half_extent(), 150.0);
    }
}
