//! Auto exposure: drives Bevy's `bevy_post_process::auto_exposure::AutoExposure`
//! from the camera's [`AutoExposureSettings`].
//!
//! Bevy's auto exposure is histogram-based with percentile filtering: it builds
//! a 64-bin luminance histogram each frame, ignores the darkest N% and brightest
//! M% of pixels, and animates the camera's exposure so the remaining "metered"
//! pixels average to middle grey. That handles dark or sparse scenes properly,
//! where a log-average meter would max out exposure on a mostly empty frame and
//! blow it to white.
//!
//! Because it *always* targets middle grey, a genuinely dark night scene gets
//! lifted toward daylight: correct eye adaptation, but not what a night scene
//! wants. The fix is Bevy's exposure **compensation curve**, which maps metered
//! brightness to an exposure offset. This builds one that is flat (no change)
//! for bright daytime metering and ramps negative for dark metering, so night
//! stays dark and day is untouched. See [`build_compensation_curve`].
//!
//! # Why this is an engine crate
//!
//! It was a marketplace plugin from 2026-08-29 to 2026-11-20, but its settings
//! component stayed in the contract crate (the level presets put it on every new
//! scene camera, switched on). So a build without the plugin saved and showed
//! "Auto Exposure: on" and did nothing, and the camera fell back to its manual
//! EV100: a scene authored at 9.7 rendered about fourteen times too bright with
//! nothing to say why. Exposure decides how every 3D scene looks, so the code
//! that acts on the setting ships wherever the setting does.
//!
//! A copy of the old plugin still installed beside the editor is not guarded
//! against: it registers the same inspector id (the registry keeps one), adds
//! Bevy's plugin only if it is missing, and writes the same `AutoExposure` onto
//! the same cameras, so the picture is unchanged and the only cost is the
//! duplicated work. Uninstalling it is the user's call.
//!
//! # Independent of the manual EV100
//!
//! Bevy meters the frame *after* the camera's manual `Exposure` has scaled it,
//! and adds its correction on top. The histogram range and the keep-dark curve
//! are therefore in exposure-scaled units, so the same settings meant different
//! things at different manual EV100s: a scene saved at EV100 9.7 metered 3.8
//! stops brighter than at 13.5, landed above the keep-dark pivot, got no
//! correction and rendered washed out, while 13.5 looked right. The settings are
//! authored against [`REFERENCE_EV100`], and both the range and the curve are
//! shifted by how far the camera's manual EV100 is from it. That cancels
//! exactly, so with auto exposure on the picture does not depend on the manual
//! value, which only takes effect when auto exposure is off.

use bevy::camera::Exposure;
use bevy::curve::cubic_splines::LinearSpline;
use bevy::math::vec2;
use bevy::post_process::auto_exposure::{
    AutoExposure, AutoExposureCompensationCurve, AutoExposurePlugin as BevyAePlugin,
};
use bevy::prelude::*;

/// The authored settings live in the **contract crate**: the level presets
/// insert them on new cameras and the debugger reads them for its EV readout,
/// and scenes store them under that path. Re-exported so
/// `renzora_auto_exposure::AutoExposureSettings` resolves too.
pub use renzora::AutoExposureSettings;

/// The manual EV100 the range and keep-dark settings are expressed against: a
/// new camera's default. See the module doc.
pub const REFERENCE_EV100: f32 = 13.5;

/// Cached compensation-curve asset, rebuilt only when the curve-shaping
/// settings change, so an asset is not churned every frame.
#[derive(Resource, Default)]
struct AeCompensation {
    handle: Handle<AutoExposureCompensationCurve>,
    key: Option<(u32, u32, u32, u32, u32)>,
    /// Stops the camera's manual EV100 sits above [`REFERENCE_EV100`]. The
    /// metered values are this many stops darker than at the reference, so the
    /// curve and the histogram range move down by the same amount.
    offset: f32,
}

