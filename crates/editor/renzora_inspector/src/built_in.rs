//! Built-in inspector entries for core Bevy components.

use bevy::prelude::*;
use bevy::pbr::Lightmap;
use bevy::light::{EnvironmentMapLight, IrradianceVolume, LightProbe, VolumetricFog, VolumetricLight};
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{
    asset_drop_target, inline_property, search_overlay, toggle_switch, AssetDragPayload,
    EditorCommands, EntityLabelColor, EntityTag, FieldDef, FieldType, FieldValue,
    InspectorEntry, InspectorRegistry, OverlayAction, OverlayEntry,
};
use renzora_material::material_ref::MaterialRef;
use renzora_scripting::ScriptComponent;
use renzora_theme::Theme;

/// Register inspector entries for built-in Bevy components.
pub fn register_built_in_inspectors(registry: &mut InspectorRegistry) {
    registry.register(name_entry());
    registry.register(transform_entry());
    registry.register(visibility_entry());
    registry.register(directional_light_entry());
    registry.register(point_light_entry());
    registry.register(spot_light_entry());
    registry.register(ambient_light_entry());
    registry.register(camera_entry());
    registry.register(camera3d_entry());
    registry.register(mesh3d_entry());
    registry.register(environment_map_light_entry());
    registry.register(light_probe_entry());
    registry.register(irradiance_volume_entry());
    registry.register(lightmap_entry());
    registry.register(volumetric_light_entry());
    registry.register(volumetric_fog_entry());
    registry.register(script_component_entry());
    registry.register(material_entry());
    registry.register(physics_body_entry());
    registry.register(collision_shape_entry());
    registry.register(character_controller_entry());
}

fn name_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "name",
        display_name: "Name",
        icon: regular::TAG,
        category: "transform",
        has_fn: |world, entity| world.get::<Name>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Name",
                field_type: FieldType::String,
                get_fn: |world, entity| {
                    world
                        .get::<Name>(entity)
                        .map(|n| FieldValue::String(n.as_str().to_string()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::String(v) = val {
                        if let Some(mut n) = world.get_mut::<Name>(entity) {
                            *n = Name::new(v);
                        }
                    }
                },
            },
            FieldDef {
                name: "Tag",
                field_type: FieldType::String,
                get_fn: |world, entity| {
                    Some(FieldValue::String(
                        world
                            .get::<EntityTag>(entity)
                            .map(|t| t.tag.clone())
                            .unwrap_or_default(),
                    ))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::String(v) = val {
                        if let Some(mut t) = world.get_mut::<EntityTag>(entity) {
                            t.tag = v;
                        } else {
                            world.entity_mut(entity).insert(EntityTag { tag: v });
                        }
                    }
                },
            },
            FieldDef {
                name: "Label Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    let c = world
                        .get::<EntityLabelColor>(entity)
                        .map(|lc| lc.0)
                        .unwrap_or([220, 222, 228]);
                    Some(FieldValue::Color([
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                    ]))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        let color = [
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8,
                        ];
                        world.entity_mut(entity).insert(EntityLabelColor(color));
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn transform_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "transform",
        display_name: "Transform",
        icon: regular::ARROWS_OUT_CARDINAL,
        category: "transform",
        has_fn: |world, entity| world.get::<Transform>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Position",
                field_type: FieldType::Vec3 { speed: 0.1 },
                get_fn: |world, entity| {
                    world
                        .get::<Transform>(entity)
                        .map(|t| FieldValue::Vec3(t.translation.into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut t) = world.get_mut::<Transform>(entity) {
                            t.translation = Vec3::from(v);
                        }
                    }
                },
            },
            FieldDef {
                name: "Rotation",
                field_type: FieldType::Vec3 { speed: 0.5 },
                get_fn: |world, entity| {
                    world.get::<Transform>(entity).map(|t| {
                        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        FieldValue::Vec3([x.to_degrees(), y.to_degrees(), z.to_degrees()])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut t) = world.get_mut::<Transform>(entity) {
                            t.rotation = Quat::from_euler(
                                EulerRot::XYZ,
                                v[0].to_radians(),
                                v[1].to_radians(),
                                v[2].to_radians(),
                            );
                        }
                    }
                },
            },
            FieldDef {
                name: "Scale",
                field_type: FieldType::Vec3 { speed: 0.01 },
                get_fn: |world, entity| {
                    world
                        .get::<Transform>(entity)
                        .map(|t| FieldValue::Vec3(t.scale.into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut t) = world.get_mut::<Transform>(entity) {
                            t.scale = Vec3::from(v);
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn visibility_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "visibility",
        display_name: "Visibility",
        icon: regular::EYE,
        category: "rendering",
        has_fn: |world, entity| world.get::<Visibility>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Visible",
            field_type: FieldType::Bool,
            get_fn: |world, entity| {
                world.get::<Visibility>(entity).map(|v| {
                    FieldValue::Bool(matches!(*v, Visibility::Inherited | Visibility::Visible))
                })
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Bool(v) = val {
                    if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
                        *vis = if v {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        };
                    }
                }
            },
        }],
        custom_ui_fn: None,
    }
}

