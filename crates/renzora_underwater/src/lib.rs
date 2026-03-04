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
pub struct UnderwaterSettings {
    pub distortion: f32,
    pub tint_r: f32,
    pub tint_g: f32,
    pub tint_b: f32,
    pub tint_strength: f32,
    pub wave_speed: f32,
    pub wave_scale: f32,
    pub time: f32,
    pub enabled: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl Default for UnderwaterSettings {
    fn default() -> Self {
        Self {
            distortion: 0.02,
            tint_r: 0.0,
            tint_g: 0.3,
            tint_b: 0.5,
            tint_strength: 0.3,
            wave_speed: 1.0,
            wave_scale: 10.0,
            time: 0.0,
            enabled: 1.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        }
    }
}

impl PostProcessEffect for UnderwaterSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/underwater.wgsl".into()
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

fn sync_time(time: Res<Time>, mut query: Query<&mut UnderwaterSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "underwater",
        display_name: "Underwater",
        icon: regular::WAVES,
        category: "post_process",
        has_fn: |world, entity| world.get::<UnderwaterSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(UnderwaterSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<UnderwaterSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<UnderwaterSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<UnderwaterSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Distortion",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<UnderwaterSettings>(entity).map(|s| FieldValue::Float(s.distortion)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<UnderwaterSettings>(entity) { s.distortion = v; } } },
            },
            FieldDef {
                name: "Tint Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| world.get::<UnderwaterSettings>(entity).map(|s| FieldValue::Color([s.tint_r, s.tint_g, s.tint_b])),
                set_fn: |world, entity, val| { if let FieldValue::Color([r, g, b]) = val { if let Some(mut s) = world.get_mut::<UnderwaterSettings>(entity) { s.tint_r = r; s.tint_g = g; s.tint_b = b; } } },
            },
            FieldDef {
                name: "Tint Strength",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<UnderwaterSettings>(entity).map(|s| FieldValue::Float(s.tint_strength)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<UnderwaterSettings>(entity) { s.tint_strength = v; } } },
            },
            FieldDef {
                name: "Wave Speed",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 10.0 },
                get_fn: |world, entity| world.get::<UnderwaterSettings>(entity).map(|s| FieldValue::Float(s.wave_speed)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<UnderwaterSettings>(entity) { s.wave_speed = v; } } },
            },
            FieldDef {
                name: "Wave Scale",
                field_type: FieldType::Float { speed: 0.1, min: 0.1, max: 50.0 },
                get_fn: |world, entity| world.get::<UnderwaterSettings>(entity).map(|s| FieldValue::Float(s.wave_scale)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<UnderwaterSettings>(entity) { s.wave_scale = v; } } },
            },
        ],
    }
}

pub struct UnderwaterPlugin;

impl Plugin for UnderwaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<UnderwaterSettings>::default(),
        );
        app.add_systems(Update, sync_time);
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
