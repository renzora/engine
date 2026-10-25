//! Animation clip format + property-track sampling.
//!
//! Split out of `core/mod.rs` (which had grown large). Holds the `.anim` clip
//! types (`AnimClip`, `BoneTrack`, markers), the property-keyframe model
//! (`PropertyTrack` / `PropertyKey` / `TrackValue` / `Interp`) and its sampler,
//! the Euler-degree rotation cache, and the deferred transform-write queue.
//! Re-exported from `core` (`pub use animation::*`) so every `renzora::Foo`
//! path stays unchanged across the dlopen boundary.

use bevy::prelude::*;
use std::collections::HashMap;

use super::PropertyValue;

// ============================================================================
// Animation clip format (shared between animation and import)
// ============================================================================

/// One animation clip, serialized to a `.anim` file (RON format).
///
/// `Default` exists so code building a clip can write `..Default::default()`
/// and keep compiling when the format grows a field, instead of every struct
/// literal across the importers, the runtime and the editor breaking at once.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AnimClip {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<BoneTrack>,
    /// Property-animation tracks: keyframes bound to arbitrary component fields
    /// (Transform translation/rotation/scale, or any reflected field). Distinct
    /// from skeletal `tracks` (bone curves) — these are sampled by a custom
    /// sampler, not Bevy's `AnimationPlayer`. `#[serde(default)]` keeps legacy
    /// `.anim` files (which have no `property_tracks` field) loadable.
    #[serde(default)]
    pub property_tracks: Vec<PropertyTrack>,
    /// Named event markers — when playback crosses one, scripts' `on_animation_event`
    /// hook fires with the marker name.
    #[serde(default)]
    pub markers: Vec<AnimMarker>,
    /// Blend shape (morph target) weight tracks, one per animated target.
    ///
    /// Played by Bevy's animation player beside the bone curves (not by the
    /// property sampler), so they crossfade, layer and blend with the
    /// skeleton exactly like a bone does. Skipped when empty, so every `.anim`
    /// written before blend shapes could be keyed loads and saves
    /// byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morph_tracks: Vec<MorphTrack>,
}

/// The keyed weight of one blend shape (morph target) of one mesh node.
///
/// One track per target rather than one per mesh holding every target's
/// weight: a face rig has dozens of shapes and a pose keys a handful, so a
/// per-mesh track would store (and the dopesheet would draw) a key for every
/// shape the animator never touched. The runtime regroups the tracks by node
/// into the single all-weights curve Bevy plays (`renzora_animation::loader`).
///
/// Easing reuses the bone keys' machinery: [`KeyCurve`]s beside the keys,
/// evaluated by [`ChannelSpline`] on the `x` component, so the dopesheet's
/// interpolation menu and the curve editor mean the same thing here.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MorphTrack {
    /// Name of the node carrying the weights (the glTF mesh node), bound by
    /// name the same way a bone track is, so it survives the sanitizing
    /// rename and plays on any model naming its face mesh the same way.
    pub target: String,
    /// Which of that node's morph targets.
    pub index: usize,
    /// The target's own name (`jawOpen`), when the mesh named its targets.
    /// Display only: the binding is `target` plus `index`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// `(time, weight)` keys, in time order.
    #[serde(default)]
    pub keys: Vec<(f32, f32)>,
    /// Per-key interpolation, index for index with `keys`; missing entries
    /// are [`KeyMode::Linear`], as for a bone track.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curves: Vec<KeyCurve>,
    /// Disabled in the timeline: not played, keys kept. See [`BoneTrack::muted`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
}

impl MorphTrack {
    pub fn new(target: impl Into<String>, index: usize, name: impl Into<String>) -> Self {
        MorphTrack { target: target.into(), index, name: name.into(), keys: Vec::new(), curves: Vec::new(), muted: false }
    }

    /// What the timeline calls the track: the target's name when it has one.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            format!("Shape {}", self.index + 1)
        } else {
            self.name.clone()
        }
    }

    pub fn key_time(&self, index: usize) -> Option<f32> {
        self.keys.get(index).map(|k| k.0)
    }

    pub fn key_curve(&self, index: usize) -> KeyCurve {
        self.curves.get(index).copied().unwrap_or(KeyCurve::LINEAR)
    }

    /// The curves sized to the keys, as [`BoneTrack::curves_mut`] does for a
    /// bone channel, so the two vectors stay index for index.
    pub fn curves_mut(&mut self) -> &mut Vec<KeyCurve> {
        let n = self.keys.len();
        self.curves.resize(n, KeyCurve::LINEAR);
        &mut self.curves
    }

    pub fn set_key_curve(&mut self, index: usize, curve: KeyCurve) {
        if let Some(c) = self.curves_mut().get_mut(index) {
            *c = curve;
        }
    }

    /// Move key `index` to `time`, leaving the keys unsorted until
    /// [`Self::sort`] (a drag sorts once, when it is let go).
    pub fn set_key_time(&mut self, index: usize, time: f32) {
        if let Some(k) = self.keys.get_mut(index) {
            k.0 = time;
        }
    }

    /// Remove key `index` and its curve together.
    pub fn remove_key(&mut self, index: usize) {
        if index >= self.keys.len() {
            return;
        }
        self.curves_mut().remove(index);
        self.keys.remove(index);
    }

    /// Put the keys back in time order, carrying each key's curve with it.
    pub fn sort(&mut self) {
        let mut order: Vec<usize> = (0..self.keys.len()).collect();
        order.sort_by(|a, b| self.keys[*a].0.total_cmp(&self.keys[*b].0));
        if order.iter().enumerate().all(|(i, j)| i == *j) {
            return;
        }
        let curves = self.curves_mut().clone();
        self.curves = order.iter().map(|&i| curves[i]).collect();
        self.keys = order.iter().map(|&i| self.keys[i]).collect();
    }

    /// Set the weight at `time`: a key within `same` seconds takes the value
    /// and keeps its curve, else a new key with `curve` is inserted in time
    /// order. Returns the key's index.
    pub fn upsert_key(&mut self, time: f32, weight: f32, curve: KeyCurve, same: f32) -> usize {
        if let Some(i) = self.keys.iter().position(|k| (k.0 - time).abs() < same) {
            self.keys[i].1 = weight;
            return i;
        }
        let at = self.keys.iter().position(|k| k.0 > time).unwrap_or(self.keys.len());
        self.curves_mut().insert(at, curve);
        self.keys.insert(at, (time, weight));
        at
    }

    /// The evaluator, as playback and the editor both sample it: the weight
    /// rides in the `x` component of a [`ChannelSpline`].
    pub fn spline(&self) -> ChannelSpline {
        let keys: Vec<(f32, Vec4)> = self.keys.iter().map(|(t, w)| (*t, Vec4::new(*w, 0.0, 0.0, 0.0))).collect();
        ChannelSpline::new(&keys, &self.curves, false)
    }

    /// The weight at `t`, held at the first and last key; `None` with no keys.
    pub fn sample(&self, t: f32) -> Option<f32> {
        (!self.keys.is_empty()).then(|| self.spline().sample(t).x)
    }
}

/// A named event marker on an animation clip's timeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimMarker {
    pub time: f32,
    pub name: String,
}

/// The rest (bind) pose of the skeleton a set of clips was authored on,
/// serialized as a `.skel` file (RON).
///
/// Retargeting needs it: a rotation key only means something relative to the
/// rest orientation of the bone it was keyed on, so playing a clip on a rig
/// whose bones rest differently (another bone-axis convention, an A-pose
/// against a T-pose, other proportions) needs both rest poses to carry the
/// motion across. It lives in its own file rather than inside `.anim` because
/// one skeleton serves every clip exported from it, and because a `.anim` is
/// rewritten by the timeline on every edit.
///
/// Transforms are each bone's **local** rest transform (relative to its
/// parent), exactly what the bone's `Transform` holds when the skeleton is
/// spawned unposed.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkeletonRestPose {
    #[serde(default)]
    pub bones: Vec<RestBone>,
    /// File names of the `.anim` clips beside this file that were extracted
    /// from the same model, so clip discovery can point each clip's slot at
    /// the skeleton it was keyed on. Several models imported into one folder
    /// share one `animations/` directory, and nothing in a `.anim` says which
    /// of them it came from; the skeleton names its clips instead of each
    /// clip naming its skeleton because a `.anim` is rewritten by the
    /// timeline. Skipped when empty, so a pose written without it is
    /// unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<String>,
}

