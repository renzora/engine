use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
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
use renzora_editor::{FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};

#[derive(Component, Clone, Copy, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct OilPaintingSettings {
    pub radius: f32,
    pub levels: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub _p3: f32,
    pub _p4: f32,
    pub _p5: f32,
    pub enabled: f32,
}

impl Default for OilPaintingSettings {
    fn default() -> Self {
        Self {
            radius: 3.0,
            levels: 8.0,
            _p1: 0.0,
            _p2: 0.0,
            _p3: 0.0,
            _p4: 0.0,
            _p5: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for OilPaintingSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/oil_painting.wgsl".into()
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
        type_id: "oil_painting",
        display_name: "Oil Painting",
        icon: regular::PAINT_BUCKET,
        category: "post_process",
        has_fn: |world, entity| world.get::<OilPaintingSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(OilPaintingSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<OilPaintingSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<OilPaintingSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<OilPaintingSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Radius",
                field_type: FieldType::Float { speed: 0.1, min: 1.0, max: 8.0 },
                get_fn: |world, entity| world.get::<OilPaintingSettings>(entity).map(|s| FieldValue::Float(s.radius)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<OilPaintingSettings>(entity) { s.radius = v; } } },
            },
            FieldDef {
                name: "Levels",
                field_type: FieldType::Float { speed: 0.5, min: 4.0, max: 32.0 },
                get_fn: |world, entity| world.get::<OilPaintingSettings>(entity).map(|s| FieldValue::Float(s.levels)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<OilPaintingSettings>(entity) { s.levels = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct OilPaintingPlugin;

impl Plugin for OilPaintingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<OilPaintingSettings>::default());
        #[cfg(feature = "editor")]
        {
            app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
        }
    }
}
