//! Editor inspector registration for HanabiEffect.

use bevy::prelude::*;
use renzora_editor_framework::{DocTabKind, InspectorEntry, InspectorRegistry};

use renzora_hanabi::{EffectSource, HanabiEffect};

fn hanabi_has(world: &World, entity: Entity) -> bool {
    world.get::<HanabiEffect>(entity).is_some()
}

fn hanabi_add(world: &mut World, entity: Entity) {
    world.entity_mut(entity).insert(HanabiEffect::default());
}

fn hanabi_remove(world: &mut World, entity: Entity) {
    world.entity_mut(entity).remove::<HanabiEffect>();
}

// ── Native (ember) drawer ────────────────────────────────────────────────────

use renzora_editor_framework::{AppEditorExt, FieldValue, SplashState};
use renzora_ember::font::EmberFonts;
use renzora_ember::inspector::{inspector_body, inspector_row, inspector_stripe};
use renzora_ember::widgets::icon_label_button;
use renzora_inspector::asset_drop_field;

#[derive(Component)]
struct HanabiEditBtn {
    entity: Entity,
}

fn hanabi_source_path(c: &HanabiEffect) -> Option<String> {
    match &c.source {
        EffectSource::Asset { path } if !path.is_empty() => Some(path.clone()),
        _ => None,
    }
}

fn hanabi_get(w: &World, e: Entity) -> Option<FieldValue> {
    w.get::<HanabiEffect>(e).map(|c| FieldValue::Asset(hanabi_source_path(c)))
}

fn hanabi_set(w: &mut World, e: Entity, v: FieldValue) {
    if let FieldValue::Asset(p) = v {
        if let Some(mut c) = w.get_mut::<HanabiEffect>(e) {
            c.source = EffectSource::Asset { path: p.unwrap_or_default() };
        }
    }
}

/// Native drawer mirroring `hanabi_custom_ui`: a `.particle` drop field + an Edit
/// button that opens the referenced effect in the particle editor.
fn hanabi_native(world: &mut World, entity: Entity) -> Entity {
    inspector_body(world, move |commands, fonts: &EmberFonts| {
        let col = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .id();

        let file = asset_drop_field(commands, fonts, entity, hanabi_get, hanabi_set, vec!["particle".into()]);
        let r_file = inspector_row(commands, &fonts.ui, "File", file);

        let edit = icon_label_button(commands, fonts, "pencil-simple", "Edit in Particle Editor");
        commands.entity(edit).insert(HanabiEditBtn { entity });
        let r_edit = inspector_row(commands, &fonts.ui, "", edit);

        let rows = [r_file, r_edit];
        for (i, r) in rows.iter().enumerate() {
            commands.entity(*r).insert(BackgroundColor(inspector_stripe(i)));
        }
        commands.entity(col).add_children(&rows);
        col
    })
}

fn hanabi_edit_click(q: Query<(&Interaction, &HanabiEditBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = btn.entity;
        commands.queue(move |w: &mut World| {
            let path = w.get::<HanabiEffect>(e).and_then(hanabi_source_path);
            if let Some(p) = path {
                renzora_editor_framework::open_asset_tab(w, std::path::Path::new(&p), DocTabKind::Particle);
            }
        });
    }
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
    };

    if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
        registry.register(entry);
    }
}

pub fn register_inspector(app: &mut App) {
    app.add_systems(Startup, register_inspector_system);
    app.register_native_inspector_ui("hanabi_effect", hanabi_native);
    app.add_systems(Update, hanabi_edit_click.run_if(in_state(SplashState::Editor)));
}
