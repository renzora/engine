//! Solari's settings and diagnostics.
//!
//! Solari (Bevy's hardware-raytraced lighting) is the engine's only GI. Its
//! settings component, DLSS modes and the ray tracing diagnostics live in this
//! layer of the contract so the host (`renzora_runtime`), `renzora_solari`, its
//! editor half and the exporter all agree on one definition without depending
//! on each other. Plugins see every item at `renzora::...`.
//!
//! Its own crate, apart from `renzora_render_api`, because it is edited far more
//! often than anything else in rendering, and only a handful of crates read it.
//! The GPU capability flags, `SolariLit` and the stand-in contracts that many
//! crates need stayed in `renzora_render_api::gi`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Solari raytraced-GI settings. Authored on a non-camera source entity
/// (typically "World Environment") and routed onto the active cameras via
/// [`EffectRouting`]. The Solari plugin consumes it: while enabled it attaches
/// Bevy's `SolariLighting` (and the HDR + prepass components it requires) to
/// each routed camera and mirrors conforming meshes into the ray-tracing scene.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
// `Default` lets tools that add components by name (the MCP `add_component`
// tool, scripts) create one; without it only the Inspector's own menu could.
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct SolariGi {
    pub enabled: bool,
    /// Stop rendering shadow maps while Solari is lighting the scene.
    ///
    /// Solari replaces shadow mapping with traced visibility, and its camera
    /// carries `SkipDeferredLighting` — which removes the only pass that would
    /// ever *read* a shadow map. Every sun cascade and every point-light cubemap
    /// is therefore rendered and then thrown away, which in a scene with many
    /// lamps is a large amount of wasted GPU time. Bevy's own guidance is to
    /// turn shadow maps off on all lights while Solari is on.
    ///
    /// The suppression is applied in the *render world*, to the extracted
    /// lights, so your `PointLight`/`DirectionalLight` components are never
    /// touched and a scene saved while Solari is active doesn't silently
    /// persist shadows-off. Turn this off if you want to compare against, or
    /// keep, raster shadows.
    #[serde(default = "shadow_map_suppression_default")]
    #[reflect(default = "shadow_map_suppression_default")]
    pub suppress_shadow_maps: bool,
    /// Give point and spot lights an invisible emissive sphere in the traced
    /// scene, so they light it at all.
    ///
    /// Solari samples exactly two kinds of light: directional, and emissive
    /// meshes. Point and spot lights contribute *nothing* — and because a Solari
    /// camera carries `SkipDeferredLighting`, Bevy's clustered-light pass (the
    /// only other thing that would apply them) never runs either. A lamp-lit
    /// scene therefore renders almost black, and nothing gets bright enough to
    /// bloom.
    ///
    /// With this on, each point/spot light gets a small sphere in the
    /// ray-tracing scene whose emissive radiance is derived from the light's
    /// luminous power, which Solari samples as a real area light — soft shadows
    /// and colour bleed included. The sphere is traced-only, never rasterized,
    /// so nothing new appears in the viewport.
    ///
    /// Caveats: a spot light loses its cone (the sphere emits in all
    /// directions), and `AmbientLight` cannot be represented this way at all.
    #[serde(default = "light_proxies_default")]
    #[reflect(default = "light_proxies_default")]
    pub light_proxies: bool,
    /// How much work Solari spends per frame, traded against noise. See
    /// [`SolariQuality`]. A scene saved before this existed loads as `High`,
    /// which is what it rendered as before.
    #[serde(default)]
    #[reflect(default)]
    pub quality: SolariQuality,
    /// ReSTIR on or off regardless of [`quality`](Self::quality); `None`
    /// follows the preset (off on `Low`, on above it). Read it through
    /// [`restir_enabled`](Self::restir_enabled). Choosing a preset in the
    /// inspector sets it back to `None`.
    #[serde(default)]
    #[reflect(default)]
    pub restir: Option<bool>,
    /// What removes Solari's noise. Read it through
    /// [`denoiser_choice`](Self::denoiser_choice), which also honours the two
    /// settings it replaced in a scene saved before it existed.
    #[serde(default)]
    #[reflect(default)]
    pub denoiser: SolariDenoiser,
    /// Superseded by [`denoiser`](Self::denoiser); kept so a scene saved with
    /// it still loads, and so its `false` (the raw image) still means that.
    /// Choosing a denoiser sets it back to `true`.
    #[serde(default = "legacy_toggle_default")]
    #[reflect(default = "legacy_toggle_default")]
    pub denoise: bool,
    /// Superseded by [`denoiser`](Self::denoiser); kept so a scene saved with
    /// it still loads, and so its `false` (built-in denoiser, never DLSS) still
    /// means that. Choosing a denoiser sets it back to `true`.
    #[serde(default = "legacy_toggle_default")]
    #[reflect(default = "legacy_toggle_default")]
    pub dlss: bool,
    /// DLSS's render scale while it is the denoiser in use.
    ///
    /// Balanced by default, not Auto. Auto let DLSS pick from the output
    /// resolution, and at an editor viewport's size it picked close to native,
    /// so every Solari ray pass ran at full resolution: the Bistro on an RTX
    /// 3060 took 57 ms of GPU a frame (13 FPS) at Auto and 24 ms (29 FPS) at
    /// Balanced, with no difference to see. Ray Reconstruction is built to
    /// rebuild detail from a lower-resolution trace.
    #[serde(default = "default_solari_dlss_mode")]
    #[reflect(default = "default_solari_dlss_mode")]
    pub dlss_mode: DlssMode,
    /// Ray trace see-through materials (alpha mode Blend: glass, clear coats,
    /// lenses), so they reflect the scene around them and show what is behind
    /// them. Solari's own image holds only opaque surfaces; with this off,
    /// see-through ones are drawn by the ordinary forward pass, which can
    /// reflect only the sky.
    #[serde(default = "glass_reflections_default")]
    #[reflect(default = "glass_reflections_default")]
    pub glass_reflections: bool,
    /// How many of the view's pixels the built-in denoiser's lighting is traced
    /// at; it upsamples the rest. See [`SolariResolution`].
    #[serde(default)]
    #[reflect(default)]
    pub resolution: SolariResolution,
}

