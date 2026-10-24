//! Retargeting and derived clips.
//!
//! A `.anim` binds to bones by name (see [`crate::bone_target`]), which is
//! enough when the clip was authored on the skeleton it plays on. It is not
//! enough for a clip from another rig: a rotation key is an orientation
//! *relative to the rest pose of the bone it was keyed on*, so the same key on
//! a bone that rests differently (another bone-axis convention, an A-pose
//! against a T-pose) twists the limb, and a hips translation keyed for a
//! 1.8 m character drags a 1.2 m one across the floor.
//!
//! # Where the source rest pose comes from
//!
//! A [`SkeletonRestPose`] in a `.skel` file, named by the clip slot
//! ([`crate::AnimClipSlot::source_skeleton`]). It is a sidecar rather than a
//! field of the `.anim` because one skeleton serves every clip exported from
//! it, and because a `.anim` is rewritten by the timeline on every edit, which
//! would drop anything the editor does not know it holds. A slot without one
//! plays exactly as before: name binding, no compensation.
//!
//! The target rest pose is the skeleton itself, captured once when the
//! animator first initialises (a model spawns in its rest pose), and kept
//! across graph rebuilds so a rebuild mid-animation does not capture a pose.
//!
//! # The math
//!
//! Each bone's rotation is carried across as a change from rest, measured in
//! the space of the hips' parent on both sides, so armature nodes above the
//! hips (an FBX unit scale, a Z-up correction) do not have to agree. Working
//! that through for a bone with source rest `S`, target rest `T`, and parent
//! rest orientations `Gs` / `Gt` gives a key `q` the target rotation
//!
//! ```text
//! q' = (Gt⁻¹ Gs) q (S⁻¹ Gs⁻¹ Gt T)
//! ```
//!
//! where both brackets are constant per bone, so a clip retargets key by key
//! without sampling its parents. With identical skeletons both are identity.
//! Only the root bone keeps its translation keys, moved by the same rotation
//! and scaled by the ratio of leg lengths; every other bone keeps the target's
//! own rest translation, which is what keeps the target's proportions.
//!
//! # Difference clips
//!
//! [`make_additive`] turns a clip into its change from its own first frame,
//! for additive layers and additive blend tree overlays. Bevy adds a clip as
//! `delta * base` for rotations and `base + delta` for translation and scale,
//! so a raw clip added on top would double every bone's rest transform.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::prelude::*;

use renzora::{AnimClip, BoneTrack, KeyCurve, RestBone, SkeletonRestPose};

use crate::loader::AnimLoadError;

// ============================================================================
// Source skeleton asset
// ============================================================================

/// A loaded `.skel` file.
#[derive(Asset, TypePath, Debug, Clone, Default)]
pub struct SourceSkeleton {
    pub skeleton: Arc<RestSkeleton>,
}

#[derive(Default, TypePath)]
pub struct SkeletonLoader;

impl AssetLoader for SkeletonLoader {
    type Asset = SourceSkeleton;
    type Settings = ();
    type Error = AnimLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let pose: SkeletonRestPose = ron::de::from_bytes(&bytes)?;
        Ok(SourceSkeleton { skeleton: Arc::new(RestSkeleton::from_pose(&pose)) })
    }

    fn extensions(&self) -> &[&str] {
        &["skel"]
    }
}

// ============================================================================
// Rest skeleton
// ============================================================================

/// One bone's rest transform and parent.
#[derive(Clone, Debug)]
pub struct RestEntry {
    /// The bone's name as first seen (the importer's spelling).
    pub name: String,
    /// The parent's key (sanitized name), `None` for a root.
    pub parent: Option<String>,
    pub local: Transform,
}

/// A skeleton's rest pose, keyed by sanitized bone name so either spelling of
/// a name finds the bone.
#[derive(Clone, Debug, Default)]
pub struct RestSkeleton {
    bones: HashMap<String, RestEntry>,
    /// Keys in insertion order, so lookups that pick "the first" are stable.
    order: Vec<String>,
}

/// How deep a parent chain may go before it is taken to be a cycle.
const MAX_DEPTH: usize = 512;

