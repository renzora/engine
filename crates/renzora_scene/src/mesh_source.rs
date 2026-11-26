//! The Inspector's Mesh section: swap what an entity's mesh comes from, a
//! library shape or an imported model, in place.
//!
//! Two rows. **Shape** lists every shape in the project's [`ShapeRegistry`];
//! picking one makes the entity that shape. **Model** takes a model file
//! dragged from the Assets panel; dropping one makes the entity that model,
//! and clearing it makes it a cube again. Either way the entity keeps its
//! name, transform, scripts, physics and place in the hierarchy.
//!
//! The work is `renzora_engine::scene_io::set_mesh_source`, which swaps the
//! description and lets the ordinary rehydration build the new mesh, so a
//! swapped entity is built exactly as a reopened scene would build it.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use renzora_editor_api::{FieldValue, InspectorEntry};
use renzora_ember::font::EmberFonts;
use renzora_ember::inspector::inspector_row;
use renzora_ember::reactive::bind_2way;
use renzora_ember::widgets::dropdown;
use renzora_engine::scene_io::{mesh_source, set_mesh_source, MeshSource};
use renzora_log_api::console_warn;
use renzora_scene_api::{MeshInstanceData, MeshPrimitive, ShapeRegistry};

/// Model files the Model slot accepts: what the importer writes and what the
/// model loader reads.
const MODEL_EXTENSIONS: [&str; 3] = ["glb", "gltf", "rvg"];

/// The shape an entity becomes when its model is cleared.
const DEFAULT_SHAPE: &str = "cube";

pub(crate) fn register(app: &mut App) {
    use renzora_editor_framework::AppEditorExt;
    app.register_inspector(mesh_entry())
        .register_native_inspector_ui("mesh_source", mesh_source_ui);
}

/// Shown on any shape or model. Not addable or removable on its own: an entity
/// gets a mesh from the Shape Library or by dropping a model, and swapping is
/// what this section is for.
fn mesh_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "mesh_source",
        display_name: "Mesh",
        icon: "cube",
        category: "rendering",
        has_fn: |world, entity| {
            world.get::<MeshPrimitive>(entity).is_some() || world.get::<MeshInstanceData>(entity).is_some()
        },
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

/// Swap, saying why in the Console when it cannot be done.
fn swap(world: &mut World, entity: Entity, source: MeshSource) {
    if let Err(why) = set_mesh_source(world, entity, source) {
        console_warn("Mesh", format!("Could not change the mesh: {why}"));
    }
}

/// The registry's shapes as (id, display name), by name.
fn shapes(world: &World) -> Vec<(String, String)> {
    let mut list: Vec<(String, String)> = world
        .get_resource::<ShapeRegistry>()
        .map(|r| r.iter().map(|e| (e.id.to_string(), e.name.to_string())).collect())
        .unwrap_or_default();
    list.sort_by_key(|(_, name)| name.to_lowercase());
    list
}

fn model_get(world: &World, entity: Entity) -> Option<FieldValue> {
    Some(FieldValue::Asset(match mesh_source(world, entity) {
        Some(MeshSource::Model(path)) => Some(path),
        _ => None,
    }))
}

fn model_set(world: &mut World, entity: Entity, value: FieldValue) {
    let FieldValue::Asset(path) = value else { return };
    let source = match path.filter(|p| !p.is_empty()) {
        Some(path) => MeshSource::Model(path),
        // Cleared: back to a shape, unless it already is one.
        None => match mesh_source(world, entity) {
            Some(MeshSource::Shape(_)) => return,
            _ => MeshSource::Shape(DEFAULT_SHAPE.into()),
        },
    };
    swap(world, entity, source);
}

fn mesh_source_ui(world: &mut World, entity: Entity) -> Entity {
    let fonts = world.get_resource::<EmberFonts>().cloned();
    let shape_list = shapes(world);
    // Entry 0 stands for "not a shape": what the dropdown shows on a model.
    let mut names: Vec<String> = vec!["\u{2015}".into()];
    names.extend(shape_list.iter().map(|(_, n)| n.clone()));
    let ids: Vec<String> = shape_list.iter().map(|(id, _)| id.clone()).collect();
    let index_of = {
        let ids = ids.clone();
        move |w: &World| match mesh_source(w, entity) {
            Some(MeshSource::Shape(id)) => ids.iter().position(|i| *i == id).map_or(0, |i| i + 1),
            _ => 0,
        }
    };
    let selected = index_of(world);

    let mut queue = CommandQueue::default();
    let root = {
        let mut commands = Commands::new(&mut queue, world);
        let root = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                Name::new("mesh-source-root"),
            ))
            .id();
        if let Some(fonts) = fonts {
            let labels: Vec<&str> = names.iter().map(String::as_str).collect();
            let dd = dropdown(&mut commands, &fonts, &labels, selected);
            bind_2way::<usize, _, _>(
                &mut commands,
                dd,
                move |w| index_of(w),
                move |w, &i| {
                    // 0 is the model placeholder; choosing it changes nothing.
                    let Some(id) = i.checked_sub(1).and_then(|i| ids.get(i)) else { return };
                    if mesh_source(w, entity) != Some(MeshSource::Shape(id.clone())) {
                        swap(w, entity, MeshSource::Shape(id.clone()));
                    }
                },
            );
            let shape_row = inspector_row(&mut commands, &fonts.ui, &renzora_lang_api::t("field.shape"), dd);

            let field = renzora_inspector::asset_drop_field(
                &mut commands,
                &fonts,
                entity,
                model_get,
                model_set,
                MODEL_EXTENSIONS.iter().map(|e| e.to_string()).collect(),
            );
            let model_row = inspector_row(&mut commands, &fonts.ui, &renzora_lang_api::t("inspector.kind.model"), field);
            commands.entity(root).add_children(&[shape_row, model_row]);
        }
        root
    };
    queue.apply(world);
    root
}
