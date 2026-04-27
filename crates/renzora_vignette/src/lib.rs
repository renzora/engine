use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use serde::{Serialize, Deserialize};
use bevy::render::{
    extract_component::ExtractComponent,
    render_graph::{InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph},
    render_resource::ShaderType,
};
use bevy::shader::ShaderRef;

use renzora_postprocess::PostProcessEffect;
#[cfg(feature = "editor")]
use egui_phosphor::regular;
#[cfg(feature = "editor")]
use renzora_editor::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};

#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct VignetteSettings {
    pub intensity: f32,
    pub radius: f32,
    pub smoothness: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub _padding1: f32,
    pub enabled: f32,
}

impl Default for VignetteSettings {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            radius: 0.9,
            smoothness: 0.3,
            color_r: 0.0,
            color_g: 0.0,
            color_b: 0.0,
            _padding1: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for VignetteSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_vignette/vignette.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "vignette",
        display_name: "Vignette",
        icon: regular::APERTURE,
        category: "post_process",
        has_fn: |world, entity| world.get::<VignetteSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(VignetteSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<VignetteSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<VignetteSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<VignetteSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 5.0 },
                get_fn: |world, entity| world.get::<VignetteSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<VignetteSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Radius",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<VignetteSettings>(entity).map(|s| FieldValue::Float(s.radius)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<VignetteSettings>(entity) { s.radius = v; } } },
            },
            FieldDef {
                name: "Smoothness",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<VignetteSettings>(entity).map(|s| FieldValue::Float(s.smoothness)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<VignetteSettings>(entity) { s.smoothness = v; } } },
            },
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| world.get::<VignetteSettings>(entity).map(|s| FieldValue::Color([s.color_r, s.color_g, s.color_b])),
                set_fn: |world, entity, val| { if let FieldValue::Color([r, g, b]) = val { if let Some(mut s) = world.get_mut::<VignetteSettings>(entity) { s.color_r = r; s.color_g = g; s.color_b = b; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

#[derive(Default)]
pub struct VignettePlugin;

impl Plugin for VignettePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] VignettePlugin");
        bevy::asset::embedded_asset!(app, "vignette.wgsl");
        app.register_type::<VignetteSettings>();
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<VignetteSettings>::default(),
        );
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(VignettePlugin);