fn key(name: &str) -> String {
    renzora::sanitize_id(name)
}

impl RestSkeleton {
    pub fn from_pose(pose: &SkeletonRestPose) -> Self {
        let mut s = RestSkeleton::default();
        for bone in &pose.bones {
            s.insert(
                &bone.name,
                bone.parent.as_deref(),
                Transform {
                    translation: Vec3::from_array(bone.translation),
                    rotation: Quat::from_array(bone.rotation).normalize(),
                    scale: Vec3::from_array(bone.scale),
                },
            );
        }
        s
    }

    /// The pose as it is written to a `.skel` file.
    pub fn to_pose(&self) -> SkeletonRestPose {
        SkeletonRestPose {
            bones: self
                .order
                .iter()
                .filter_map(|k| self.bones.get(k))
                .map(|e| RestBone {
                    name: e.name.clone(),
                    parent: e.parent.as_ref().and_then(|p| self.bones.get(p)).map(|p| p.name.clone()),
                    translation: e.local.translation.to_array(),
                    rotation: e.local.rotation.to_array(),
                    scale: e.local.scale.to_array(),
                })
                .collect(),
            ..Default::default()
        }
    }

    /// Add a bone. A bone already present keeps its first rest transform.
    pub fn insert(&mut self, name: &str, parent: Option<&str>, local: Transform) {
        let k = key(name);
        if self.bones.contains_key(&k) {
            return;
        }
        self.order.push(k.clone());
        self.bones.insert(k, RestEntry { name: name.to_string(), parent: parent.map(key), local });
    }

    pub fn get(&self, name: &str) -> Option<&RestEntry> {
        self.bones.get(&key(name))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.bones.contains_key(&key(name))
    }

    pub fn len(&self) -> usize {
        self.bones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// Every bone's key (sanitized name), in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.order.iter().map(String::as_str)
    }

    /// The bone's name followed by its ancestors', nearest first: the path a
    /// layer mask is matched along.
    pub fn path(&self, name: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut at = Some(key(name));
        while let Some(k) = at {
            let Some(e) = self.bones.get(&k) else { break };
            out.push(e.name.as_str());
            if out.len() > MAX_DEPTH {
                break;
            }
            at = e.parent.clone();
        }
        out
    }

    /// The root-motion / retarget root: `configured` if the skeleton has it,
    /// else the first bone named like hips or a pelvis.
    pub fn root_bone(&self, configured: Option<&str>) -> Option<String> {
        pick_root_bone(self.order.iter().filter_map(|k| self.bones.get(k)).map(|e| e.name.as_str()), configured)
    }

    /// Rest orientation of `name` in the space of `frame` (a key), which is
    /// the product of local rotations from just below `frame` down to the
    /// bone. A bone not under `frame` gets its orientation from its topmost
    /// ancestor.
    pub fn rotation_in(&self, name: &str, frame: Option<&str>) -> Quat {
        let mut q = Quat::IDENTITY;
        let mut at = Some(key(name));
        let mut depth = 0;
        while let Some(k) = at {
            if frame == Some(k.as_str()) {
                break;
            }
            let Some(e) = self.bones.get(&k) else { break };
            q = e.local.rotation * q;
            depth += 1;
            if depth > MAX_DEPTH {
                break;
            }
            at = e.parent.clone();
        }
        q
    }

    /// The parent's rest orientation in `frame`; identity for a root or a
    /// bone directly under `frame`.
    fn parent_rotation_in(&self, name: &str, frame: Option<&str>) -> Quat {
        match self.get(name).and_then(|e| e.parent.clone()) {
            Some(parent) => self.rotation_in(&parent, frame),
            None => Quat::IDENTITY,
        }
    }

    fn children(&self, k: &str) -> Vec<&str> {
        self.order
            .iter()
            .filter(|c| self.bones.get(*c).and_then(|e| e.parent.as_deref()) == Some(k))
            .map(|c| c.as_str())
            .collect()
    }

