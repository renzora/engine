//! `Sun` — the scene's directional light, described by angles.
//!
//! Here rather than in `renzora_lighting` for the usual reason: six crates read
//! it — the engine, the environment map, the gizmos, the level presets, the
//! lighting inspector and the night-stars plugin — and one of those is now a
//! plugin loaded at runtime, so the type has to live where a binary and a
//! `dlopen`ed library can both name it. `renzora_lighting` re-exports it, so
//! every existing `renzora_lighting::Sun` path still resolves.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A directional light positioned by azimuth and elevation angles.
///
/// This is a higher-level wrapper around Bevy's `DirectionalLight` that lets
/// you control the sun position with intuitive angle parameters instead of
/// raw quaternion rotation. A sync system automatically updates the
/// `DirectionalLight` and `Transform` each frame.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct Sun {
    /// Azimuth angle in degrees (0–360, compass direction of the sun).
    pub azimuth: f32,
    /// Elevation angle in degrees (−90 to 90, height above horizon).
    pub elevation: f32,
    /// Light color (RGB, 0–1 range).
    pub color: Vec3,
    /// Illuminance in lux.
    pub illuminance: f32,
    /// Whether this light casts shadows.
    pub shadows_enabled: bool,
    /// Whether this light casts screen-space **contact shadows** — small-scale
    /// shadows where objects meet surfaces, filling in detail shadow maps miss.
    /// Needs the camera's depth prepass + a `ContactShadows` component (the
    /// editor/runtime cameras carry one). Bevy 0.19 built-in.
    pub contact_shadows: bool,
    /// Angular diameter of the sun disc in degrees (Earth's sun ≈ 0.53°).
    pub angular_diameter: f32,
    /// Brightness multiplier for the sun disk (0 = no disk, 1 = physical, >1 = overexposed).
    pub sun_disk_intensity: f32,
    /// Strength of the glare around the sun: the haze a camera sees spreading
    /// from it when it is in view. 0 turns it off; the default is 1.5.
    ///
    /// Bloom cannot make this. It caps how much any one pixel adds to the glow,
    /// so a disc a hundred thousand times brighter than the sky blooms exactly
    /// as much as a bright cloud. The glare is drawn on its own, by
    /// `renzora_atmosphere`, and fades as clouds or scenery cover the disc.
    #[serde(default = "default_glare_intensity")]
    #[reflect(default = "default_glare_intensity")]
    pub glare_intensity: f32,
    /// How far the glare spreads, in degrees from the sun's centre.
    #[serde(default = "default_glare_size")]
    #[reflect(default = "default_glare_size")]
    pub glare_size: f32,
    /// Moonlight, in lux, that the sun's light becomes once the sun has set.
    /// 0 keeps nights unlit.
    ///
    /// Brighter than a real full moon (about 0.3 lux) on purpose: the camera's
    /// auto exposure cannot open far enough to make physical moonlight
    /// readable, and the keep-dark curve still holds the result to night.
    #[serde(default = "default_moon_illuminance")]
    #[reflect(default = "default_moon_illuminance")]
    pub moon_illuminance: f32,
    /// Colour of the moonlight (RGB, 0 to 1). Cool, as moonlit scenes read.
    #[serde(default = "default_moon_color")]
    #[reflect(default = "default_moon_color")]
    pub moon_color: Vec3,
}

fn default_glare_intensity() -> f32 {
    1.5
}

fn default_glare_size() -> f32 {
    6.0
}

fn default_moon_illuminance() -> f32 {
    30.0
}

fn default_moon_color() -> Vec3 {
    Vec3::new(0.62, 0.72, 1.0)
}

impl Default for Sun {
    fn default() -> Self {
        Self {
            // Mid-morning: high enough to light a street between buildings,
            // low enough that walls are modelled and shadows fall long. At 25
            // degrees a street was in shade end to end; nearer noon the light
            // went flat.
            azimuth: 245.5,
            elevation: 38.5,
            color: Vec3::new(1.0, 0.95, 0.88),
            // Tuned against the default camera (EV100 13.5, manual): at
            // 40,000 lux the sky light outweighed the sun and a sunlit
            // street read flat and grey, with weak shadows.
            illuminance: 120_000.0,
            shadows_enabled: true,
            // On: it is what seats small things on the ground (a bollard, a
            // chair leg) where the shadow map is too coarse to reach.
            contact_shadows: true,
            angular_diameter: 0.53,
            sun_disk_intensity: 1.0,
            glare_intensity: default_glare_intensity(),
            glare_size: default_glare_size(),
            moon_illuminance: default_moon_illuminance(),
            moon_color: default_moon_color(),
        }
    }
}

