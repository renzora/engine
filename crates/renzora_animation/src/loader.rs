//! Bevy AssetLoader for `.anim` files → AnimationClip.

use std::fmt;

use bevy::animation::animation_curves::{AnimatableCurve, WeightsCurve};
use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::curve::iterable::IterableCurve;
use bevy::curve::UnevenSampleAutoCurve;
use bevy::prelude::*;

use renzora::ChannelSpline;

use crate::clip::AnimClip;

// ============================================================================
// Error
// ============================================================================

#[derive(Debug)]
pub enum AnimLoadError {
    Io(std::io::Error),
    Ron(ron::de::SpannedError),
}

impl fmt::Display for AnimLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimLoadError::Io(e) => write!(f, "IO error: {e}"),
            AnimLoadError::Ron(e) => write!(f, "RON parse error: {e}"),
        }
    }
}

impl std::error::Error for AnimLoadError {}

impl From<std::io::Error> for AnimLoadError {
    fn from(e: std::io::Error) -> Self {
        AnimLoadError::Io(e)
    }
}

impl From<ron::de::SpannedError> for AnimLoadError {
    fn from(e: ron::de::SpannedError) -> Self {
        AnimLoadError::Ron(e)
    }
}

// ============================================================================
// Loader
// ============================================================================

#[derive(Default, TypePath)]
pub struct AnimClipLoader;

impl AssetLoader for AnimClipLoader {
    type Asset = AnimationClip;
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

        let anim_clip: AnimClip = ron::de::from_bytes(&bytes)?;
        Ok(to_animation_clip(&anim_clip))
    }

    fn extensions(&self) -> &[&str] {
        &["anim"]
    }
}

/// Build the Bevy clip that plays an [`AnimClip`]'s bone and blend shape
/// tracks (see [`morph_curves`]).
///
/// Shared by the loader and by the editor, which rebuilds the loaded clip in
/// place when you key a pose, so the key plays the next frame rather than
/// after the file is saved and reloaded.
///
/// A channel with a single key holds that value for the whole clip. It used to
/// get no curve at all, so the first key set on a bone did nothing until a
/// second one existed.
///
/// A channel whose keys are all [`renzora::KeyMode::Linear`] (every channel
/// of a file written before key modes, and every imported one) still gets
/// Bevy's own sampled curve, so it plays exactly as it always did. A channel
/// with any eased, held or hand-tangented key gets a [`SplineCurve`] instead,
/// which evaluates the same [`ChannelSpline`] the editor draws, so what the
/// curve editor shows is what plays.
pub fn to_animation_clip(anim_clip: &AnimClip) -> AnimationClip {
    use bevy::animation::animated_field;

    let mut clip = AnimationClip::default();
    clip.set_duration(anim_clip.duration);
    let span = Interval::new(0.0, anim_clip.duration.max(1e-3)).unwrap_or(Interval::UNIT);

    // A track disabled in the timeline gets no curve, so the bone holds its
    // rest pose and the clip plays as if the track were not there.
    for track in anim_clip.tracks.iter().filter(|t| !t.muted) {
        let target = crate::bone_target(&track.bone_name);

        let translations: Vec<(f32, Vec3)> = track.translations.iter().map(|(t, v)| (*t, Vec3::from_array(*v))).collect();
        match translations.as_slice() {
            [] => {}
            [(_, v)] => clip.add_curve_to_target(
                target,
                AnimatableCurve::new(animated_field!(Transform::translation), ConstantCurve::new(span, *v)),
            ),
            _ => match SplineCurve::eased(track.channel_spline(0)) {
                Some(curve) => clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::translation), Vec3Spline(curve))),
                None => {
                    if let Ok(curve) = UnevenSampleAutoCurve::new(translations) {
                        clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::translation), curve));
                    }
                }
            },
        }

        let rotations: Vec<(f32, Quat)> = track.rotations.iter().map(|(t, q)| (*t, Quat::from_array(*q))).collect();
        match rotations.as_slice() {
            [] => {}
            [(_, q)] => clip.add_curve_to_target(
                target,
                AnimatableCurve::new(animated_field!(Transform::rotation), ConstantCurve::new(span, *q)),
            ),
            _ => match SplineCurve::eased(track.channel_spline(1)) {
                Some(curve) => clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::rotation), QuatSpline(curve))),
                None => {
                    if let Ok(curve) = UnevenSampleAutoCurve::new(rotations) {
                        clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::rotation), curve));
                    }
                }
            },
        }

        let scales: Vec<(f32, Vec3)> = track.scales.iter().map(|(t, v)| (*t, Vec3::from_array(*v))).collect();
        match scales.as_slice() {
            [] => {}
            [(_, v)] => clip.add_curve_to_target(
                target,
                AnimatableCurve::new(animated_field!(Transform::scale), ConstantCurve::new(span, *v)),
            ),
            _ => match SplineCurve::eased(track.channel_spline(2)) {
                Some(curve) => clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::scale), Vec3Spline(curve))),
                None => {
                    if let Ok(curve) = UnevenSampleAutoCurve::new(scales) {
                        clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::scale), curve));
                    }
                }
            },
        }
    }

    for (target, curve) in morph_curves(anim_clip) {
        clip.add_curve_to_target(target, WeightsCurve(curve));
    }
    clip
}

