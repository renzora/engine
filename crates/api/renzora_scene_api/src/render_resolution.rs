//! A camera's render-resolution scale. Part of the scene vocabulary because
//! `CameraRenderResolution`, a saved camera component, wraps it.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Render-resolution scale for a camera. The camera's render target is sized at
/// this fraction of the on-screen panel size; the displayed image is upscaled to
/// fill the panel. Lower resolutions trade sharpness for a large fill-rate win on
/// the fullscreen-bound passes (atmosphere / prepass / auto-exposure).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Reflect, Serialize, Deserialize,
)]
#[reflect(Serialize, Deserialize)]
#[type_path = "renzora::core::viewport_types"]
pub enum RenderResolution {
    #[default]
    Full,
    Half,
    Quarter,
}

impl RenderResolution {
    pub const ALL: &'static [RenderResolution] = &[Self::Full, Self::Half, Self::Quarter];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Half => "Half",
            Self::Quarter => "Quarter",
        }
    }

    /// Parse a label (as produced by [`Self::label`]) back into a variant.
    pub fn from_label(label: &str) -> Self {
        match label {
            "Half" => Self::Half,
            "Quarter" => Self::Quarter,
            _ => Self::Full,
        }
    }

    /// Multiplier applied to the panel size to get the render-target size.
    pub fn scale(&self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Half => 0.5,
            Self::Quarter => 0.25,
        }
    }
}
