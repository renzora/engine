//! Build a single hierarchy tree row, faithful to `renzora_ui::widgets::tree_row`:
//! 12px indent per depth, 24px rows, connector lines, a caret, a type icon, and
//! the label — plus the alternating row stripe and label-color tint.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy_egui::egui::Color32;
use egui_phosphor::regular::{CARET_DOWN, CARET_RIGHT};

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{rgb, TEXT_PRIMARY};

use crate::state::EntityNode;

use super::components::{HierCaret, HierRow};

const INDENT: f32 = 12.0;
const ROW_H: f32 = 24.0;
const BASE_X: f32 = 4.0;
const LINE_OFFSET: f32 = INDENT / 2.0 - 1.0; // 5.0
const CENTER_Y: f32 = ROW_H / 2.0;

const HIER_LINE: (u8, u8, u8) = (64, 64, 76);
const ROW_ODD: (u8, u8, u8) = (38, 38, 45);
const CARET_EXPANDED: (u8, u8, u8) = (150, 150, 160);
const CARET_COLLAPSED: (u8, u8, u8) = (110, 110, 120);

/// A flattened, ready-to-render tree row.
pub(crate) struct RowData<'a> {
    pub node: &'a EntityNode,
    pub depth: usize,
    pub is_last: bool,
    pub parent_lines: Vec<bool>,
    pub row_index: usize,
    pub is_expanded: bool,
}

fn c32(c: Color32) -> Color {
    Color::srgba_u8(c.r(), c.g(), c.b(), c.a())
}

fn vline(commands: &mut Commands, x: f32, top: f32, full: bool, height: f32) -> Entity {
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
        .spawn((node, BackgroundColor(rgb(HIER_LINE)), FocusPolicy::Pass))
        .id()
}

fn hline(commands: &mut Commands, x: f32, width: f32) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(CENTER_Y - 0.75),
                width: Val::Px(width),
                height: Val::Px(1.5),
                ..default()
            },
            BackgroundColor(rgb(HIER_LINE)),
            FocusPolicy::Pass,
        ))
        .id()
}

/// Build one row entity; returns it (caller parents it into the list).
pub(crate) fn build_row(commands: &mut Commands, fonts: &EmberFonts, d: &RowData) -> Entity {
    let node = d.node;
    let has_children = !node.children.is_empty();
    let content_x = BASE_X + d.depth as f32 * INDENT;

    // Base (unselected) background: label-color tint, else the alternating stripe.
    let base_bg = if let Some([r, g, b]) = node.label_color {
        Color::srgba_u8(r, g, b, 26)
    } else if d.row_index % 2 == 1 {
        rgb(ROW_ODD)
    } else {
        Color::NONE
    };
    let label_color = node
        .label_color
        .map(|[r, g, b]| Color::srgb_u8(r, g, b))
        .unwrap_or_else(|| rgb(TEXT_PRIMARY));

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(ROW_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(base_bg),
            Interaction::default(),
            Name::new("hier-row"),
        ))
        .id();

    let mut kids: Vec<Entity> = Vec::new();

    // Ancestor continuation lines.
    for (level, &has_more) in d.parent_lines.iter().enumerate() {
        if has_more {
            let x = BASE_X + level as f32 * INDENT + LINE_OFFSET;
            kids.push(vline(commands, x, 0.0, true, 0.0));
        }
    }
    // This node's connector (vertical stub + horizontal arm).
    if d.depth > 0 {
        let x = BASE_X + (d.depth - 1) as f32 * INDENT + LINE_OFFSET;
        // └ stops at center for the last sibling, ├ runs full height otherwise.
        kids.push(vline(commands, x, 0.0, !d.is_last, CENTER_Y));
        kids.push(hline(commands, x, 5.0));
    }

    // 3px left color stripe for label-colored entities.
    if let Some([r, g, b]) = node.label_color {
        kids.push(
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb_u8(r, g, b)),
                    FocusPolicy::Pass,
                ))
                .id(),
        );
    }

    // Indent spacer.
    kids.push(
        commands
            .spawn((Node {
                width: Val::Px(content_x),
                height: Val::Px(ROW_H),
                flex_shrink: 0.0,
                ..default()
            },))
            .id(),
    );

    // Caret (16px slot; interactive only when the node has children).
    let caret = commands
        .spawn((Node {
            width: Val::Px(16.0),
            height: Val::Px(ROW_H),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            ..default()
        },))
        .id();
    if has_children {
        let (glyph, color) = if d.is_expanded {
            (CARET_DOWN, CARET_EXPANDED)
        } else {
            (CARET_RIGHT, CARET_COLLAPSED)
        };
        let g = commands
            .spawn((
                Text::new(glyph),
                TextFont {
                    font: fonts.phosphor.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(rgb(color)),
                FocusPolicy::Pass,
            ))
            .id();
        commands.entity(caret).add_child(g);
        commands.entity(caret).insert((
            Interaction::default(),
            HierCaret { entity: node.entity },
        ));
    }
    kids.push(caret);

    // Type icon (a raw Phosphor glyph string carried on the node).
    let icon = commands
        .spawn((
            Text::new(node.icon),
            TextFont {
                font: fonts.phosphor.clone(),
                font_size: 14.0,
                ..default()
            },
            TextColor(c32(node.icon_color)),
            Node {
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    kids.push(icon);

    // Label.
    let label = commands
        .spawn((
            Text::new(&node.name),
            ui_font(&fonts.ui, 13.0),
            TextColor(label_color),
            Node {
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    kids.push(label);

    commands.entity(row).add_children(&kids);
    commands.entity(row).insert(HierRow {
        entity: node.entity,
        base_bg,
        label,
        label_color,
    });
    row
}
