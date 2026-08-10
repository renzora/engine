use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TonemappingSettings {
    /// 0=None, 1=Reinhard, 2=ReinhardLuminance, 3=AcesFitted,
    /// 4=AgX, 5=SomewhatBoring, 6=TonyMcMapface, 7=BlenderFilmic
    pub mode: u32,
    pub ev100: f32,
    pub enabled: bool,
}

impl Default for TonemappingSettings {
    fn default() -> Self {
        // TonyMcMapface (mode 6) — modern picture-formation algorithm
        // that preserves saturated highlights better than AgX or ACES.
        // It's also Bevy's default tonemapper for HDR cameras, so this
        // matches what users see before adding any tonemapping settings.
        Self {
            mode: 6,
            ev100: 9.7,
            enabled: true,
        }
    }
}

fn mode_to_tonemapping(mode: u32) -> Tonemapping {
    let tm = match mode {
        0 => Tonemapping::None,
        1 => Tonemapping::Reinhard,
        2 => Tonemapping::ReinhardLuminance,
        3 => Tonemapping::AcesFitted,
        4 => Tonemapping::AgX,
        5 => Tonemapping::SomewhatBoringDisplayTransform,
        6 => Tonemapping::TonyMcMapface,
        7 => Tonemapping::BlenderFilmic,
        _ => Tonemapping::TonyMcMapface,
    };
    substitute_if_no_luts(tm)
}

/// Identity when the LUTs are compiled in.
#[cfg(feature = "tonemapping_luts")]
fn substitute_if_no_luts(tm: Tonemapping) -> Tonemapping {
    tm
}

/// Swap LUT-sampling curves for the closest table-free one.
///
/// `AgX`, `TonyMcMapface` and `BlenderFilmic` read KTX2 lookup tables that Bevy
/// only embeds with its `tonemapping_luts` feature. Without them Bevy does not
/// fall back — it logs `TonyMcMapFace tonemapping requires the tonemapping_luts
/// feature` and renders the whole screen magenta. Since the DEFAULT mode is 6
/// (TonyMcMapface), stripping the LUTs would break every export that didn't also
/// change its tonemapper, so map those three onto `AcesFitted`: also filmic, and
/// evaluated in the shader with no table.
#[cfg(not(feature = "tonemapping_luts"))]
fn substitute_if_no_luts(tm: Tonemapping) -> Tonemapping {
    match tm {
        Tonemapping::AgX | Tonemapping::TonyMcMapface | Tonemapping::BlenderFilmic => {
            Tonemapping::AcesFitted
        }
        other => other,
    }
}

fn sync_tonemapping(
    mut commands: Commands,
    sources: Query<(Entity, Ref<TonemappingSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    break;
                }
                let tm = if settings.enabled {
                    mode_to_tonemapping(settings.mode)
                } else {
                    Tonemapping::None
                };
                // Tonemapping = the picture-formation curve only. Exposure (ev100)
                // is a CAMERA lens attribute now (edited in the Camera section,
                // driven by Auto Exposure when on) — tonemapping no longer touches it.
                commands.entity(*target).insert(tm);
                break;
            }
        }
    }
}

// ── Deband Dither ──

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DebandDitherSettings {
    pub enabled: bool,
}

impl Default for DebandDitherSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_deband_dither(
    mut commands: Commands,
    sources: Query<(Entity, Ref<DebandDitherSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    break;
                }
                commands.entity(*target).insert(if settings.enabled {
                    DebandDither::Enabled
                } else {
                    DebandDither::Disabled
                });
                break;
            }
        }
    }
}

fn cleanup_deband_dither(
    mut commands: Commands,
    mut removed: RemovedComponents<DebandDitherSettings>,
    routing: Res<renzora::EffectRouting>,
    alive: Query<()>,
) {
    // Despawn vs. deliberate removal — see `cleanup_tonemapping`.
    if removed.read().any(|e| alive.contains(e)) {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.insert(DebandDither::Disabled);
            }
        }
    }
}

fn cleanup_tonemapping(
    mut commands: Commands,
    mut removed: RemovedComponents<TonemappingSettings>,
    routing: Res<renzora::EffectRouting>,
    alive: Query<()>,
) {
    // Only a *deliberate* removal means "no tone curve".
    //
    // `RemovedComponents` also fires when the entity is despawned, and the whole
    // image-quality bucket — tonemapping, bloom, AE, TAA, exposure — is seeded
    // onto the scene camera (`renzora_level_presets::seed_camera_effects`). So
    // deleting the camera used to force `Tonemapping::None` onto every routed
    // camera, including the editor viewport, which then rendered raw HDR: a
    // blown-out white sky with no way back, since with no source `sync_tonemapping`
    // never inserts a curve again. Adding a camera "fixed" it only because that
    // re-seeded the settings.
    //
    // An entity that still exists lost the component on purpose (the inspector's
    // remove button); one that doesn't was despawned, which says nothing about
    // what curve the remaining cameras should use.
    let deliberate = removed.read().any(|e| alive.contains(e));
    if deliberate {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                // `Tonemapping::default()` is TonyMcMapface — identical to what
                // was already showing — so resetting to the default would make
                // removal look like a no-op. `None` is the visibly-off state and
                // matches `enabled: false` in sync, so toggling and removing agree.
                ec.insert(Tonemapping::None);
            }
        }
    }
}

#[derive(Default)]
pub struct TonemappingPlugin;

impl Plugin for TonemappingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TonemappingPlugin");
        app.register_type::<TonemappingSettings>();
        app.register_type::<DebandDitherSettings>();
        app.add_systems(
            Update,
            (
                sync_tonemapping,
                cleanup_tonemapping,
                sync_deband_dither,
                cleanup_deband_dither,
            ),
        );
        // Catch LUT-based curves this crate did not choose. `Camera3d`/`Camera2d`
        // pull `Tonemapping` as a required component, and its `Default` IS
        // TonyMcMapface — so in a build with the LUTs stripped every camera
        // arrives already broken, before any `TonemappingSettings` routing runs.
        // Substituting only inside `mode_to_tonemapping` was not enough: that
        // path never sees a camera with no settings entity.
        #[cfg(not(feature = "tonemapping_luts"))]
        app.add_systems(Update, force_lutless_tonemapping);
    }
}

/// Rewrite any LUT-sampling `Tonemapping` to a table-free curve.
///
/// Runs only in a build with `tonemapping_luts` stripped. Change-detected, so it
/// costs one filtered query per frame in steady state.
#[cfg(not(feature = "tonemapping_luts"))]
fn force_lutless_tonemapping(mut q: Query<&mut Tonemapping, Changed<Tonemapping>>) {
    for mut tm in q.iter_mut() {
        let fixed = substitute_if_no_luts(*tm);
        if fixed != *tm {
            *tm = fixed;
        }
    }
}

renzora::add!(TonemappingPlugin);
