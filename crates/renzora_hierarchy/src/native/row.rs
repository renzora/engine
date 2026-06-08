//! Build one hierarchy row from an owned [`RowSnapshot`], with reactive bindings
//! for selection/hover (so selecting/hovering never rebuilds the row — the keyed
//! list only rebuilds a row when its *content* hash changes).

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;
use renzora_editor_framework::{EditorSelection, TreeDropZone};
use renzora_ember::font::{icon_glyph, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_bg, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::cursor_icon::HoverCursor;

use super::components::{
    HierCaretToggle, HierDropEdge, HierLockToggle, HierRowClick, HierVisToggle,
};
use super::drag::HierDrag;

const INDENT: f32 = 12.0;
const ROW_H: f32 = 24.0;
const BASE_X: f32 = 4.0;
const LINE_OFFSET: f32 = INDENT / 2.0 - 1.0; // 5.0
const CENTER_Y: f32 = ROW_H / 2.0;
/// Right-edge zone reserved for the eye + lock toggles (2 × (18 icon + 4 margin)).
const SUFFIX_W: f32 = 44.0;

/// Resolve a Phosphor icon *name* (kebab-case) to its glyph string for direct
/// `Text` rendering, via the ember/hui phosphor map. Falls back to a generic
/// glyph for unknown names.
fn glyph_str(name: &str) -> String {
    icon_glyph(name).unwrap_or('\u{E4C6}').to_string()
}


/// An owned, ready-to-build row (captured into the keyed-list snapshot).
pub(crate) struct RowSnapshot {
    pub entity: Entity,
    pub name: String,
    pub icon: &'static str,
    pub icon_color: Color,
    pub label_color: Option<[u8; 3]>,
    pub is_visible: bool,
    pub is_locked: bool,
    pub is_default_camera: bool,
    pub depth: usize,
    pub is_last: bool,
    pub parent_lines: Vec<bool>,
    pub is_expanded: bool,
    pub has_children: bool,
    /// This row is being inline-renamed → render an edit field instead of label.
    pub is_renaming: bool,
}

impl RowSnapshot {
    /// A stable identity for the keyed list (the entity).
    pub fn key(&self) -> u64 {
        self.entity.to_bits()
    }

    /// Content hash — everything that changes how the row *looks/positions*,
    /// excluding selection/hover (those ride on bindings).
    pub fn content_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.entity.to_bits().hash(&mut h);
        self.depth.hash(&mut h);
        self.is_last.hash(&mut h);
        self.is_expanded.hash(&mut h);
        self.has_children.hash(&mut h);
        self.name.hash(&mut h);
        self.icon.hash(&mut h);
        self.is_visible.hash(&mut h);
        self.is_locked.hash(&mut h);
        self.is_default_camera.hash(&mut h);
        self.label_color.hash(&mut h);
        self.parent_lines.hash(&mut h);
        self.is_renaming.hash(&mut h);
        h.finish()
    }
}

/// Composite `top` over `base` (straight sRGBA) — for the hover wash.
fn over(base: Color, top: Color) -> Color {
    let b = base.to_srgba();
    let t = top.to_srgba();
    let a = t.alpha + b.alpha * (1.0 - t.alpha);
    if a <= 0.0 {
        return Color::NONE;
    }
    Color::srgba(
        (t.red * t.alpha + b.red * b.alpha * (1.0 - t.alpha)) / a,
        (t.green * t.alpha + b.green * b.alpha * (1.0 - t.alpha)) / a,
        (t.blue * t.alpha + b.blue * b.alpha * (1.0 - t.alpha)) / a,
        a,
    )
}