/// One bone of a [`SkeletonRestPose`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RestBone {
    /// The bone's name as the clips spell it (`mixamorig:Hips`).
    pub name: String,
    /// The parent bone's name, `None` for a root.
    #[serde(default)]
    pub parent: Option<String>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    #[serde(default = "rest_bone_unit_scale")]
    pub scale: [f32; 3],
}

fn rest_bone_unit_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

/// Write a [`SkeletonRestPose`] to a `.skel` file (RON format).
pub fn write_skeleton_file(skeleton: &SkeletonRestPose, path: &std::path::Path) -> Result<(), String> {
    let ron_str = ron::ser::to_string_pretty(skeleton, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialization error: {}", e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    std::fs::write(path, ron_str).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

/// Animation curves for a single bone/target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoneTrack {
    pub bone_name: String,
    pub translations: Vec<(f32, [f32; 3])>,
    pub rotations: Vec<(f32, [f32; 4])>,
    pub scales: Vec<(f32, [f32; 3])>,
    /// Disabled in the timeline: the clip plays as if this track were absent,
    /// but its keys are kept so it can be switched back on. Skipped when false,
    /// so every `.anim` written before the flag existed (or with it off) is
    /// byte-identical.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
    /// How each key moves on to the next one, and its tangents. Kept beside
    /// the key vectors rather than inside them so the `(time, value)` pairs,
    /// which importers, the runtime and every older `.anim` already speak, stay
    /// as they are. Missing entries read as [`KeyMode::Linear`], which is what
    /// bone keys always did, so a file written before this field existed plays
    /// exactly as it did; and an all-empty set is skipped when written.
    #[serde(default, skip_serializing_if = "BoneKeyCurves::is_empty")]
    pub curves: BoneKeyCurves,
}

// ----------------------------------------------------------------------------
// Bone key curves: per-key interpolation and tangents
// ----------------------------------------------------------------------------

/// How a bone key leaves for the next key, and where its tangents come from.
///
/// The set is the one animators know from other editors: two straight modes,
/// and four smooth ones that differ only in who sets the tangents. A smooth
/// segment is a cubic Hermite between the two keys (per component; a rotation
/// is a quaternion, interpolated per component and normalised).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize, Reflect)]
pub enum KeyMode {
    /// A straight line to the next key (`slerp` for a rotation). What every
    /// bone key written before key modes existed does, so it is the default.
    #[default]
    Linear,
    /// Hold this key's value until the next key.
    Constant,
    /// Smooth, with tangents worked out from the neighbouring keys and clamped
    /// so the curve never overshoots them: level at a peak or a valley, and at
    /// the first and last key. What a newly keyed pose gets, so poses ease
    /// into each other instead of moving at a constant, robotic speed.
    Auto,
    /// Smooth, with level tangents: eases out of and into the key.
    Flat,
    /// Smooth, with a hand-set tangent shared by both sides (read from
    /// [`KeyCurve::out_tan`]).
    Free,
    /// Hand-set tangents, independent on each side: a corner in the curve.
    Broken,
}

impl KeyMode {
    pub const ALL: [KeyMode; 6] = [KeyMode::Auto, KeyMode::Linear, KeyMode::Constant, KeyMode::Flat, KeyMode::Free, KeyMode::Broken];

    /// The segment after this key is a cubic, not a line or a hold.
    pub fn is_smooth(self) -> bool {
        matches!(self, KeyMode::Auto | KeyMode::Flat | KeyMode::Free | KeyMode::Broken)
    }

    /// The tangents are the stored ones, not derived from the keys.
    pub fn is_hand_set(self) -> bool {
        matches!(self, KeyMode::Free | KeyMode::Broken)
    }

    pub fn label(self) -> &'static str {
        match self {
            KeyMode::Linear => "Linear",
            KeyMode::Constant => "Constant",
            KeyMode::Auto => "Auto",
            KeyMode::Flat => "Flat",
            KeyMode::Free => "Free",
            KeyMode::Broken => "Broken",
        }
    }
}

fn is_zero4(v: &[f32; 4]) -> bool {
    v.iter().all(|c| *c == 0.0)
}

/// One bone key's interpolation and hand-set tangents.
///
/// Tangents are slopes, value units per second per component: metres a
/// second for a position, quaternion components a second for a rotation
/// (measured in the sign the key is stored in, so a key the sampler flips onto
/// its neighbour's hemisphere flips its tangents with it). Only
/// [`KeyMode::Free`] (`out_tan`, both sides) and [`KeyMode::Broken`] (each
/// side) read them; the other modes derive theirs, which is why they are left
/// out of the file when zero.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct KeyCurve {
    #[serde(default)]
    pub mode: KeyMode,
    #[serde(default, skip_serializing_if = "is_zero4")]
    pub in_tan: [f32; 4],
    #[serde(default, skip_serializing_if = "is_zero4")]
    pub out_tan: [f32; 4],
}

impl KeyCurve {
    pub const LINEAR: KeyCurve = KeyCurve { mode: KeyMode::Linear, in_tan: [0.0; 4], out_tan: [0.0; 4] };
    /// What a key made in the editor starts as.
    pub const AUTO: KeyCurve = KeyCurve { mode: KeyMode::Auto, in_tan: [0.0; 4], out_tan: [0.0; 4] };

    pub fn new(mode: KeyMode) -> Self {
        KeyCurve { mode, ..KeyCurve::LINEAR }
    }
}

/// Per-key curves of a [`BoneTrack`], index for index with its key vectors.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BoneKeyCurves {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub translations: Vec<KeyCurve>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rotations: Vec<KeyCurve>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scales: Vec<KeyCurve>,
}

impl BoneKeyCurves {
    pub fn is_empty(&self) -> bool {
        self.translations.is_empty() && self.rotations.is_empty() && self.scales.is_empty()
    }

    /// One channel's curves (0 translation, 1 rotation, 2 scale: the editor's
    /// lane numbering).
    pub fn channel(&self, channel: u8) -> &Vec<KeyCurve> {
        match channel {
            0 => &self.translations,
            1 => &self.rotations,
            _ => &self.scales,
        }
    }

    pub fn channel_mut(&mut self, channel: u8) -> &mut Vec<KeyCurve> {
        match channel {
            0 => &mut self.translations,
            1 => &mut self.rotations,
            _ => &mut self.scales,
        }
    }
}

impl BoneTrack {
    /// An empty track for `bone_name`.
    pub fn new(bone_name: impl Into<String>) -> Self {
        BoneTrack {
            bone_name: bone_name.into(),
            translations: Vec::new(),
            rotations: Vec::new(),
            scales: Vec::new(),
            muted: false,
            curves: BoneKeyCurves::default(),
        }
    }

    /// Keys on one channel (0 translation, 1 rotation, 2 scale).
    pub fn key_count(&self, channel: u8) -> usize {
        match channel {
            0 => self.translations.len(),
            1 => self.rotations.len(),
            _ => self.scales.len(),
        }
    }

    /// One channel's keys as `(time, value)`, a position or scale padded to
    /// four components so every channel shares one sampler.
    pub fn channel_keys(&self, channel: u8) -> Vec<(f32, Vec4)> {
        match channel {
            0 => self.translations.iter().map(|(t, v)| (*t, Vec3::from_array(*v).extend(0.0))).collect(),
            1 => self.rotations.iter().map(|(t, q)| (*t, Vec4::from_array(*q))).collect(),
            _ => self.scales.iter().map(|(t, v)| (*t, Vec3::from_array(*v).extend(0.0))).collect(),
        }
    }

    /// Key `index`'s curve; a key with none (any key of an older file) is
    /// [`KeyCurve::LINEAR`].
    pub fn key_curve(&self, channel: u8, index: usize) -> KeyCurve {
        self.curves.channel(channel).get(index).copied().unwrap_or(KeyCurve::LINEAR)
    }

