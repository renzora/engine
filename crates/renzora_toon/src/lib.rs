use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponent,
    render_graph::{InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph},
    render_resource::ShaderType,
};
use bevy::shader::ShaderRef;
use renzora_postprocess::PostProcessEffect;
use egui_phosphor::regular;
use renzora_editor::{FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};

#[derive(Component, Clone, Copy, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct ToonSettings {
    pub levels: f32,
    pub edge_threshold: f32,
    pub edge_thickness: f32,
    pub saturation_boost: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub enabled: f32,
}

impl Default for ToonSettings {
    fn default() -> Self {
        Self {
            levels: 4.0,
            edge_threshold: 0.1,
            edge_thickness: 1.0,
            saturation_boost: 1.2,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for ToonSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/toon.wgsl".into()
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

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "toon",
        display_name: "Toon",
        icon: regular::PAINT_BRUSH,
        category: "post_process",
        has_fn: |world, entity| world.get::<ToonSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(ToonSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<ToonSettings>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<ToonSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<ToonSettings>(entity) {
                s.enabled = if val { 1.0 } else { 0.0 };
            }
        }),
        fields: vec![
            FieldDef {
                name: "Levels",
                field_type: FieldType::Float { speed: 0.1, min: 2.0, max: 16.0 },
                get_fn: |world, entity| {
                    world.get::<ToonSettings>(entity).map(|s| FieldValue::Float(s.levels))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ToonSettings>(entity) {
                            s.levels = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Edge Threshold",
                field_type: FieldType::Float { speed: 0.005, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<ToonSettings>(entity).map(|s| FieldValue::Float(s.edge_threshold))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ToonSettings>(entity) {
                            s.edge_threshold = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Edge Thickness",
                field_type: FieldType::Float { speed: 0.05, min: 0.5, max: 5.0 },
                get_fn: |world, entity| {
                    world.get::<ToonSettings>(entity).map(|s| FieldValue::Float(s.edge_thickness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ToonSettings>(entity) {
                            s.edge_thickness = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Saturation Boost",
                field_type: FieldType::Float { speed: 0.02, min: 0.0, max: 3.0 },
                get_fn: |world, entity| {
                    world.get::<ToonSettings>(entity).map(|s| FieldValue::Float(s.saturation_boost))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<ToonSettings>(entity) {
                            s.saturation_boost = v;
                        }
                    }
                },
            },
        ],
    }
}

pub struct ToonPlugin;

impl Plugin for ToonPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ToonSettings>::default());
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