/// Build one row entity (returns it; the keyed list parents it).
pub(crate) fn build_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    s: &RowSnapshot,
    row_index: usize,
) -> Entity {
    let content_x = BASE_X + s.depth as f32 * INDENT;

    // An entity's label color tints its row; otherwise alternate odd/even
    // striping (shared with the inspector for a consistent look).
    let base_bg = if let Some([r, g, b]) = s.label_color {
        Color::srgba_u8(r, g, b, 26)
    } else {
        renzora_ember::inspector::inspector_stripe(row_index)
    };
    let label_color = s
        .label_color
        .map(|[r, g, b]| Color::srgb_u8(r, g, b))
        .unwrap_or_else(|| rgb(text_primary()));

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

    for (level, &has_more) in s.parent_lines.iter().enumerate() {
        if has_more {
            let x = BASE_X + level as f32 * INDENT + LINE_OFFSET;
            kids.push(renzora_ember::widgets::tree_vline(commands, x, 0.0, true, 0.0));
        }
    }
    if s.depth > 0 {
        let x = BASE_X + (s.depth - 1) as f32 * INDENT + LINE_OFFSET;
        kids.push(renzora_ember::widgets::tree_vline(commands, x, 0.0, !s.is_last, CENTER_Y));
        kids.push(renzora_ember::widgets::tree_hline(commands, x, CENTER_Y, 5.0));
    }

    if let Some([r, g, b]) = s.label_color {
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

    // Caret — interactive (toggles expansion) when the row has children.
    let caret = commands
        .spawn(Node {
            width: Val::Px(16.0),
            height: Val::Px(ROW_H),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    if s.has_children {
        let (glyph, color) = if s.is_expanded {
            ("caret-down", renzora_ember::theme::text_muted())
        } else {
            ("caret-right", renzora_ember::theme::placeholder())
        };
        let g = commands
            .spawn((
                Text::new(glyph_str(glyph)),
                TextFont {
                    font: fonts.phosphor.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(rgb(color)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(caret).insert((
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            HierCaretToggle(s.entity),
        ));
        commands.entity(caret).add_child(g);
    } else {
        commands.entity(caret).insert(Pickable::IGNORE);
    }
    kids.push(caret);

    // Type icon.
    let icon = commands
        .spawn((
            Text::new(glyph_str(s.icon)),
            TextFont {
                font: fonts.phosphor.clone(),
                font_size: 14.0,
                ..default()
            },
            TextColor(s.icon_color),
            Node {
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    kids.push(icon);

    // Label container + label (+ default-camera star).
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
    // The label entity (for the selection text-color bind), or `None` while the
    // inline rename field is shown in its place.
    let label_ent: Option<Entity> = if s.is_renaming {
        // The rename field sits above the click layer, so it takes clicks for
        // editing while the rest of the row still selects.
        let field = super::rename::build_rename_field(commands, fonts, s.entity, &s.name);
        commands.entity(label_box).add_child(field);
        None
    } else {
        let label = commands
            .spawn((
                Text::new(&s.name),
                ui_font(&fonts.ui, 13.0),
                TextColor(label_color),
                bevy::text::TextLayout::new_with_no_wrap(),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(label_box).add_child(label);
        if s.is_default_camera {
            let star = commands
                .spawn((
                    Text::new(glyph_str("star")),
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
        Some(label)
    };
    kids.push(label_box);

    kids.push(suffix_toggle(
        commands,
        fonts,
        if s.is_visible { "eye" } else { "eye-slash" },
        if s.is_visible {
            rgb(text_muted())
        } else {
            rgb(placeholder())
        },
        Some(HierVisToggle {
            entity: s.entity,
            visible: s.is_visible,
        }),
        None,
    ));
    kids.push(suffix_toggle(
        commands,
        fonts,
        if s.is_locked {
            "lock-simple"
        } else {
            "lock-simple-open"
        },
        if s.is_locked {
            rgb(close_red())
        } else {
            rgb(placeholder())
        },
        None,
        Some(HierLockToggle {
            entity: s.entity,
            locked: s.is_locked,
        }),
    ));

    if !s.is_visible {
        kids.push(
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        right: Val::Px(SUFFIX_W),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.235)),
                    Pickable::IGNORE,
                ))
                .id(),
        );
    }

    // Click layer (covers row except suffix zone).
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
            bevy::ui::RelativeCursorPosition::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            HierRowClick { entity: s.entity },
            Name::new("hier-row-hit"),
        ))
        .id();
    // Drop-indicator lines at the row's top/bottom edges (hidden until a drag
    // targets this row Before/After).
    let edge_before = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(-1.0),
                height: Val::Px(2.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Pickable::IGNORE,
            HierDropEdge {
                entity: s.entity,
                after: false,
            },
        ))
        .id();
    let edge_after = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(-1.0),
                height: Val::Px(2.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Pickable::IGNORE,
            HierDropEdge {
                entity: s.entity,
                after: true,
            },
        ))
        .id();
    kids.push(edge_before);
    kids.push(edge_after);
    // Full-row background layer (visual, lowest z).
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
            BackgroundColor(base_bg),
            Pickable::IGNORE,
            Name::new("hier-row-bg"),
        ))
        .id();
    kids.insert(0, click_layer);
    kids.insert(0, bg_visual);

    commands.entity(row).add_children(&kids);

    // Reactive selection/hover — value-diffed, so selecting only repaints the
    // rows whose computed color changed (no rebuild).
    let (ent, click, base) = (s.entity, click_layer, base_bg);
    bind_bg(commands, bg_visual, move |world: &World| {
        // Drop-target tint while dragging onto this row (reparent).
        if let Some(drag) = world.get_resource::<HierDrag>() {
            if drag.active
                && matches!(drag.target, Some((t, TreeDropZone::AsChild)) if t == ent)
            {
                return over(base, rgb(accent()).with_alpha(0.35));
            }
        }
        if world
            .get_resource::<EditorSelection>()
            .is_some_and(|sel| sel.is_selected(ent))
        {
            return rgb(accent()).with_alpha(0.63);
        }
        let hovered = world
            .get::<Interaction>(click)
            .is_some_and(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
        if hovered {
            over(base, Color::srgba(1.0, 1.0, 1.0, 0.06))
        } else {
            base
        }
    });
    if let Some(label) = label_ent {
        bind_text_color(commands, label, move |world: &World| {
            if world
                .get_resource::<EditorSelection>()
                .is_some_and(|sel| sel.is_selected(ent))
            {
                Color::WHITE
            } else {
                label_color
            }
        });
    }

    row
}

/// A right-edge toggle icon (eye / lock). Exactly one marker arg is `Some`.
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
            Text::new(glyph_str(glyph)),
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