    /// One channel's curves, sized to its keys first: padded with
    /// [`KeyCurve::LINEAR`] (so keys of an older file keep playing as they
    /// did) and trimmed of any left over from deleted keys. Every edit that
    /// inserts, removes or reorders keys goes through this, so the two vectors
    /// stay index for index.
    pub fn curves_mut(&mut self, channel: u8) -> &mut Vec<KeyCurve> {
        let n = self.key_count(channel);
        let curves = self.curves.channel_mut(channel);
        curves.resize(n, KeyCurve::LINEAR);
        curves
    }

    /// Set key `index`'s curve.
    pub fn set_key_curve(&mut self, channel: u8, index: usize, curve: KeyCurve) {
        if let Some(c) = self.curves_mut(channel).get_mut(index) {
            *c = curve;
        }
    }

    pub fn key_time(&self, channel: u8, index: usize) -> Option<f32> {
        match channel {
            0 => self.translations.get(index).map(|k| k.0),
            1 => self.rotations.get(index).map(|k| k.0),
            _ => self.scales.get(index).map(|k| k.0),
        }
    }

    /// Move key `index` to `time`. The channel is left unsorted (a drag moves
    /// a key past its neighbours and sorts once, when it is let go); see
    /// [`Self::sort_channel`].
    pub fn set_key_time(&mut self, channel: u8, index: usize, time: f32) {
        let slot = match channel {
            0 => self.translations.get_mut(index).map(|k| &mut k.0),
            1 => self.rotations.get_mut(index).map(|k| &mut k.0),
            _ => self.scales.get_mut(index).map(|k| &mut k.0),
        };
        if let Some(t) = slot {
            *t = time;
        }
    }

    /// Remove key `index` and its curve together.
    pub fn remove_key(&mut self, channel: u8, index: usize) {
        if index >= self.key_count(channel) {
            return;
        }
        self.curves_mut(channel).remove(index);
        match channel {
            0 => {
                self.translations.remove(index);
            }
            1 => {
                self.rotations.remove(index);
            }
            _ => {
                self.scales.remove(index);
            }
        }
    }

    /// Put a channel's keys back in time order, carrying each key's curve
    /// with it. Stable, so two keys dragged onto one time keep their order.
    pub fn sort_channel(&mut self, channel: u8) {
        let n = self.key_count(channel);
        let times: Vec<f32> = (0..n).filter_map(|i| self.key_time(channel, i)).collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|a, b| times[*a].total_cmp(&times[*b]));
        if order.iter().enumerate().all(|(i, j)| i == *j) {
            return;
        }
        let curves = self.curves_mut(channel).clone();
        *self.curves.channel_mut(channel) = order.iter().map(|&i| curves[i]).collect();
        match channel {
            0 => self.translations = order.iter().map(|&i| self.translations[i]).collect(),
            1 => self.rotations = order.iter().map(|&i| self.rotations[i]).collect(),
            _ => self.scales = order.iter().map(|&i| self.scales[i]).collect(),
        }
    }

    /// Set the value at `time` (a position or scale in `xyz`, a rotation as
    /// `xyzw`): a key within `same` seconds of it takes the value and keeps
    /// its curve, else a new key is inserted in time order with `curve`.
    /// Returns the key's index.
    pub fn upsert_key(&mut self, channel: u8, time: f32, value: Vec4, curve: KeyCurve, same: f32) -> usize {
        let n = self.key_count(channel);
        let existing = (0..n).find(|&i| self.key_time(channel, i).is_some_and(|t| (t - time).abs() < same));
        let at = existing.unwrap_or_else(|| (0..n).find(|&i| self.key_time(channel, i).is_some_and(|t| t > time)).unwrap_or(n));
        if existing.is_none() {
            self.curves_mut(channel).insert(at, curve);
        }
        let v3 = value.truncate().to_array();
        match (channel, existing) {
            (0, Some(i)) => self.translations[i].1 = v3,
            (0, None) => self.translations.insert(at, (time, v3)),
            (1, Some(i)) => self.rotations[i].1 = value.to_array(),
            (1, None) => self.rotations.insert(at, (time, value.to_array())),
            (_, Some(i)) => self.scales[i].1 = v3,
            (_, None) => self.scales.insert(at, (time, v3)),
        }
        at
    }

    /// The evaluator for one channel, as playback and the editor both sample it.
    pub fn channel_spline(&self, channel: u8) -> ChannelSpline {
        ChannelSpline::new(&self.channel_keys(channel), self.curves.channel(channel), channel == 1)
    }
}

/// One bone channel, ready to sample: the keys, each key's mode, and its
/// tangents resolved (auto tangents worked out, hand-set ones read, rotations
/// aligned onto one hemisphere).
///
/// Built once per channel per clip rebuild, not per sample. Reflected because
/// it is the payload of the curve the runtime hands Bevy's animation player.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct ChannelSpline {
    times: Vec<f32>,
    values: Vec<Vec4>,
    modes: Vec<KeyMode>,
    in_tan: Vec<Vec4>,
    out_tan: Vec<Vec4>,
    /// Keys whose quaternion was negated to sit on the previous key's
    /// hemisphere. `q` and `-q` are the same rotation, but interpolating
    /// components across the flip would swing the long way round.
    flipped: Vec<bool>,
    rotation: bool,
}

/// Slopes below this (value per second) are level: no tangent to clamp.
const FLAT_SLOPE: f32 = 1e-9;

impl ChannelSpline {
    /// Build the evaluator. `curves` may be shorter than `keys` (an older
    /// file has none): the missing keys are linear.
    pub fn new(keys: &[(f32, Vec4)], curves: &[KeyCurve], rotation: bool) -> Self {
        let n = keys.len();
        let times: Vec<f32> = keys.iter().map(|k| k.0).collect();
        let mut values: Vec<Vec4> = keys.iter().map(|k| k.1).collect();
        let mut flipped = vec![false; n];
        if rotation {
            for i in 1..n {
                if values[i].dot(values[i - 1]) < 0.0 {
                    values[i] = -values[i];
                    flipped[i] = true;
                }
            }
        }
        let curve = |i: usize| curves.get(i).copied().unwrap_or(KeyCurve::LINEAR);
        let modes: Vec<KeyMode> = (0..n).map(|i| curve(i).mode).collect();
        let slope = |a: usize, b: usize| (values[b] - values[a]) / (times[b] - times[a]).max(1e-6);
        let mut in_tan = vec![Vec4::ZERO; n];
        let mut out_tan = vec![Vec4::ZERO; n];
        for i in 0..n {
            let prev = (i > 0).then(|| slope(i - 1, i));
            let next = (i + 1 < n).then(|| slope(i, i + 1));
            let sign = if flipped[i] { -1.0 } else { 1.0 };
            let c = curve(i);
            let (a, b) = match c.mode {
                KeyMode::Linear => (prev.unwrap_or(Vec4::ZERO), next.unwrap_or(Vec4::ZERO)),
                KeyMode::Constant | KeyMode::Flat => (Vec4::ZERO, Vec4::ZERO),
                KeyMode::Auto => {
                    let m = match (prev, next) {
                        (Some(p), Some(q)) => auto_tangent(p, q, slope(i - 1, i + 1)),
                        _ => Vec4::ZERO,
                    };
                    (m, m)
                }
                KeyMode::Free => {
                    let m = Vec4::from_array(c.out_tan) * sign;
                    (m, m)
                }
                KeyMode::Broken => (Vec4::from_array(c.in_tan) * sign, Vec4::from_array(c.out_tan) * sign),
            };
            in_tan[i] = a;
            out_tan[i] = b;
        }
        ChannelSpline { times, values, modes, in_tan, out_tan, flipped, rotation }
    }

    pub fn len(&self) -> usize {
        self.times.len()
    }

    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Every key is linear: the channel plays exactly as bone keys always
    /// did, so the runtime can keep using Bevy's own sampled curve for it.
    pub fn all_linear(&self) -> bool {
        self.modes.iter().all(|m| *m == KeyMode::Linear)
    }