fn directional_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "directional_light",
        display_name: "Directional Light",
        icon: regular::SUN,
        category: "lighting",
        has_fn: |world, entity| world.get::<DirectionalLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Illuminance",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 0.0,
                    max: 200_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<DirectionalLight>(entity)
                        .map(|l| FieldValue::Float(l.illuminance))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<DirectionalLight>(entity) {
                            l.illuminance = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<DirectionalLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<DirectionalLight>(entity) {
                            l.color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadows",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<DirectionalLight>(entity)
                        .map(|l| FieldValue::Bool(l.shadows_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<DirectionalLight>(entity) {
                            l.shadows_enabled = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn point_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "point_light",
        display_name: "Point Light",
        icon: regular::LIGHTBULB,
        category: "lighting",
        has_fn: |world, entity| world.get::<PointLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<PointLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 0.0,
                    max: 10_000_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PointLight>(entity)
                        .map(|l| FieldValue::Float(l.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Range",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 1000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PointLight>(entity)
                        .map(|l| FieldValue::Float(l.range))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.range = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadows",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<PointLight>(entity)
                        .map(|l| FieldValue::Bool(l.shadows_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.shadows_enabled = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn spot_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "spot_light",
        display_name: "Spot Light",
        icon: regular::FLASHLIGHT,
        category: "lighting",
        has_fn: |world, entity| world.get::<SpotLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<SpotLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 0.0,
                    max: 10_000_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Range",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 1000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.range))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.range = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Inner Angle",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.57,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.inner_angle))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.inner_angle = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Outer Angle",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.57,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.outer_angle))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.outer_angle = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadows",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Bool(l.shadows_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.shadows_enabled = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn ambient_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ambient_light",
        display_name: "Ambient Light",
        icon: regular::SUN_DIM,
        category: "lighting",
        has_fn: |world, entity| world.get::<AmbientLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<AmbientLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<AmbientLight>(entity) {
                            l.color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Brightness",
                field_type: FieldType::Float {
                    speed: 10.0,
                    min: 0.0,
                    max: 100_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<AmbientLight>(entity)
                        .map(|l| FieldValue::Float(l.brightness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<AmbientLight>(entity) {
                            l.brightness = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn camera_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera",
        display_name: "Camera",
        icon: regular::VIDEO_CAMERA,
        category: "camera",
        has_fn: |world, entity| world.get::<Camera>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Order",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: -100.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Camera>(entity)
                        .map(|c| FieldValue::Float(c.order as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut c) = world.get_mut::<Camera>(entity) {
                            c.order = v as isize;
                        }
                    }
                },
            },
            FieldDef {
                name: "Active",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<Camera>(entity)
                        .map(|c| FieldValue::Bool(c.is_active))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut c) = world.get_mut::<Camera>(entity) {
                            c.is_active = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Output",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    world.get::<Camera>(entity).map(|c| {
                        FieldValue::ReadOnly(format!("{:?}", c.output_mode))
                    })
                },
                set_fn: |_, _, _| {},
            },
        ],
        custom_ui_fn: None,
    }
}

fn camera3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera3d",
        display_name: "Camera 3D",
        icon: regular::APERTURE,
        category: "camera",
        has_fn: |world, entity| world.get::<Camera3d>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Type",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<Camera3d>(entity)
                    .map(|_| FieldValue::ReadOnly("Perspective 3D".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
        custom_ui_fn: None,
    }
}

fn mesh3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "mesh3d",
        display_name: "Mesh 3D",
        icon: regular::CUBE,
        category: "rendering",
        has_fn: |world, entity| world.get::<Mesh3d>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Mesh",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<Mesh3d>(entity)
                    .map(|_| FieldValue::ReadOnly("Mesh attached".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
        custom_ui_fn: None,
    }
}

/// Image extensions accepted for auto-material creation.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"];

fn material_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "material_ref",
        display_name: "Material",
        icon: regular::PAINT_BRUSH,
        category: "rendering",
        has_fn: |world, entity| {
            world.get::<MaterialRef>(entity).is_some()
                || world.get::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>(entity).is_some()
                || world.get::<Mesh3d>(entity).is_some()
        },
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<MaterialRef>();
            world.entity_mut(entity).remove::<renzora_material::resolver::MaterialResolved>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(material_custom_ui),
    }
}

/// Custom UI for the Material section: shows current MaterialRef path
/// and accepts drops of `.material` files or images (auto-creates a material).
fn material_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let current_path = world
        .get::<MaterialRef>(entity)
        .map(|m| m.0.clone())
        .unwrap_or_default();

    let payload = world.get_resource::<AssetDragPayload>();

    // Drop target that accepts .material files AND images
    let mut all_exts: Vec<&str> = vec!["material"];
    all_exts.extend_from_slice(IMAGE_EXTENSIONS);
    let ext_refs: Vec<&str> = all_exts;

    let current_display = if current_path.is_empty() { None } else { Some(current_path.as_str()) };

    let drop_result = asset_drop_target(
        ui,
        egui::Id::new(("material_drop", entity)),
        current_display,
        &ext_refs,
        "Drop material or image",
        theme,
        payload,
    );

    if let Some(ref dropped) = drop_result.dropped_path {
        let ext = dropped
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let dropped_path = dropped.clone();
        if ext == "material" {
            // Direct .material file drop — set MaterialRef + sync editor state
            cmds.push(move |world: &mut World| {
                let mat_path = if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
                    project.make_asset_relative(&dropped_path)
                } else {
                    dropped_path.to_string_lossy().to_string()
                };

                // Load the graph so the shared shader handle gets the right shader
                let fs_path = if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
                    project.resolve_path(&mat_path).to_string_lossy().to_string()
                } else {
                    mat_path.clone()
                };
                if let Ok(json) = std::fs::read_to_string(&fs_path) {
                    if let Ok(graph) = serde_json::from_str::<renzora_material::graph::MaterialGraph>(&json) {
                        let result = renzora_material::codegen::compile(&graph);
                        let mut state = world.resource_mut::<renzora_material_editor::MaterialEditorState>();
                        state.editing_entity = Some(entity);
                        state.compiled_wgsl = Some(result.fragment_shader);
                        state.compile_errors = result.errors;
                        state.graph = graph;
                        state.edit_mode = renzora_material_editor::MaterialEditMode::Existing { path: mat_path.clone(), entity };
                    }
                }

                // Remove old resolved state so resolver picks up the new ref
                world.entity_mut(entity).remove::<renzora_material::resolver::MaterialResolved>();
                if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
                    mr.0 = mat_path;
                } else {
                    world.entity_mut(entity).insert(MaterialRef(mat_path));
                }
            });
        } else if IMAGE_EXTENSIONS.iter().any(|e| ext == *e) {
            // Image drop — auto-create a .material file with texture→output
            cmds.push(move |world: &mut World| {
                let (tex_path, mat_save_dir) = {
                    let project = world.get_resource::<renzora_core::CurrentProject>();
                    let tex = if let Some(p) = project {
                        p.make_asset_relative(&dropped_path)
                    } else {
                        dropped_path.to_string_lossy().to_string()
                    };
                    let dir = project
                        .map(|p| p.path.join("materials"))
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    (tex, dir)
                };

                // Build a simple graph: TextureSample → Surface Output
                let mat_name = dropped_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("material")
                    .to_string();

                let mut graph = renzora_material::graph::MaterialGraph::new(
                    &mat_name,
                    renzora_material::graph::MaterialDomain::Surface,
                );
                let tex_id = graph.add_node("texture/sample", [-200.0, 0.0]);
                if let Some(node) = graph.get_node_mut(tex_id) {
                    node.input_values.insert(
                        "texture".to_string(),
                        renzora_material::graph::PinValue::TexturePath(tex_path),
                    );
                }
                let output_id = graph.output_node().unwrap().id;
                graph.connect(tex_id, "color", output_id, "base_color");

                // Save the .material file
                let _ = std::fs::create_dir_all(&mat_save_dir);
                let mat_file = mat_save_dir.join(format!("{}.material", mat_name));
                if let Ok(json) = serde_json::to_string_pretty(&graph) {
                    let _ = std::fs::write(&mat_file, &json);
                }

                // Compute asset-relative path for the saved .material
                let mat_asset_path = {
                    let project = world.get_resource::<renzora_core::CurrentProject>();
                    if let Some(p) = project {
                        p.make_asset_relative(&mat_file)
                    } else {
                        mat_file.to_string_lossy().to_string()
                    }
                };

                // Sync editor state so the shared shader gets compiled correctly
                let result = renzora_material::codegen::compile(&graph);
                {
                    let mut state = world.resource_mut::<renzora_material_editor::MaterialEditorState>();
                    state.editing_entity = Some(entity);
                    state.compiled_wgsl = Some(result.fragment_shader);
                    state.compile_errors = result.errors;
                    state.graph = graph;
                    state.edit_mode = renzora_material_editor::MaterialEditMode::Existing { path: mat_asset_path.clone(), entity };
                }

                // Assign MaterialRef
                world.entity_mut(entity).remove::<renzora_material::resolver::MaterialResolved>();
                if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
                    mr.0 = mat_asset_path;
                } else {
                    world.entity_mut(entity).insert(MaterialRef(mat_asset_path));
                }
            });
        }
    }

    if drop_result.cleared {
        cmds.push(move |world: &mut World| {
            world.entity_mut(entity).remove::<MaterialRef>();
            world.entity_mut(entity).remove::<renzora_material::resolver::MaterialResolved>();
            world.entity_mut(entity).remove::<MeshMaterial3d<renzora_material::runtime::GraphMaterial>>();
            // Restore default StandardMaterial so the mesh renders again
            let default_mat = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
            world.entity_mut(entity).insert(MeshMaterial3d(default_mat));
        });
    }
}