    /// Length of a leg, from the upper leg down to the foot, in the units of
    /// the root bone's parent space (where its translation keys live).
    ///
    /// Found by name (`upleg` or `thigh` under the root, then down the chain
    /// to a `foot` or `ankle`), since nothing else tells a leg from an arm.
    pub fn leg_length(&self, root: &str) -> Option<f32> {
        let root_key = key(root);
        let root_entry = self.bones.get(&root_key)?;
        // Positions in the root's parent space.
        let mut frames: HashMap<String, GlobalTransform> = HashMap::new();
        frames.insert(root_key.clone(), GlobalTransform::from(root_entry.local));
        let mut stack = vec![root_key.clone()];
        let mut upleg: Option<String> = None;
        while let Some(k) = stack.pop() {
            let parent = frames[&k];
            for child in self.children(&k) {
                let local = self.bones[child].local;
                frames.insert(child.to_string(), parent * GlobalTransform::from(local));
                if upleg.is_none() && (child.contains("upleg") || child.contains("thigh")) {
                    upleg = Some(child.to_string());
                }
                stack.push(child.to_string());
            }
            if frames.len() > MAX_DEPTH * 4 {
                break;
            }
        }
        let mut at = upleg?;
        let mut length = 0.0;
        for _ in 0..8 {
            let kids = self.children(&at);
            let next = kids
                .iter()
                .find(|c| ["leg", "calf", "shin", "knee", "foot", "ankle"].iter().any(|n| c.contains(n)))
                .or(kids.first())
                .map(|c| c.to_string());
            let Some(next) = next else { break };
            length += frames[&at].translation().distance(frames[&next].translation());
            if next.contains("foot") || next.contains("ankle") {
                return (length > f32::EPSILON).then_some(length);
            }
            at = next;
        }
        (length > f32::EPSILON).then_some(length)
    }
}

/// Pick the root-motion / retarget root bone among `names`: the configured
/// one if present, else the first whose name ends in `hips` or `pelvis`.
pub fn pick_root_bone<'a>(names: impl Iterator<Item = &'a str>, configured: Option<&str>) -> Option<String> {
    let names: Vec<&str> = names.collect();
    if let Some(wanted) = configured.filter(|c| !c.trim().is_empty()) {
        let wanted = key(wanted);
        return names.iter().find(|n| key(n) == wanted).map(|n| n.to_string());
    }
    names
        .iter()
        .find(|n| {
            let k = key(n);
            k.ends_with("hips") || k.ends_with("pelvis")
        })
        .map(|n| n.to_string())
}

/// Capture the rest skeleton under `root` (excluding `root` itself) through
/// accessors, so a system's queries and a `&World` both can.
pub fn capture_skeleton(
    root: Entity,
    children_of: impl Fn(Entity) -> Vec<Entity>,
    bone_of: impl Fn(Entity) -> Option<(String, Transform)>,
) -> RestSkeleton {
    let mut skeleton = RestSkeleton::default();
    let mut stack: Vec<(Entity, Option<String>)> = children_of(root).into_iter().map(|c| (c, None)).collect();
    let mut visited = 0usize;
    while let Some((entity, parent)) = stack.pop() {
        visited += 1;
        if visited > 65_536 {
            break;
        }
        let name = match bone_of(entity) {
            Some((name, local)) => {
                skeleton.insert(&name, parent.as_deref(), local);
                Some(name)
            }
            None => parent,
        };
        for child in children_of(entity) {
            stack.push((child, name.clone()));
        }
    }
    skeleton
}

/// The rest pose of the skeleton under `root`, ready for
/// [`renzora::write_skeleton_file`]. Call it while the model is unposed (just
/// spawned), since it records the bones as they are.
pub fn capture_rest_pose(world: &World, root: Entity) -> SkeletonRestPose {
    capture_skeleton(
        root,
        |e| world.get::<Children>(e).map(|c| c.iter().collect()).unwrap_or_default(),
        |e| {
            let name = world.get::<Name>(e)?;
            let bind = world.get::<renzora::BindName>(e);
            let t = world.get::<Transform>(e)?;
            Some((renzora::BindName::of(name, bind).to_string(), *t))
        },
    )
    .to_pose()
}