    pub fn times(&self) -> &[f32] {
        &self.times
    }

    /// Key `i`'s resolved `(in, out)` tangents, in the sign the key is stored
    /// in (so they can be written back into [`KeyCurve`] as they are).
    pub fn tangents(&self, i: usize) -> Option<(Vec4, Vec4)> {
        let sign = if *self.flipped.get(i)? { -1.0 } else { 1.0 };
        Some((self.in_tan[i] * sign, self.out_tan[i] * sign))
    }

    /// The value at `t`, held at the first and last key outside them. A
    /// rotation comes back normalised.
    pub fn sample(&self, t: f32) -> Vec4 {
        let n = self.times.len();
        if n == 0 {
            return if self.rotation { Vec4::W } else { Vec4::ZERO };
        }
        if n == 1 || t <= self.times[0] {
            return self.values[0];
        }
        if t >= self.times[n - 1] {
            return self.values[n - 1];
        }
        let i = self.times.partition_point(|k| *k <= t).saturating_sub(1).min(n - 2);
        let (t0, t1) = (self.times[i], self.times[i + 1]);
        let (v0, v1) = (self.values[i], self.values[i + 1]);
        let h = (t1 - t0).max(1e-6);
        let u = ((t - t0) / h).clamp(0.0, 1.0);
        match self.modes[i] {
            KeyMode::Constant => v0,
            KeyMode::Linear => {
                if self.rotation {
                    Vec4::from(Quat::from_vec4(v0).slerp(Quat::from_vec4(v1), u))
                } else {
                    v0.lerp(v1, u)
                }
            }
            _ => {
                let v = hermite(v0, self.out_tan[i] * h, v1, self.in_tan[i + 1] * h, u);
                if self.rotation {
                    v.try_normalize().unwrap_or(v0)
                } else {
                    v
                }
            }
        }
    }

    pub fn sample_vec3(&self, t: f32) -> Vec3 {
        self.sample(t).truncate()
    }

    pub fn sample_quat(&self, t: f32) -> Quat {
        Quat::from_vec4(self.sample(t)).normalize()
    }
}

/// Cubic Hermite between `p0` and `p1` at `u` in `0..=1`, with the tangents
/// already scaled by the segment's length.
pub fn hermite(p0: Vec4, m0: Vec4, p1: Vec4, m1: Vec4, u: f32) -> Vec4 {
    let u2 = u * u;
    let u3 = u2 * u;
    p0 * (2.0 * u3 - 3.0 * u2 + 1.0) + m0 * (u3 - 2.0 * u2 + u) + p1 * (-2.0 * u3 + 3.0 * u2) + m1 * (u3 - u2)
}

/// The clamped auto tangent at a key whose segment slopes are `prev` and
/// `next`, and whose neighbours' chord slope is `chord` (Catmull-Rom).
///
/// Per component: level where the key is a peak or a valley (the slopes
/// disagree in sign, or one is level), else the chord slope clamped to three
/// times the shallower segment slope. Fritsch and Carlson show a Hermite
/// segment whose end tangents are both within three times its own slope never
/// leaves the range of its two keys, so this is what stops a smooth pose from
/// swinging past where it was keyed.
fn auto_tangent(prev: Vec4, next: Vec4, chord: Vec4) -> Vec4 {
    let one = |p: f32, q: f32, c: f32| {
        if p * q <= 0.0 || p.abs() < FLAT_SLOPE || q.abs() < FLAT_SLOPE {
            0.0
        } else {
            let limit = 3.0 * p.abs().min(q.abs());
            c.clamp(-limit, limit)
        }
    };
    Vec4::new(one(prev.x, next.x, chord.x), one(prev.y, next.y, chord.y), one(prev.z, next.z, chord.z), one(prev.w, next.w, chord.w))
}

/// Euler degrees `(x, y, z)` of a rotation, decomposed `YXZ` like the rest of
/// the editor ([`rotation_euler_deg`]).
pub fn quat_to_euler_deg(q: Quat) -> Vec3 {
    let (y, x, z) = q.to_euler(EulerRot::YXZ);
    Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
}

/// `a` moved by whole turns to sit within half a turn of `near`, so a curve
/// of Euler angles read off quaternions does not jump from 180 to -180.
pub fn wrap_deg_near(a: f32, near: f32) -> f32 {
    a - ((a - near) / 360.0).round() * 360.0
}

/// Step, seconds, of the central differences below. Small enough to read a
/// tangent, large enough that `f32` keeps its digits.
const TANGENT_STEP: f32 = 1e-3;

/// The Euler slope (degrees per second, `x y z`) a quaternion tangent `qt`
/// moves the rotation `q` at. How the curve editor shows a rotation key's
/// handle on its Euler curves.
pub fn euler_slope_of_quat_tangent(q: Vec4, qt: Vec4) -> Vec3 {
    let h = TANGENT_STEP;
    let a = quat_to_euler_deg(Quat::from_vec4(q - qt * h).normalize());
    let b = quat_to_euler_deg(Quat::from_vec4(q + qt * h).normalize());
    let d = Vec3::new(wrap_deg_near(b.x, a.x) - a.x, wrap_deg_near(b.y, a.y) - a.y, wrap_deg_near(b.z, a.z) - a.z);
    d / (2.0 * h)
}

/// The quaternion tangent at `q` that moves its Euler angles at `slope`
/// (degrees per second): the inverse of [`euler_slope_of_quat_tangent`], for
/// writing back a handle dragged on an Euler curve. Measured in `q`'s own
/// sign.
pub fn quat_tangent_of_euler_slope(q: Vec4, slope: Vec3) -> Vec4 {
    let h = TANGENT_STEP;
    let e = quat_to_euler_deg(Quat::from_vec4(q).normalize());
    let at = |d: Vec3| {
        let r = euler_deg_to_quat(e + d);
        let v = Vec4::from(r);
        if v.dot(q) < 0.0 {
            -v
        } else {
            v
        }
    };
    (at(slope * h) - at(-slope * h)) / (2.0 * h)
}

// ----------------------------------------------------------------------------
// Property animation (keyframes bound to component fields)
// ----------------------------------------------------------------------------

/// Editor/runtime cache of the **Euler angles** (degrees, `YXZ` order) last
/// dialed into a rotation — by the inspector or a rotation animation. **Keyed per
/// component** so several rotation fields on one entity (e.g. `Transform` *and*
/// `EnvironmentMapLight`) each keep their own slot instead of fighting over one.
///
/// A quaternion stores only an orientation, so converting it back to Euler angles
/// is lossy: the middle axis (pitch) wraps at ±90° and full turns (360°, 720°) collapse
/// onto the same value. This keeps the *typed* angles intact so the inspector
/// shows what you entered and a 0→360 rotation key pair animates a real spin.
/// Each slot's `quat` is a staleness fingerprint: when the live rotation no
/// longer matches it, something else moved the entity and the angles are
/// re-derived from the quaternion (the lossy fallback).
///
/// Mirrors Godot's `Node3D::euler_rotation` + dirty-flag design. Not reflected,
/// so it's transient editor state and never serialized into a scene.
#[derive(Component, Clone, Debug, Default)]
pub struct EditorEulerCache {
    slots: HashMap<String, EulerSlot>,
}

#[derive(Clone, Copy, Debug)]
struct EulerSlot {
    deg: Vec3,
    quat: Quat,
}

impl EditorEulerCache {
    /// Cached degrees under `key` if they still describe `rotation` (`q` and `-q`
    /// are the same orientation, so both signs count as a match).
    pub fn degrees_for(&self, key: &str, rotation: Quat) -> Option<Vec3> {
        self.slots.get(key).and_then(|s| {
            (s.quat.abs_diff_eq(rotation, 1e-4) || s.quat.abs_diff_eq(-rotation, 1e-4))
                .then_some(s.deg)
        })
    }

    /// Store typed Euler `deg` under `key`; returns the quaternion they produce.
    pub fn store(&mut self, key: &str, deg: Vec3) -> Quat {
        let quat = euler_deg_to_quat(deg);
        self.slots.insert(key.to_string(), EulerSlot { deg, quat });
        quat
    }
}

