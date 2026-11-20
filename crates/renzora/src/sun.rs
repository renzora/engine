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
    /// How far a contact shadow reaches, in metres: the length of the ray each
    /// pixel marches toward the sun. Longer catches bigger overhangs, but
    /// spreads the same steps thinner.
    ///
    /// Bevy keeps these three on the camera (`ContactShadows`), not the light;
    /// `renzora_lighting` copies them onto every camera that draws them, so
    /// they are set where the switch is.
    #[serde(default = "default_contact_shadow_length")]
    #[reflect(default = "default_contact_shadow_length")]
    pub contact_shadow_length: f32,
    /// How thick each surface in the depth buffer is assumed to be, in
    /// metres. The depth buffer only knows a surface's front, so a ray passing
    /// further behind it than this counts as passing behind the object, not
    /// into it. Too thin lets shadows leak through; too thick casts them from
    /// thin things as if they were solid.
    #[serde(default = "default_contact_shadow_thickness")]
    #[reflect(default = "default_contact_shadow_thickness")]
    pub contact_shadow_thickness: f32,
    /// Steps along each ray. More finds smaller occluders over a long reach,
    /// and costs linearly.
    #[serde(default = "default_contact_shadow_steps")]
    #[reflect(default = "default_contact_shadow_steps")]
    pub contact_shadow_steps: u32,
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
    /// Take the moon's phase from today's date (the real lunar month), rather
    /// than from [`moon_day`](Self::moon_day).
    #[serde(default = "default_true")]
    #[reflect(default = "default_true")]
    pub moon_follows_calendar: bool,
    /// Day of the lunar month, 0 to 29.5: 0 new, 7.4 first quarter, 14.8
    /// full, 22.1 last quarter. Used when the calendar is not followed.
    #[serde(default = "default_moon_day")]
    #[reflect(default = "default_moon_day")]
    pub moon_day: f32,
    /// Colour of the moon's face (RGB, 0 to 1). Warm orange for a harvest
    /// moon, near white for a high one.
    #[serde(default = "default_moon_tint")]
    #[reflect(default = "default_moon_tint")]
    pub moon_tint: Vec3,
    /// A lunar eclipse, 0 to 1: the Earth's shadow sweeps across the moon and
    /// at 1 covers it, leaving it dim and copper red (a blood moon). An
    /// eclipse happens at full moon, so the phase is drawn as full while one
    /// is under way, and the moonlight dims and reddens with it.
    #[serde(default)]
    #[reflect(default)]
    pub moon_eclipse: f32,
    /// Strength of the glow around the moon. 0 turns it off.
    #[serde(default = "default_moon_glow")]
    #[reflect(default = "default_moon_glow")]
    pub moon_glow: f32,
    /// Apparent diameter of the moon in degrees. Larger than the real 0.52
    /// by default, so its face reads in a wide game view.
    #[serde(default = "default_moon_size")]
    #[reflect(default = "default_moon_size")]
    pub moon_size: f32,
}

/// Length of the lunar month (new moon to new moon), in days.
pub const SYNODIC_MONTH_DAYS: f32 = 29.530_588;

fn default_true() -> bool {
    true
}

/// Longer than Bevy's 0.3. Tested in the Bistro under a 120,000 lux sun: at
/// 0.3 contact shadows only darkened the very edges of sills and mouldings; at
/// 1.5 they filled window reveals and cornice undersides but the rays began
/// catching the bumps of the cobbles, which read as grain. 0.8 sits between.
pub fn default_contact_shadow_length() -> f32 {
    0.8
}

/// Bevy's `ContactShadows` default.
pub fn default_contact_shadow_thickness() -> f32 {
    0.1
}

/// Doubled from Bevy's 16 with the longer reach above, so each step covers
/// about the distance it did in the 1.5 m / 32 step test that looked right.
pub fn default_contact_shadow_steps() -> u32 {
    32
}

/// The bounds the inspector offers for [`Sun::contact_shadow_steps`], applied
/// again when the steps reach the GPU so a hand-edited scene cannot ask for a
/// march long enough to stall it.
pub const CONTACT_SHADOW_STEPS: std::ops::RangeInclusive<u32> = 4..=64;

fn default_moon_day() -> f32 {
    SYNODIC_MONTH_DAYS * 0.5
}

fn default_moon_tint() -> Vec3 {
    Vec3::new(1.0, 0.97, 0.92)
}

fn default_moon_glow() -> f32 {
    1.0
}

fn default_moon_size() -> f32 {
    1.6
}

/// Days since the last new moon, from the system clock, or `None` where there
/// is no clock to read (the web build).
///
/// Counted from the new moon of 2000-01-06 18:14 UTC in whole synodic months.
/// The real month wanders by up to about 14 hours around its mean, which is
/// well under a day and invisible in the moon's shape.
pub fn calendar_moon_age() -> Option<f32> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        const REFERENCE_NEW_MOON_UNIX: f64 = 947_182_440.0;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?;
        let days = (now.as_secs_f64() - REFERENCE_NEW_MOON_UNIX) / 86_400.0;
        Some(days.rem_euclid(SYNODIC_MONTH_DAYS as f64) as f32)
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn default_glare_intensity() -> f32 {
    1.5
}

fn default_glare_size() -> f32 {
    6.0
}