fn environment_map_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "environment_map_light",
        display_name: "Environment Map",
        icon: regular::GLOBE_HEMISPHERE_EAST,
        category: "lighting",
        has_fn: |world, entity| world.get::<EnvironmentMapLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(EnvironmentMapLight {
                diffuse_map: Handle::default(),
                specular_map: Handle::default(),
                intensity: 1.0,
                rotation: Quat::IDENTITY,
                affects_lightmapped_mesh_diffuse: true,
            });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<EnvironmentMapLight>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<EnvironmentMapLight>(entity)
                        .map(|l| FieldValue::Float(l.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<EnvironmentMapLight>(entity) {
                            l.intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Affects Lightmapped",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<EnvironmentMapLight>(entity)
                        .map(|l| FieldValue::Bool(l.affects_lightmapped_mesh_diffuse))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<EnvironmentMapLight>(entity) {
                            l.affects_lightmapped_mesh_diffuse = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Rotation",
                field_type: FieldType::Vec3 { speed: 0.5 },
                get_fn: |world, entity| {
                    world.get::<EnvironmentMapLight>(entity).map(|l| {
                        let (x, y, z) = l.rotation.to_euler(EulerRot::XYZ);
                        FieldValue::Vec3([x.to_degrees(), y.to_degrees(), z.to_degrees()])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut l) = world.get_mut::<EnvironmentMapLight>(entity) {
                            l.rotation = Quat::from_euler(
                                EulerRot::XYZ,
                                v[0].to_radians(),
                                v[1].to_radians(),
                                v[2].to_radians(),
                            );
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn light_probe_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "light_probe",
        display_name: "Light Probe",
        icon: regular::BROADCAST,
        category: "lighting",
        has_fn: |world, entity| world.get::<LightProbe>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(LightProbe);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<LightProbe>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Info",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<LightProbe>(entity)
                    .map(|_| FieldValue::ReadOnly("Marker — add EnvironmentMapLight or IrradianceVolume".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
        custom_ui_fn: None,
    }
}

fn irradiance_volume_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "irradiance_volume",
        display_name: "Irradiance Volume",
        icon: regular::CUBE_TRANSPARENT,
        category: "lighting",
        has_fn: |world, entity| world.get::<IrradianceVolume>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(IrradianceVolume {
                voxels: Handle::default(),
                intensity: 1.0,
                affects_lightmapped_meshes: true,
            });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<IrradianceVolume>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<IrradianceVolume>(entity)
                        .map(|v| FieldValue::Float(v.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(f) = val {
                        if let Some(mut v) = world.get_mut::<IrradianceVolume>(entity) {
                            v.intensity = f;
                        }
                    }
                },
            },
            FieldDef {
                name: "Affects Lightmapped",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<IrradianceVolume>(entity)
                        .map(|v| FieldValue::Bool(v.affects_lightmapped_meshes))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut v) = world.get_mut::<IrradianceVolume>(entity) {
                            v.affects_lightmapped_meshes = b;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn lightmap_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "lightmap",
        display_name: "Lightmap",
        icon: regular::IMAGE,
        category: "lighting",
        has_fn: |world, entity| world.get::<Lightmap>(entity).is_some(),
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Lightmap>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "UV Min",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    world.get::<Lightmap>(entity).map(|l| {
                        FieldValue::ReadOnly(format!("({:.2}, {:.2})", l.uv_rect.min.x, l.uv_rect.min.y))
                    })
                },
                set_fn: |_, _, _| {},
            },
            FieldDef {
                name: "UV Max",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    world.get::<Lightmap>(entity).map(|l| {
                        FieldValue::ReadOnly(format!("({:.2}, {:.2})", l.uv_rect.max.x, l.uv_rect.max.y))
                    })
                },
                set_fn: |_, _, _| {},
            },
        ],
        custom_ui_fn: None,
    }
}

fn volumetric_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_light",
        display_name: "Volumetric Light",
        icon: regular::SUN_HORIZON,
        category: "lighting",
        has_fn: |world, entity| world.get::<VolumetricLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(VolumetricLight);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<VolumetricLight>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Info",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<VolumetricLight>(entity)
                    .map(|_| FieldValue::ReadOnly("Enables god rays (requires VolumetricFog on camera)".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
        custom_ui_fn: None,
    }
}

fn script_component_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "script_component",
        display_name: "Scripts",
        icon: regular::CODE,
        category: "scripting",
        has_fn: |world, entity| world.get::<ScriptComponent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(ScriptComponent::new());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<ScriptComponent>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(script_component_ui),
    }
}

