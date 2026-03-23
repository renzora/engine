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
pub struct EdgeGlowSettings {
    pub threshold: f32,
    pub glow_intensity: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub enabled: f32,
}

impl Default for EdgeGlowSettings {
    fn default() -> Self {
        Self {
            threshold: 0.1,
            glow_intensity: 2.0,
            color_r: 0.0,
            color_g: 1.0,
            color_b: 1.0,
            _p1: 0.0,
            _p2: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for EdgeGlowSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_edge_glow/edge_glow.wgsl".into()
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
        type_id: "edge_glow",
        display_name: "Edge Glow",
        icon: regular::SPARKLE,
        category: "post_process",
        has_fn: |world, entity| world.get::<EdgeGlowSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(EdgeGlowSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<EdgeGlowSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<EdgeGlowSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<EdgeGlowSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Threshold",
                field_type: FieldType::Float { speed: 0.005, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<EdgeGlowSettings>(entity).map(|s| FieldValue::Float(s.threshold)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<EdgeGlowSettings>(entity) { s.threshold = v; } } },
            },
            FieldDef {
                name: "Glow Intensity",
                field_type: FieldType::Float { speed: 0.05, min: 0.0, max: 5.0 },
                get_fn: |world, entity| world.get::<EdgeGlowSettings>(entity).map(|s| FieldValue::Float(s.glow_intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<EdgeGlowSettings>(entity) { s.glow_intensity = v; } } },
            },
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| world.get::<EdgeGlowSettings>(entity).map(|s| FieldValue::Color([s.color_r, s.color_g, s.color_b])),
                set_fn: |world, entity, val| { if let FieldValue::Color([r, g, b]) = val { if let Some(mut s) = world.get_mut::<EdgeGlowSettings>(entity) { s.color_r = r; s.color_g = g; s.color_b = b; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct EdgeGlowPlugin;

impl Plugin for EdgeGlowPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] EdgeGlowPlugin");
        bevy::asset::embedded_asset!(app, "edge_glow.wgsl");
        app.register_type::<EdgeGlowSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<EdgeGlowSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