/// Tested on the Bistro under the default auto exposure: at 30 lux the
/// street was black but for its lamps; 150 reads as moonlit.
fn default_moon_illuminance() -> f32 {
    150.0
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
            contact_shadow_length: default_contact_shadow_length(),
            contact_shadow_thickness: default_contact_shadow_thickness(),
            contact_shadow_steps: default_contact_shadow_steps(),
            angular_diameter: 0.53,
            sun_disk_intensity: 1.0,
            glare_intensity: default_glare_intensity(),
            glare_size: default_glare_size(),
            moon_illuminance: default_moon_illuminance(),
            moon_color: default_moon_color(),
            moon_follows_calendar: true,
            moon_day: default_moon_day(),
            moon_tint: default_moon_tint(),
            moon_eclipse: 0.0,
            moon_glow: default_moon_glow(),
            moon_size: default_moon_size(),
        }
    }
}

impl Sun {
    /// Compute the direction the light travels (away from the sun toward the scene).
    pub fn direction(&self) -> Vec3 {
        direction_from_angles(self.azimuth, self.elevation)
    }

    /// How much of the sun is up, 0 to 1: full above 5 degrees, nothing below
    /// -1. Its light, disc and glare fade together over that band.
    pub fn daylight(&self) -> f32 {
        horizon_factor(self.elevation)
    }

    /// How far the moon has risen, 0 to 1: nothing while any of the sun is
    /// up, full once the sun is 7 degrees down. It starts where
    /// [`daylight`](Self::daylight) ends, so sun and moon never light the scene
    /// at once.
    pub fn moonrise(&self) -> f32 {
        moon_factor(self.elevation)
    }

    /// Days into the lunar month, 0 to 29.5: from the date when the calendar
    /// is followed, else [`moon_day`](Self::moon_day).
    pub fn moon_age(&self) -> f32 {
        let age = if self.moon_follows_calendar {
            calendar_moon_age().unwrap_or(self.moon_day)
        } else {
            self.moon_day
        };
        age.rem_euclid(SYNODIC_MONTH_DAYS)
    }

    /// The moon's phase angle in radians, signed: 0 full, +-PI new, positive
    /// while waxing. Its magnitude is the angle at the moon between the sun
    /// and the viewer. An eclipse pulls it to full, where eclipses happen.
    pub fn moon_phase_angle(&self) -> f32 {
        let elongation = std::f32::consts::TAU * self.moon_age() / SYNODIC_MONTH_DAYS;
        (std::f32::consts::PI - elongation) * (1.0 - self.moon_eclipse.clamp(0.0, 1.0))
    }

    /// How much of the moon's face is lit, 0 (new) to 1 (full).
    pub fn moon_lit_fraction(&self) -> f32 {
        0.5 * (1.0 + self.moon_phase_angle().cos())
    }

    /// The direction moonlight travels: from the opposite side of the sky,
    /// as high as the sun is low, held between 15 and 75 degrees so the moon
    /// neither skims the ground nor stands directly overhead.
    pub fn moon_direction(&self) -> Vec3 {
        direction_from_angles(self.azimuth + 180.0, (-self.elevation).clamp(15.0, 75.0))
    }
}

/// See [`Sun::daylight`]. Free so `renzora_lighting` can answer for a bare
/// elevation.
pub fn horizon_factor(elevation_deg: f32) -> f32 {
    const FADE_START_DEG: f32 = -1.0;
    const FADE_END_DEG: f32 = 5.0;
    ((elevation_deg - FADE_START_DEG) / (FADE_END_DEG - FADE_START_DEG)).clamp(0.0, 1.0)
}

/// See [`Sun::moonrise`].
pub fn moon_factor(elevation_deg: f32) -> f32 {
    const RISE_START_DEG: f32 = -1.0;
    const RISE_END_DEG: f32 = -7.0;
    ((elevation_deg - RISE_START_DEG) / (RISE_END_DEG - RISE_START_DEG)).clamp(0.0, 1.0)
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
        assert!(sun.moon_follows_calendar);
        assert_eq!(sun.moon_eclipse, 0.0);
        assert_eq!(sun.moon_size, default_moon_size());
    }

    /// Day 0 is new, mid-month full, a quarter lit half, and an eclipse
    /// shows a full moon whatever the day.
    #[test]
    fn the_moon_phase_follows_the_day() {
        let on = |day: f32| Sun { moon_follows_calendar: false, moon_day: day, ..default() };
        assert!(on(0.0).moon_lit_fraction() < 1e-4);
        assert!((on(SYNODIC_MONTH_DAYS * 0.5).moon_lit_fraction() - 1.0).abs() < 1e-4);
        assert!((on(SYNODIC_MONTH_DAYS * 0.25).moon_lit_fraction() - 0.5).abs() < 1e-3);
        assert!(on(3.0).moon_phase_angle() > 0.0, "waxing");
        assert!(on(25.0).moon_phase_angle() < 0.0, "waning");
        let eclipsed = Sun { moon_eclipse: 1.0, ..on(3.0) };
        assert!((eclipsed.moon_lit_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_calendar_gives_a_day_of_the_month() {
        if let Some(age) = calendar_moon_age() {
            assert!((0.0..SYNODIC_MONTH_DAYS).contains(&age));
        }
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
