//! Screen transition post-process effect.
//!
//! Unlike every other effect (which processes a single image), this one blends
//! TWO images: the live incoming frame and a frozen snapshot of the outgoing
//! frame. That makes video-editor-style transitions possible between camera
//! cuts — crossfade/dissolve, wipe, slide and iris.
//!
//! The unified post-process node (see `renzora::postprocess`) keeps a per-view
//! snapshot texture: while `progress >= 1.0` (idle) it is refreshed every frame,
//! and while `progress < 1.0` (a transition is running) it is frozen, so the
//! shader can mix the frozen outgoing frame against the live incoming one.
//!
//! Drive it from a script by animating `progress` 0 → 1 at each cut:
//! ```lua
//! set("ScreenTransitionSettings.mode", 0)        -- 0 crossfade, 1 wipe, 2 slide, 3 iris
//! set("ScreenTransitionSettings.progress", 0.0)  -- start (snapshot freezes)
//! -- move the camera to the new shot, then ramp progress to 1.0 over time
//! ```

use bevy::prelude::*;
use bevy::render::{extract_component::ExtractComponent, render_resource::ShaderType};
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use egui_phosphor::regular;
#[cfg(feature = "editor")]
use renzora_editor_framework::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_postprocess::PostProcessEffect;

/// 8×f32 to satisfy `ShaderType` 16-byte alignment (matches `VignetteSettings`).
#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct ScreenTransitionSettings {
    /// 0 = fully show the frozen (outgoing) frame, 1 = fully show the live
    /// (incoming) frame. Idle sits at 1.0. Animate 0 → 1 to play a transition.
    pub progress: f32,
    /// 0 = crossfade, 1 = wipe, 2 = slide, 3 = iris.
    pub mode: f32,
    /// Wipe/slide axis: 0 = left→right, 1 = right→left, 2 = top→bottom, 3 = bottom→top.
    pub direction: f32,
    /// Edge softness for wipe / iris (0 = hard edge).
    pub smoothness: f32,
    /// Optional border/flash color (used by wipe & iris edges).
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub _padding: f32,
}

impl Default for ScreenTransitionSettings {
    fn default() -> Self {
        Self {
            // Idle at 1.0: shows the live frame and keeps the snapshot refreshing,
            // so an unanimated effect is invisible.
            progress: 1.0,
            mode: 0.0,
            direction: 0.0,
            smoothness: 0.03,
            color_r: 0.0,
            color_g: 0.0,
            color_b: 0.0,
            _padding: 0.0,
        }
    }
}

impl PostProcessEffect for ScreenTransitionSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_screen_transition/screen_transition.wgsl".into()
    }
    fn has_extra_texture() -> bool {
        true
    }
    fn extra_texture_is_snapshot() -> bool {
        true
    }
    fn freeze_snapshot(&self) -> bool {
        // A transition is in progress whenever progress hasn't reached 1.0;
        // freeze the snapshot so the outgoing frame is preserved during the blend.
        self.progress < 0.999
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "screen_transition",
        display_name: "Screen Transition",
        icon: regular::FILM_STRIP,
        category: "effects",
        has_fn: |world, entity| world.get::<ScreenTransitionSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(ScreenTransitionSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<ScreenTransitionSettings>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Progress",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<ScreenTransitionSettings>(entity)
                        .map(|s| FieldValue::Float(s.progress))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ScreenTransitionSettings>(entity) {
                            s.progress = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Mode (0=fade 1=wipe 2=slide 3=iris)",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 3.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<ScreenTransitionSettings>(entity)
                        .map(|s| FieldValue::Float(s.mode))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ScreenTransitionSettings>(entity) {
                            s.mode = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Direction (0=L 1=R 2=U 3=D)",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 3.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<ScreenTransitionSettings>(entity)
                        .map(|s| FieldValue::Float(s.direction))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ScreenTransitionSettings>(entity) {
                            s.direction = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Smoothness",
                field_type: FieldType::Float {
                    speed: 0.005,
                    min: 0.0,
                    max: 0.5,
                },
                get_fn: |world, entity| {
                    world
                        .get::<ScreenTransitionSettings>(entity)
                        .map(|s| FieldValue::Float(s.smoothness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ScreenTransitionSettings>(entity) {
                            s.smoothness = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Edge Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<ScreenTransitionSettings>(entity)
                        .map(|s| FieldValue::Color([s.color_r, s.color_g, s.color_b]))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut s) = world.get_mut::<ScreenTransitionSettings>(entity) {
                            s.color_r = r;
                            s.color_g = g;
                            s.color_b = b;
                        }
                    }
                },
            },
        ],
    }
}

#[derive(Default)]
pub struct ScreenTransitionPlugin;

impl Plugin for ScreenTransitionPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ScreenTransitionPlugin");
        bevy::asset::embedded_asset!(app, "screen_transition.wgsl");
        app.register_type::<ScreenTransitionSettings>();
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<ScreenTransitionSettings>::default(),
        );
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(ScreenTransitionPlugin);
