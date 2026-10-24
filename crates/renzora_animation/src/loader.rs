//! Bevy AssetLoader for `.anim` files → AnimationClip.

use std::fmt;

use bevy::animation::animation_curves::AnimatableCurve;
use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::curve::UnevenSampleAutoCurve;
use bevy::prelude::*;

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

/// Build the Bevy clip that plays an [`AnimClip`]'s bone tracks.
///
/// Shared by the loader and by the editor, which rebuilds the loaded clip in
/// place when you key a pose, so the key plays the next frame rather than
/// after the file is saved and reloaded.
///
/// A channel with a single key holds that value for the whole clip. It used to
/// get no curve at all, so the first key set on a bone did nothing until a
/// second one existed.
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
            _ => {
                if let Ok(curve) = UnevenSampleAutoCurve::new(translations) {
                    clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::translation), curve));
                }
            }
        }

        let rotations: Vec<(f32, Quat)> = track.rotations.iter().map(|(t, q)| (*t, Quat::from_array(*q))).collect();
        match rotations.as_slice() {
            [] => {}
            [(_, q)] => clip.add_curve_to_target(
                target,
                AnimatableCurve::new(animated_field!(Transform::rotation), ConstantCurve::new(span, *q)),
            ),
            _ => {
                if let Ok(curve) = UnevenSampleAutoCurve::new(rotations) {
                    clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::rotation), curve));
                }
            }
        }

        let scales: Vec<(f32, Vec3)> = track.scales.iter().map(|(t, v)| (*t, Vec3::from_array(*v))).collect();
        match scales.as_slice() {
            [] => {}
            [(_, v)] => clip.add_curve_to_target(
                target,
                AnimatableCurve::new(animated_field!(Transform::scale), ConstantCurve::new(span, *v)),
            ),
            _ => {
                if let Ok(curve) = UnevenSampleAutoCurve::new(scales) {
                    clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::scale), curve));
                }
            }
        }
    }
    clip
}
