//! Controls: sliders a user makes for any rig, each one a named pose of bones
//! and blend shapes that the slider dials in.
//!
//! The Face panel's expressions ([`crate::expressions`]) were the first set of
//! these, but they are a fixed table that only knows the face rig's bones. A
//! control is the same idea as data: pose the model, save the difference from
//! its rest pose under a name, and the slider then moves those bones (and
//! blend shapes) by that much times its value. A tail curl, a hand's fist, a
//! door's swing and a smile are all one kind of thing.
//!
//! A model's controls live in a RON file beside it, `<stem>.controls`
//! ([`EXTENSION`]), so they travel with the model and are shared by every
//! scene that uses it. Every field has a serde default, so a file written by
//! an older editor still reads and a newer field never makes it fail.
//!
//! Values mix the way expressions do: rotations compose (in the order the
//! controls are given), translations and scale offsets sum, and blend shape
//! weights sum and are clamped to `0..1`. A signed control (its range goes
//! below zero) either has its own pose for the negative end or, when it has
//! none, mirrors the positive one, which is what a "look left / right" slider
//! wants.

use bevy::math::{EulerRot, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::expressions::{LidTuning, EXPRESSIONS};

/// The file extension, beside the model: `head.glb` gets `head.controls`.
pub const EXTENSION: &str = "controls";

/// Below this a bone's change from rest is noise, not part of the pose: the
/// gizmo and the slider round trips leave differences far smaller than this.
pub const CAPTURE_TOLERANCE: f32 = 1e-4;

/// Below this a blend shape weight counts as off when capturing.
pub const SHAPE_TOLERANCE: f32 = 1e-3;

/// Every control a model has, in the order its panel lists them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlSet {
    /// Bumped if the meaning of a field ever changes; files written before a
    /// field existed read it as its default instead.
    pub version: u32,
    pub controls: Vec<Control>,
}

/// One slider.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Control {
    pub name: String,
    /// The slider's range. Below zero makes it signed.
    pub min: f32,
    pub max: f32,
    /// Where the slider rests before it is touched.
    pub default: f32,
    /// What the control does at `+1`.
    pub targets: Vec<ControlTarget>,
    /// What it does at `-1`, for a signed control whose two ends are two
    /// different poses (a brow that raises one way and knits the other).
    /// Empty means the negative end is the positive pose reversed.
    pub negative: Vec<ControlTarget>,
    /// A per-model measurement to apply on top of the stored offsets.
    pub tuning: Tuning,
}

impl Default for Control {
    fn default() -> Self {
        Self { name: String::new(), min: 0.0, max: 1.0, default: 0.0, targets: Vec::new(), negative: Vec::new(), tuning: Tuning::None }
    }
}

/// Measurements a control takes from the model it is used on rather than
/// from its file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tuning {
    #[default]
    None,
    /// A blink on a face rig: each lid turns by the angle measured when the
    /// head was rigged (`face::BLINK_EXTRA`), keeping the stored angle's
    /// direction, and comes forward by the measured push
    /// (`face::BLINK_PUSH_EXTRA`) so it clears a bulging eyeball. The face
    /// preset's blinks are tuned, so a head re-rigged with better points
    /// blinks right without its controls being made again.
    Blink,
}

/// One thing a control moves, at full strength.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ControlTarget {
    /// A bone's local offset from its rest pose.
    Bone {
        /// The bone's name as the pose tools know it.
        bone: String,
        /// Degrees about the bone's own X, Y, Z (applied in that order),
        /// after its rest rotation.
        #[serde(default)]
        rotate: [f32; 3],
        /// Added to the rest translation, in the bone's parent's units.
        #[serde(default)]
        translate: [f32; 3],
        /// Added to the rest scale.
        #[serde(default)]
        scale: [f32; 3],
    },
    /// A blend shape (morph target) weight.
    BlendShape {
        /// The name of the node carrying the weights.
        mesh: String,
        /// The target's index on that mesh, used when `name` is empty or the
        /// mesh no longer has a target of that name.
        #[serde(default)]
        index: usize,
        /// The target's name, when the mesh names its targets. Preferred
        /// over `index`, which an exporter is free to reorder.
        #[serde(default)]
        name: String,
        #[serde(default)]
        weight: f32,
    },
}

