//! UI nodes using Bevy's built-in UI system

use bevy::prelude::*;
use std::collections::HashMap;

use crate::core::{EditorEntity, SceneNode};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::definition::{NodeCategory, NodeDefinition};
use crate::shared::{UIPanelData, UILabelData, UIButtonData, UIImageData};

/// UIPanel - a container node for UI elements
pub static UI_PANEL: NodeDefinition = NodeDefinition {
    type_id: "ui.panel",
    display_name: "Panel",
    category: NodeCategory::UI,
    default_name: "Panel",
    spawn_fn: spawn_ui_panel,
    serialize_fn: Some(serialize_ui_panel),
    deserialize_fn: Some(deserialize_ui_panel),
    priority: 0,
};

fn spawn_ui_panel(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let panel_data = UIPanelData::default();

    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: UI_PANEL.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(UI_PANEL.type_id),
        panel_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_ui_panel(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let panel = world.get::<UIPanelData>(entity)?;
    let mut data = HashMap::new();
    data.insert("width".to_string(), serde_json::json!(panel.width));
    data.insert("height".to_string(), serde_json::json!(panel.height));
    data.insert("background_color".to_string(), serde_json::json!([
        panel.background_color.x,
        panel.background_color.y,
        panel.background_color.z,
        panel.background_color.w
    ]));
    data.insert("border_radius".to_string(), serde_json::json!(panel.border_radius));
    data.insert("padding".to_string(), serde_json::json!(panel.padding));
    Some(data)
}

fn deserialize_ui_panel(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let width = data.get("width").and_then(|v| v.as_f64()).unwrap_or(200.0) as f32;
    let height = data.get("height").and_then(|v| v.as_f64()).unwrap_or(100.0) as f32;

    let background_color = data
        .get("background_color")
        .and_then(|v| v.as_array())
        .map(|arr| {
            Vec4::new(
                arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.2) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.2) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.25) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vec4::new(0.2, 0.2, 0.25, 1.0));

    let border_radius = data.get("border_radius").and_then(|v| v.as_f64()).unwrap_or(4.0) as f32;
    let padding = data.get("padding").and_then(|v| v.as_f64()).unwrap_or(8.0) as f32;

    entity_commands.insert(UIPanelData {
        width,
        height,
        background_color,
        border_radius,
        padding,
    });
}

/// UILabel - text display
pub static UI_LABEL: NodeDefinition = NodeDefinition {
    type_id: "ui.label",
    display_name: "Label",
    category: NodeCategory::UI,
    default_name: "Label",
    spawn_fn: spawn_ui_label,
    serialize_fn: Some(serialize_ui_label),
    deserialize_fn: Some(deserialize_ui_label),
    priority: 1,
};