/// Quaternion from Euler **degrees**, applied yaw → pitch → roll (`YXZ`).
///
/// The order matters and is not arbitrary. It matches what the scripting side
/// already does (`set_rotation` in `renzora_scripting`, and the `rotation_x/y/z`
/// context fields), so typing `(30, 45, 0)` in the inspector and calling
/// `set_rotation(30, 45, 0)` from a script now land on the same orientation.
/// They did not while this built `XYZ` and scripting built `YXZ`.
pub fn euler_deg_to_quat(deg: Vec3) -> Quat {
    Quat::from_euler(
        EulerRot::YXZ,
        deg.y.to_radians(),
        deg.x.to_radians(),
        deg.z.to_radians(),
    )
}

/// The Euler degrees to display/record for `rotation` under `key`, preferring
/// the cache when it still matches, else deriving from the quaternion.
///
/// The fallback decomposes as `YXZ`, so the axis that gets pinned to ±90° — the
/// unavoidable lossy one, whichever order you pick — is **pitch**, not yaw.
/// That is what people expect: things spin around Y far more often than they
/// tip past straight up. Decomposing as `XYZ` made yaw the pinned axis, so a
/// plain spin script (`rotate(0, 90 * delta, 0)`) showed X and Z snapping
/// between 0 and ±180 every time yaw crossed 90°, with Y folding back — a
/// display artefact of a perfectly smooth rotation.
pub fn rotation_euler_deg(rotation: Quat, cache: Option<&EditorEulerCache>, key: &str) -> Vec3 {
    if let Some(deg) = cache.and_then(|c| c.degrees_for(key, rotation)) {
        return deg;
    }
    let (y, x, z) = rotation.to_euler(EulerRot::YXZ);
    Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
}

/// Store `deg` under `key` in the entity's [`EditorEulerCache`] (inserting the
/// component if absent) and return the quaternion to apply to the rotation.
pub fn cache_euler_deg(world: &mut World, entity: Entity, key: &str, deg: Vec3) -> Quat {
    if world.get::<EditorEulerCache>(entity).is_none() {
        world.entity_mut(entity).insert(EditorEulerCache::default());
    }
    world
        .get_mut::<EditorEulerCache>(entity)
        .map(|mut c| c.store(key, deg))
        .unwrap_or_else(|| euler_deg_to_quat(deg))
}

/// A keyframe track bound to one field of one component on a target entity.
///
/// The track is resolved relative to the entity that owns the `AnimatorComponent`:
/// `target` is `""`/`"self"` for that entity, otherwise the `Name` of a descendant.
/// `component` is the reflected short type-name (case-insensitive, e.g. `"transform"`,
/// `"directional_light"`) and `field` is a dotted reflection path (e.g. `"translation"`,
/// `"rotation"`, `"scale"`, `"illuminance"`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyTrack {
    #[serde(default)]
    pub target: String,
    pub component: String,
    pub field: String,
    #[serde(default)]
    pub keys: Vec<PropertyKey>,
    /// Disabled in the timeline: never sampled, keys kept. See
    /// [`BoneTrack::muted`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
}

/// One keyframe: a value at a time, plus how to interpolate toward the next key.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyKey {
    pub time: f32,
    pub value: TrackValue,
    #[serde(default)]
    pub interp: Interp,
}

/// The animatable value kinds a property keyframe can hold. Mirrors the subset of
/// component-field types that can be sampled/interpolated. `Transform::rotation`
/// now records `Vec3` **Euler degrees** (component-lerped, so a 0→360 key pair
/// animates a full spin); `Quat` is retained for backward-compatibility with
/// older clips (slerp — shortest path, can't express a spin).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TrackValue {
    Float(f32),
    Vec3([f32; 3]),
    Quat([f32; 4]),
    Color([f32; 4]),
    Bool(bool),
}

/// How a keyframe interpolates toward the next keyframe.
///
/// `Eased` carries a Bevy [`EaseFunction`] (the same easing set the engine uses
/// for script tweens and UI transitions): the `0..=1` time fraction is remapped
/// through the curve *before* the component lerp, so a single pair of keys can
/// ease-in/out, overshoot (`BackOut`), or bounce. `Eq` is intentionally **not**
/// derived — `EaseFunction` holds `f32` parameters (`Elastic`, `Steps`) and is
/// only `PartialEq`.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Interp {
    /// Linear blend (component lerp; `slerp` for quaternions).
    #[default]
    Linear,
    /// Hold this key's value until the next key (constant / step).
    Stepped,
    /// Remap the time fraction through a Bevy easing curve, then component-lerp.
    Eased(EaseFunction),
}

impl Interp {
    /// Remap a linear `0..=1` blend fraction through this key's curve. `Linear`
    /// is the identity; `Stepped` holds the start value (and is also short-circuited
    /// in [`sample_property_track`] before this is reached); `Eased` samples the
    /// Bevy curve. The result can exceed `0..=1` for overshoot/elastic curves —
    /// that's intentional (it's what makes `BackOut`/`Elastic` overshoot).
    pub fn ease(self, frac: f32) -> f32 {
        match self {
            Interp::Linear => frac,
            Interp::Stepped => 0.0,
            // `sample_clamped` clamps `frac` into the curve's `0..=1` domain; the
            // *output* is left unclamped so overshoot/bounce read through.
            Interp::Eased(f) => f.sample_clamped(frac),
        }
    }
}

impl TrackValue {
    /// Convert to a [`PropertyValue`] for the reflection write path. `Quat` has no
    /// `PropertyValue` equivalent (it's handled only by the Transform fast-path),
    /// so it returns `None`.
    pub fn to_property_value(&self) -> Option<PropertyValue> {
        match self {
            TrackValue::Float(v) => Some(PropertyValue::Float(*v)),
            TrackValue::Vec3(v) => Some(PropertyValue::Vec3(*v)),
            TrackValue::Color(v) => Some(PropertyValue::Color(*v)),
            TrackValue::Bool(v) => Some(PropertyValue::Bool(*v)),
            TrackValue::Quat(_) => None,
        }
    }

    /// Build a `TrackValue` from a reflected [`PropertyValue`]. `Int` widens to
    /// `Float`; `String` has no animatable representation.
    pub fn from_property_value(pv: &PropertyValue) -> Option<TrackValue> {
        match pv {
            PropertyValue::Float(v) => Some(TrackValue::Float(*v)),
            PropertyValue::Int(v) => Some(TrackValue::Float(*v as f32)),
            PropertyValue::Bool(v) => Some(TrackValue::Bool(*v)),
            PropertyValue::Vec3(v) => Some(TrackValue::Vec3(*v)),
            PropertyValue::Color(v) => Some(TrackValue::Color(*v)),
            PropertyValue::String(_) => None,
        }
    }

    /// Linear blend between two values of the same kind (`t` in `0..=1`). Returns
    /// `a` if the kinds differ. Quaternions use `slerp`; bools snap at the midpoint.
    pub fn lerp(a: &TrackValue, b: &TrackValue, t: f32) -> TrackValue {
        match (a, b) {
            (TrackValue::Float(x), TrackValue::Float(y)) => TrackValue::Float(x + (y - x) * t),
            (TrackValue::Vec3(x), TrackValue::Vec3(y)) => TrackValue::Vec3([
                x[0] + (y[0] - x[0]) * t,
                x[1] + (y[1] - x[1]) * t,
                x[2] + (y[2] - x[2]) * t,
            ]),
            (TrackValue::Color(x), TrackValue::Color(y)) => TrackValue::Color([
                x[0] + (y[0] - x[0]) * t,
                x[1] + (y[1] - x[1]) * t,
                x[2] + (y[2] - x[2]) * t,
                x[3] + (y[3] - x[3]) * t,
            ]),
            (TrackValue::Quat(x), TrackValue::Quat(y)) => {
                let qa = bevy::prelude::Quat::from_array(*x);
                let qb = bevy::prelude::Quat::from_array(*y);
                TrackValue::Quat(qa.slerp(qb, t).to_array())
            }
            (TrackValue::Bool(x), TrackValue::Bool(y)) => {
                TrackValue::Bool(if t < 0.5 { *x } else { *y })
            }
            _ => *a,
        }
    }
}