impl Control {
    /// Whether the range goes below zero.
    pub fn signed(&self) -> bool {
        self.min.min(self.max) < 0.0
    }

    /// `value` held to the control's range.
    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min.min(self.max), self.max.max(self.min))
    }

    /// The pose a value uses and how strongly: the negative end's own pose
    /// at `|value|` when it has one, else the positive pose at `value`
    /// (negative values reversing it).
    fn side(&self, value: f32) -> (&[ControlTarget], f32) {
        if value < 0.0 && !self.negative.is_empty() {
            (&self.negative, -value)
        } else {
            (&self.targets, value)
        }
    }

    /// Every target on either end.
    pub fn all_targets(&self) -> impl Iterator<Item = &ControlTarget> {
        self.targets.iter().chain(self.negative.iter())
    }
}

impl ControlSet {
    pub fn find(&self, name: &str) -> Option<&Control> {
        self.controls.iter().find(|c| c.name == name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut Control> {
        self.controls.iter_mut().find(|c| c.name == name)
    }

    /// `base`, or `base 2`, `base 3`... whichever no control is called yet,
    /// so saving twice under one name never makes two controls the panel
    /// cannot tell apart.
    pub fn unique_name(&self, base: &str) -> String {
        let base = base.trim();
        let base = if base.is_empty() { "Control" } else { base };
        if self.find(base).is_none() {
            return base.to_string();
        }
        (2..).map(|n| format!("{base} {n}")).find(|n| self.find(n).is_none()).expect("an unused name")
    }

    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())
    }

    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }
}

/// One bone's combined offset from rest.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneMix {
    pub bone: String,
    /// Applied after the rest rotation.
    pub rotation: Quat,
    /// Added to the rest translation.
    pub translation: Vec3,
    /// Added to the rest scale.
    pub scale: Vec3,
}

/// One blend shape's combined weight.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeMix {
    pub mesh: String,
    pub index: usize,
    pub name: String,
    /// Summed over every control, clamped to `0..1`.
    pub weight: f32,
}

/// What a set of slider values does to the model.
///
/// Every bone and shape a given control can move is listed, even at a value
/// of zero (as an identity offset or a zero weight): a slider dragged back to
/// zero has to put what it moved back to rest, not leave it where it was.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mixed {
    pub bones: Vec<BoneMix>,
    pub shapes: Vec<ShapeMix>,
}

/// What [`Tuning`] reads from the model being posed.
pub struct Tuner<'a> {
    /// A lid bone's measured closing angle and push, by bone name.
    pub lid: &'a dyn Fn(&str) -> Option<LidTuning>,
    /// The face's scale (the distance between the eyes), which a lid's push
    /// is measured in.
    pub face_scale: f32,
}

impl Tuner<'_> {
    /// No measurements: every control does exactly what its file says.
    pub const NONE: Tuner<'static> = Tuner { lid: &|_| None, face_scale: 1.0 };
}

/// A rotation of `q`'s axis by `t` times its angle.
///
/// By axis and angle rather than by scaling Euler angles so a captured pose
/// that turns about several axes at once eases in along one arc. For a turn
/// about a single axis (every face preset rotation) the two are the same, so
/// the preset moves exactly as the fixed expressions did. Works for `t`
/// outside `0..1` too, which a range past 1 needs.
fn scale_rotation(q: Quat, t: f32) -> Quat {
    // The short way round: q and -q are the same rotation.
    let q = if q.w < 0.0 { -q } else { q };
    let (axis, angle) = q.normalize().to_axis_angle();
    if angle.abs() < 1e-7 || !axis.is_finite() {
        return Quat::IDENTITY;
    }
    Quat::from_axis_angle(axis, angle * t)
}

