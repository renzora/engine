//! Bevy AssetLoader for `.anim` files → AnimationClip.

use std::fmt;

use bevy::animation::AnimationTargetId;
use bevy::animation::animation_curves::AnimatableCurve;
use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::math::curve::UnevenSampleAutoCurve;
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
        let mut clip = AnimationClip::default();
        clip.set_duration(anim_clip.duration);

        for track in &anim_clip.tracks {
            let target = AnimationTargetId::from_name(&Name::new(track.bone_name.clone()));

            // Translations
            if track.translations.len() >= 2 {
                let keyframes: Vec<(f32, Vec3)> = track
                    .translations
                    .iter()
                    .map(|(t, v)| (*t, Vec3::new(v[0], v[1], v[2])))
                    .collect();
                if let Ok(curve) = UnevenSampleAutoCurve::new(keyframes) {
                    clip.add_curve_to_target(
                        target,
                        AnimatableCurve::new(
                            bevy::animation::animated_field!(Transform::translation),
                            curve,
                        ),
                    );
                }
            }

            // Rotations
            if track.rotations.len() >= 2 {
                let keyframes: Vec<(f32, Quat)> = track
                    .rotations
                    .iter()
                    .map(|(t, q)| (*t, Quat::from_xyzw(q[0], q[1], q[2], q[3])))
                    .collect();
                if let Ok(curve) = UnevenSampleAutoCurve::new(keyframes) {
                    clip.add_curve_to_target(
                        target,
                        AnimatableCurve::new(
                            bevy::animation::animated_field!(Transform::rotation),
                            curve,
                        ),
                    );
                }
            }

            // Scales
            if track.scales.len() >= 2 {
                let keyframes: Vec<(f32, Vec3)> = track
                    .scales
                    .iter()
                    .map(|(t, v)| (*t, Vec3::new(v[0], v[1], v[2])))
                    .collect();
                if let Ok(curve) = UnevenSampleAutoCurve::new(keyframes) {
                    clip.add_curve_to_target(
                        target,
                        AnimatableCurve::new(
                            bevy::animation::animated_field!(Transform::scale),
                            curve,
                        ),
                    );
                }
            }
        }

        Ok(clip)
    }

    fn extensions(&self) -> &[&str] {
        &["anim"]
    }
}