// ============================================================================
// Key helpers
// ============================================================================

/// Quaternion product on raw components, for tangents, which are not unit
/// quaternions and must not be normalised.
fn qmul(a: Vec4, b: Vec4) -> Vec4 {
    Vec4::new(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    )
}

fn qv(q: Quat) -> Vec4 {
    Vec4::from_array(q.to_array())
}

/// Apply a linear map to a channel's hand-set tangents, which change with the
/// keys they belong to (a tangent is a slope of the value).
fn map_tangents(curves: &mut [KeyCurve], f: impl Fn(Vec4) -> Vec4) {
    for c in curves {
        c.in_tan = f(Vec4::from_array(c.in_tan)).to_array();
        c.out_tan = f(Vec4::from_array(c.out_tan)).to_array();
    }
}

fn vec3_tangent(f: impl Fn(Vec3) -> Vec3) -> impl Fn(Vec4) -> Vec4 {
    move |t| f(t.truncate()).extend(t.w)
}

// ============================================================================
// Retargeting
// ============================================================================

/// Below this a rest length or scale is treated as zero rather than divided by.
const EPS: f32 = 1e-6;

/// The ratio a root translation is scaled by: leg length when both skeletons
/// name their legs, else the rest height of the root bone, which is the leg
/// length for the usual layout of hips over a root on the floor.
pub fn proportion_ratio(src: &RestSkeleton, src_root: &str, dst: &RestSkeleton, dst_root: &str) -> f32 {
    if let (Some(s), Some(d)) = (src.leg_length(src_root), dst.leg_length(dst_root)) {
        return d / s;
    }
    let s = src.get(src_root).map(|e| e.local.translation.length()).unwrap_or(0.0);
    let d = dst.get(dst_root).map(|e| e.local.translation.length()).unwrap_or(0.0);
    if s > EPS && d > EPS { d / s } else { 1.0 }
}

/// Look `name` up in a source-to-target bone name map, matching either
/// spelling of the source name.
pub fn map_bone_name(map: &HashMap<String, String>, name: &str) -> String {
    let k = key(name);
    map.iter().find(|(from, _)| key(from) == k).map(|(_, to)| to.clone()).unwrap_or_else(|| name.to_string())
}

/// Carry a clip from the skeleton it was authored on (`src`) onto `dst`.
///
/// With no source skeleton only the bone map applies. A bone either skeleton
/// lacks keeps its keys unchanged, so a clip on a partly matching rig degrades
/// to the old name binding bone by bone instead of failing as a whole.
pub fn retarget_clip(
    clip: &AnimClip,
    src: Option<&RestSkeleton>,
    dst: &RestSkeleton,
    map: &HashMap<String, String>,
    root_hint: Option<&str>,
) -> AnimClip {
    let src_root = src.and_then(|s| s.root_bone(root_hint));
    let dst_root = src_root.as_deref().map(|r| map_bone_name(map, r));
    let frame_src = src_root.as_deref().and_then(|r| src?.get(r)?.parent.clone());
    let frame_dst = dst_root.as_deref().and_then(|r| dst.get(r)?.parent.clone());
    let ratio = match (src, &src_root, &dst_root) {
        (Some(s), Some(sr), Some(dr)) if dst.contains(dr) => proportion_ratio(s, sr, dst, dr),
        _ => 1.0,
    };

    let mut tracks = Vec::with_capacity(clip.tracks.len());
    for track in &clip.tracks {
        let mut out = track.clone();
        out.bone_name = map_bone_name(map, &track.bone_name);
        let (Some(src), Some(s), Some(t)) = (src, src.and_then(|s| s.get(&track.bone_name)), dst.get(&out.bone_name)) else {
            tracks.push(out);
            continue;
        };
        let is_root = src_root.as_deref().is_some_and(|r| key(r) == key(&track.bone_name));
        let gs = src.parent_rotation_in(&track.bone_name, frame_src.as_deref());
        let gt = dst.parent_rotation_in(&out.bone_name, frame_dst.as_deref());
        let pre = gt.inverse() * gs;
        let post = s.local.rotation.inverse() * gs.inverse() * gt * t.local.rotation;
        retarget_track(&mut out, BoneRetarget { pre, post, is_root, ratio, src_rest: s.local, dst_rest: t.local });
        tracks.push(out);
    }
    // A blend shape weight means the same on any rig, so only the mesh
    // node's name goes through the map.
    let mut morph_tracks = clip.morph_tracks.clone();
    for m in &mut morph_tracks {
        m.target = map_bone_name(map, &m.target);
    }
    AnimClip { tracks, morph_tracks, ..clip.clone() }
}

