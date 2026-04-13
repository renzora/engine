use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use serde::{Serialize, Deserialize};
use bevy::render::{
    extract_component::ExtractComponent,
    render_graph::{InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph},
    render_resource::ShaderType,
};
use bevy::shader::ShaderRef;
use renzora::postprocess::PostProcessEffect;
#[cfg(feature = "editor")]
use egui_phosphor::regular;
#[cfg(feature = "editor")]
use renzora::editor::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};

#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct OutlineSettings {
    pub thickness: f32,
    pub threshold: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub mix_mode: f32,
    pub _padding1: f32,
    pub enabled: f32,
}

impl Default for OutlineSettings {
    fn default() -> Self {
        Self {
            thickness: 1.0,
            threshold: 0.1,
            color_r: 0.0,
            color_g: 0.0,
            color_b: 0.0,
            mix_mode: 0.0,
            _padding1: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for OutlineSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_outline/outline.wgsl".into()
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
        type_id: "outline",
        display_name: "Outline",
        icon: regular::FRAME_CORNERS,
        category: "post_process",
        has_fn: |world, entity| world.get::<OutlineSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(OutlineSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<OutlineSettings>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<OutlineSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<OutlineSettings>(entity) {
                s.enabled = if val { 1.0 } else { 0.0 };
            }
        }),
        fields: vec![
            FieldDef {
                name: "Thickness",
                field_type: FieldType::Float { speed: 0.05, min: 0.5, max: 5.0 },
                get_fn: |world, entity| {
                    world.get::<OutlineSettings>(entity).map(|s| FieldValue::Float(s.thickness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<OutlineSettings>(entity) {
                            s.thickness = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Threshold",
                field_type: FieldType::Float { speed: 0.005, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<OutlineSettings>(entity).map(|s| FieldValue::Float(s.threshold))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<OutlineSettings>(entity) {
                            s.threshold = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<OutlineSettings>(entity).map(|s| FieldValue::Color([s.color_r, s.color_g, s.color_b]))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut s) = world.get_mut::<OutlineSettings>(entity) {
                            s.color_r = r;
                            s.color_g = g;
                            s.color_b = b;
                        }
                    }
                },
            },
            FieldDef {
                name: "Mix Mode",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<OutlineSettings>(entity).map(|s| FieldValue::Float(s.mix_mode))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<OutlineSettings>(entity) {
                            s.mix_mode = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

#[derive(Default)]
pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] OutlinePlugin");
        bevy::asset::embedded_asset!(app, "outline.wgsl");
        app.register_type::<OutlineSettings>();
        app.add_plugins(renzora::postprocess::PostProcessPlugin::<OutlineSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(OutlinePlugin);