/// Where the curve's two points go, at the reference EV100 shifted by
/// `offset`: `(range_min, compensation at the dark end)` and `(pivot, 0)`.
///
/// Split out of [`build_compensation_curve`] so the shape can be tested without
/// an `App`.
fn curve_points(s: &AutoExposureSettings, offset: f32) -> [Vec2; 2] {
    let lo = s.range_min;
    let top = s.range_max.max(lo + 0.002);
    let pivot = s.keep_dark_pivot_ev.clamp(lo + 0.001, top);
    let strength = s.keep_dark_strength.max(0.0);
    let comp_lo = -strength * (pivot - lo);
    // Built at the reference, then moved along the metering axis only: the
    // compensation for a given scene is the same, it is just read at a
    // different metered value.
    [vec2(lo - offset, comp_lo), vec2(pivot - offset, 0.0)]
}

/// (Re)build the exposure-compensation curve when its shaping inputs change.
///
/// The curve maps metered scene log-luminance (EV-100, x) to exposure
/// compensation in F-stops (y): flat `0` from `keep_dark_pivot_ev` upward
/// (daytime metering untouched), ramping down to `-strength*(pivot-range_min)`
/// at the dark end so dark scenes are not lifted to middle grey. A larger
/// `keep_dark_strength` darkens night harder; a higher `keep_dark_pivot_ev`
/// pulls more of the dim range down.
fn build_compensation_curve(
    sources: Query<(&AutoExposureSettings, Option<&Exposure>)>,
    scene_cameras: Query<&Exposure, With<renzora::SceneCamera>>,
    mut curves: ResMut<Assets<AutoExposureCompensationCurve>>,
    mut comp: ResMut<AeCompensation>,
) {
    // One source drives the look (normally the scene camera); prefer an enabled
    // one, else just the first present.
    let Some((s, own_exposure)) = sources
        .iter()
        .find(|(s, _)| s.enabled)
        .or_else(|| sources.iter().next())
    else {
        return;
    };
    // The manual EV100 the metering happens under: the source's own when it is
    // the camera, else the scene camera's (older scenes keep the settings on
    // the World Environment).
    let manual = own_exposure
        .or_else(|| scene_cameras.iter().next())
        .map_or(REFERENCE_EV100, |e| e.ev100);
    let offset = manual - REFERENCE_EV100;
    let key = (
        s.keep_dark_strength.to_bits(),
        s.keep_dark_pivot_ev.to_bits(),
        s.range_min.to_bits(),
        s.range_max.to_bits(),
        offset.to_bits(),
    );
    if comp.key == Some(key) {
        return;
    }

    // A SINGLE linear segment. The compensation curve clamps metered values to
    // its x-range, so one segment already gives "flat above the pivot, ramp
    // below". One segment is also what keeps `from_curve` happy: its
    // discontinuity check compares consecutive segment joins with exact float
    // equality, which a multi-segment curve with off-round values trips with
    // `DiscontinuityFound`.
    let points = curve_points(s, offset).to_vec();
    match AutoExposureCompensationCurve::from_curve(LinearSpline::new(points)) {
        Ok(curve) => {
            comp.handle = curves.add(curve);
            comp.key = Some(key);
            comp.offset = offset;
        }
        Err(e) => warn!("[auto_exposure] compensation curve build failed: {e:?}"),
    }
}

/// Route [`AutoExposureSettings`] from its source onto every camera that renders
/// it. When disabled, Bevy's `AutoExposure` is removed and the camera reverts to
/// its manual `Exposure.ev100`.
fn sync_auto_exposure(
    mut commands: Commands,
    sources: Query<(Entity, Ref<AutoExposureSettings>)>,
    routing: Res<renzora::EffectRouting>,
    comp: Res<AeCompensation>,
) {
    let routing_changed = routing.is_changed();
    // Re-apply when the compensation curve was rebuilt too, otherwise a tuning
    // change would not reach the camera until the settings themselves changed.
    let comp_changed = comp.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !comp_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                if settings.enabled {
                    commands.entity(*target).insert(AutoExposure {
                        range: (settings.range_min - comp.offset)
                            ..=(settings.range_max - comp.offset),
                        filter: settings.filter_low..=settings.filter_high,
                        speed_brighten: settings.speed_brighten,
                        speed_darken: settings.speed_darken,
                        exponential_transition_distance: settings
                            .exponential_transition_distance,
                        compensation_curve: comp.handle.clone(),
                        ..default()
                    });
                } else {
                    commands.entity(*target).remove::<AutoExposure>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<AutoExposure>();
            }
        }
    }
}

