//! Editor inspector registration for HanabiEffect.

use bevy::prelude::*;
use bevy_egui::egui;
use renzora_editor::{
    asset_drop_target, inline_property, section_header, AssetDragPayload, DocTabKind,
    EditorCommands, InspectorEntry, InspectorRegistry,
};
use renzora_theme::Theme;

use crate::data::*;

fn hanabi_has(world: &World, entity: Entity) -> bool {
    world.get::<HanabiEffect>(entity).is_some()
}

fn hanabi_add(world: &mut World, entity: Entity) {
    world.entity_mut(entity).insert(HanabiEffect::default());
}

fn hanabi_remove(world: &mut World, entity: Entity) {
    world.entity_mut(entity).remove::<HanabiEffect>();
}

fn hanabi_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(data) = world.get::<HanabiEffect>(entity) else {
        return;
    };

    // This component is just a *reference* to a `.particle` effect — all the
    // authoring lives in the particle editor. So the inspector only loads a
    // file (drag-drop) and offers an Edit button to open it in a document tab.
    section_header(ui, "Effect", theme);

    let payload = world.get_resource::<AssetDragPayload>();
    let exts = ["particle"];
    let current = match &data.source {
        EffectSource::Asset { path } if !path.is_empty() => Some(path.as_str()),
        _ => None,
    };

    let result = inline_property(ui, 0, "File", theme, |ui| {
        asset_drop_target(
            ui,
            ui.id().with("hanabi_source"),
            current,
            &exts,
            "Drag a .particle here",
            theme,
            payload,
        )
    });

    if let Some(path) = result.dropped_path {
        let rel = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.make_asset_relative(&path))
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        cmds.push(move |w: &mut World| {
            if let Some(mut c) = w.get_mut::<HanabiEffect>(entity) {
                c.source = EffectSource::Asset { path: rel };
            }
        });
    }
    if result.cleared {
        cmds.push(move |w: &mut World| {
            if let Some(mut c) = w.get_mut::<HanabiEffect>(entity) {
                c.source = EffectSource::Asset {
                    path: String::new(),
                };
            }
        });
    }

    // Edit button — opens the referenced `.particle` in the particle editor.
    let edit_path: Option<String> = match &data.source {
        EffectSource::Asset { path } if !path.is_empty() => Some(path.clone()),
        _ => None,
    };
    inline_property(ui, 1, "", theme, |ui| {
        let resp = ui.add_enabled(
            edit_path.is_some(),
            egui::Button::new("Edit in Particle Editor"),
        );
        if resp.clicked() {
            if let Some(p) = edit_path.clone() {
                cmds.push(move |w: &mut World| {
                    renzora_editor::open_asset_tab(
                        w,
                        std::path::Path::new(&p),
                        DocTabKind::Particle,
                    );
                });
            }
        }
        resp
    });
}

fn register_inspector_system(world: &mut World) {
    let entry = InspectorEntry {
        type_id: "hanabi_effect",
        display_name: "Hanabi Effect",
        icon: egui_phosphor::regular::SPARKLE,
        category: "effects",
        has_fn: hanabi_has,
        add_fn: Some(hanabi_add),
        remove_fn: Some(hanabi_remove),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(hanabi_custom_ui),
    };

    if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
        registry.register(entry);
    }
}

pub fn register_inspector(app: &mut App) {
    app.add_systems(Startup, register_inspector_system);
}