fn spawn_ui_label(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let label_data = UILabelData::default();

    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: UI_LABEL.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(UI_LABEL.type_id),
        label_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_ui_label(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let label = world.get::<UILabelData>(entity)?;
    let mut data = HashMap::new();
    data.insert("text".to_string(), serde_json::json!(label.text));
    data.insert("font_size".to_string(), serde_json::json!(label.font_size));
    data.insert("color".to_string(), serde_json::json!([
        label.color.x,
        label.color.y,
        label.color.z,
        label.color.w
    ]));
    Some(data)
}

fn deserialize_ui_label(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let text = data.get("text").and_then(|v| v.as_str()).unwrap_or("Label").to_string();
    let font_size = data.get("font_size").and_then(|v| v.as_f64()).unwrap_or(16.0) as f32;

    let color = data
        .get("color")
        .and_then(|v| v.as_array())
        .map(|arr| {
            Vec4::new(
                arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vec4::ONE);

    entity_commands.insert(UILabelData {
        text,
        font_size,
        color,
    });
}

/// UIButton - clickable button
pub static UI_BUTTON: NodeDefinition = NodeDefinition {
    type_id: "ui.button",
    display_name: "Button",
    category: NodeCategory::UI,
    default_name: "Button",
    spawn_fn: spawn_ui_button,
    serialize_fn: Some(serialize_ui_button),
    deserialize_fn: Some(deserialize_ui_button),
    priority: 2,
};

fn spawn_ui_button(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let button_data = UIButtonData::default();

    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: UI_BUTTON.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(UI_BUTTON.type_id),
        button_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_ui_button(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let button = world.get::<UIButtonData>(entity)?;
    let mut data = HashMap::new();
    data.insert("text".to_string(), serde_json::json!(button.text));
    data.insert("width".to_string(), serde_json::json!(button.width));
    data.insert("height".to_string(), serde_json::json!(button.height));
    data.insert("font_size".to_string(), serde_json::json!(button.font_size));
    data.insert("normal_color".to_string(), serde_json::json!([
        button.normal_color.x,
        button.normal_color.y,
        button.normal_color.z,
        button.normal_color.w
    ]));
    data.insert("hover_color".to_string(), serde_json::json!([
        button.hover_color.x,
        button.hover_color.y,
        button.hover_color.z,
        button.hover_color.w
    ]));
    data.insert("pressed_color".to_string(), serde_json::json!([
        button.pressed_color.x,
        button.pressed_color.y,
        button.pressed_color.z,
        button.pressed_color.w
    ]));
    data.insert("text_color".to_string(), serde_json::json!([
        button.text_color.x,
        button.text_color.y,
        button.text_color.z,
        button.text_color.w
    ]));
    Some(data)
}

fn deserialize_ui_button(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let text = data.get("text").and_then(|v| v.as_str()).unwrap_or("Button").to_string();
    let width = data.get("width").and_then(|v| v.as_f64()).unwrap_or(120.0) as f32;
    let height = data.get("height").and_then(|v| v.as_f64()).unwrap_or(40.0) as f32;
    let font_size = data.get("font_size").and_then(|v| v.as_f64()).unwrap_or(16.0) as f32;

    fn parse_color(data: &HashMap<String, serde_json::Value>, key: &str, default: Vec4) -> Vec4 {
        data.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                Vec4::new(
                    arr.get(0).and_then(|v| v.as_f64()).unwrap_or(default.x as f64) as f32,
                    arr.get(1).and_then(|v| v.as_f64()).unwrap_or(default.y as f64) as f32,
                    arr.get(2).and_then(|v| v.as_f64()).unwrap_or(default.z as f64) as f32,
                    arr.get(3).and_then(|v| v.as_f64()).unwrap_or(default.w as f64) as f32,
                )
            })
            .unwrap_or(default)
    }

    entity_commands.insert(UIButtonData {
        text,
        width,
        height,
        font_size,
        normal_color: parse_color(data, "normal_color", Vec4::new(0.3, 0.3, 0.35, 1.0)),
        hover_color: parse_color(data, "hover_color", Vec4::new(0.4, 0.4, 0.45, 1.0)),
        pressed_color: parse_color(data, "pressed_color", Vec4::new(0.2, 0.2, 0.25, 1.0)),
        text_color: parse_color(data, "text_color", Vec4::ONE),
    });
}

/// UIImage - displays an image
pub static UI_IMAGE: NodeDefinition = NodeDefinition {
    type_id: "ui.image",
    display_name: "Image",
    category: NodeCategory::UI,
    default_name: "Image",
    spawn_fn: spawn_ui_image,
    serialize_fn: Some(serialize_ui_image),
    deserialize_fn: Some(deserialize_ui_image),
    priority: 3,
};

fn spawn_ui_image(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let image_data = UIImageData::default();

    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: UI_IMAGE.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(UI_IMAGE.type_id),
        image_data,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_ui_image(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let image = world.get::<UIImageData>(entity)?;
    let mut data = HashMap::new();
    data.insert("texture_path".to_string(), serde_json::json!(image.texture_path));
    data.insert("width".to_string(), serde_json::json!(image.width));
    data.insert("height".to_string(), serde_json::json!(image.height));
    data.insert("tint".to_string(), serde_json::json!([
        image.tint.x,
        image.tint.y,
        image.tint.z,
        image.tint.w
    ]));
    Some(data)
}

fn deserialize_ui_image(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let texture_path = data.get("texture_path").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let width = data.get("width").and_then(|v| v.as_f64()).unwrap_or(100.0) as f32;
    let height = data.get("height").and_then(|v| v.as_f64()).unwrap_or(100.0) as f32;

    let tint = data
        .get("tint")
        .and_then(|v| v.as_array())
        .map(|arr| {
            Vec4::new(
                arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vec4::ONE);

    entity_commands.insert(UIImageData {
        texture_path,
        width,
        height,
        tint,
    });
}