fn glass_reflections_default() -> bool {
    true
}

/// The resolution Solari traces its lighting at under the built-in denoiser,
/// which upsamples it to the view.
///
/// Only the lighting is coarser: geometry and textures are rasterized at the
/// view's full resolution and the denoiser's upsampling follows their edges, so
/// what softens is lighting detail finer than a traced cell, such as a thin
/// shadow. Solari's cost is mostly per traced cell, so each step down saves
/// most of what the cells it drops cost.
///
/// DLSS Ray Reconstruction has its own resolution ([`DlssMode`]) and ignores
/// this; so do the raw image and the Ray Tracing panel, which need every pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum SolariResolution {
    /// Every pixel.
    #[default]
    Native,
    /// About 67% of the width and height: under half the rays.
    Quality,
    /// About 58%: about a third of the rays.
    Balanced,
    /// 50%: a quarter of the rays.
    Performance,
}

impl SolariResolution {
    /// Every choice, in the order the inspector lists them.
    pub const ALL: [Self; 4] = [Self::Native, Self::Quality, Self::Balanced, Self::Performance];

    /// The inspector dropdown labels, index for index with [`ALL`](Self::ALL).
    pub const LABELS: [&'static str; 4] = ["Native", "Quality", "Balanced", "Performance"];

    /// The fraction of the view's width and height the lighting is traced at.
    /// The same fractions as DLSS's modes of the same names, so switching
    /// between the two denoisers keeps the cost about the same.
    pub fn scale(self) -> f32 {
        match self {
            Self::Native => 1.0,
            Self::Quality => 2.0 / 3.0,
            Self::Balanced => 0.58,
            Self::Performance => 0.5,
        }
    }

    /// The inspector dropdown label.
    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    /// The choice with this [`label`](Self::label), if any.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.label() == label)
    }
}

impl SolariGi {
    /// Whether ReSTIR runs: the [`restir`](Self::restir) override, or the
    /// quality preset's own choice.
    pub fn restir_enabled(&self) -> bool {
        self.restir.unwrap_or(self.quality != SolariQuality::Low)
    }

    /// The denoiser asked for, with a scene saved before
    /// [`denoiser`](Self::denoiser) existed read the way it was meant: its
    /// `denoise: false` was the raw image, its `dlss: false` the built-in
    /// denoiser only.
    pub fn denoiser_choice(&self) -> SolariDenoiser {
        if !self.denoise {
            SolariDenoiser::Off
        } else if !self.dlss && self.denoiser == SolariDenoiser::Auto {
            SolariDenoiser::BuiltIn
        } else {
            self.denoiser
        }
    }

