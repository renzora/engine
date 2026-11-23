use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::view::ColorGrading;
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
        // AgX (mode 4). Compared side by side on a sunlit street it kept
        // contrast and colour where TonyMcMapface lifted the shadows into a
        // grey veil, and it holds bright lights to a soft, natural white.
        Self {
            mode: 4,
            ev100: 9.7,
            enabled: true,
        }
    }
}

fn mode_to_tonemapping(mode: u32) -> Tonemapping {
    let tm = match mode {
        // "None" has always meant "no tone curve". Bevy 0.20 made
        // `Tonemapping::None` a full passthrough that also skips colour grading
        // and dithering; `Linear` is the old meaning.
        0 => Tonemapping::Linear,
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
    routing: Res<renzora_scene_api::EffectRouting>,
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
                    Tonemapping::Linear
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

/// Route the source camera's `ColorGrading` onto every viewport camera.
///
/// `ColorGrading` is Bevy's own component, which every 3D camera carries, and
/// nothing routed it: the scene camera's grading reached the screen only in
/// Play, where that camera renders, while the editor viewport kept a neutral
/// grade whatever the scene asked for. So a graded scene looked different in
/// the viewport and in the game.
///
/// Copied as-is rather than through a `*Settings` wrapper, like `Exposure` in
/// `renzora_auto_exposure`. That is also why a source that *is* its own target
/// (a shipped game's camera) is skipped: writing the component onto the entity
/// it was read from would mark it changed and copy it again every frame.
fn sync_color_grading(
    sources: Query<Ref<ColorGrading>>,
    routing: Res<renzora_scene_api::EffectRouting>,
    mut commands: Commands,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            if src == *target {
                break;
            }
            if let Ok(grading) = sources.get(src) {
                if !routing_changed && !grading.is_changed() {
                    break;
                }
                commands.entity(*target).insert(ColorGrading::clone(&grading));
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
    routing: Res<renzora_scene_api::EffectRouting>,
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
    routing: Res<renzora_scene_api::EffectRouting>,
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
    routing: Res<renzora_scene_api::EffectRouting>,
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
                // removal look like a no-op. `Linear` (no curve, grading kept) is
                // the visibly-off state and matches `enabled: false` in sync, so
                // toggling and removing agree.
                ec.insert(Tonemapping::Linear);
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
        register_color_grading_serde(app);
        app.add_systems(
            Update,
            (
                sync_tonemapping,
                cleanup_tonemapping,
                sync_color_grading,
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

/// Let a scene save and load the camera's `ColorGrading`.
///
/// `ColorGrading::global` holds a `midtones_range: Range<f32>`, and Bevy
/// reflects `Range` as an opaque value registered with `Clone` alone, no serde.
/// The scene saver drops any component it cannot serialize, silently, so every
/// camera's grading vanished on save: graded, saved, reopened, neutral. Range
/// does implement serde, so registering the type data is all it lacked.
fn register_color_grading_serde(app: &mut App) {
    use bevy::reflect::{ReflectDeserialize, ReflectSerialize};
    app.register_type::<ColorGrading>();
    app.register_type::<core::ops::Range<f32>>();
    app.register_type_data::<core::ops::Range<f32>, ReflectSerialize>();
    app.register_type_data::<core::ops::Range<f32>, ReflectDeserialize>();
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


#[cfg(test)]
mod tests {
    use super::*;
    use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
    use bevy::reflect::{FromReflect, TypeRegistry};
    use serde::de::DeserializeSeed;

    fn grading() -> ColorGrading {
        let mut grading = ColorGrading::default();
        grading.global.exposure = 0.3;
        grading.global.temperature = 0.02;
        grading.shadows.lift = 0.015;
        grading.midtones.saturation = 1.08;
        grading
    }

    /// What the scene saver does with a component: serialize it through the
    /// registry, and drop it if that fails.
    fn to_ron(registry: &TypeRegistry, grading: &ColorGrading) -> Result<String, ron::Error> {
        ron::ser::to_string(&TypedReflectSerializer::new(grading, registry))
    }

    /// Without the `Range<f32>` registration, grading cannot be serialized,
    /// which is what silently dropped it from every saved scene.
    #[test]
    fn grading_needs_the_range_registration() {
        let mut registry = TypeRegistry::default();
        registry.register::<ColorGrading>();
        assert!(to_ron(&registry, &grading()).is_err());
    }

    /// With it, a graded camera survives a save and a load unchanged.
    #[test]
    fn grading_survives_a_save_and_load() {
        let mut app = App::new();
        register_color_grading_serde(&mut app);
        let registry = app.world().resource::<AppTypeRegistry>().read();

        let original = grading();
        let saved = to_ron(&registry, &original).expect("grading serializes");
        let registration = registry.get(std::any::TypeId::of::<ColorGrading>()).unwrap();
        let mut deserializer = ron::de::Deserializer::from_str(&saved).unwrap();
        let loaded = TypedReflectDeserializer::new(registration, &registry)
            .deserialize(&mut deserializer)
            .expect("grading deserializes");
        let loaded = ColorGrading::from_reflect(loaded.as_partial_reflect()).unwrap();

        assert_eq!(loaded.global.exposure, original.global.exposure);
        assert_eq!(loaded.global.temperature, original.global.temperature);
        assert_eq!(loaded.shadows.lift, original.shadows.lift);
        assert_eq!(loaded.midtones.saturation, original.midtones.saturation);
        assert_eq!(loaded.global.midtones_range, original.global.midtones_range);
    }
}