/// Route the source camera's manual `Exposure.ev100` onto every viewport target
/// camera.
///
/// In the editor the entity being edited (the scene camera) is the *source*,
/// while the camera that actually renders is a separate viewport *target*, so
/// an edit to `Exposure` on the source has no visible effect unless it is copied
/// across, exactly as bloom and tonemapping are routed. In a shipped game the
/// source *is* the render camera (`src == target`), so it is skipped: the edit
/// already landed on it, and re-inserting `Exposure` onto itself would spin
/// change detection forever (same component type in and out, unlike the
/// `*Settings` wrappers).
fn sync_exposure(
    sources: Query<Ref<Exposure>>,
    routing: Res<renzora::EffectRouting>,
    mut commands: Commands,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            if src == *target {
                break;
            }
            if let Ok(exposure) = sources.get(src) {
                if !routing_changed && !exposure.is_changed() {
                    break;
                }
                commands
                    .entity(*target)
                    .insert(Exposure { ev100: exposure.ev100 });
                break;
            }
        }
    }
}

fn cleanup_auto_exposure(
    mut commands: Commands,
    mut removed: RemovedComponents<AutoExposureSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<AutoExposure>();
            }
        }
    }
}

/// Mirror the camera's `Exposure.ev100` into `CameraExposureState` so scripts
/// (the `camera_ev` global) and HUDs can display it.
///
/// This is the *manual* EV: with auto exposure active the metering result lives
/// in a GPU buffer, and reading it back is not worth a GPU-to-CPU copy for a
/// HUD number.
fn mirror_camera_ev(
    cameras: Query<&Exposure, With<Camera3d>>,
    mut state: ResMut<renzora::core::CameraExposureState>,
) {
    if let Some(exposure) = cameras.iter().next() {
        if (state.ev100 - exposure.ev100).abs() > 1e-4 {
            state.ev100 = exposure.ev100;
        }
    }
}

#[derive(Default)]
pub struct AutoExposurePlugin;

impl Plugin for AutoExposurePlugin {
    fn build(&self, app: &mut App) {
        // Bevy's auto exposure is opt-in, not part of `DefaultPlugins`; without
        // it an `AutoExposure` component is inert.
        if !app.is_plugin_added::<BevyAePlugin>() {
            app.add_plugins(BevyAePlugin);
        }
        // Registered by Bevy's plugin too; init it here as well (idempotent) so
        // building a curve cannot panic on a missing `Assets` resource.
        app.init_asset::<AutoExposureCompensationCurve>();
        app.init_resource::<renzora::core::CameraExposureState>();
        app.init_resource::<AeCompensation>();
        app.register_type::<AutoExposureSettings>();
        // Build the curve before applying it, so a rebuilt curve reaches the
        // camera the same frame.
        app.add_systems(
            Update,
            (build_compensation_curve, sync_auto_exposure).chain(),
        );
        app.add_systems(
            Update,
            (cleanup_auto_exposure, sync_exposure, mirror_camera_ev),
        );
    }
}

renzora::add!(AutoExposurePlugin, Runtime);

#[cfg(test)]
mod tests {
    use super::*;

    /// At the reference EV100 the ramp runs from the range's dark end up to the
    /// pivot, reaching zero there, so daytime metering is left alone.
    #[test]
    fn the_curve_is_flat_above_the_pivot_and_ramps_below() {
        let s = AutoExposureSettings::default();
        let [dark, pivot] = curve_points(&s, 0.0);
        assert_eq!(dark.x, s.range_min);
        assert_eq!(pivot, vec2(s.keep_dark_pivot_ev, 0.0));
        assert!(dark.y < 0.0, "the dark end is pulled down");
    }

    /// A camera whose manual EV100 sits above the reference meters darker, so
    /// the curve moves down the metering axis by the same number of stops and
    /// the compensation itself is unchanged.
    #[test]
    fn a_different_manual_ev100_moves_the_curve_not_its_shape() {
        let s = AutoExposureSettings::default();
        let at_reference = curve_points(&s, 0.0);
        let shifted = curve_points(&s, -3.8);
        for (a, b) in at_reference.iter().zip(shifted.iter()) {
            assert_eq!(b.x, a.x + 3.8);
            assert_eq!(b.y, a.y);
        }
    }
}