/// The per-bone constants of a retarget.
#[derive(Clone, Copy, Debug)]
pub struct BoneRetarget {
    pub pre: Quat,
    pub post: Quat,
    pub is_root: bool,
    pub ratio: f32,
    pub src_rest: Transform,
    pub dst_rest: Transform,
}

/// Rewrite one bone's keys (and hand-set tangents) for the target.
pub fn retarget_track(track: &mut BoneTrack, r: BoneRetarget) {
    let (pre, post) = (r.pre, r.post);
    for (_, q) in &mut track.rotations {
        *q = (pre * Quat::from_array(*q) * post).normalize().to_array();
    }
    map_tangents(&mut track.curves.rotations, |tan| qmul(qmul(qv(pre), tan), qv(post)));

    if r.is_root {
        let (rest_dst, rest_src, ratio) = (r.dst_rest.translation, r.src_rest.translation, r.ratio);
        for (_, v) in &mut track.translations {
            *v = (rest_dst + pre * ((Vec3::from_array(*v) - rest_src) * ratio)).to_array();
        }
        map_tangents(&mut track.curves.translations, vec3_tangent(|t| pre * (t * ratio)));
    } else {
        // The target's own bone lengths are what keep its proportions.
        track.translations.clear();
        track.curves.translations.clear();
    }

    let factor = Vec3::from_array(
        [0, 1, 2].map(|i| if r.src_rest.scale[i].abs() > EPS { r.dst_rest.scale[i] / r.src_rest.scale[i] } else { 1.0 }),
    );
    for (_, v) in &mut track.scales {
        *v = (Vec3::from_array(*v) * factor).to_array();
    }
    map_tangents(&mut track.curves.scales, vec3_tangent(|t| t * factor));
}

// ============================================================================
// Difference clips
// ============================================================================

/// Below this a delta is no motion at all, and the channel is dropped.
const DELTA_EPS: f32 = 1e-5;