/// Sample a property track at time `t` (seconds). Returns `None` for an empty
/// track. Clamps to the first/last key outside the keyed range. `Stepped` keys
/// hold their value until the next key; `Linear` keys blend toward the next.
pub fn sample_property_track(track: &PropertyTrack, t: f32) -> Option<TrackValue> {
    let keys = &track.keys;
    if keys.is_empty() {
        return None;
    }
    if t <= keys[0].time {
        return Some(keys[0].value);
    }
    let last = &keys[keys.len() - 1];
    if t >= last.time {
        return Some(last.value);
    }
    // Find the bracketing pair [i, i+1] with keys[i].time <= t < keys[i+1].time.
    // Keys are kept sorted by time on edit.
    let mut i = 0;
    while i + 1 < keys.len() && keys[i + 1].time <= t {
        i += 1;
    }
    let k0 = &keys[i];
    let k1 = &keys[i + 1];
    if matches!(k0.interp, Interp::Stepped) {
        return Some(k0.value);
    }
    let span = (k1.time - k0.time).max(f32::EPSILON);
    let frac = ((t - k0.time) / span).clamp(0.0, 1.0);
    // Remap the linear fraction through the key's easing curve before lerping, so
    // the editor scrub preview and runtime playback share the exact same shape.
    Some(TrackValue::lerp(&k0.value, &k1.value, k0.interp.ease(frac)))
}

#[cfg(test)]
mod property_anim_tests {
    use super::*;

    fn ftrack(keys: Vec<PropertyKey>) -> PropertyTrack {
        PropertyTrack { target: String::new(), component: "x".into(), field: "y".into(), keys, muted: false }
    }

    #[test]
    fn muted_flag_is_omitted_when_off_and_round_trips_when_on() {
        let mut track = ftrack(vec![]);
        let off = ron::ser::to_string(&track).unwrap();
        assert!(!off.contains("muted"), "an unmuted track must serialize as before: {off}");
        track.muted = true;
        let back: PropertyTrack = ron::from_str(&ron::ser::to_string(&track).unwrap()).unwrap();
        assert!(back.muted);
    }

    #[test]
    fn legacy_anim_without_property_tracks_loads() {
        // A `.anim` written before property tracks existed must still parse.
        let ron = r#"(name:"walk",duration:1.5,tracks:[])"#;
        let clip: AnimClip = ron::from_str(ron).unwrap();
        assert_eq!(clip.duration, 1.5);
        assert!(clip.property_tracks.is_empty());
    }

    #[test]
    fn anim_with_property_tracks_round_trips() {
        let clip = AnimClip {
            name: "sun".into(),
            duration: 2.0,
            tracks: vec![],
            property_tracks: vec![ftrack(vec![
                PropertyKey { time: 0.0, value: TrackValue::Quat([0.0, 0.0, 0.0, 1.0]), interp: Interp::Linear },
                PropertyKey { time: 2.0, value: TrackValue::Float(3.0), interp: Interp::Stepped },
            ])],
            markers: vec![],
            ..Default::default()
        };
        let s = ron::ser::to_string(&clip).unwrap();
        let back: AnimClip = ron::from_str(&s).unwrap();
        assert_eq!(back.property_tracks.len(), 1);
        assert_eq!(back.property_tracks[0].keys.len(), 2);
        assert_eq!(back.property_tracks[0].keys[1].interp, Interp::Stepped);
    }

    #[test]
    fn sample_linear_midpoint() {
        let track = ftrack(vec![
            PropertyKey { time: 0.0, value: TrackValue::Vec3([0.0, 0.0, 0.0]), interp: Interp::Linear },
            PropertyKey { time: 2.0, value: TrackValue::Vec3([2.0, 4.0, 0.0]), interp: Interp::Linear },
        ]);
        match sample_property_track(&track, 1.0).unwrap() {
            TrackValue::Vec3(v) => {
                assert!((v[0] - 1.0).abs() < 1e-5);
                assert!((v[1] - 2.0).abs() < 1e-5);
            }
            other => panic!("expected Vec3, got {other:?}"),
        }
    }

    #[test]
    fn sample_stepped_holds_previous() {
        let track = ftrack(vec![
            PropertyKey { time: 0.0, value: TrackValue::Float(0.0), interp: Interp::Stepped },
            PropertyKey { time: 2.0, value: TrackValue::Float(10.0), interp: Interp::Linear },
        ]);
        assert!(matches!(sample_property_track(&track, 1.9), Some(TrackValue::Float(v)) if v == 0.0));
    }

    #[test]
    fn sample_clamps_outside_range() {
        let track = ftrack(vec![PropertyKey {
            time: 1.0,
            value: TrackValue::Float(5.0),
            interp: Interp::Linear,
        }]);
        assert!(matches!(sample_property_track(&track, 0.0), Some(TrackValue::Float(v)) if v == 5.0));
        assert!(matches!(sample_property_track(&track, 9.0), Some(TrackValue::Float(v)) if v == 5.0));
        assert!(sample_property_track(&ftrack(vec![]), 0.0).is_none());
    }