/// Recursively scan a directory for script files (.lua, .rhai).
/// Returns (display_name, absolute_path) pairs.
fn scan_script_files(root: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let mut results = Vec::new();
    scan_script_files_inner(root, root, &mut results);
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

fn scan_script_files_inner(
    dir: &std::path::Path,
    root: &std::path::Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.starts_with('.')) {
            continue;
        }
        if path.is_dir() {
            // Skip common non-script directories
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "target" | "node_modules" | ".git") {
                continue;
            }
            scan_script_files_inner(&path, root, out);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "lua" | "rhai") {
                // Display name = relative path from project root
                let display = path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                out.push((display, path));
            }
        }
    }
}

/// Convert an absolute path to project-relative, falling back to the original path.
fn make_relative(abs: std::path::PathBuf, world: &World) -> std::path::PathBuf {
    if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
        if let Ok(rel) = abs.strip_prefix(&project.path) {
            return rel.to_path_buf();
        }
    }
    abs
}

fn format_script_value(value: &renzora_scripting::ScriptValue) -> String {
    use renzora_scripting::ScriptValue;
    match value {
        ScriptValue::Float(v) => format!("{:.3}", v),
        ScriptValue::Int(v) => format!("{}", v),
        ScriptValue::Bool(v) => format!("{}", v),
        ScriptValue::String(v) => v.clone(),
        ScriptValue::Entity(v) => v.clone(),
        ScriptValue::Vec2(v) => format!("({:.2}, {:.2})", v.x, v.y),
        ScriptValue::Vec3(v) => format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z),
        ScriptValue::Color(v) => format!("({:.2}, {:.2}, {:.2}, {:.2})", v.x, v.y, v.z, v.w),
    }
}