/// Combine slider values (by control name) into offsets. Unknown names are
/// skipped, values are held to each control's range, and the result lists
/// bones and shapes in the order they first appear.
pub fn mix(set: &ControlSet, values: &[(&str, f32)], tuner: &Tuner) -> Mixed {
    let mut out = Mixed::default();
    for (name, value) in values {
        let Some(control) = set.find(name) else { continue };
        let value = control.clamp(*value);
        // Everything this control could move, at rest, so dragging it back
        // to zero restores it (see [`Mixed`]).
        for t in control.all_targets() {
            match t {
                ControlTarget::Bone { bone, .. } => {
                    bone_entry(&mut out, bone);
                }
                ControlTarget::BlendShape { mesh, index, name, .. } => {
                    shape_entry(&mut out, mesh, *index, name);
                }
            }
        }
        let (targets, v) = control.side(value);
        if v.abs() < 1e-6 {
            continue;
        }
        let blinking = control.tuning == Tuning::Blink;
        for t in targets {
            match t {
                ControlTarget::Bone { bone, rotate, translate, scale } => {
                    let lid = if blinking { (tuner.lid)(bone) } else { None };
                    // The stored angle's sign says which way the lid goes
                    // (down for an upper lid, up for a lower one); the head
                    // says how far. Same rule as `expressions::mix_tuned`.
                    let x = match lid {
                        Some(l) => l.degrees.abs() * rotate[0].signum(),
                        None => rotate[0],
                    };
                    let full = Quat::from_euler(EulerRot::XYZ, x.to_radians(), rotate[1].to_radians(), rotate[2].to_radians());
                    let r = scale_rotation(full, v);
                    // Forward is +Z in the face rig's frame.
                    let push = Vec3::Z * lid.map_or(0.0, |l| l.push) * tuner.face_scale;
                    let entry = bone_entry(&mut out, bone);
                    entry.rotation = (entry.rotation * r).normalize();
                    entry.translation += (Vec3::from_array(*translate) + push) * v;
                    entry.scale += Vec3::from_array(*scale) * v;
                }
                ControlTarget::BlendShape { mesh, index, name, weight } => {
                    shape_entry(&mut out, mesh, *index, name).weight += weight * v;
                }
            }
        }
    }
    for s in &mut out.shapes {
        s.weight = s.weight.clamp(0.0, 1.0);
    }
    out
}

fn bone_entry<'a>(out: &'a mut Mixed, bone: &str) -> &'a mut BoneMix {
    let i = match out.bones.iter().position(|b| b.bone == bone) {
        Some(i) => i,
        None => {
            out.bones.push(BoneMix { bone: bone.to_string(), rotation: Quat::IDENTITY, translation: Vec3::ZERO, scale: Vec3::ZERO });
            out.bones.len() - 1
        }
    };
    &mut out.bones[i]
}

fn shape_entry<'a>(out: &'a mut Mixed, mesh: &str, index: usize, name: &str) -> &'a mut ShapeMix {
    let same = |s: &ShapeMix| s.mesh == mesh && if name.is_empty() || s.name.is_empty() { s.index == index } else { s.name == name };
    let i = match out.shapes.iter().position(same) {
        Some(i) => i,
        None => {
            out.shapes.push(ShapeMix { mesh: mesh.to_string(), index, name: name.to_string(), weight: 0.0 });
            out.shapes.len() - 1
        }
    };
    &mut out.shapes[i]
}

/// A bone's local transform, without depending on Bevy's transform crate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalPose {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

/// The bones of a pose that differ from rest, as targets that reproduce it
/// at full strength: the rotation after rest, the translation and the scale
/// added to it. `bones` is `(name, rest, now)`; a bone that moved less than
/// [`CAPTURE_TOLERANCE`] is left out, so a control only claims what was
/// actually posed and does not pin every other bone to rest.
pub fn capture_bones(bones: &[(String, LocalPose, LocalPose)]) -> Vec<ControlTarget> {
    let mut out = Vec::new();
    for (name, rest, now) in bones {
        let mut q = (rest.rotation.inverse() * now.rotation).normalize();
        if q.w < 0.0 {
            q = -q;
        }
        let t = now.translation - rest.translation;
        let s = now.scale - rest.scale;
        let turned = q.angle_between(Quat::IDENTITY) > CAPTURE_TOLERANCE;
        let moved = t.abs().max_element() > CAPTURE_TOLERANCE * rest.translation.length().max(1.0);
        let scaled = s.abs().max_element() > CAPTURE_TOLERANCE;
        if !(turned || moved || scaled) {
            continue;
        }
        let (x, y, z) = q.to_euler(EulerRot::XYZ);
        let clean = |v: Vec3| v.to_array().map(|c| if c.abs() < 1e-7 { 0.0 } else { c });
        out.push(ControlTarget::Bone {
            bone: name.clone(),
            rotate: if turned { [x.to_degrees(), y.to_degrees(), z.to_degrees()] } else { [0.0; 3] },
            translate: if moved { clean(t) } else { [0.0; 3] },
            scale: if scaled { clean(s) } else { [0.0; 3] },
        });
    }
    out
}

