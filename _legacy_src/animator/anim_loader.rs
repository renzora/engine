//! Bevy AssetLoader for `.anim` files → AnimationClip.

use std::fmt;
use bevy::prelude::*;
use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::animation::AnimationTargetId;
use bevy::animation::animation_curves::AnimatableCurve;
use bevy::math::curve::UnevenSampleAutoCurve;

use super::anim_file::AnimFile;

// ============================================================================
// Error type
// ============================================================================

#[derive(Debug)]
pub enum AnimLoadError {
    Io(std::io::Error),
    Ron(ron::de::SpannedError),
    Curve(String),
}

impl fmt::Display for AnimLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimLoadError::Io(e) => write!(f, "IO error: {e}"),
            AnimLoadError::Ron(e) => write!(f, "RON parse error: {e}"),
            AnimLoadError::Curve(e) => write!(f, "Curve error: {e}"),
        }
    }
}

impl std::error::Error for AnimLoadError {}

impl From<std::io::Error> for AnimLoadError {
    fn from(e: std::io::Error) -> Self { AnimLoadError::Io(e) }
}

impl From<ron::de::SpannedError> for AnimLoadError {
    fn from(e: ron::de::SpannedError) -> Self { AnimLoadError::Ron(e) }
}

// ============================================================================
// Loader
// ============================================================================

#[derive(Default, TypePath)]
pub struct AnimFileLoader;

impl AssetLoader for AnimFileLoader {
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

        let anim_file: AnimFile = ron::de::from_bytes(&bytes)?;
        let mut clip = AnimationClip::default();
        clip.set_duration(anim_file.duration);

        for track in &anim_file.tracks {
            let target = AnimationTargetId::from_name(&Name::new(track.bone_name.clone()));

            // Translations
            if !track.translations.is_empty() {
                let keyframes: Vec<(f32, Vec3)> = track.translations
                    .iter()
                    .map(|(t, v)| (*t, Vec3::new(v[0], v[1], v[2])))
                    .collect();
                match UnevenSampleAutoCurve::new(keyframes) {
                    Ok(curve) => {
                        clip.add_curve_to_target(
                            target,
                            AnimatableCurve::new(bevy::animation::animated_field!(Transform::translation), curve),
                        );
                    }
                    Err(e) => {
                        warn!("AnimFileLoader: translation curve error for '{}': {:?}", track.bone_name, e);
                    }
                }
            }

            // Rotations
            if !track.rotations.is_empty() {
                let keyframes: Vec<(f32, Quat)> = track.rotations
                    .iter()
                    .map(|(t, q)| (*t, Quat::from_xyzw(q[0], q[1], q[2], q[3])))
                    .collect();
                match UnevenSampleAutoCurve::new(keyframes) {
                    Ok(curve) => {
                        clip.add_curve_to_target(
                            target,
                            AnimatableCurve::new(bevy::animation::animated_field!(Transform::rotation), curve),
                        );
                    }
                    Err(e) => {
                        warn!("AnimFileLoader: rotation curve error for '{}': {:?}", track.bone_name, e);
                    }
                }
            }

            // Scales
            if !track.scales.is_empty() {
                let keyframes: Vec<(f32, Vec3)> = track.scales
                    .iter()
                    .map(|(t, v)| (*t, Vec3::new(v[0], v[1], v[2])))
                    .collect();
                match UnevenSampleAutoCurve::new(keyframes) {
                    Ok(curve) => {
                        clip.add_curve_to_target(
                            target,
                            AnimatableCurve::new(bevy::animation::animated_field!(Transform::scale), curve),
                        );
                    }
                    Err(e) => {
                        warn!("AnimFileLoader: scale curve error for '{}': {:?}", track.bone_name, e);
                    }
                }
            }
        }

        Ok(clip)
    }

    fn extensions(&self) -> &[&str] {
        &["anim"]
    }
}