/// Every mesh node's morph tracks, regrouped into the one curve per node
/// Bevy's player animates [`bevy::mesh::morph::MorphWeights`] with.
///
/// Bound by node name through [`crate::bone_target`], exactly like a bone, so
/// the face mesh of a model gets its id from the same walk that tags the
/// skeleton (`systems::ensure_animation_targets` tags every named entity under
/// the animator) and a blend shape clip plays on any model whose face mesh
/// has the same name.
pub fn morph_curves(anim_clip: &AnimClip) -> Vec<(bevy::animation::AnimationTargetId, MorphWeightsCurve)> {
    let mut groups: Vec<(String, Vec<&renzora::MorphTrack>)> = Vec::new();
    for track in anim_clip.morph_tracks.iter().filter(|t| !t.muted && !t.keys.is_empty()) {
        let key = renzora::sanitize_id(&track.target);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, tracks)) => tracks.push(track),
            None => groups.push((key, vec![track])),
        }
    }
    groups
        .into_iter()
        .map(|(_, tracks)| {
            let count = tracks.iter().map(|t| t.index + 1).max().unwrap_or(0);
            let mut channels = vec![ChannelSpline::default(); count];
            let mut start = 0.0f32;
            let mut end = anim_clip.duration.max(1e-3);
            for t in tracks.iter().rev() {
                // Reversed so the first track for an index wins, as a bone's
                // first track does.
                channels[t.index] = t.spline();
                start = start.min(t.keys[0].0);
                end = end.max(t.keys[t.keys.len() - 1].0);
            }
            let target = crate::bone_target(&tracks[0].target);
            (target, MorphWeightsCurve { channels, start, end: end.max(start + 1e-3) })
        })
        .collect()
}

/// One node's morph weights over time: one spline per morph target, the
/// weight in each spline's `x`.
///
/// Bevy's weights evaluator writes the weights it is given from the first
/// target on, so a target below the highest keyed one that has no track of
/// its own still has to be given a value: it plays at zero, a blend shape's
/// rest weight. Targets above the highest keyed one are left alone.
#[derive(Debug, Clone, Reflect)]
pub struct MorphWeightsCurve {
    channels: Vec<ChannelSpline>,
    start: f32,
    end: f32,
}

impl IterableCurve<f32> for MorphWeightsCurve {
    fn domain(&self) -> Interval {
        Interval::new(self.start, self.end).unwrap_or(Interval::UNIT)
    }

    fn sample_iter_unchecked(&self, t: f32) -> impl Iterator<Item = f32> {
        self.channels.iter().map(move |c| c.sample(t).x)
    }
}

/// A bone channel with eased keys, as a curve Bevy's animation player can
/// sample: the channel's [`ChannelSpline`] over its first to last key, the
/// same domain [`UnevenSampleAutoCurve`] gives a linear channel (the player
/// clamps to it, so the ends hold).
///
/// Evaluated exactly rather than baked into dense samples: a baked curve would
/// need a sample rate picked for the fastest motion in the clip, and would
/// round a `Constant` key's step into a ramp one sample wide.
#[derive(Debug, Clone, Reflect)]
pub struct SplineCurve {
    spline: ChannelSpline,
    start: f32,
    end: f32,
}

impl SplineCurve {
    /// The curve for `spline`, or `None` when every key is linear (the
    /// caller keeps Bevy's own curve for those) or there is nothing to span.
    fn eased(spline: ChannelSpline) -> Option<Self> {
        if spline.len() < 2 || spline.all_linear() {
            return None;
        }
        let (start, end) = (*spline.times().first()?, *spline.times().last()?);
        Some(SplineCurve { spline, start, end: end.max(start + 1e-4) })
    }

    fn domain(&self) -> Interval {
        Interval::new(self.start, self.end).unwrap_or(Interval::UNIT)
    }
}

/// [`SplineCurve`] read as a position or a scale.
#[derive(Debug, Clone, Reflect)]
pub struct Vec3Spline(SplineCurve);

impl Curve<Vec3> for Vec3Spline {
    fn domain(&self) -> Interval {
        self.0.domain()
    }
    fn sample_unchecked(&self, t: f32) -> Vec3 {
        self.0.spline.sample_vec3(t)
    }
}

