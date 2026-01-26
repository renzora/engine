//! UI component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::shared::{UIButtonData, UIImageData, UILabelData, UIPanelData};
use crate::ui::property_row;

// ============================================================================
// Component Definitions
// ============================================================================

pub static UI_PANEL: ComponentDefinition = ComponentDefinition {
    type_id: "ui_panel",
    display_name: "UI Panel",
    category: ComponentCategory::UI,
    icon: "\u{e922}", // Layout icon
    priority: 0,
    add_fn: add_ui_panel,
    remove_fn: remove_ui_panel,
    has_fn: has_ui_panel,
    serialize_fn: serialize_ui_panel,
    deserialize_fn: deserialize_ui_panel,
    inspector_fn: inspect_ui_panel,
    conflicts_with: &["ui_label", "ui_button", "ui_image"],
    requires: &[],
};

pub static UI_LABEL: ComponentDefinition = ComponentDefinition {
    type_id: "ui_label",
    display_name: "UI Label",
    category: ComponentCategory::UI,
    icon: "\u{e8ed}", // Text icon
    priority: 1,
    add_fn: add_ui_label,
    remove_fn: remove_ui_label,
    has_fn: has_ui_label,
    serialize_fn: serialize_ui_label,
    deserialize_fn: deserialize_ui_label,
    inspector_fn: inspect_ui_label,
    conflicts_with: &["ui_panel", "ui_button", "ui_image"],
    requires: &[],
};

pub static UI_BUTTON: ComponentDefinition = ComponentDefinition {
    type_id: "ui_button",
    display_name: "UI Button",
    category: ComponentCategory::UI,
    icon: "\u{e9ca}", // Button icon
    priority: 2,
    add_fn: add_ui_button,
    remove_fn: remove_ui_button,
    has_fn: has_ui_button,
    serialize_fn: serialize_ui_button,
    deserialize_fn: deserialize_ui_button,
    inspector_fn: inspect_ui_button,
    conflicts_with: &["ui_panel", "ui_label", "ui_image"],
    requires: &[],
};

pub static UI_IMAGE: ComponentDefinition = ComponentDefinition {
    type_id: "ui_image",
    display_name: "UI Image",
    category: ComponentCategory::UI,
    icon: "\u{e9ce}", // Image icon
    priority: 3,
    add_fn: add_ui_image,
    remove_fn: remove_ui_image,
    has_fn: has_ui_image,
    serialize_fn: serialize_ui_image,
    deserialize_fn: deserialize_ui_image,
    inspector_fn: inspect_ui_image,
    conflicts_with: &["ui_panel", "ui_label", "ui_button"],
    requires: &[],
};

/// Register all UI components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&UI_PANEL);
    registry.register(&UI_LABEL);
    registry.register(&UI_BUTTON);
    registry.register(&UI_IMAGE);
}

// ============================================================================
// UI Panel
// ============================================================================

fn add_ui_panel(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(UIPanelData::default());
}

fn remove_ui_panel(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<UIPanelData>();
}

fn has_ui_panel(world: &World, entity: Entity) -> bool {
    world.get::<UIPanelData>(entity).is_some()
}

fn serialize_ui_panel(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<UIPanelData>(entity)?;
    Some(json!({
        "width": data.width,
        "height": data.height,
        "background_color": [data.background_color.x, data.background_color.y, data.background_color.z, data.background_color.w],
        "border_radius": data.border_radius,
        "padding": data.padding
    }))
}

fn deserialize_ui_panel(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let bg_color = data
        .get("background_color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Vec4::new(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(0.2) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.2) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.25) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vec4::new(0.2, 0.2, 0.25, 1.0));

    entity_commands.insert(UIPanelData {
        width: data.get("width").and_then(|v| v.as_f64()).unwrap_or(200.0) as f32,
        height: data
            .get("height")
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0) as f32,
        background_color: bg_color,
        border_radius: data
            .get("border_radius")
            .and_then(|v| v.as_f64())
            .unwrap_or(4.0) as f32,
        padding: data
            .get("padding")
            .and_then(|v| v.as_f64())
            .unwrap_or(8.0) as f32,
    });
}

