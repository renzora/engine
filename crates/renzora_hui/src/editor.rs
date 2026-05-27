//! Editor integration for HTML templates: make them creatable from the
//! hierarchy's "+ Add Entity" overlay, identifiable in the tree, and editable
//! (template path) in the inspector.
//!
//! Dragging/positioning needs no work here: `spawn_html_template_at` creates the
//! instance as an absolutely-positioned `UiWidget`, which the existing canvas
//! editor selects and drags like any other widget, and `renzora_hui`'s observer
//! keeps the actual markup under a child `HtmlNode` so bevy_hui never resets the
//! instance's position. So this module is just editor registrations.

use bevy::prelude::*;
use bevy_hui::prelude::HtmlNode;
use egui_phosphor::regular;
use renzora_editor::{
    AppEditorExt, ComponentIconEntry, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry,
};

use crate::template::HtmlTemplatePath;

/// Default template a freshly-created HTML entity points at, so it shows
/// something immediately instead of an empty node.
const DEFAULT_TEMPLATE: &str = "ui/example_menu.html";

pub struct HuiEditorPlugin;

impl Plugin for HuiEditorPlugin {
    fn build(&self, app: &mut App) {
        register_editor_entries(app);
    }
}

fn register_editor_entries(app: &mut App) {
    // "+ Add Entity" → UI → "HTML Template". Spawns a draggable, absolutely-
    // positioned instance under a UI Canvas; the runtime observer builds the
    // markup beneath it.
    app.register_entity_preset(EntityPreset {
        id: "html_template",
        display_name: "HTML Template",
        icon: regular::CODE,
        category: "ui",
        spawn_fn: |world| {
            renzora_game_ui::spawn::spawn_html_template_at(
                world,
                std::path::Path::new(DEFAULT_TEMPLATE),
                None,
            )
        },
    });

    // Distinctive icon + type label in the hierarchy tree.
    app.register_component_icon(ComponentIconEntry {
        type_id: std::any::TypeId::of::<HtmlTemplatePath>(),
        name: "HTML Template",
        icon: regular::CODE,
        color: [120, 170, 220],
        priority: 96,
        dynamic_icon_fn: None,
    });

    // Inspector: pick/replace the .html the instance displays. Adding the
    // component (also via "Add Component") seeds the default template.
    app.register_inspector(InspectorEntry {
        type_id: "html_template",
        display_name: "HTML Template",
        icon: regular::CODE,
        category: "ui",
        has_fn: |world, entity| world.get::<HtmlTemplatePath>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(HtmlTemplatePath(DEFAULT_TEMPLATE.to_string()));
        }),
        remove_fn: Some(|world, entity| {
            // Drop the path and any markup child it built.
            let children: Vec<Entity> = world
                .get::<Children>(entity)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            for child in children {
                if world.get::<HtmlNode>(child).is_some() {
                    world.entity_mut(child).despawn();
                }
            }
            world.entity_mut(entity).remove::<HtmlTemplatePath>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Template",
            field_type: FieldType::Asset {
                extensions: vec!["html".into()],
            },
            get_fn: |world, entity| {
                let path = world
                    .get::<HtmlTemplatePath>(entity)
                    .map(|p| if p.0.is_empty() { None } else { Some(p.0.clone()) })
                    .unwrap_or(None);
                Some(FieldValue::Asset(path))
            },
            // Always insert (replace) so the binding observer fires and rebuilds
            // the markup child for the new path.
            set_fn: |world, entity, val| {
                if let FieldValue::Asset(path) = val {
                    world
                        .entity_mut(entity)
                        .insert(HtmlTemplatePath(path.unwrap_or_default()));
                }
            },
        }],
        custom_ui_fn: None,
    });
}