fn to_display_name(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn script_component_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(sc) = world.get::<ScriptComponent>(entity) else {
        return;
    };
    let scripts = sc.scripts.clone();
    let payload = world.get_resource::<AssetDragPayload>();

    if scripts.is_empty() {
        // Empty state — show a drop zone to add the first script
        ui.add_space(4.0);
        let drop = asset_drop_target(
            ui,
            ui.id().with("script_drop_empty"),
            None,
            &["lua", "rhai"],
            "Drop a script file here",
            theme,
            payload,
        );
        if let Some(path) = drop.dropped_path {
            let rel = make_relative(path, world);
            cmds.push(move |w: &mut World| {
                if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                    sc.add_file_script(rel);
                }
            });
        }
        ui.add_space(4.0);
    } else {
        // Render each script entry
        let mut remove_index: Option<usize> = None;
        let mut toggle_index: Option<usize> = None;

        for (i, entry) in scripts.iter().enumerate() {
            let _script_name = if let Some(ref path) = entry.script_path {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            } else if !entry.script_id.is_empty() {
                entry.script_id.clone()
            } else {
                format!("Script #{}", entry.id)
            };

            let row_idx = i * 3; // spacing for alternating rows

            // Script path drop target
            let current_path = entry.script_path.as_ref().map(|p| p.to_string_lossy().to_string());
            let current_str = current_path.as_deref();

            inline_property(ui, row_idx, "Script", theme, |ui| {
                let drop = asset_drop_target(
                    ui,
                    ui.id().with(("script_path", i)),
                    current_str,
                    &["lua", "rhai"],
                    "Drop script file",
                    theme,
                    payload,
                );
                if let Some(path) = drop.dropped_path {
                    let idx = i;
                    let rel = make_relative(path, world);
                    cmds.push(move |w: &mut World| {
                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                            if let Some(entry) = sc.scripts.get_mut(idx) {
                                entry.script_path = Some(rel);
                            }
                        }
                    });
                }
                if drop.cleared {
                    let idx = i;
                    cmds.push(move |w: &mut World| {
                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                            if let Some(entry) = sc.scripts.get_mut(idx) {
                                entry.script_path = None;
                            }
                        }
                    });
                }
            });

            // Enabled toggle
            inline_property(ui, row_idx + 1, "Enabled", theme, |ui| {
                let id = ui.id().with(("script_enabled", i));
                if toggle_switch(ui, id, entry.enabled) {
                    toggle_index = Some(i);
                }
            });

            // Script variables (props) — editable
            if !entry.variables.iter_all().next().is_none() {
                use renzora_scripting::ScriptValue;
                ui.add_space(2.0);
                for (var_name, var_value) in entry.variables.iter_all() {
                    let label = to_display_name(var_name);
                    let script_idx = i;
                    let vname = var_name.clone();
                    match var_value.clone() {
                        ScriptValue::Float(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                ui.add(egui::DragValue::new(&mut v).speed(0.1));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Float(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Int(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                ui.add(egui::DragValue::new(&mut v).speed(1.0));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Int(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Bool(v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let id = ui.id().with(("script_var_bool", script_idx, &vname));
                                if toggle_switch(ui, id, v) {
                                    let new_val = !v;
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Bool(new_val));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::String(mut s) | ScriptValue::Entity(mut s) => {
                            let is_entity = matches!(var_value, ScriptValue::Entity(_));
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = s.clone();
                                ui.add(
                                    egui::TextEdit::singleline(&mut s)
                                        .desired_width(ui.available_width()),
                                );
                                if s != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                let val = if is_entity {
                                                    ScriptValue::Entity(s)
                                                } else {
                                                    ScriptValue::String(s)
                                                };
                                                entry.variables.set(vname, val);
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Vec2(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                let w = ((ui.available_width() - 32.0) / 2.0).max(30.0);
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.label(egui::RichText::new("X").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.x).speed(0.1));
                                ui.label(egui::RichText::new("Y").size(10.0).color(egui::Color32::from_rgb(130, 200, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.y).speed(0.1));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Vec2(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Vec3(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                let w = ((ui.available_width() - 48.0) / 3.0).max(30.0);
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.label(egui::RichText::new("X").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.x).speed(0.1));
                                ui.label(egui::RichText::new("Y").size(10.0).color(egui::Color32::from_rgb(130, 200, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.y).speed(0.1));
                                ui.label(egui::RichText::new("Z").size(10.0).color(egui::Color32::from_rgb(90, 150, 230)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.z).speed(0.1));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Vec3(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Color(mut c) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = c;
                                let mut rgb = [c.x, c.y, c.z];
                                if ui.color_edit_button_rgb(&mut rgb).changed() {
                                    c.x = rgb[0];
                                    c.y = rgb[1];
                                    c.z = rgb[2];
                                }
                                if c != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Color(c));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    }
                }
                ui.add_space(2.0);
            }

            // Remove button
            inline_property(ui, row_idx + 2, "", theme, |ui| {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(format!("{} Remove", regular::TRASH))
                            .size(11.0)
                            .color(theme.semantic.error.to_color32()),
                    ))
                    .clicked()
                {
                    remove_index = Some(i);
                }
            });

            // Separator between script entries
            if i < scripts.len() - 1 {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
            }
        }

        // Apply deferred actions
        if let Some(idx) = toggle_index {
            let new_enabled = !scripts[idx].enabled;
            cmds.push(move |w: &mut World| {
                if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                    if let Some(entry) = sc.scripts.get_mut(idx) {
                        entry.enabled = new_enabled;
                    }
                }
            });
        }
        if let Some(idx) = remove_index {
            cmds.push(move |w: &mut World| {
                if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                    sc.remove_script(idx);
                }
            });
        }
    }

    // Add Script button — opens search overlay
    let overlay_id = egui::Id::new("script_search_overlay_visible");
    let show_overlay = ui.ctx().data_mut(|d| {
        *d.get_persisted_mut_or_insert_with(overlay_id, || false)
    });

    if show_overlay {
        // Scan project directory recursively for script files
        let available = world
            .get_resource::<renzora_core::CurrentProject>()
            .map(|p| scan_script_files(&p.path))
            .unwrap_or_default();

        // We need owned strings for the overlay entries — store them so refs stay valid
        let entry_data: Vec<(String, String, String, std::path::PathBuf)> = available
            .iter()
            .map(|(name, path)| {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                let icon = match ext {
                    "lua" => regular::CODE,
                    "rhai" => regular::CODE,
                    _ => regular::FILE,
                };
                let category = ext.to_string();
                (name.clone(), icon.to_string(), category, path.clone())
            })
            .collect();

        let entries: Vec<OverlayEntry> = entry_data
            .iter()
            .map(|(name, icon, cat, _path)| OverlayEntry {
                id: name.as_str(),
                label: name.as_str(),
                icon: icon.as_str(),
                category: if cat.is_empty() { "scripts" } else { cat.as_str() },
            })
            .collect();

        let search_id = egui::Id::new("script_search_text");
        let mut search_text: String = ui.ctx().data_mut(|d| {
            d.get_persisted_mut_or_insert_with(search_id, String::new).clone()
        });

        let ctx = ui.ctx().clone();
        match search_overlay(&ctx, "add_script_overlay", "Add Script", &entries, &mut search_text, theme) {
            OverlayAction::Selected(name) => {
                // Find the path for this script — id is the display name
                if let Some((_, _, _, path)) = entry_data.iter().find(|(n, _, _, _)| *n == name) {
                    let rel = make_relative(path.clone(), world);
                    cmds.push(move |w: &mut World| {
                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                            sc.add_file_script(rel);
                        }
                    });
                }
                ui.ctx().data_mut(|d| d.insert_persisted(overlay_id, false));
                ui.ctx().data_mut(|d| d.insert_persisted(search_id, String::new()));
            }
            OverlayAction::Closed => {
                ui.ctx().data_mut(|d| d.insert_persisted(overlay_id, false));
                ui.ctx().data_mut(|d| d.insert_persisted(search_id, String::new()));
            }
            OverlayAction::None => {
                // Persist search text changes
                ui.ctx().data_mut(|d| d.insert_persisted(search_id, search_text));
            }
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} Add Script", regular::PLUS))
                    .size(11.0),
            ))
            .clicked()
        {
            ui.ctx().data_mut(|d| d.insert_persisted(overlay_id, true));
        }
    });

    // Drop zone at the bottom to add new scripts
    ui.add_space(4.0);
    let drop = asset_drop_target(
        ui,
        ui.id().with("script_drop_add"),
        None,
        &["lua", "rhai"],
        "Drop to add script",
        theme,
        payload,
    );
    if let Some(path) = drop.dropped_path {
        let rel = make_relative(path, world);
        cmds.push(move |w: &mut World| {
            if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                sc.add_file_script(rel);
            }
        });
    }
    ui.add_space(4.0);
}