fn inspect_ui_panel(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<UIPanelData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Width
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut data.width).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Height
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut data.height).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Background Color
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Background");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (data.background_color.x * 255.0) as u8,
                    (data.background_color.y * 255.0) as u8,
                    (data.background_color.z * 255.0) as u8,
                    (data.background_color.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.background_color = Vec4::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    // Border Radius
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Border Radius");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.border_radius)
                            .speed(0.5)
                            .range(0.0..=50.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Padding
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Padding");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.padding)
                            .speed(0.5)
                            .range(0.0..=50.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// UI Label
// ============================================================================

fn add_ui_label(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(UILabelData::default());
}

fn remove_ui_label(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<UILabelData>();
}

fn has_ui_label(world: &World, entity: Entity) -> bool {
    world.get::<UILabelData>(entity).is_some()
}

fn serialize_ui_label(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<UILabelData>(entity)?;
    Some(json!({
        "text": data.text,
        "font_size": data.font_size,
        "color": [data.color.x, data.color.y, data.color.z, data.color.w]
    }))
}

fn deserialize_ui_label(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Vec4::new(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vec4::ONE);

    entity_commands.insert(UILabelData {
        text: data
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("Label")
            .to_string(),
        font_size: data
            .get("font_size")
            .and_then(|v| v.as_f64())
            .unwrap_or(16.0) as f32,
        color,
    });
}

fn inspect_ui_label(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<UILabelData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Text
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.text_edit_singleline(&mut data.text).changed() {
                    changed = true;
                }
            });
        });
    });

    // Font Size
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Font Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.font_size)
                            .speed(0.5)
                            .range(8.0..=72.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Color
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (data.color.x * 255.0) as u8,
                    (data.color.y * 255.0) as u8,
                    (data.color.z * 255.0) as u8,
                    (data.color.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.color = Vec4::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// UI Button
// ============================================================================

fn add_ui_button(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(UIButtonData::default());
}

fn remove_ui_button(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<UIButtonData>();
}

fn has_ui_button(world: &World, entity: Entity) -> bool {
    world.get::<UIButtonData>(entity).is_some()
}

fn serialize_ui_button(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<UIButtonData>(entity)?;
    Some(json!({
        "text": data.text,
        "width": data.width,
        "height": data.height,
        "font_size": data.font_size,
        "normal_color": [data.normal_color.x, data.normal_color.y, data.normal_color.z, data.normal_color.w],
        "hover_color": [data.hover_color.x, data.hover_color.y, data.hover_color.z, data.hover_color.w],
        "pressed_color": [data.pressed_color.x, data.pressed_color.y, data.pressed_color.z, data.pressed_color.w],
        "text_color": [data.text_color.x, data.text_color.y, data.text_color.z, data.text_color.w]
    }))
}

fn deserialize_ui_button(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    fn parse_color(data: &serde_json::Value, key: &str, default: Vec4) -> Vec4 {
        data.get(key)
            .and_then(|c| c.as_array())
            .map(|arr| {
                Vec4::new(
                    arr.first().and_then(|v| v.as_f64()).unwrap_or(default.x as f64) as f32,
                    arr.get(1).and_then(|v| v.as_f64()).unwrap_or(default.y as f64) as f32,
                    arr.get(2).and_then(|v| v.as_f64()).unwrap_or(default.z as f64) as f32,
                    arr.get(3).and_then(|v| v.as_f64()).unwrap_or(default.w as f64) as f32,
                )
            })
            .unwrap_or(default)
    }

    let defaults = UIButtonData::default();
    entity_commands.insert(UIButtonData {
        text: data
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("Button")
            .to_string(),
        width: data
            .get("width")
            .and_then(|v| v.as_f64())
            .unwrap_or(120.0) as f32,
        height: data
            .get("height")
            .and_then(|v| v.as_f64())
            .unwrap_or(40.0) as f32,
        font_size: data
            .get("font_size")
            .and_then(|v| v.as_f64())
            .unwrap_or(16.0) as f32,
        normal_color: parse_color(data, "normal_color", defaults.normal_color),
        hover_color: parse_color(data, "hover_color", defaults.hover_color),
        pressed_color: parse_color(data, "pressed_color", defaults.pressed_color),
        text_color: parse_color(data, "text_color", defaults.text_color),
    });
}

fn inspect_ui_button(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<UIButtonData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Text
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.text_edit_singleline(&mut data.text).changed() {
                    changed = true;
                }
            });
        });
    });

    // Width
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut data.width).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Height
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut data.height).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Font Size
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Font Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.font_size)
                            .speed(0.5)
                            .range(8.0..=72.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Text Color
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (data.text_color.x * 255.0) as u8,
                    (data.text_color.y * 255.0) as u8,
                    (data.text_color.z * 255.0) as u8,
                    (data.text_color.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.text_color = Vec4::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// UI Image
// ============================================================================

fn add_ui_image(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(UIImageData::default());
}

fn remove_ui_image(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<UIImageData>();
}

fn has_ui_image(world: &World, entity: Entity) -> bool {
    world.get::<UIImageData>(entity).is_some()
}

fn serialize_ui_image(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<UIImageData>(entity)?;
    Some(json!({
        "texture_path": data.texture_path,
        "width": data.width,
        "height": data.height,
        "tint": [data.tint.x, data.tint.y, data.tint.z, data.tint.w]
    }))
}

fn deserialize_ui_image(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let tint = data
        .get("tint")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Vec4::new(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vec4::ONE);

    entity_commands.insert(UIImageData {
        texture_path: data
            .get("texture_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        width: data
            .get("width")
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0) as f32,
        height: data
            .get("height")
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0) as f32,
        tint,
    });
}

fn inspect_ui_image(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<UIImageData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Texture Path
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Texture");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.text_edit_singleline(&mut data.texture_path).changed() {
                    changed = true;
                }
            });
        });
    });

    // Width
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut data.width).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Height
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut data.height).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Tint
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Tint");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (data.tint.x * 255.0) as u8,
                    (data.tint.y * 255.0) as u8,
                    (data.tint.z * 255.0) as u8,
                    (data.tint.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.tint = Vec4::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    changed
}