    #[test]
    fn sample_eased_remaps_fraction() {
        // QuadraticIn eases the *fraction*, not the value range, so endpoints stay
        // exact (frac 0→0, 1→1) while the midpoint is pulled toward the start:
        // ease(0.5) = 0.25, so a 0→10 track reads 2.5 at the halfway time.
        let track = ftrack(vec![
            PropertyKey { time: 0.0, value: TrackValue::Float(0.0), interp: Interp::Eased(EaseFunction::QuadraticIn) },
            PropertyKey { time: 2.0, value: TrackValue::Float(10.0), interp: Interp::Linear },
        ]);
        assert!(matches!(sample_property_track(&track, 0.0), Some(TrackValue::Float(v)) if (v - 0.0).abs() < 1e-4));
        assert!(matches!(sample_property_track(&track, 2.0), Some(TrackValue::Float(v)) if (v - 10.0).abs() < 1e-4));
        match sample_property_track(&track, 1.0).unwrap() {
            TrackValue::Float(v) => assert!((v - 2.5).abs() < 1e-4, "midpoint was {v}"),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn eased_interp_round_trips() {
        // New `Eased` variant must survive a RON round-trip, and legacy clips
        // (no `Eased`) must still load — both guard backward compatibility.
        let track = ftrack(vec![PropertyKey {
            time: 0.0,
            value: TrackValue::Float(1.0),
            interp: Interp::Eased(EaseFunction::BackOut),
        }]);
        let s = ron::ser::to_string(&track).unwrap();
        let back: PropertyTrack = ron::from_str(&s).unwrap();
        assert_eq!(back.keys[0].interp, Interp::Eased(EaseFunction::BackOut));
    }

    #[test]
    fn quat_slerp_endpoints() {
        let a = TrackValue::Quat([0.0, 0.0, 0.0, 1.0]);
        let b = TrackValue::Quat([0.0, 0.0, 1.0, 0.0]);
        assert_eq!(TrackValue::lerp(&a, &b, 0.0), a);
        match TrackValue::lerp(&a, &b, 1.0) {
            TrackValue::Quat(q) => {
                // slerp to b (allowing sign flip — q and -q are the same rotation).
                let dot = q[2] * 1.0 + q[3] * 0.0;
                assert!(dot.abs() > 0.99);
            }
            other => panic!("expected Quat, got {other:?}"),
        }
    }

    #[test]
    fn track_value_conversions() {
        assert!(matches!(TrackValue::Float(1.0).to_property_value(), Some(PropertyValue::Float(_))));
        assert!(TrackValue::Quat([0.0, 0.0, 0.0, 1.0]).to_property_value().is_none());
        assert!(matches!(
            TrackValue::from_property_value(&PropertyValue::Vec3([1.0, 2.0, 3.0])),
            Some(TrackValue::Vec3(_))
        ));
        assert!(matches!(
            TrackValue::from_property_value(&PropertyValue::Int(4)),
            Some(TrackValue::Float(v)) if v == 4.0
        ));
    }
}

#[cfg(test)]
mod morph_track_tests {
    use super::*;

    /// The clip as it was before morph tracks: what every `.anim` on disk
    /// was written from.
    #[derive(serde::Serialize)]
    struct OldAnimClip {
        name: String,
        duration: f32,
        tracks: Vec<BoneTrack>,
        property_tracks: Vec<PropertyTrack>,
        markers: Vec<AnimMarker>,
    }

    fn bone() -> BoneTrack {
        let mut t = BoneTrack::new("hips");
        t.translations = vec![(0.0, [0.0, 1.0, 0.0]), (1.0, [0.0, 1.5, 0.0])];
        t
    }

    #[test]
    fn an_old_anim_loads_and_saves_byte_identical() {
        let pretty = ron::ser::PrettyConfig::default();
        let old = OldAnimClip {
            name: "walk".into(),
            duration: 1.0,
            tracks: vec![bone()],
            property_tracks: vec![],
            markers: vec![AnimMarker { time: 0.5, name: "step".into() }],
        };
        let on_disk = ron::ser::to_string_pretty(&old, pretty.clone()).unwrap();
        let clip: AnimClip = ron::from_str(&on_disk).unwrap();
        assert!(clip.morph_tracks.is_empty());
        assert_eq!(ron::ser::to_string_pretty(&clip, pretty).unwrap(), on_disk);
    }

    #[test]
    fn morph_tracks_round_trip_with_their_curves() {
        let mut m = MorphTrack::new("Face", 3, "jawOpen");
        m.upsert_key(0.0, 0.0, KeyCurve::LINEAR, 1e-3);
        m.upsert_key(1.0, 1.0, KeyCurve::AUTO, 1e-3);
        m.muted = true;
        let clip = AnimClip { name: "talk".into(), duration: 1.0, morph_tracks: vec![m.clone()], ..Default::default() };
        let back: AnimClip = ron::from_str(&ron::ser::to_string(&clip).unwrap()).unwrap();
        assert_eq!(back.morph_tracks, vec![m]);
        // A track with no name or curves leaves both out of the file.
        let mut bare = MorphTrack::new("Face", 0, "");
        bare.keys.push((0.0, 0.5));
        let s = ron::ser::to_string(&bare).unwrap();
        assert!(!s.contains("name") && !s.contains("curves") && !s.contains("muted"), "{s}");
    }

    #[test]
    fn morph_weights_sample_linear_held_and_eased() {
        let mut m = MorphTrack::new("Face", 0, "");
        m.keys = vec![(0.0, 0.0), (2.0, 1.0)];
        assert!((m.sample(1.0).unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(m.sample(-1.0), Some(0.0));
        assert_eq!(m.sample(5.0), Some(1.0));
        m.set_key_curve(0, KeyCurve::new(KeyMode::Constant));
        assert_eq!(m.sample(1.9), Some(0.0));
        m.set_key_curve(0, KeyCurve::AUTO);
        m.set_key_curve(1, KeyCurve::AUTO);
        // Level tangents at both ends: smoothstep, behind the line early on.
        assert!(m.sample(0.5).unwrap() < 0.25 - 0.05);
        assert!(MorphTrack::new("Face", 0, "").sample(0.0).is_none());
    }

    #[test]
    fn morph_key_edits_keep_curves_beside_their_keys() {
        let mut m = MorphTrack::new("Face", 0, "");
        m.keys = vec![(0.0, 0.0), (1.0, 1.0)];
        assert_eq!(m.upsert_key(0.5, 0.3, KeyCurve::AUTO, 1e-3), 1);
        assert_eq!(m.key_curve(1).mode, KeyMode::Auto);
        assert_eq!(m.key_curve(0).mode, KeyMode::Linear);
        // Keying the same time again keeps the curve and takes the value.
        m.set_key_curve(1, KeyCurve::new(KeyMode::Constant));
        assert_eq!(m.upsert_key(0.5, 0.9, KeyCurve::AUTO, 1e-3), 1);
        assert_eq!(m.keys[1], (0.5, 0.9));
        assert_eq!(m.key_curve(1).mode, KeyMode::Constant);
        m.set_key_time(1, 2.0);
        m.sort();
        assert_eq!(m.keys[2], (2.0, 0.9));
        assert_eq!(m.key_curve(2).mode, KeyMode::Constant);
        m.remove_key(2);
        assert_eq!((m.keys.len(), m.curves.len()), (2, 2));
    }
}

#[cfg(test)]
mod bone_curve_tests {
    use super::*;

    fn vkeys(vals: &[(f32, f32)]) -> Vec<(f32, Vec4)> {
        vals.iter().map(|(t, v)| (*t, Vec4::new(*v, 0.0, 0.0, 0.0))).collect()
    }

    fn all(mode: KeyMode, n: usize) -> Vec<KeyCurve> {
        vec![KeyCurve::new(mode); n]
    }

    #[test]
    fn a_track_from_before_key_curves_loads_and_plays_linear() {
        let ron = r#"(bone_name:"hips",translations:[(0.0,(0.0,0.0,0.0)),(1.0,(2.0,0.0,0.0))],rotations:[],scales:[])"#;
        let track: BoneTrack = ron::from_str(ron).unwrap();
        assert!(track.curves.is_empty());
        assert_eq!(track.key_curve(0, 1), KeyCurve::LINEAR);
        let s = track.channel_spline(0);
        assert!(s.all_linear());
        assert!((s.sample(0.25).x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_track_without_curves_writes_no_curves_field() {
        let mut track = BoneTrack::new("hips");
        track.translations.push((0.0, [0.0; 3]));
        let s = ron::ser::to_string(&track).unwrap();
        assert!(!s.contains("curves"), "{s}");
        track.set_key_curve(0, 0, KeyCurve::AUTO);
        let back: BoneTrack = ron::from_str(&ron::ser::to_string(&track).unwrap()).unwrap();
        assert_eq!(back.key_curve(0, 0).mode, KeyMode::Auto);
        // Zero tangents are left out of the file.
        assert!(!ron::ser::to_string(&track).unwrap().contains("in_tan"));
    }

    #[test]
    fn curves_mut_pads_with_linear_and_trims_deleted_keys() {
        let mut track = BoneTrack::new("arm");
        track.scales = vec![(0.0, [1.0; 3]), (1.0, [1.0; 3]), (2.0, [1.0; 3])];
        assert_eq!(track.curves_mut(2).len(), 3);
        assert!(track.curves.scales.iter().all(|c| *c == KeyCurve::LINEAR));
        track.scales.pop();
        assert_eq!(track.curves_mut(2).len(), 2);
    }

    #[test]
    fn key_edits_carry_each_keys_curve() {
        let mut track = BoneTrack::new("arm");
        // An older file's keys: no curves at all.
        track.translations = vec![(0.0, [0.0; 3]), (1.0, [1.0; 3])];
        let i = track.upsert_key(0, 0.5, Vec4::splat(5.0), KeyCurve::AUTO, 1e-3);
        assert_eq!(i, 1);
        assert_eq!(track.key_curve(0, 0).mode, KeyMode::Linear);
        assert_eq!(track.key_curve(0, 1).mode, KeyMode::Auto);
        // Keying the same time again replaces the value, not the curve.
        track.set_key_curve(0, 1, KeyCurve::new(KeyMode::Constant));
        assert_eq!(track.upsert_key(0, 0.5, Vec4::splat(6.0), KeyCurve::AUTO, 1e-3), 1);
        assert_eq!(track.key_curve(0, 1).mode, KeyMode::Constant);
        assert_eq!(track.translations[1].1, [6.0; 3]);
        // Dragged past its neighbour, the curve follows the key.
        track.set_key_time(0, 1, 2.0);
        track.sort_channel(0);
        assert_eq!(track.key_time(0, 2), Some(2.0));
        assert_eq!(track.key_curve(0, 2).mode, KeyMode::Constant);
        assert_eq!(track.key_curve(0, 1).mode, KeyMode::Linear);
        track.remove_key(0, 2);
        assert_eq!(track.curves.translations.len(), 2);
    }

    #[test]
    fn hermite_meets_its_ends() {
        let (a, b) = (Vec4::splat(1.0), Vec4::splat(3.0));
        assert_eq!(hermite(a, Vec4::splat(5.0), b, Vec4::splat(-2.0), 0.0), a);
        assert!((hermite(a, Vec4::splat(5.0), b, Vec4::splat(-2.0), 1.0) - b).length() < 1e-6);
    }

    #[test]
    fn auto_between_two_keys_eases_in_and_out() {
        // Both ends are level, so the segment is smoothstep: slow, fast, slow.
        let s = ChannelSpline::new(&vkeys(&[(0.0, 0.0), (1.0, 10.0)]), &all(KeyMode::Auto, 2), false);
        assert!((s.sample(0.5).x - 5.0).abs() < 1e-4);
        assert!((s.sample(0.25).x - 1.5625).abs() < 1e-4, "{}", s.sample(0.25).x);
        assert_eq!(s.tangents(0), Some((Vec4::ZERO, Vec4::ZERO)));
    }

    #[test]
    fn auto_is_level_at_a_peak() {
        let s = ChannelSpline::new(&vkeys(&[(0.0, 0.0), (1.0, 5.0), (2.0, 0.0)]), &all(KeyMode::Auto, 3), false);
        assert_eq!(s.tangents(1).unwrap().1.x, 0.0);
        // Never above the keyed peak.
        for i in 0..=200 {
            assert!(s.sample(i as f32 / 100.0).x <= 5.0 + 1e-4);
        }
    }

    #[test]
    fn auto_follows_the_neighbours_on_a_slope() {
        // Evenly rising keys: the chord slope, which the clamp leaves alone.
        let s = ChannelSpline::new(&vkeys(&[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]), &all(KeyMode::Auto, 3), false);
        assert!((s.tangents(1).unwrap().1.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn auto_never_overshoots_uneven_keys() {
        // A big rise then almost nothing: an unclamped Catmull-Rom tangent
        // carries the curve past the third key.
        let keys = vkeys(&[(0.0, 0.0), (1.0, 10.0), (2.0, 10.5), (3.0, 0.0)]);
        let s = ChannelSpline::new(&keys, &all(KeyMode::Auto, 4), false);
        for seg in 0..3 {
            let (a, b) = (keys[seg].1.x, keys[seg + 1].1.x);
            let (lo, hi) = (a.min(b) - 1e-4, a.max(b) + 1e-4);
            for i in 0..=50 {
                let v = s.sample(seg as f32 + i as f32 / 50.0).x;
                assert!(v >= lo && v <= hi, "segment {seg} left [{lo}, {hi}] at {v}");
            }
        }
    }

    #[test]
    fn constant_holds_and_linear_is_a_line() {
        let curves = vec![KeyCurve::new(KeyMode::Constant), KeyCurve::new(KeyMode::Linear), KeyCurve::LINEAR];
        let s = ChannelSpline::new(&vkeys(&[(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)]), &curves, false);
        assert_eq!(s.sample(0.99).x, 1.0);
        assert!((s.sample(1.5).x - 4.0).abs() < 1e-5);
        assert!(!s.all_linear());
    }

    #[test]
    fn free_and_broken_read_their_tangents() {
        let mut c = KeyCurve::new(KeyMode::Free);
        c.out_tan = [4.0, 0.0, 0.0, 0.0];
        c.in_tan = [99.0, 0.0, 0.0, 0.0];
        let s = ChannelSpline::new(&vkeys(&[(0.0, 0.0), (1.0, 0.0)]), &[c, KeyCurve::new(KeyMode::Flat)], false);
        assert_eq!(s.tangents(0).unwrap(), (Vec4::new(4.0, 0.0, 0.0, 0.0), Vec4::new(4.0, 0.0, 0.0, 0.0)));
        // Leaving at slope 4 rises before it comes back down.
        assert!(s.sample(0.1).x > 0.2);
        c.mode = KeyMode::Broken;
        let s = ChannelSpline::new(&vkeys(&[(0.0, 0.0), (1.0, 0.0)]), &[c, KeyCurve::LINEAR], false);
        assert_eq!(s.tangents(0).unwrap().0.x, 99.0);
    }

    #[test]
    fn a_linear_key_after_a_smooth_one_is_reached_along_the_line() {
        let curves = [KeyCurve::AUTO, KeyCurve::LINEAR];
        let s = ChannelSpline::new(&vkeys(&[(0.0, 0.0), (1.0, 2.0)]), &curves, false);
        assert_eq!(s.tangents(1).unwrap().0.x, 2.0);
    }

    #[test]
    fn smooth_rotations_stay_unit_and_hit_their_keys() {
        let a = Quat::IDENTITY;
        let b = Quat::from_rotation_y(1.2);
        let c = Quat::from_rotation_y(2.0) * Quat::from_rotation_x(0.5);
        let keys = vec![(0.0, Vec4::from(a)), (1.0, Vec4::from(b)), (2.0, Vec4::from(c))];
        let s = ChannelSpline::new(&keys, &all(KeyMode::Auto, 3), true);
        for i in 0..=40 {
            let q = s.sample(i as f32 / 20.0);
            assert!((q.length() - 1.0).abs() < 1e-4);
        }
        assert!(s.sample_quat(1.0).angle_between(b) < 1e-4);
    }

    #[test]
    fn a_rotation_key_stored_on_the_other_hemisphere_takes_the_short_way() {
        // `-b` is the same rotation as `b`. Interpolating the stored
        // components would swing through the long way round.
        let a = Quat::IDENTITY;
        let b = Quat::from_rotation_z(0.4);
        let keys = vec![(0.0, Vec4::from(a)), (1.0, -Vec4::from(b))];
        let s = ChannelSpline::new(&keys, &all(KeyMode::Auto, 2), true);
        let mid = s.sample_quat(0.5);
        assert!(mid.angle_between(Quat::from_rotation_z(0.2)) < 1e-3, "{mid:?}");
        // The legacy path agrees: slerp takes the short way too.
        let s = ChannelSpline::new(&keys, &[], true);
        assert!(s.sample_quat(0.5).angle_between(Quat::from_rotation_z(0.2)) < 1e-4);
    }

    #[test]
    fn euler_slopes_round_trip_through_quaternion_tangents() {
        let q = Vec4::from(euler_deg_to_quat(Vec3::new(20.0, 35.0, -10.0)));
        let slope = Vec3::new(30.0, -45.0, 12.0);
        let qt = quat_tangent_of_euler_slope(q, slope);
        let back = euler_slope_of_quat_tangent(q, qt);
        assert!((back - slope).length() < 0.5, "{back:?}");
    }

    #[test]
    fn wrapped_angles_stay_beside_their_neighbour() {
        assert!((wrap_deg_near(-179.0, 179.0) - 181.0).abs() < 1e-4);
        assert!((wrap_deg_near(10.0, 0.0) - 10.0).abs() < 1e-4);
        assert!((wrap_deg_near(370.0, 0.0) - 10.0).abs() < 1e-4);
    }
}

// ============================================================================
// TransformWrite (deferred transform mutations from scripts)
// ============================================================================

/// Deferred transform write — batched and applied by the scripting command processor.
#[derive(Debug)]
pub struct TransformWrite {
    pub entity: bevy::ecs::entity::Entity,
    pub new_position: Option<bevy::prelude::Vec3>,
    pub new_rotation: Option<bevy::prelude::Vec3>,
    pub translation: Option<bevy::prelude::Vec3>,
    pub rotation_delta: Option<bevy::prelude::Vec3>,
    pub new_scale: Option<bevy::prelude::Vec3>,
    pub look_at: Option<bevy::prelude::Vec3>,
}

/// Queue for batched transform writes.
#[derive(bevy::prelude::Resource, Default)]
pub struct TransformWriteQueue {
    pub writes: Vec<TransformWrite>,
}

/// Write an AnimClip to a `.anim` file (RON format).
pub fn write_anim_file(clip: &AnimClip, path: &std::path::Path) -> Result<(), String> {
    let ron_str = ron::ser::to_string_pretty(clip, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialization error: {}", e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    std::fs::write(path, ron_str).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}