    /// Ask for `choice`, clearing the superseded settings so they no longer
    /// override it.
    pub fn set_denoiser(&mut self, choice: SolariDenoiser) {
        self.denoiser = choice;
        self.denoise = true;
        self.dlss = true;
    }
}

/// What removes the noise from Solari's lighting.
///
/// One setting rather than a DLSS toggle beside a denoiser toggle: DLSS Ray
/// Reconstruction is itself a denoiser, so the two toggles described one
/// choice, and the denoiser toggle did nothing whenever DLSS was running.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum SolariDenoiser {
    /// NVIDIA DLSS Ray Reconstruction where this build, GPU, driver and
    /// renderer can run it (see [`DlssSupport`]); Renzora's built-in denoiser
    /// everywhere else, so it costs nothing to leave on.
    #[default]
    Auto,
    /// Renzora's built-in denoiser, on any GPU that can ray trace, even where
    /// DLSS could run.
    BuiltIn,
    /// Nothing: the raw estimate, which is what to compare a denoiser against.
    Off,
}

impl SolariDenoiser {
    /// Every choice, in the order the inspector lists them.
    pub const ALL: [Self; 3] = [Self::Auto, Self::BuiltIn, Self::Off];

    /// The inspector dropdown labels, index for index with [`ALL`](Self::ALL).
    ///
    /// `Auto` was labelled plain "DLSS", which read as "DLSS is running" in a
    /// build or on a GPU without it, while the built-in denoiser quietly did
    /// the work. The label now says it is conditional, and the inspector puts
    /// a warning on it where DLSS cannot run. The variant keeps its name since
    /// scenes have saved it; only the label changed.
    pub const LABELS: [&'static str; 3] = ["DLSS (if available)", "Built-in", "Off (raw)"];

    /// The inspector dropdown label.
    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    /// The choice with this [`label`](Self::label), if any.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.label() == label)
    }

    /// Whether this choice uses DLSS Ray Reconstruction when it can run.
    pub fn wants_dlss(self) -> bool {
        self == Self::Auto
    }

    /// Whether the built-in denoiser runs wherever DLSS does not.
    pub fn wants_built_in(self) -> bool {
        self != Self::Off
    }
}

/// Default for the superseded [`SolariGi::denoise`] and [`SolariGi::dlss`]:
/// `true`, which leaves [`SolariGi::denoiser`] in charge.
fn legacy_toggle_default() -> bool {
    true
}

/// [`SolariGi::dlss_mode`]'s default. See the field for why it is not Auto.
fn default_solari_dlss_mode() -> DlssMode {
    DlssMode::Balanced
}

/// DLSS's quality modes: how far below the output resolution the frame is
/// rendered before DLSS reconstructs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum DlssMode {
    /// DLSS picks the mode from the output resolution.
    #[default]
    Auto,
    /// Native resolution: denoised and anti-aliased, not upscaled.
    Dlaa,
    /// Renders at about 67% of the output resolution.
    Quality,
    /// Renders at about 58%.
    Balanced,
    /// Renders at 50%.
    Performance,
    /// Renders at about 33%.
    UltraPerformance,
}

impl DlssMode {
    /// Every mode, in the order the inspector lists them.
    pub const ALL: [Self; 6] = [
        Self::Auto,
        Self::Dlaa,
        Self::Quality,
        Self::Balanced,
        Self::Performance,
        Self::UltraPerformance,
    ];

    /// The inspector dropdown labels, index for index with [`ALL`](Self::ALL).
    pub const LABELS: [&'static str; 6] =
        ["Auto", "DLAA", "Quality", "Balanced", "Performance", "Ultra Performance"];

    /// The inspector dropdown label.
    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    /// The mode with this [`label`](Self::label), if any.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.label() == label)
    }
}

/// What this build and this GPU can do with DLSS, written at startup by the
/// Solari plugin and read by the inspector to say which denoiser is running,
/// and by anti-aliasing to say whether its DLSS method can run. Neither can see
/// Bevy's own DLSS types: they exist only in a build compiled with the DLSS SDK.
///
/// Absent means the Solari plugin is not in the build at all.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DlssSupport {
    /// The binary was built with the DLSS SDK.
    pub compiled: bool,
    /// The GPU, driver and renderer support DLSS Ray Reconstruction on this
    /// run.
    pub ray_reconstruction: bool,
    /// The GPU, driver and renderer support DLSS Super Resolution (upscaling
    /// and anti-aliasing, without Solari) on this run.
    pub super_resolution: bool,
}

