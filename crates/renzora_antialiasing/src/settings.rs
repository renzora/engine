//! The one anti-aliasing component a camera carries, and its method.

use bevy::prelude::*;
use renzora::DlssMode;
use serde::{Deserialize, Serialize};

/// How a camera smooths its edges. One method at a time: they all attack the
/// same jaggies, and two at once only blurs twice (TAA under FXAA), or fights
/// over one jitter (TAA and DLSS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum AntiAliasingMethod {
    /// Multisampling: renders several samples per pixel at triangle edges.
    /// Sharp and stable, but only edges, and only where the render path allows
    /// it (see [`crate::compat`]).
    Msaa,
    /// Fast approximate AA: a cheap full-screen blur along detected edges.
    Fxaa,
    /// Subpixel morphological AA: edge detection like FXAA, sharper result.
    Smaa,
    /// Temporal AA: jitters the camera and blends frames. The best on thin
    /// detail and shimmer (foliage, specular), and what most of the rendering
    /// here is tuned for; can ghost on fast motion.
    #[default]
    Taa,
    /// NVIDIA DLSS Super Resolution: upscaling and anti-aliasing in one.
    Dlss,
}

impl AntiAliasingMethod {
    /// Every method, in the order the inspector lists them.
    pub const ALL: [Self; 5] = [Self::Msaa, Self::Fxaa, Self::Smaa, Self::Taa, Self::Dlss];

    /// The inspector dropdown labels, index for index with [`ALL`](Self::ALL).
    pub const LABELS: [&'static str; 5] = ["MSAA", "FXAA", "SMAA", "TAA", "DLSS"];

    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.label() == label)
    }

    /// The labels of this method's quality choices, which the inspector's
    /// Quality dropdown shows while this method is selected.
    pub fn quality_labels(self) -> &'static [&'static str] {
        match self {
            Self::Msaa => &MSAA_LABELS,
            Self::Fxaa => &FXAA_LABELS,
            Self::Smaa => &SMAA_LABELS,
            // Bevy's TAA has no quality setting of its own.
            Self::Taa => &["Standard"],
            Self::Dlss => &DlssMode::LABELS,
        }
    }
}

const MSAA_LABELS: [&str; 3] = ["2x", "4x", "8x"];
const MSAA_SAMPLES: [u32; 3] = [2, 4, 8];
const FXAA_LABELS: [&str; 5] = ["Low", "Medium", "High", "Ultra", "Extreme"];
const SMAA_LABELS: [&str; 4] = ["Low", "Medium", "High", "Ultra"];

/// A camera's anti-aliasing. Lives on the scene camera, like the rest of its
/// image settings, and `EffectRouting` carries it onto the cameras that render
/// it (the editor's viewports, or the camera itself in a game).
///
/// Each method keeps its own quality, so switching method and back returns to
/// what was set before rather than to a default.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AntiAliasing {
    pub enabled: bool,
    pub method: AntiAliasingMethod,
    /// MSAA samples per pixel: 2, 4 or 8.
    pub msaa_samples: u32,
    /// FXAA sensitivity, 0 (Low) to 4 (Extreme).
    pub fxaa_quality: u32,
    /// SMAA preset, 0 (Low) to 3 (Ultra).
    pub smaa_quality: u32,
    pub dlss_mode: DlssMode,
}

impl Default for AntiAliasing {
    fn default() -> Self {
        Self {
            enabled: true,
            method: AntiAliasingMethod::Taa,
            msaa_samples: 4,
            // Bevy's own defaults for both.
            fxaa_quality: 2,
            smaa_quality: 2,
            dlss_mode: DlssMode::Auto,
        }
    }
}

impl AntiAliasing {
    /// The selected method's quality, as an index into its
    /// [`quality_labels`](AntiAliasingMethod::quality_labels).
    pub fn quality_index(&self) -> usize {
        match self.method {
            AntiAliasingMethod::Msaa => MSAA_SAMPLES
                .iter()
                .position(|&s| s == self.msaa_samples)
                .unwrap_or(1),
            AntiAliasingMethod::Fxaa => self.fxaa_quality as usize,
            AntiAliasingMethod::Smaa => self.smaa_quality as usize,
            AntiAliasingMethod::Taa => 0,
            AntiAliasingMethod::Dlss => self.dlss_mode as usize,
        }
    }

    /// Set the selected method's quality from an index into its labels. Out of
    /// range is clamped, so a keyframe or a script cannot store a value the
    /// method has no meaning for.
    pub fn set_quality_index(&mut self, i: usize) {
        let last = self.method.quality_labels().len().saturating_sub(1);
        let i = i.min(last);
        match self.method {
            AntiAliasingMethod::Msaa => self.msaa_samples = MSAA_SAMPLES[i],
            AntiAliasingMethod::Fxaa => self.fxaa_quality = i as u32,
            AntiAliasingMethod::Smaa => self.smaa_quality = i as u32,
            AntiAliasingMethod::Taa => {}
            AntiAliasingMethod::Dlss => self.dlss_mode = DlssMode::ALL[i],
        }
    }

    /// The MSAA setting this component asks for, valid or not: anything but
    /// 2, 4 or 8 is read as 4.
    pub fn msaa(&self) -> Msaa {
        match self.msaa_samples {
            2 => Msaa::Sample2,
            8 => Msaa::Sample8,
            _ => Msaa::Sample4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_round_trip() {
        for m in AntiAliasingMethod::ALL {
            assert_eq!(AntiAliasingMethod::from_label(m.label()), Some(m));
        }
    }

    #[test]
    fn each_method_keeps_its_own_quality() {
        let mut aa = AntiAliasing { method: AntiAliasingMethod::Smaa, ..default() };
        aa.set_quality_index(3);
        aa.method = AntiAliasingMethod::Msaa;
        aa.set_quality_index(2);
        assert_eq!(aa.msaa_samples, 8);
        aa.method = AntiAliasingMethod::Smaa;
        assert_eq!(aa.quality_index(), 3);
    }

    #[test]
    fn out_of_range_quality_is_clamped() {
        let mut aa = AntiAliasing { method: AntiAliasingMethod::Dlss, ..default() };
        aa.set_quality_index(99);
        assert_eq!(aa.dlss_mode, DlssMode::UltraPerformance);
        aa.method = AntiAliasingMethod::Taa;
        aa.set_quality_index(5);
        assert_eq!(aa.quality_index(), 0);
    }
}
