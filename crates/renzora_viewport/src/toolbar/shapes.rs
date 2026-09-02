//! The add-shape dropdown: spawn any registered shape straight from the toolbar.
//!
//! Reads the same `ShapeRegistry` the shape-library panel uses, so the two can
//! never drift. Population is deferred to an exclusive system because the
//! registry is filled by the engine's plugin at startup, after the toolbar is
//! built.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora::core::ShapeRegistry;
use renzora_editor_framework::EditorCommands;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{rgb, text_primary};
use renzora_ember::widgets::{
    icon_popup_trigger, popup_anchor, popup_panel_aligned, scroll_area, Popup, PopupAlign,
};
use renzora_theme::ThemeManager;
use renzora_undo::{execute, SpawnShapeCmd, UndoContext};

use super::rows::{section_label, separator_row};
use super::{col, BTN_H};

/// The shapes-menu trigger button (for hover / open background tinting).
#[derive(Component)]
pub(super) struct ShapeMenuTrigger;

/// The (initially empty) column inside the shapes popup that `populate_shapes`
/// fills from the registry.
#[derive(Component)]
struct ShapeMenuContainer;

/// Marks a shapes list that's already been filled, so it isn't refilled.
#[derive(Component)]
struct ShapesPopulated;

/// A selectable shape row — carries the registry id so the click handler can
/// look the rest up (name + default color) at spawn time.
#[derive(Component, Clone)]
pub(super) struct ShapeSpawn {
    id: String,
}

pub(super) fn build_shapes_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Empty column that `populate_shapes` fills; wrapped in a capped scroll area
    // since the registry holds ~30 shapes across several categories.
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            ShapeMenuContainer,
            Name::new("vp-shapes-list"),
        ))
        .id();
    let scroll = scroll_area(commands, container, 360.0);

    // Left-aligned: this is the leftmost toolbar control, so a right-aligned
    // panel would grow off the left edge of the window.
    let panel = popup_panel_aligned(commands, &[scroll], PopupAlign::Left);
    let trigger = icon_popup_trigger(commands, fonts, "shapes", panel);
    commands.entity(trigger).insert(ShapeMenuTrigger);
    popup_anchor(commands, trigger, panel)
}

/// A label + icon row that spawns shape `id` when clicked.
fn shape_row(commands: &mut Commands, fonts: &EmberFonts, icon: &str, name: &str, id: &str) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 14.0);
    let label = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ShapeSpawn { id: id.to_string() },
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-shape-row"),
        ))
        .id();
    commands.entity(row).add_children(&[glyph, label]);
    row
}

/// Fill an empty `ShapeMenuContainer` from `ShapeRegistry`, grouped by category
/// (a section label whenever the category changes, separators between groups).
/// Exclusive so it can spawn rows from the registry's `&World` data; runs until
/// the registry is populated and the container exists.
pub(super) fn populate_shapes(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // Snapshot (icon, name, id, category) so the borrow of the registry ends
    // before we open a `Commands` over the world.
    let shapes: Vec<(String, String, String, String)> = {
        let Some(reg) = world.get_resource::<ShapeRegistry>() else {
            return;
        };
        reg.iter()
            .map(|e| {
                (
                    e.icon.to_string(),
                    e.name.to_string(),
                    e.id.to_string(),
                    e.category.to_string(),
                )
            })
            .collect()
    };
    if shapes.is_empty() {
        return; // shapes not registered yet
    }
    let mut cq = world.query_filtered::<Entity, (With<ShapeMenuContainer>, Without<ShapesPopulated>)>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let mut children: Vec<Entity> = Vec::new();
        let mut last_cat: Option<&str> = None;
        for (icon, name, id, category) in &shapes {
            if last_cat != Some(category.as_str()) {
                if last_cat.is_some() {
                    children.push(separator_row(&mut commands));
                }
                children.push(section_label(&mut commands, &fonts, category));
                last_cat = Some(category.as_str());
            }
            children.push(shape_row(&mut commands, &fonts, icon, name, id));
        }
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ShapesPopulated);
    }
    queue.apply(world);
}

/// Spawn the clicked shape at the origin (matching the hierarchy "Add Entity"
/// menu) through the undo system, then leave the menu open so several shapes can
/// be added in a row.
pub(super) fn shape_spawn_click(
    q: Query<(&Interaction, &ShapeSpawn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, shape) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let id = shape.id.clone();
        cmds.push(move |w: &mut World| {
            let Some((shape_id, name, color)) = w
                .get_resource::<ShapeRegistry>()
                .and_then(|r| r.get(&id))
                .map(|e| (e.id.to_string(), e.name.to_string(), e.default_color))
            else {
                warn!("Shape '{id}' not found in registry");
                return;
            };
            execute(
                w,
                UndoContext::Scene,
                Box::new(SpawnShapeCmd {
                    entity: Entity::PLACEHOLDER,
                    shape_id,
                    name,
                    position: Vec3::ZERO,
                    color,
                }),
            );
        });
    }
}

/// Hover/open tinting for the shapes trigger + hover highlight for its rows.
pub(super) fn update_shape_menu(
    theme: Option<Res<ThemeManager>>,
    mut trigger: Query<
        (&Interaction, &Popup, &mut BackgroundColor),
        (With<ShapeMenuTrigger>, Without<ShapeSpawn>),
    >,
    mut rows: Query<(&Interaction, &mut BackgroundColor), With<ShapeSpawn>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in &mut trigger {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    for (interaction, mut bg) in &mut rows {
        let want = if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}
