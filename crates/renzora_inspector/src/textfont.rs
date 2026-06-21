//! Native inspector drawer for bevy's `TextFont` — pick the font (from the
//! shared [`FontRegistry`]) and size for any text-bearing entity. Paired with
//! the `"text_font"` `InspectorEntry` registered in the editor framework.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::text::{FontSize, TextFont};

use renzora_ember::font::{ui_font, EmberFonts, FontRegistry};
use renzora_ember::reactive::bind_2way;
use renzora_ember::theme::{rgb, text_muted, value_text};
use renzora_ember::widgets::{drag_value, font_picker, DragRange};

pub fn register(app: &mut App) {
    use renzora_editor_framework::AppEditorExt;
    app.register_native_inspector_ui("text_font", textfont_native);
}

/// Resolve the entity's current `TextFont.font` back to a registry name (so the
/// picker shows the right selection). Unknown fonts fall back to "Default".
fn current_font_name(w: &World, e: Entity) -> String {
    let Some(tf) = w.get::<TextFont>(e) else {
        return "Default".into();
    };
    w.get_resource::<FontRegistry>()
        .and_then(|r| r.entries.iter().find(|en| en.source == tf.font).map(|en| en.name.clone()))
        .unwrap_or_else(|| "Default".into())
}

/// Set the entity's `TextFont.font` from a registry font name.
fn set_font(w: &mut World, e: Entity, name: &str) {
    let Some(src) = w.get_resource::<FontRegistry>().and_then(|r| r.resolve(name)) else {
        return;
    };
    if let Some(mut tf) = w.get_mut::<TextFont>(e) {
        tf.font = src;
    }
}

fn current_size(w: &World, e: Entity) -> f32 {
    w.get::<TextFont>(e)
        .map(|tf| match tf.font_size {
            FontSize::Px(v) => v,
            _ => 14.0,
        })
        .unwrap_or(14.0)
}

/// A `label : control` row.
fn labeled_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(48.0),
                ..default()
            },
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("text-font-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, control]);
    row
}

fn textfont_native(world: &mut World, entity: Entity) -> Entity {
    // Snapshot resources before borrowing the World for `Commands`.
    let fonts = world.get_resource::<EmberFonts>().cloned();
    let registry = world.get_resource::<FontRegistry>().cloned();
    let size_init = current_size(world, entity);

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
                Name::new("text-font-root"),
            ))
            .id();

        if let (Some(fonts), Some(registry)) = (fonts, registry) {
            let picker = font_picker(
                &mut commands,
                &fonts,
                &registry,
                move |w| current_font_name(w, entity),
                move |w, name| set_font(w, entity, &name),
            );
            let font_row = labeled_row(&mut commands, &fonts, "Font", picker);

            let dv = drag_value(&mut commands, &fonts.ui, "", value_text(), size_init, 0.5);
            commands.entity(dv).insert(DragRange { min: 4.0, max: 200.0 });
            bind_2way::<f32, _, _>(
                &mut commands,
                dv,
                move |w| current_size(w, entity),
                move |w, &v| {
                    if let Some(mut tf) = w.get_mut::<TextFont>(entity) {
                        tf.font_size = FontSize::Px(v);
                    }
                },
            );
            let size_row = labeled_row(&mut commands, &fonts, "Size", dv);

            commands.entity(root).add_children(&[font_row, size_row]);
        }
        root
    };
    queue.apply(world);
    root
}
