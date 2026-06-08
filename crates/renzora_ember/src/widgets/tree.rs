//! Tree — a collapsible hierarchy node.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::phosphor_map::icon_glyph;

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

use super::common::text_node;

#[derive(Component)]
pub(crate) struct EmberTreeToggle {
    kids: Entity,
    caret: Entity,
    open: bool,
}

/// A hierarchy tree node: an indented row (folder/file icon + label) with an
/// optional collapsible list of child nodes.
pub fn tree_node(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    depth: usize,
    children: Vec<Entity>,
    open: bool,
) -> Entity {
    let has_children = !children.is_empty();
    let node = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("tree-node"),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::new(
                    Val::Px(depth as f32 * 14.0 + 4.0),
                    Val::Px(4.0),
                    Val::Px(2.0),
                    Val::Px(2.0),
                ),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("tree-row"),
        ))
        .id();
    let caret = if has_children {
        icon_text(
            commands,
            &fonts.phosphor,
            if open { "caret-down" } else { "caret-right" },
            text_muted(),
            10.0,
        )
    } else {
        commands
            .spawn((Node {
                width: Val::Px(10.0),
                ..default()
            },))
            .id()
    };
    let icon = icon_text(
        commands,
        &fonts.phosphor,
        if has_children { "folder" } else { "file" },
        text_muted(),
        13.0,
    );
    let lbl = text_node(commands, &fonts.ui, label, 12.0, text_primary());
    commands.entity(row).add_children(&[caret, icon, lbl]);
    let kids = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                display: if open { Display::Flex } else { Display::None },
                ..default()
            },
            Name::new("tree-children"),
        ))
        .id();
    commands.entity(kids).add_children(&children);
    if has_children {
        // A vertical guide line down the left of the children (under the caret).
        let line = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(depth as f32 * 14.0 + 9.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(rgb(tree_line())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("tree-line"),
            ))
            .id();
        commands.entity(kids).add_child(line);
        commands
            .entity(row)
            .insert(EmberTreeToggle { kids, caret, open });
    }
    commands.entity(node).add_children(&[row, kids]);
    node
}

pub(crate) fn tree_toggle(
    mut rows: Query<(&Interaction, &mut EmberTreeToggle), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, mut tt) in &mut rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        tt.open = !tt.open;
        if let Ok(mut n) = nodes.get_mut(tt.kids) {
            n.display = if tt.open {
                Display::Flex
            } else {
                Display::None
            };
        }
        if let Ok(mut t) = texts.get_mut(tt.caret) {
            let glyph = icon_glyph(if tt.open { "caret-down" } else { "caret-right" })
                .unwrap_or('\u{E4C6}');
            *t = Text::new(glyph.to_string());
        }
    }
}

// ── Shared tree guide lines ──────────────────────────────────────────────────
// The 1.5px connector lines drawn down + across a tree's indent columns. The
// hierarchy and asset-browser trees both compose these so the guides match and
// theme from one place (`tree_line()`).

/// A vertical tree guide at column `x`, starting at `top`. `full` draws to the
/// bottom of the row; otherwise it's `height` px tall.
pub fn tree_vline(commands: &mut Commands, x: f32, top: f32, full: bool, height: f32) -> Entity {
    let mut node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x - 0.75),
        top: Val::Px(top),
        width: Val::Px(1.5),
        ..default()
    };
    if full {
        node.bottom = Val::Px(0.0);
    } else {
        node.height = Val::Px(height);
    }
    commands
        .spawn((node, BackgroundColor(rgb(tree_line())), bevy::picking::Pickable::IGNORE))
        .id()
}

/// A short horizontal tree guide joining a vertical line to the row content,
/// centered at `y`.
pub fn tree_hline(commands: &mut Commands, x: f32, y: f32, width: f32) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y - 0.75),
                width: Val::Px(width),
                height: Val::Px(1.5),
                ..default()
            },
            BackgroundColor(rgb(tree_line())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id()
}