/// The clip as a change from its own first frame, for additive playback.
///
/// A channel whose delta is zero throughout is dropped rather than kept as a
/// zero curve: added to a bone the base does not animate, a zero translation
/// or scale delta would set that bone's translation or scale to zero.
pub fn make_additive(clip: &AnimClip) -> AnimClip {
    let mut tracks = Vec::new();
    for track in &clip.tracks {
        let mut out = track.clone();

        if let Some(&(_, first)) = track.rotations.first() {
            let inv = Quat::from_array(first).normalize().inverse();
            for (_, q) in &mut out.rotations {
                *q = (Quat::from_array(*q) * inv).normalize().to_array();
            }
            map_tangents(&mut out.curves.rotations, |tan| qmul(tan, qv(inv)));
            let still = out.rotations.iter().all(|(_, q)| Quat::from_array(*q).dot(Quat::IDENTITY).abs() > 1.0 - DELTA_EPS);
            if still {
                out.rotations.clear();
                out.curves.rotations.clear();
            }
        }

        for (keys, curves) in [
            (&mut out.translations, &mut out.curves.translations),
            (&mut out.scales, &mut out.curves.scales),
        ] {
            if let Some(&(_, first)) = keys.first() {
                let first = Vec3::from_array(first);
                for (_, v) in keys.iter_mut() {
                    *v = (Vec3::from_array(*v) - first).to_array();
                }
                if keys.iter().all(|(_, v)| Vec3::from_array(*v).length() < DELTA_EPS) {
                    keys.clear();
                    curves.clear();
                }
            }
        }

        if !(out.rotations.is_empty() && out.translations.is_empty() && out.scales.is_empty()) {
            tracks.push(out);
        }
    }
    // Bevy adds weights as `base + delta`, like a translation.
    let morph_tracks = clip
        .morph_tracks
        .iter()
        .filter_map(|m| {
            let first = m.keys.first()?.1;
            let mut out = m.clone();
            for (_, w) in &mut out.keys {
                *w -= first;
            }
            out.keys.iter().any(|(_, w)| w.abs() >= DELTA_EPS).then_some(out)
        })
        .collect();
    AnimClip { tracks, property_tracks: Vec::new(), markers: Vec::new(), morph_tracks, ..clip.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bone(name: &str, parent: Option<&str>, t: Vec3, r: Quat) -> RestBone {
        RestBone {
            name: name.into(),
            parent: parent.map(|p| p.into()),
            translation: t.to_array(),
            rotation: r.to_array(),
            scale: [1.0; 3],
        }
    }

    fn track(name: &str, translations: Vec<(f32, Vec3)>, rotations: Vec<(f32, Quat)>) -> BoneTrack {
        BoneTrack {
            bone_name: name.into(),
            translations: translations.into_iter().map(|(t, v)| (t, v.to_array())).collect(),
            rotations: rotations.into_iter().map(|(t, q)| (t, q.to_array())).collect(),
            scales: Vec::new(),
            muted: false,
            curves: Default::default(),
        }
    }

    fn clip(tracks: Vec<BoneTrack>) -> AnimClip {
        AnimClip { name: "c".into(), duration: 1.0, tracks, ..Default::default() }
    }

    /// A blend shape's difference clip is its change from the first key, and
    /// a shape that never moves is dropped (added, it would do nothing).
    #[test]
    fn a_difference_clip_carries_blend_shapes_as_deltas() {
        let mut c = clip(vec![]);
        let mut moving = renzora::MorphTrack::new("Face", 0, "");
        moving.keys = vec![(0.0, 0.2), (1.0, 0.7)];
        let mut still = renzora::MorphTrack::new("Face", 1, "");
        still.keys = vec![(0.0, 0.4), (1.0, 0.4)];
        c.morph_tracks = vec![moving, still];
        let out = make_additive(&c);
        assert_eq!(out.morph_tracks.len(), 1);
        assert_eq!(out.morph_tracks[0].keys[0].1, 0.0);
        assert!((out.morph_tracks[0].keys[1].1 - 0.5).abs() < 1e-6);
        // Retargeting keeps the weights and maps only the node's name.
        let map = HashMap::from([("Face".to_string(), "Head".to_string())]);
        let moved = retarget_clip(&c, None, &RestSkeleton::default(), &map, None);
        assert_eq!(moved.morph_tracks[0].target, "Head");
        assert_eq!(moved.morph_tracks[0].keys, c.morph_tracks[0].keys);
    }

    fn rig(arm_rest: Quat, hips_height: f32, leg: f32) -> RestSkeleton {
        RestSkeleton::from_pose(&SkeletonRestPose {
            bones: vec![
                bone("Armature", None, Vec3::ZERO, Quat::IDENTITY),
                bone("Hips", Some("Armature"), Vec3::Y * hips_height, Quat::IDENTITY),
                bone("UpperArm", Some("Hips"), Vec3::X, arm_rest),
                bone("Forearm", Some("UpperArm"), Vec3::X, Quat::IDENTITY),
                bone("LeftUpLeg", Some("Hips"), Vec3::X * 0.1, Quat::IDENTITY),
                bone("LeftLeg", Some("LeftUpLeg"), -Vec3::Y * leg * 0.5, Quat::IDENTITY),
                bone("LeftFoot", Some("LeftLeg"), -Vec3::Y * leg * 0.5, Quat::IDENTITY),
            ],
            ..Default::default()
        })
    }

    fn close(a: Quat, b: Quat) -> bool {
        a.dot(b).abs() > 1.0 - 1e-4
    }

    /// Identical skeletons must retarget to the clip itself, or every
    /// Mixamo-on-Mixamo clip would change the moment a `.skel` was assigned.
    #[test]
    fn identical_skeletons_leave_the_clip_alone() {
        let skel = rig(Quat::from_rotation_z(0.3), 1.0, 0.9);
        let key = Quat::from_rotation_y(0.7) * Quat::from_rotation_z(0.3);
        let c = clip(vec![
            track("Hips", vec![(0.0, Vec3::new(0.2, 1.0, 0.5))], vec![(0.0, Quat::from_rotation_x(0.1))]),
            track("UpperArm", vec![], vec![(0.0, key)]),
        ]);
        let out = retarget_clip(&c, Some(&skel), &skel, &HashMap::new(), None);
        assert!(close(Quat::from_array(out.tracks[1].rotations[0].1), key));
        let hips = Vec3::from_array(out.tracks[0].translations[0].1);
        assert!(hips.distance(Vec3::new(0.2, 1.0, 0.5)) < 1e-5, "{hips}");
    }

    /// The rest pose itself must land on the target's rest pose, whatever the
    /// two rests are: that is the whole point of compensating.
    #[test]
    fn the_source_rest_pose_becomes_the_target_rest_pose() {
        let src = rig(Quat::from_rotation_z(-0.8), 1.0, 0.9);
        let dst = rig(Quat::from_rotation_x(1.2) * Quat::from_rotation_z(0.4), 1.0, 0.9);
        let c = clip(vec![track("UpperArm", vec![], vec![(0.0, Quat::from_rotation_z(-0.8))])]);
        let out = retarget_clip(&c, Some(&src), &dst, &HashMap::new(), None);
        let got = Quat::from_array(out.tracks[0].rotations[0].1);
        assert!(close(got, dst.get("UpperArm").unwrap().local.rotation), "{got:?}");
    }

    /// A change from rest is carried across in the hips' space, so raising
    /// the arm 30 degrees about the world Z axis raises the target's arm by
    /// the same world rotation even though the two arms rest differently.
    #[test]
    fn a_change_from_rest_carries_across_in_model_space() {
        let src_rest = Quat::from_rotation_z(-0.8);
        let dst_rest = Quat::from_rotation_y(0.5);
        let src = rig(src_rest, 1.0, 0.9);
        let dst = rig(dst_rest, 1.0, 0.9);
        let raise = Quat::from_rotation_z(0.5);
        let c = clip(vec![track("UpperArm", vec![], vec![(0.0, raise * src_rest)])]);
        let out = retarget_clip(&c, Some(&src), &dst, &HashMap::new(), None);
        let got = Quat::from_array(out.tracks[0].rotations[0].1);
        assert!(close(got, raise * dst_rest), "{got:?}");
    }

    /// Hips translation scales with the legs, and other bones lose their
    /// translation keys so the target keeps its own proportions.
    #[test]
    fn hips_scale_with_leg_length_and_other_translations_drop() {
        let src = rig(Quat::IDENTITY, 1.0, 1.0);
        let dst = rig(Quat::IDENTITY, 0.5, 0.5);
        let c = clip(vec![
            track("Hips", vec![(0.0, Vec3::new(0.0, 1.0, 0.0)), (1.0, Vec3::new(0.0, 1.0, 2.0))], vec![]),
            track("Forearm", vec![(0.0, Vec3::X * 3.0)], vec![(0.0, Quat::IDENTITY)]),
        ]);
        let out = retarget_clip(&c, Some(&src), &dst, &HashMap::new(), None);
        let start = Vec3::from_array(out.tracks[0].translations[0].1);
        let end = Vec3::from_array(out.tracks[0].translations[1].1);
        assert!(start.distance(Vec3::new(0.0, 0.5, 0.0)) < 1e-5, "{start}");
        assert!(end.distance(Vec3::new(0.0, 0.5, 1.0)) < 1e-5, "{end}");
        assert!(out.tracks[1].translations.is_empty());
        assert_eq!(out.tracks[1].rotations.len(), 1);
    }

    #[test]
    fn a_bone_map_renames_tracks() {
        let dst = RestSkeleton::default();
        let map = HashMap::from([("mixamorig:Head".to_string(), "head_bone".to_string())]);
        let c = clip(vec![track("mixamorig_head", vec![], vec![(0.0, Quat::IDENTITY)])]);
        let out = retarget_clip(&c, None, &dst, &map, None);
        assert_eq!(out.tracks[0].bone_name, "head_bone");
    }

    #[test]
    fn leg_length_follows_the_named_chain() {
        let skel = rig(Quat::IDENTITY, 1.0, 0.8);
        let leg = skel.leg_length("Hips").unwrap();
        assert!((leg - 0.8).abs() < 1e-5, "{leg}");
    }

    /// The first frame of a difference clip is no motion, and a later frame
    /// is exactly the change from the first.
    #[test]
    fn a_difference_clip_is_the_change_from_its_first_frame() {
        let a = Quat::from_rotation_y(0.4);
        let b = Quat::from_rotation_x(0.3) * a;
        let c = clip(vec![
            track("Hips", vec![(0.0, Vec3::Y), (1.0, Vec3::new(0.0, 1.5, 0.0))], vec![(0.0, a), (1.0, b)]),
            track("Still", vec![(0.0, Vec3::X), (1.0, Vec3::X)], vec![(0.0, a), (1.0, a)]),
        ]);
        let out = make_additive(&c);
        assert_eq!(out.tracks.len(), 1, "a bone that never moves adds nothing");
        let t = &out.tracks[0];
        assert!(close(Quat::from_array(t.rotations[0].1), Quat::IDENTITY));
        // Bevy adds a rotation as `delta * base`.
        assert!(close(Quat::from_array(t.rotations[1].1) * a, b));
        assert!(Vec3::from_array(t.translations[1].1).distance(Vec3::Y * 0.5) < 1e-5);
    }

    /// A `.skel` as the importer writes it, with the clips it claims, loads
    /// as the rest skeleton retargeting uses, and one without the list (as
    /// written before it existed) still loads.
    #[test]
    fn an_imported_skeleton_file_loads() {
        let text = r#"(bones:[(name:"Armature",parent:None,translation:(0.0,0.0,0.0),rotation:(0.0,0.0,0.0,1.0),scale:(1.0,1.0,1.0)),(name:"mixamorig:Hips",parent:Some("Armature"),translation:(0.0,1.0,0.0),rotation:(0.0,0.0,0.0,1.0),scale:(1.0,1.0,1.0))],clips:["Walk.anim"])"#;
        let pose: SkeletonRestPose = ron::from_str(text).unwrap();
        assert_eq!(pose.clips, vec!["Walk.anim".to_string()]);
        let skel = RestSkeleton::from_pose(&pose);
        assert_eq!(skel.root_bone(None).as_deref(), Some("mixamorig:Hips"));
        assert_eq!(skel.path("mixamorig_hips"), vec!["mixamorig:Hips", "Armature"]);
        let old: SkeletonRestPose = ron::from_str(r#"(bones:[])"#).unwrap();
        assert!(old.clips.is_empty());
    }

    #[test]
    fn a_pose_round_trips_through_the_file_format() {
        let skel = rig(Quat::from_rotation_z(0.2), 1.0, 0.9);
        let text = ron::ser::to_string(&skel.to_pose()).unwrap();
        let back: SkeletonRestPose = ron::from_str(&text).unwrap();
        assert_eq!(back, skel.to_pose());
        assert_eq!(back.bones[1].parent.as_deref(), Some("Armature"));
    }

    #[test]
    fn the_root_bone_is_found_by_name_or_configuration() {
        let skel = rig(Quat::IDENTITY, 1.0, 1.0);
        assert_eq!(skel.root_bone(None).as_deref(), Some("Hips"));
        assert_eq!(skel.root_bone(Some("armature")).as_deref(), Some("Armature"));
        assert_eq!(skel.root_bone(Some("missing")), None);
    }
}
