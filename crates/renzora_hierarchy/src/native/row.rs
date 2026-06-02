//! Build a single hierarchy tree row, faithful to `renzora_ui::widgets::tree_row`:
//! 12px indent per depth, 24px rows, connector lines, a caret, a type icon, and
//! the label — plus the alternating row stripe and label-color tint.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;
use bevy_egui::egui::Color32;
use renzora_hui::cursor_icon::HoverCursor;
use egui_phosphor::regular::{
    CARET_DOWN, CARET_RIGHT, EYE, EYE_SLASH, LOCK_SIMPLE, LOCK_SIMPLE_OPEN, STAR,
};

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{rgb, ACCENT_BLUE, TEXT_PRIMARY};

use crate::state::EntityNode;

use super::components::{HierLockToggle, HierRow, HierRowClick, HierVisToggle};

const INDENT: f32 = 12.0;
const ROW_H: f32 = 24.0;
/// Right-edge zone reserved for the eye + lock toggles (2 × (18 icon + 4 margin)).
const SUFFIX_W: f32 = 44.0;
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
    pub is_selected: bool,
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
        .spawn((node, BackgroundColor(rgb(HIER_LINE)), Pickable::IGNORE))
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
            Pickable::IGNORE,
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

    // Bake the selected look in at build time (white label on the accent bg) so a
    // freshly rebuilt row shows selection immediately; `base_bg`/`label_color` are
    // stored on `HierRow` so the visual system can restore the unselected look.
    let init_bg = if d.is_selected {
        rgb(ACCENT_BLUE).with_alpha(0.63)
    } else {
        base_bg
    };
    let init_label = if d.is_selected {
        Color::WHITE
    } else {
        label_color
    };

    // The row is a plain layout box with no Interaction of its own; the
    // selection target is a separate full-row layer (below) so the caret/eye/
    // lock — its siblings — win their clicks purely by z-order.
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
                    Pickable::IGNORE,
                ))
                .id(),
        );
    }

    // Indent spacer.
    kids.push(
        commands
            .spawn((
                Node {
                    width: Val::Px(content_x),
                    height: Val::Px(ROW_H),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id(),
    );

    // Caret (16px slot; purely visual — expansion is toggled by clicking the row,
    // which also covers the caret area, so the caret never captures the click).
    let caret = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(ROW_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
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
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(caret).add_child(g);
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
            Pickable::IGNORE,
        ))
        .id();
    kids.push(icon);

    // Label container (flex-grows so the eye/lock toggles pin to the right edge;
    // clips long names instead of running them under the toggles).
    let label_box = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                height: Val::Px(ROW_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                overflow: Overflow::clip(),
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(&node.name),
            ui_font(&fonts.ui, 13.0),
            TextColor(init_label),
            bevy::text::TextLayout::new_with_no_wrap(),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(label_box).add_child(label);
    if node.is_default_camera {
        let star = commands
            .spawn((
                Text::new(STAR),
                TextFont {
                    font: fonts.phosphor.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgb_u8(255, 200, 80)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(label_box).add_child(star);
    }
    kids.push(label_box);

    // Right-edge toggles: visibility then lock.
    kids.push(suffix_toggle(
        commands,
        fonts,
        if node.is_visible { EYE } else { EYE_SLASH },
        if node.is_visible {
            Color::srgb_u8(140, 180, 220)
        } else {
            Color::srgb_u8(90, 90, 100)
        },
        Some(HierVisToggle {
            entity: node.entity,
            visible: node.is_visible,
        }),
        None,
    ));
    kids.push(suffix_toggle(
        commands,
        fonts,
        if node.is_locked {
            LOCK_SIMPLE
        } else {
            LOCK_SIMPLE_OPEN
        },
        if node.is_locked {
            Color::srgb_u8(220, 80, 80)
        } else {
            Color::srgb_u8(90, 90, 100)
        },
        None,
        Some(HierLockToggle {
            entity: node.entity,
            locked: node.is_locked,
        }),
    ));

    // Dim overlay for hidden entities (covers the content, not the toggles).
    if !node.is_visible {
        kids.push(
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        right: Val::Px(44.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.235)),
                    Pickable::IGNORE,
                ))
                .id(),
        );
    }

    // Click layer: covers the row except the suffix zone, so clicking the eye/
    // lock (which live in that zone) never lands here. Carries the select/expand
    // target.
    let click_layer = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                right: Val::Px(SUFFIX_W),
                ..default()
            },
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            HierRowClick {
                entity: node.entity,
                has_children,
            },
            Name::new("hier-row-hit"),
        ))
        .id();
    // Full-row background layer (visual only, lowest z): the bg/selection tint
    // spans the whole row including under the toggles.
    let bg_visual = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(init_bg),
            Pickable::IGNORE,
            HierRow {
                entity: node.entity,
                base_bg,
                label,
                label_color,
                click: click_layer,
            },
            Name::new("hier-row-bg"),
        ))
        .id();
    kids.insert(0, click_layer);
    kids.insert(0, bg_visual);

    commands.entity(row).add_children(&kids);
    row
}

/// A right-edge toggle icon (eye / lock). Exactly one of the two marker args is `Some`.
fn suffix_toggle(
    commands: &mut Commands,
    fonts: &EmberFonts,
    glyph: &str,
    color: Color,
    vis: Option<HierVisToggle>,
    lock: Option<HierLockToggle>,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(ROW_H),
                margin: UiRect::right(Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("hier-suffix"),
        ))
        .id();
    if let Some(v) = vis {
        commands.entity(btn).insert(v);
    }
    if let Some(l) = lock {
        commands.entity(btn).insert(l);
    }
    let g = commands
        .spawn((
            Text::new(glyph),
            TextFont {
                font: fonts.phosphor.clone(),
                font_size: 13.0,
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(btn).add_child(g);
    btn
}