fn volumetric_fog_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_fog",
        display_name: "Volumetric Fog",
        icon: regular::CLOUD_FOG,
        category: "lighting",
        has_fn: |world, entity| world.get::<VolumetricFog>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(VolumetricFog::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<VolumetricFog>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Ambient Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<VolumetricFog>(entity).map(|f| {
                        let c = f.ambient_color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.ambient_color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Ambient Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<VolumetricFog>(entity)
                        .map(|f| FieldValue::Float(f.ambient_intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.ambient_intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Step Count",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 1.0,
                    max: 256.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<VolumetricFog>(entity)
                        .map(|f| FieldValue::Float(f.step_count as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.step_count = v as u32;
                        }
                    }
                },
            },
            FieldDef {
                name: "Jitter",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<VolumetricFog>(entity)
                        .map(|f| FieldValue::Float(f.jitter))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.jitter = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

// ── Physics Body ────────────────────────────────────────────────────────────

fn physics_body_entry() -> InspectorEntry {
    use renzora_physics::PhysicsBodyData;

    InspectorEntry {
        type_id: "physics_body",
        display_name: "Physics Body",
        icon: regular::CUBE,
        category: "physics",
        has_fn: |world, entity| world.get::<PhysicsBodyData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PhysicsBodyData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PhysicsBodyData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(physics_body_ui),
    }
}

fn physics_body_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    use renzora_physics::{PhysicsBodyData, PhysicsBodyType};

    let Some(body) = world.get::<PhysicsBodyData>(entity) else { return };
    let body = body.clone();

    // Body type combo
    inline_property(ui, 0, "Body Type", theme, |ui| {
        let current = match body.body_type {
            PhysicsBodyType::RigidBody => "Rigid Body",
            PhysicsBodyType::StaticBody => "Static Body",
            PhysicsBodyType::KinematicBody => "Kinematic Body",
        };
        egui::ComboBox::from_id_salt("physics_body_type")
            .selected_text(current)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (bt, label) in [
                    (PhysicsBodyType::RigidBody, "Rigid Body"),
                    (PhysicsBodyType::StaticBody, "Static Body"),
                    (PhysicsBodyType::KinematicBody, "Kinematic Body"),
                ] {
                    if ui.selectable_label(body.body_type == bt, label).clicked() {
                        cmds.push(move |w: &mut World| {
                            if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                                b.body_type = bt;
                            }
                        });
                    }
                }
            });
    });

    // Mass
    inline_property(ui, 1, "Mass", theme, |ui| {
        let mut v = body.mass;
        if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.001..=f32::MAX)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.mass = v; }
            });
        }
    });

    // Gravity Scale
    inline_property(ui, 0, "Gravity Scale", theme, |ui| {
        let mut v = body.gravity_scale;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(-10.0..=10.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.gravity_scale = v; }
            });
        }
    });

    // Linear Damping
    inline_property(ui, 1, "Linear Damping", theme, |ui| {
        let mut v = body.linear_damping;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=100.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.linear_damping = v; }
            });
        }
    });

    // Angular Damping
    inline_property(ui, 0, "Angular Damping", theme, |ui| {
        let mut v = body.angular_damping;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=100.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.angular_damping = v; }
            });
        }
    });

    // Lock axes
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Lock Axes").size(11.0).color(theme.text.secondary.to_color32()));

    inline_property(ui, 1, "Translation", theme, |ui| {
        ui.horizontal(|ui| {
            let locks = [
                ("X", body.lock_translation_x),
                ("Y", body.lock_translation_y),
                ("Z", body.lock_translation_z),
            ];
            for (i, (label, current)) in locks.iter().enumerate() {
                let mut checked = *current;
                if ui.checkbox(&mut checked, *label).changed() {
                    let val = checked;
                    let axis = i;
                    cmds.push(move |w: &mut World| {
                        if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                            match axis {
                                0 => b.lock_translation_x = val,
                                1 => b.lock_translation_y = val,
                                _ => b.lock_translation_z = val,
                            }
                        }
                    });
                }
            }
        });
    });

    inline_property(ui, 0, "Rotation", theme, |ui| {
        ui.horizontal(|ui| {
            let locks = [
                ("X", body.lock_rotation_x),
                ("Y", body.lock_rotation_y),
                ("Z", body.lock_rotation_z),
            ];
            for (i, (label, current)) in locks.iter().enumerate() {
                let mut checked = *current;
                if ui.checkbox(&mut checked, *label).changed() {
                    let val = checked;
                    let axis = i;
                    cmds.push(move |w: &mut World| {
                        if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                            match axis {
                                0 => b.lock_rotation_x = val,
                                1 => b.lock_rotation_y = val,
                                _ => b.lock_rotation_z = val,
                            }
                        }
                    });
                }
            }
        });
    });
}

