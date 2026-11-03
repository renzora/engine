//! Editor-only half of `renzora_antialiasing`: the Anti-Aliasing rows of the
//! Camera section, and the Sharpening inspector.
//!
//! Anti-aliasing is a property of the camera, so on a camera it is drawn as two
//! rows of the Camera section (added with `extend_inspector`, since that section
//! belongs to the editor framework), not as a section of its own. The data stays
//! in [`AntiAliasing`]: saving, routing to the viewport cameras and the
//! migration of old scenes all work on that component and are unchanged by
//! where it is drawn.
//!
//! `renzora_antialiasing` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspectors (renzora editor contract + Phosphor icons),
//! registered `renzora::add!(AntiAliasingEditorPlugin, Editor)` and linked only
//! by the editor bundle.

use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy::prelude::*;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_antialiasing::compat::method_warning;
use renzora_antialiasing::{AntiAliasing, AntiAliasingMethod, CasSettings};

/// The Camera section's Anti-Aliasing choices: Off, then every method.
const CAMERA_OPTIONS: [&str; 6] = ["Off", "MSAA", "FXAA", "SMAA", "TAA", "DLSS"];

/// Whether `entity` is a camera whose Camera section draws these rows. Not a 2D
/// camera: the methods are all 3D passes, and the 2D camera has no prepass for
/// TAA or DLSS to read.
fn draws_on(world: &World, entity: Entity) -> bool {
    world.get::<Camera>(entity).is_some() && !is_2d(world, entity)
}

/// The rows hide on a 2D camera; see [`draws_on`].
fn is_2d(world: &World, entity: Entity) -> bool {
    world.get::<Camera2d>(entity).is_some()
}

/// The two rows: in the Camera section on a camera, and as the whole of the
/// component's own section anywhere else.
///
/// A camera with no [`AntiAliasing`] (an old or hand-made one) reads as Off,
/// and choosing a method gives it one. Off is the component's `enabled`
/// switched off, so the method and each method's quality are still there when
/// it is turned back on.
fn camera_rows() -> Vec<FieldDef> {
    vec![
        FieldDef {
            name: "Anti-Aliasing",
            field_type: FieldType::CheckedEnum {
                options: &CAMERA_OPTIONS,
                warning: |world, entity, i| {
                    // Index 0 is Off, which always works.
                    let method = *AntiAliasingMethod::ALL.get(i.checked_sub(1)?)?;
                    method_warning(world, entity, method)
                },
            },
            get_fn: |w, e| {
                if is_2d(w, e) {
                    return None;
                }
                let label = match w.get::<AntiAliasing>(e) {
                    Some(aa) if aa.enabled => aa.method.label(),
                    _ => CAMERA_OPTIONS[0],
                };
                Some(FieldValue::Enum(label.to_string()))
            },
            set_fn: |w, e, v| {
                let FieldValue::Enum(label) = v else { return };
                let method = AntiAliasingMethod::from_label(&label);
                match (w.get_mut::<AntiAliasing>(e), method) {
                    (Some(mut aa), Some(m)) => {
                        aa.method = m;
                        aa.enabled = true;
                    }
                    (Some(mut aa), None) => aa.enabled = false,
                    (None, Some(m)) => {
                        w.entity_mut(e).insert(AntiAliasing { method: m, ..default() });
                    }
                    (None, None) => {}
                }
            },
        },
        FieldDef {
            name: "AA Quality",
            // The choices follow the method (samples for MSAA, presets for
            // SMAA, DLSS's render scale). Empty while Off, which both hides the
            // row (its value reads `None`) and changes the section's signature,
            // so the row appears and goes as anti-aliasing is turned on and off.
            field_type: FieldType::DynamicEnum {
                options: |w, e| match w.get::<AntiAliasing>(e) {
                    Some(aa) if aa.enabled && !is_2d(w, e) => {
                        aa.method.quality_labels().iter().map(|l| l.to_string()).collect()
                    }
                    _ => Vec::new(),
                },
            },
            get_fn: |w, e| match w.get::<AntiAliasing>(e) {
                Some(aa) if aa.enabled && !is_2d(w, e) => {
                    Some(FieldValue::Float(aa.quality_index() as f32))
                }
                _ => None,
            },
            set_fn: |w, e, v| {
                if let (FieldValue::Float(i), Some(mut s)) = (v, w.get_mut::<AntiAliasing>(e)) {
                    s.set_quality_index(i.round().max(0.0) as usize);
                }
            },
        },
    ]
}

/// The component's own section, for the one case the Camera rows do not
/// cover: an [`AntiAliasing`] on an entity that is not a 3D camera (a
/// scripted or hand-edited scene). It is not offered in Add Component; on a
/// camera, the Camera section's rows are the way to set it.
fn anti_aliasing_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "anti_aliasing",
        display_name: "Anti-Aliasing",
        icon: "grid-four",
        category: "camera",
        has_fn: |world, entity| {
            world.get::<AntiAliasing>(entity).is_some() && !draws_on(world, entity)
        },
        add_fn: None,
        // Only the setting: the cameras it was routed onto lose what it gave
        // them on the next reconcile, which knows which components were its own.
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AntiAliasing>();
        }),
        // No enable switch: Off is an option of the dropdown, as on a camera.
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: camera_rows(),
    }
}

fn cas_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "cas",
        display_name: "Sharpening (CAS)",
        icon: "diamonds-four",
        category: "camera",
        has_fn: |world, entity| world.get::<CasSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CasSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(CasSettings, ContrastAdaptiveSharpening)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<CasSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<CasSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::float_field!("Strength", CasSettings, sharpening_strength, 0.01, 0.0, 1.0),
            renzora::bool_field!("Denoise", CasSettings, denoise),
        ],
    }
}

/// Editor-scope companion to `renzora_antialiasing::AntiAliasingPlugin`.
#[derive(Default)]
pub struct AntiAliasingEditorPlugin;

impl Plugin for AntiAliasingEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AntiAliasingEditorPlugin");
        app.extend_inspector("camera", camera_rows());
        app.register_inspector(anti_aliasing_entry());
        app.register_inspector(cas_entry());
    }
}

renzora::add!(AntiAliasingEditorPlugin, Editor);
