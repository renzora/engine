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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
}

/// A named event marker on an animation clip's timeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimMarker {
    pub time: f32,
    pub name: String,
}

/// Animation curves for a single bone/target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoneTrack {
    pub bone_name: String,
    pub translations: Vec<(f32, [f32; 3])>,
    pub rotations: Vec<(f32, [f32; 4])>,
    pub scales: Vec<(f32, [f32; 3])>,
}

// ----------------------------------------------------------------------------
// Property animation (keyframes bound to component fields)
// ----------------------------------------------------------------------------

/// Editor/runtime cache of the **Euler angles** (degrees, XYZ order) last dialed
/// into a rotation — by the inspector or a rotation animation. **Keyed per
/// component** so several rotation fields on one entity (e.g. `Transform` *and*
/// `EnvironmentMapLight`) each keep their own slot instead of fighting over one.
///
/// A quaternion stores only an orientation, so converting it back to Euler angles
/// is lossy: the middle axis wraps at ±90° and full turns (360°, 720°) collapse
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

/// Quaternion from Euler **degrees** (XYZ order).
pub fn euler_deg_to_quat(deg: Vec3) -> Quat {
    Quat::from_euler(
        EulerRot::XYZ,
        deg.x.to_radians(),
        deg.y.to_radians(),
        deg.z.to_radians(),
    )
}

/// The Euler degrees (XYZ) to display/record for `rotation` under `key`,
/// preferring the cache when it still matches, else deriving from the quaternion
/// (which wraps the middle axis to ±90° — the unavoidable lossy fallback).
pub fn rotation_euler_deg(rotation: Quat, cache: Option<&EditorEulerCache>, key: &str) -> Vec3 {
    if let Some(deg) = cache.and_then(|c| c.degrees_for(key, rotation)) {
        return deg;
    }
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
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
        PropertyTrack { target: String::new(), component: "x".into(), field: "y".into(), keys }
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

// ============================================================================
// TransformWrite (deferred transform mutations from scripts/blueprints)
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