// ── Collision Shape ─────────────────────────────────────────────────────────

fn collision_shape_entry() -> InspectorEntry {
    use renzora_physics::CollisionShapeData;

    InspectorEntry {
        type_id: "collision_shape",
        display_name: "Collision Shape",
        icon: regular::BOUNDING_BOX,
        category: "physics",
        has_fn: |world, entity| world.get::<CollisionShapeData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CollisionShapeData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CollisionShapeData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(collision_shape_ui),
    }
}

fn collision_shape_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    use renzora_physics::{CollisionShapeData, CollisionShapeType};

    let Some(shape) = world.get::<CollisionShapeData>(entity) else { return };
    let shape = shape.clone();

    // Shape type combo
    inline_property(ui, 0, "Shape", theme, |ui| {
        let current = match shape.shape_type {
            CollisionShapeType::Box => "Box",
            CollisionShapeType::Sphere => "Sphere",
            CollisionShapeType::Capsule => "Capsule",
            CollisionShapeType::Cylinder => "Cylinder",
        };
        egui::ComboBox::from_id_salt("collision_shape_type")
            .selected_text(current)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (st, label) in [
                    (CollisionShapeType::Box, "Box"),
                    (CollisionShapeType::Sphere, "Sphere"),
                    (CollisionShapeType::Capsule, "Capsule"),
                    (CollisionShapeType::Cylinder, "Cylinder"),
                ] {
                    if ui.selectable_label(shape.shape_type == st, label).clicked() {
                        cmds.push(move |w: &mut World| {
                            if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                                s.shape_type = st;
                            }
                        });
                    }
                }
            });
    });

    // Shape-specific parameters
    match shape.shape_type {
        CollisionShapeType::Box => {
            inline_property(ui, 1, "Half Extents", theme, |ui| {
                let mut v = [shape.half_extents.x, shape.half_extents.y, shape.half_extents.z];
                let mut changed = false;
                ui.horizontal(|ui| {
                    for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                        ui.label(egui::RichText::new(*label).size(10.0).color(theme.text.muted.to_color32()));
                        if ui.add(egui::DragValue::new(&mut v[i]).speed(0.01).range(0.001..=f32::MAX)).changed() {
                            changed = true;
                        }
                    }
                });
                if changed {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                            s.half_extents = Vec3::new(v[0], v[1], v[2]);
                        }
                    });
                }
            });
        }
        CollisionShapeType::Sphere => {
            inline_property(ui, 1, "Radius", theme, |ui| {
                let mut v = shape.radius;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.radius = v; }
                    });
                }
            });
        }
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            inline_property(ui, 1, "Radius", theme, |ui| {
                let mut v = shape.radius;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.radius = v; }
                    });
                }
            });
            inline_property(ui, 0, "Half Height", theme, |ui| {
                let mut v = shape.half_height;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.half_height = v; }
                    });
                }
            });
        }
    }

    // Offset
    inline_property(ui, 1, "Offset", theme, |ui| {
        let mut v = [shape.offset.x, shape.offset.y, shape.offset.z];
        let mut changed = false;
        ui.horizontal(|ui| {
            for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                ui.label(egui::RichText::new(*label).size(10.0).color(theme.text.muted.to_color32()));
                if ui.add(egui::DragValue::new(&mut v[i]).speed(0.01)).changed() {
                    changed = true;
                }
            }
        });
        if changed {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                    s.offset = Vec3::new(v[0], v[1], v[2]);
                }
            });
        }
    });

    // Friction
    inline_property(ui, 0, "Friction", theme, |ui| {
        let mut v = shape.friction;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=2.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.friction = v; }
            });
        }
    });

    // Restitution
    inline_property(ui, 1, "Restitution", theme, |ui| {
        let mut v = shape.restitution;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=2.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.restitution = v; }
            });
        }
    });

    // Is Sensor
    inline_property(ui, 0, "Is Sensor", theme, |ui| {
        let id = ui.id().with("collision_is_sensor");
        if toggle_switch(ui, id, shape.is_sensor) {
            let val = !shape.is_sensor;
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.is_sensor = val; }
            });
        }
    });
}