/// The blend shapes that are on, as targets at their current weights.
/// `shapes` is `(mesh, index, name, weight)`.
pub fn capture_shapes(shapes: &[(String, usize, String, f32)]) -> Vec<ControlTarget> {
    shapes
        .iter()
        .filter(|(.., w)| w.abs() > SHAPE_TOLERANCE)
        .map(|(mesh, index, name, weight)| ControlTarget::BlendShape { mesh: mesh.clone(), index: *index, name: name.clone(), weight: *weight })
        .collect()
}

/// The face rig's expressions as editable controls, the starting point for a
/// face: the same sliders the Face panel always had, now renamable and
/// re-capturable like any other.
///
/// The expressions' translations are in face-scale units (the distance
/// between the eyes) so one table fits any head; a control's are in model
/// units, so they are scaled by this head's `face_scale` as they are written.
/// The blinks keep their tuning, so the lid angles still come from the head.
pub fn face_preset(face_scale: f32) -> ControlSet {
    let controls = EXPRESSIONS
        .iter()
        .map(|e| Control {
            name: e.label.to_string(),
            min: if e.signed { -1.0 } else { 0.0 },
            max: 1.0,
            default: 0.0,
            targets: e
                .offsets
                .iter()
                .map(|o| ControlTarget::Bone {
                    bone: o.bone.to_string(),
                    rotate: o.rotate,
                    translate: (Vec3::from_array(o.translate) * face_scale).to_array(),
                    scale: [0.0; 3],
                })
                .collect(),
            negative: Vec::new(),
            tuning: if matches!(e.id, "blink_l" | "blink_r") { Tuning::Blink } else { Tuning::None },
        })
        .collect();
    ControlSet { version: 1, controls }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions;
    use crate::face::names;

    fn bone(set: &Mixed, name: &str) -> BoneMix {
        set.bones.iter().find(|b| b.bone == name).cloned().unwrap_or_else(|| panic!("{name} not in the mix"))
    }

    fn ctl(name: &str, targets: Vec<ControlTarget>) -> Control {
        Control { name: name.into(), targets, ..Default::default() }
    }

    fn turn(b: &str, x: f32, y: f32, z: f32) -> ControlTarget {
        ControlTarget::Bone { bone: b.into(), rotate: [x, y, z], translate: [0.0; 3], scale: [0.0; 3] }
    }

    fn shape(mesh: &str, index: usize, weight: f32) -> ControlTarget {
        ControlTarget::BlendShape { mesh: mesh.into(), index, name: String::new(), weight }
    }

    /// The preset moves the face exactly as the fixed expressions did, tuned
    /// blinks included, at any mix of values.
    #[test]
    fn the_face_preset_matches_the_expressions() {
        let scale = 0.065;
        let preset = face_preset(scale);
        let tuned = |b: &str| match b {
            names::LID_UPPER_L => Some(LidTuning { degrees: 48.0, push: 0.08 }),
            names::LID_LOWER_L => Some(LidTuning { degrees: 10.0, push: 0.0 }),
            _ => None,
        };
        let tuner = Tuner { lid: &tuned, face_scale: scale };
        let ids = [("smile", 0.7), ("blink_l", 0.6), ("look_up", -0.4), ("pucker", 0.3), ("jaw_open", 1.0)];
        let by_label: Vec<(&str, f32)> = ids.iter().map(|(id, v)| (expressions::find(id).expect("an expression").label, *v)).collect();
        let want = expressions::mix_tuned(&ids, scale, &tuned);
        let got = mix(&preset, &by_label, &tuner);
        for (b, q, t) in want {
            let m = bone(&got, b);
            assert!(m.rotation.abs_diff_eq(q, 1e-5) || m.rotation.abs_diff_eq(-q, 1e-5), "{b}: {:?} vs {q:?}", m.rotation);
            assert!(m.translation.abs_diff_eq(t, 1e-6), "{b}: {:?} vs {t:?}", m.translation);
        }
    }

    #[test]
    fn controls_add_up() {
        let set = ControlSet {
            version: 1,
            controls: vec![
                ctl("curl", vec![turn("tail_1", 30.0, 0.0, 0.0), shape("body", 0, 0.6)]),
                ctl("more", vec![turn("tail_1", 10.0, 0.0, 0.0), shape("body", 0, 0.7)]),
            ],
        };
        let m = mix(&set, &[("curl", 1.0), ("more", 1.0)], &Tuner::NONE);
        let angle = bone(&m, "tail_1").rotation.to_euler(EulerRot::XYZ).0.to_degrees();
        assert!((angle - 40.0).abs() < 1e-3, "{angle}");
        // Weights sum and are held to 0..1.
        assert_eq!(m.shapes.len(), 1);
        assert!((m.shapes[0].weight - 1.0).abs() < 1e-6);
        let half = mix(&set, &[("curl", 0.5)], &Tuner::NONE);
        assert!((half.shapes[0].weight - 0.3).abs() < 1e-6);
    }

    /// A control at zero still lists what it moves, at rest, so dragging the
    /// slider back to zero puts the bones back.
    #[test]
    fn a_control_at_zero_restores_what_it_moves() {
        let set = ControlSet { version: 1, controls: vec![ctl("fist", vec![turn("finger", 80.0, 0.0, 0.0), shape("hand", 2, 1.0)])] };
        let m = mix(&set, &[("fist", 0.0)], &Tuner::NONE);
        assert_eq!(bone(&m, "finger").rotation, Quat::IDENTITY);
        assert_eq!(m.shapes[0].weight, 0.0);
        // A name the set does not have moves nothing.
        assert_eq!(mix(&set, &[("wave", 1.0)], &Tuner::NONE), Mixed::default());
    }

    #[test]
    fn a_signed_control_mirrors_or_uses_its_own_negative_pose() {
        let mut look = ctl("look", vec![turn("eye", 0.0, 20.0, 0.0)]);
        look.min = -1.0;
        assert!(look.signed());
        let set = ControlSet { version: 1, controls: vec![look.clone()] };
        let m = mix(&set, &[("look", -0.5)], &Tuner::NONE);
        let y = bone(&m, "eye").rotation.to_euler(EulerRot::XYZ).1.to_degrees();
        assert!((y + 10.0).abs() < 1e-3, "{y}");

        look.negative = vec![turn("brow", 15.0, 0.0, 0.0)];
        let set = ControlSet { version: 1, controls: vec![look] };
        let m = mix(&set, &[("look", -1.0)], &Tuner::NONE);
        // The positive pose's bone is listed (at rest) and the negative one
        // moves.
        assert_eq!(bone(&m, "eye").rotation, Quat::IDENTITY);
        let x = bone(&m, "brow").rotation.to_euler(EulerRot::XYZ).0.to_degrees();
        assert!((x - 15.0).abs() < 1e-3);
        // Unsigned controls are held to their range.
        let up = ControlSet { version: 1, controls: vec![ctl("up", vec![turn("b", 10.0, 0.0, 0.0)])] };
        assert_eq!(bone(&mix(&up, &[("up", -1.0)], &Tuner::NONE), "b").rotation, Quat::IDENTITY);
    }

    /// Capturing a pose then playing the control at 1 gives the pose back,
    /// and only the bones that moved are captured.
    #[test]
    fn a_captured_pose_plays_back() {
        let rest = LocalPose { translation: Vec3::new(0.0, 1.0, 0.0), rotation: Quat::from_rotation_y(0.3), scale: Vec3::ONE };
        let now = LocalPose {
            translation: Vec3::new(0.1, 1.0, 0.05),
            rotation: Quat::from_rotation_y(0.3) * Quat::from_euler(EulerRot::XYZ, 0.4, -0.2, 0.1),
            scale: Vec3::new(1.2, 1.0, 1.0),
        };
        let still = LocalPose { translation: Vec3::X, rotation: Quat::IDENTITY, scale: Vec3::ONE };
        let nudged = LocalPose { translation: Vec3::X + Vec3::splat(1e-6), ..still };
        let targets = capture_bones(&[("arm".into(), rest, now), ("leg".into(), still, nudged)]);
        assert_eq!(targets.len(), 1, "{targets:?}");
        let set = ControlSet { version: 1, controls: vec![ctl("pose", targets)] };
        let m = bone(&mix(&set, &[("pose", 1.0)], &Tuner::NONE), "arm");
        let rotation = (rest.rotation * m.rotation).normalize();
        assert!(rotation.abs_diff_eq(now.rotation, 1e-5) || rotation.abs_diff_eq(-now.rotation, 1e-5));
        assert!((rest.translation + m.translation).abs_diff_eq(now.translation, 1e-5));
        assert!((rest.scale + m.scale).abs_diff_eq(now.scale, 1e-5));
        // Half way turns half the angle about the same axis.
        let half = bone(&mix(&set, &[("pose", 0.5)], &Tuner::NONE), "arm");
        let full_angle = (rest.rotation.inverse() * now.rotation).angle_between(Quat::IDENTITY);
        assert!((half.rotation.angle_between(Quat::IDENTITY) - full_angle * 0.5).abs() < 1e-4);
    }

    #[test]
    fn only_shapes_that_are_on_are_captured() {
        let got = capture_shapes(&[("face".into(), 0, "smile".into(), 0.8), ("face".into(), 1, "frown".into(), 0.0)]);
        assert_eq!(got, vec![ControlTarget::BlendShape { mesh: "face".into(), index: 0, name: "smile".into(), weight: 0.8 }]);
    }

    /// A file keeps everything through a save, and a minimal or older one
    /// still reads, missing fields taking their defaults.
    #[test]
    fn files_round_trip_and_evolve() {
        let mut set = face_preset(0.06);
        set.controls[0].negative = vec![shape("face", 3, 0.5)];
        let text = set.to_ron().expect("serialises");
        assert_eq!(ControlSet::from_ron(&text).expect("reads back"), set);

        let old = r#"(controls: [(name: "Curl", targets: [Bone(bone: "tail_1", rotate: (30.0, 0.0, 0.0))])])"#;
        let read = ControlSet::from_ron(old).expect("a minimal file reads");
        let c = &read.controls[0];
        assert_eq!((c.min, c.max, c.default, c.tuning), (0.0, 1.0, 0.0, Tuning::None));
        assert!(matches!(&c.targets[0], ControlTarget::Bone { translate, .. } if *translate == [0.0; 3]));
    }

    /// The example in `docs/.../editor/rigging.md#the-file` reads as written.
    #[test]
    fn the_documented_example_reads() {
        let text = r#"(
    version: 1,
    controls: [
        (
            name: "Tail Curl",
            min: 0.0,
            max: 1.0,
            targets: [
                Bone(bone: "tail_1", rotate: (35.0, 0.0, 0.0)),
                Bone(bone: "tail_2", rotate: (40.0, 0.0, 0.0)),
                BlendShape(mesh: "body", name: "tailFluff", weight: 0.6),
            ],
        ),
    ],
)"#;
        let set = ControlSet::from_ron(text).expect("the documented file reads");
        assert_eq!(set.controls[0].targets.len(), 3);
        let m = mix(&set, &[("Tail Curl", 0.5)], &Tuner::NONE);
        assert!((m.shapes[0].weight - 0.3).abs() < 1e-6);
    }

    #[test]
    fn names_stay_unique() {
        let set = ControlSet { version: 1, controls: vec![ctl("Smile", vec![]), ctl("Smile 2", vec![])] };
        assert_eq!(set.unique_name("Smile"), "Smile 3");
        assert_eq!(set.unique_name("  "), "Control");
        assert_eq!(set.unique_name("Frown"), "Frown");
    }
}