/// Solari's cost/noise preset, mapped by the Solari plugin onto the sample
/// counts and bounce depth of Bevy's `SolariLighting`.
///
/// The step that matters most is ReSTIR, which `Low` turns off. It reuses
/// samples across neighbouring pixels and past frames, so the denoiser
/// (`SolariGi::denoiser`) starts from far fewer missing samples. Without it the
/// denoiser has to blur much harder, so `Low` is the fastest and visibly the
/// softest; it is meant for moving around a scene, not for judging lighting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum SolariQuality {
    /// No ReSTIR, one bounce, fewest samples.
    Low,
    /// ReSTIR on, two bounces, half of Bevy's sample counts.
    Medium,
    /// ReSTIR on with Bevy's own defaults (three bounces).
    #[default]
    High,
    /// ReSTIR on, four bounces, double the samples. For stills.
    Ultra,
}

impl SolariQuality {
    /// Every preset, in the order the inspector lists them.
    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    /// The inspector dropdown labels, index for index with [`ALL`](Self::ALL).
    pub const LABELS: [&'static str; 4] = ["Low", "Medium", "High", "Ultra"];

    /// The inspector dropdown label.
    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    /// The preset with this [`label`](Self::label), if any.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.label() == label)
    }
}

/// Default for [`SolariGi::suppress_shadow_maps`], used both by `serde` and by
/// reflection so a scene saved before the field existed loads with suppression
/// on rather than with `bool::default()` (`false`).
fn shadow_map_suppression_default() -> bool {
    true
}

/// Default for [`SolariGi::light_proxies`]. On, because without it point and
/// spot lights are simply absent from a Solari render.
fn light_proxies_default() -> bool {
    true
}

impl Default for SolariGi {
    fn default() -> Self {
        Self {
            enabled: true,
            suppress_shadow_maps: shadow_map_suppression_default(),
            light_proxies: light_proxies_default(),
            quality: SolariQuality::default(),
            restir: None,
            denoiser: SolariDenoiser::default(),
            denoise: legacy_toggle_default(),
            dlss: legacy_toggle_default(),
            dlss_mode: default_solari_dlss_mode(),
            glass_reflections: true,
            resolution: SolariResolution::default(),
        }
    }
}

// ── Ray tracing diagnostics ────────────────────────────────────────────────

/// What the editor's Ray Tracing panel asks Solari to show and measure on the
/// primary viewport. Written by the panel, read by `renzora_solari`; a shipped
/// game never has one.
#[derive(Resource, Clone, Copy, Debug)]
pub struct SolariDiagnosticsRequest {
    /// What to draw in place of the viewport's image.
    pub view: SolariDebugMode,
    /// Where [`SolariDebugMode::RawSplit`] divides the image, as a fraction of
    /// its width.
    pub split: f32,
    /// Measure every frame into [`SolariDiagnostics`]. On while the panel is
    /// visible, since the measuring costs GPU time.
    pub stats: bool,
    /// Accumulate the path-traced reference.
    pub reference: bool,
    /// Start the reference over; cleared once it has been passed on.
    pub reset_reference: bool,
}

impl Default for SolariDiagnosticsRequest {
    fn default() -> Self {
        Self {
            view: SolariDebugMode::None,
            split: 0.5,
            stats: false,
            reference: false,
            reset_reference: false,
        }
    }
}

impl SolariDiagnosticsRequest {
    /// Whether this asks anything of the renderer at all.
    pub fn is_active(&self) -> bool {
        self.view != SolariDebugMode::None || self.stats || self.reference
    }
}

/// A visualization of Solari's internals, drawn over the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SolariDebugMode {
    /// The image as rendered.
    #[default]
    None,
    /// The raw estimate on one side, the denoised result on the other.
    RawSplit,
    /// How many frames each pixel's denoised lighting averages.
    HistoryLength,
    /// Where the denoiser threw its history away this frame.
    Rejected,
    /// The denoiser's estimate of each pixel's noise.
    NoiseEstimate,
    /// How many reused samples each ReSTIR reservoir stands for.
    ReservoirConfidence,
    /// The world cache's stored light, which bounce light is built from.
    WorldCache,
    /// Where the ray-traced scene does not match what was drawn.
    NotTraced,
    /// The path-traced reference.
    Reference,
    /// The image's difference from the reference.
    ReferenceError,
}

impl SolariDebugMode {
    /// Every mode, in the order the panel lists them.
    pub const ALL: [Self; 10] = [
        Self::None,
        Self::RawSplit,
        Self::HistoryLength,
        Self::Rejected,
        Self::NoiseEstimate,
        Self::ReservoirConfidence,
        Self::WorldCache,
        Self::NotTraced,
        Self::Reference,
        Self::ReferenceError,
    ];