/// [`SplineCurve`] read as a rotation.
#[derive(Debug, Clone, Reflect)]
pub struct QuatSpline(SplineCurve);

impl Curve<Quat> for QuatSpline {
    fn domain(&self) -> Interval {
        self.0.domain()
    }
    fn sample_unchecked(&self, t: f32) -> Quat {
        self.0.spline.sample_quat(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::{BoneTrack, KeyCurve, KeyMode};

    fn track(mode: Option<KeyMode>) -> BoneTrack {
        let mut t = BoneTrack::new("hips");
        t.translations = vec![(0.0, [0.0; 3]), (1.0, [2.0, 0.0, 0.0])];
        if let Some(m) = mode {
            t.set_key_curve(0, 0, KeyCurve::new(m));
            t.set_key_curve(0, 1, KeyCurve::new(m));
        }
        t
    }

    #[test]
    fn linear_keys_keep_bevys_own_curve() {
        assert!(SplineCurve::eased(track(None).channel_spline(0)).is_none());
        assert!(SplineCurve::eased(track(Some(KeyMode::Linear)).channel_spline(0)).is_none());
    }

    #[test]
    fn eased_keys_sample_the_spline_over_the_keyed_span() {
        let curve = Vec3Spline(SplineCurve::eased(track(Some(KeyMode::Auto)).channel_spline(0)).unwrap());
        assert_eq!(curve.domain(), Interval::new(0.0, 1.0).unwrap());
        assert!((curve.sample_unchecked(0.5).x - 1.0).abs() < 1e-5);
        // Eased: well behind the straight line a quarter of the way in.
        assert!(curve.sample_unchecked(0.25).x < 0.4);
    }

    #[test]
    fn a_clip_with_eased_keys_builds() {
        let mut clip = crate::extract::create_empty_clip("c", 1.0);
        clip.tracks.push(track(Some(KeyMode::Auto)));
        let built = to_animation_clip(&clip);
        assert_eq!(built.curves().len(), 1);
    }

    fn morph(target: &str, index: usize, keys: Vec<(f32, f32)>) -> renzora::MorphTrack {
        let mut m = renzora::MorphTrack::new(target, index, "");
        m.keys = keys;
        m
    }

    /// A node's tracks become one curve that writes every weight up to the
    /// highest keyed target, unkeyed ones at their rest weight of zero.
    #[test]
    fn morph_tracks_group_into_one_curve_per_node() {
        let mut clip = crate::extract::create_empty_clip("talk", 2.0);
        clip.morph_tracks = vec![
            morph("Face", 2, vec![(0.0, 0.0), (2.0, 1.0)]),
            morph("Face", 0, vec![(1.0, 0.25)]),
            // The other spelling of the same node joins its group.
            morph("face", 1, vec![(0.0, 0.5)]),
            morph("Teeth", 0, vec![(0.0, 1.0)]),
        ];
        let curves = morph_curves(&clip);
        assert_eq!(curves.len(), 2);
        let (target, face) = &curves[0];
        assert_eq!(*target, crate::bone_target("Face"));
        let at_one: Vec<f32> = face.sample_iter_clamped(1.0).collect();
        assert_eq!(at_one.len(), 3);
        assert!((at_one[0] - 0.25).abs() < 1e-6 && (at_one[1] - 0.5).abs() < 1e-6 && (at_one[2] - 0.5).abs() < 1e-6, "{at_one:?}");
        // Held past the clip's end.
        let late: Vec<f32> = face.sample_iter_clamped(9.0).collect();
        assert!((late[2] - 1.0).abs() < 1e-6);
        assert_eq!(to_animation_clip(&clip).curves().len(), 2);
    }

    #[test]
    fn a_muted_or_empty_morph_track_plays_nothing() {
        let mut clip = crate::extract::create_empty_clip("talk", 1.0);
        let mut muted = morph("Face", 0, vec![(0.0, 1.0)]);
        muted.muted = true;
        clip.morph_tracks = vec![muted, morph("Face", 1, vec![])];
        assert!(morph_curves(&clip).is_empty());
    }

    /// An eased morph key plays the same spline the editor draws.
    #[test]
    fn eased_morph_keys_play_their_spline() {
        let mut clip = crate::extract::create_empty_clip("talk", 1.0);
        let mut m = morph("Face", 0, vec![(0.0, 0.0), (1.0, 1.0)]);
        m.set_key_curve(0, KeyCurve::AUTO);
        m.set_key_curve(1, KeyCurve::AUTO);
        let expected = m.sample(0.25).unwrap();
        clip.morph_tracks = vec![m];
        let (_, curve) = &morph_curves(&clip)[0];
        let got = curve.sample_iter_clamped(0.25).next().unwrap();
        assert!((got - expected).abs() < 1e-6 && got < 0.2, "{got}");
    }
}