impl Sun {
    /// Compute the direction the light travels (away from the sun toward the scene).
    pub fn direction(&self) -> Vec3 {
        direction_from_angles(self.azimuth, self.elevation)
    }

    /// The direction moonlight travels: from the opposite side of the sky,
    /// as high as the sun is low, held between 15 and 75 degrees so the moon
    /// neither skims the ground nor stands directly overhead.
    pub fn moon_direction(&self) -> Vec3 {
        direction_from_angles(self.azimuth + 180.0, (-self.elevation).clamp(15.0, 75.0))
    }
}

fn direction_from_angles(azimuth_deg: f32, elevation_deg: f32) -> Vec3 {
    let az = azimuth_deg.to_radians();
    let el = elevation_deg.to_radians();
    Vec3::new(-el.cos() * az.sin(), -el.sin(), -el.cos() * az.cos())
}

/// Night stars — procedural starfield rendered on a sky dome.
///
/// Same boundary reason as [`Sun`]:
/// `renzora_level_presets` builds a night sky by constructing one of these while
/// compiled into the editor binary, and the renderer is a plugin.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NightStarsData {
    /// Star density (0 = very few, 1 = dense starfield)
    pub density: f32,
    /// Brightness multiplier (0..10)
    pub brightness: f32,
    /// Star angular size (0.2 = tiny dots, 5.0 = large blobs)
    pub star_size: f32,
    /// Twinkling animation speed (0 = static, 10 = fast)
    pub twinkle_speed: f32,
    /// Twinkling intensity (0 = no twinkle, 1 = strong)
    pub twinkle_amount: f32,
    /// Elevation angle at which stars fade in above the horizon (0..1)
    pub horizon_fade: f32,
    /// Star color tint (RGB)
    pub color: (f32, f32, f32),
    /// When disabled the sky dome is despawned (no draw cost) and
    /// re-spawned on enable — same as removing the component, but lets
    /// the inspector keep the settings around for a quick toggle.
    pub enabled: bool,
}

impl Default for NightStarsData {
    fn default() -> Self {
        Self {
            density: 0.55,
            brightness: 1.5,
            star_size: 1.2,
            twinkle_speed: 1.0,
            twinkle_amount: 0.35,
            horizon_fade: 0.08,
            color: (1.0, 0.97, 0.9),
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scene saved before the glare existed reconstructs, with the glare
    /// at its defaults. Without `#[reflect(default)]` on the new fields the
    /// whole `Sun` fails to load, and the error names `Sun`, not the field.
    #[test]
    fn a_sun_saved_before_the_glare_still_loads() {
        use bevy::reflect::structs::DynamicStruct;
        use bevy::reflect::FromReflect;

        let mut old = DynamicStruct::default();
        old.insert("azimuth", 120.0f32);
        old.insert("elevation", 30.0f32);
        old.insert("color", Vec3::ONE);
        old.insert("illuminance", 40_000.0f32);
        old.insert("shadows_enabled", true);
        old.insert("contact_shadows", false);
        old.insert("angular_diameter", 0.53f32);
        old.insert("sun_disk_intensity", 1.0f32);

        let sun = Sun::from_reflect(&old).expect("an old sun must load, not fail the scene");
        assert_eq!(sun.azimuth, 120.0);
        assert_eq!(sun.glare_intensity, default_glare_intensity());
        assert_eq!(sun.glare_size, default_glare_size());
        assert_eq!(sun.moon_illuminance, default_moon_illuminance());
        assert_eq!(sun.moon_color, default_moon_color());
    }

    /// The moon rises opposite the sun: a sun just set in the west puts the
    /// moon low in the east, and the light always comes from above.
    #[test]
    fn the_moon_is_opposite_the_sun_and_above_the_horizon() {
        let sun = Sun { azimuth: 270.0, elevation: -30.0, ..default() };
        let toward_moon = -sun.moon_direction();
        assert!(toward_moon.y > 0.4, "the moon is up: {toward_moon}");
        let toward_set_sun = -sun.direction();
        assert!(
            toward_moon.x * toward_set_sun.x + toward_moon.z * toward_set_sun.z < 0.0,
            "the moon is across the sky from the sun"
        );
    }
}