// ── Character Controller ────────────────────────────────────────────────────

fn character_controller_entry() -> InspectorEntry {
    use renzora_physics::CharacterControllerData;

    InspectorEntry {
        type_id: "character_controller",
        display_name: "Character Controller",
        icon: regular::PERSON_SIMPLE_RUN,
        category: "physics",
        has_fn: |world, entity| world.get::<CharacterControllerData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CharacterControllerData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CharacterControllerData>();
            world.entity_mut(entity).remove::<renzora_physics::CharacterControllerState>();
            world.entity_mut(entity).remove::<renzora_physics::CharacterControllerInput>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(character_controller_ui),
    }
}

fn character_controller_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    use renzora_physics::CharacterControllerData;

    let Some(cc) = world.get::<CharacterControllerData>(entity) else { return };
    let cc = cc.clone();

    // Auto Input toggle
    inline_property(ui, 0, "Auto Input", theme, |ui| {
        let id = ui.id().with("cc_auto_input");
        if toggle_switch(ui, id, cc.auto_input) {
            let val = !cc.auto_input;
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.auto_input = val; }
            });
        }
    });

    // Action names (only shown when auto_input is on)
    if cc.auto_input {
        inline_property(ui, 1, "Move Action", theme, |ui| {
            let mut v = cc.move_action.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(100.0)).changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.move_action = v; }
                });
            }
        });
        inline_property(ui, 0, "Jump Action", theme, |ui| {
            let mut v = cc.jump_action.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(100.0)).changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.jump_action = v; }
                });
            }
        });
        inline_property(ui, 1, "Sprint Action", theme, |ui| {
            let mut v = cc.sprint_action.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(100.0)).changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.sprint_action = v; }
                });
            }
        });
    }

    // Move Speed
    inline_property(ui, 0, "Move Speed", theme, |ui| {
        let mut v = cc.move_speed;
        if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.0..=100.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.move_speed = v; }
            });
        }
    });

    // Sprint Multiplier
    inline_property(ui, 1, "Sprint Multiplier", theme, |ui| {
        let mut v = cc.sprint_multiplier;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(1.0..=5.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.sprint_multiplier = v; }
            });
        }
    });

    // Jump Force
    inline_property(ui, 0, "Jump Force", theme, |ui| {
        let mut v = cc.jump_force;
        if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.0..=50.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.jump_force = v; }
            });
        }
    });

    // Gravity Scale
    inline_property(ui, 1, "Gravity Scale", theme, |ui| {
        let mut v = cc.gravity_scale;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(-10.0..=10.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.gravity_scale = v; }
            });
        }
    });

    // Max Slope Angle
    inline_property(ui, 0, "Max Slope Angle", theme, |ui| {
        let mut v = cc.max_slope_angle;
        if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(0.0..=90.0).suffix("°")).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.max_slope_angle = v; }
            });
        }
    });

    // Ground Distance
    inline_property(ui, 1, "Ground Distance", theme, |ui| {
        let mut v = cc.ground_distance;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.01..=1.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.ground_distance = v; }
            });
        }
    });

    // Air Control
    inline_property(ui, 0, "Air Control", theme, |ui| {
        let mut v = cc.air_control;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(0.0..=1.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.air_control = v; }
            });
        }
    });

    // Coyote Time
    inline_property(ui, 1, "Coyote Time", theme, |ui| {
        let mut v = cc.coyote_time;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0).suffix("s")).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.coyote_time = v; }
            });
        }
    });

    // Jump Buffer Time
    inline_property(ui, 0, "Jump Buffer", theme, |ui| {
        let mut v = cc.jump_buffer_time;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0).suffix("s")).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.jump_buffer_time = v; }
            });
        }
    });

    // Runtime state (read-only, shown when playing)
    if let Some(state) = world.get::<renzora_physics::CharacterControllerState>(entity) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Runtime State").size(11.0).color(theme.text.muted.to_color32()));
        inline_property(ui, 1, "Grounded", theme, |ui| {
            ui.label(if state.is_grounded { "Yes" } else { "No" });
        });
        inline_property(ui, 0, "Velocity", theme, |ui| {
            ui.label(format!("{:.1}, {:.1}, {:.1}", state.velocity.x, state.velocity.y, state.velocity.z));
        });
        inline_property(ui, 1, "Speed", theme, |ui| {
            let speed = Vec3::new(state.velocity.x, 0.0, state.velocity.z).length();
            ui.label(format!("{:.1}", speed));
        });
    }
}