    /// The panel's labels, index for index with [`ALL`](Self::ALL).
    pub const LABELS: [&'static str; 10] = [
        "Final image",
        "Raw | Denoised",
        "History length",
        "Rejected history",
        "Noise estimate",
        "ReSTIR confidence",
        "World cache",
        "Not in ray-traced scene",
        "Path-traced reference",
        "Error vs reference",
    ];

    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.label() == label)
    }

    /// What reading the mode's colours takes: one line for the panel to show
    /// under the dropdown.
    pub fn legend(self) -> &'static str {
        match self {
            Self::None => "",
            Self::RawSplit => "Left of the line: Solari's raw estimate. Right: after denoising.",
            Self::HistoryLength => "Blue: no history yet. Red: the most frames the denoiser keeps (32).",
            Self::Rejected => "Red: history thrown away this frame (a new or changed surface).",
            Self::NoiseEstimate => "Blue: clean. Red: one frame's noise four times the light itself.",
            Self::ReservoirConfidence => "Blue: one sample. Red: the most a reservoir can hold. Blank without ReSTIR.",
            Self::WorldCache => "The cached light bounce rays read; blocky where cells are large.",
            Self::NotTraced => "Magenta: what was drawn here is missing from, or different in, the ray-traced scene.",
            Self::Reference => "A conventional path tracer's image, converging while Accumulate is on.",
            Self::ReferenceError => "Blue: matches the reference. Red: 50% or more off.",
        }
    }
}

/// What the Ray Tracing panel shows, written by `renzora_solari`.
#[derive(Resource, Clone, Debug, Default)]
pub struct SolariDiagnostics {
    /// The latest frame measured, a few frames behind the screen.
    pub frame: Option<SolariFrameStats>,
    pub coverage: SolariCoverage,
    /// GPU time of each Solari pass, in milliseconds, by pass name.
    pub gpu_ms: Vec<(String, f64)>,
    /// World cache cells in use.
    pub world_cache_cells: Option<f64>,
    /// DLSS Ray Reconstruction is denoising the measured view, so the built-in
    /// denoiser's figures do not apply.
    pub dlss_running: bool,
}

/// Which meshes made it into the ray-traced scene.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SolariCoverage {
    /// Instances in the ray-traced scene.
    pub mirrored: usize,
    /// Of those, traced through a copy stripped of attributes Solari rejects.
    pub stripped_copies: usize,
    /// Of those, traced with a plain stand-in for a node-graph material.
    pub stand_ins: usize,
    /// Meshes Solari cannot trace (no UVs, not an indexed triangle list).
    pub skipped: usize,
    /// Meshes held back on purpose: hidden, off the scene's layer, or blended.
    pub excluded: usize,
}

/// One frame of Solari's measurements. See `bevy_solari`'s `SolariDebugStats`,
/// which this mirrors so the editor needs nothing from the renderer's crate.
#[derive(Clone, Debug, Default)]
pub struct SolariFrameStats {
    pub width: u32,
    pub height: u32,
    pub covered_pixels: u64,
    pub rays_initial: u64,
    pub shadow_rays_initial: u64,
    pub rays_reuse: u64,
    pub shadow_rays_reuse: u64,
    pub rays_world_cache: u64,
    pub shadow_rays_world_cache: u64,
    pub rays_debug: u64,
    pub denoiser_active: bool,
    pub restir: bool,
    pub mean_history: f32,
    pub short_history_fraction: f32,
    pub rejected_fraction: f32,
    pub mean_confidence: f32,
    pub confidence_cap: f32,
    pub grain_raw: f32,
    pub grain_denoised: f32,
    pub not_traced_fraction: f32,
    /// Brightness histograms of the exposed image before and after denoising,
    /// in bins of `histogram_stops_per_bin` stops from `2^histogram_min_log2`.
    pub histogram_raw: Vec<u32>,
    pub histogram_denoised: Vec<u32>,
    pub histogram_min_log2: f32,
    pub histogram_stops_per_bin: f32,
    pub reference_samples: u32,
    pub reference_error: Option<f32>,
}

impl SolariFrameStats {
    /// Every ray the lighting traced, not counting the diagnostics' own.
    pub fn lighting_rays(&self) -> u64 {
        self.rays_initial + self.rays_reuse + self.rays_world_cache
    }
}
