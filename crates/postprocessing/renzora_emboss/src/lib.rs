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
pub struct EmbossSettings {
    pub strength: f32,
    pub mix_amount: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub _p3: f32,
    pub _p4: f32,
    pub _p5: f32,
    pub enabled: f32,
}

impl Default for EmbossSettings {
    fn default() -> Self {
        Self {
            strength: 1.0,
            mix_amount: 0.5,
            _p1: 0.0,
            _p2: 0.0,
            _p3: 0.0,
            _p4: 0.0,
            _p5: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for EmbossSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/emboss.wgsl".into()
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
        type_id: "emboss",
        display_name: "Emboss",
        icon: regular::CUBE,
        category: "post_process",
        has_fn: |world, entity| world.get::<EmbossSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(EmbossSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<EmbossSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<EmbossSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<EmbossSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Strength",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 3.0 },
                get_fn: |world, entity| world.get::<EmbossSettings>(entity).map(|s| FieldValue::Float(s.strength)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<EmbossSettings>(entity) { s.strength = v; } } },
            },
            FieldDef {
                name: "Mix",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<EmbossSettings>(entity).map(|s| FieldValue::Float(s.mix_amount)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<EmbossSettings>(entity) { s.mix_amount = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct EmbossPlugin;

impl Plugin for EmbossPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<EmbossSettings>::default());
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
