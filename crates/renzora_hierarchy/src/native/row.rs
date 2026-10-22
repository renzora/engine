//! Build one hierarchy row from an owned [`RowSnapshot`], with reactive bindings
//! for selection/hover (so selecting/hovering never rebuilds the row — the keyed
//! list only rebuilds a row when its *content* hash changes).

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;
use renzora_editor_framework::{EditorSelection, TreeDropZone};
use renzora_ember::font::{icon_glyph, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::UiInteraction;

use super::components::{
    BadgeKind, HierAssetBadge, HierCaretToggle, HierDropEdge, HierLockToggle, HierRowArea,
    HierRowClick, HierRowLabel, HierVisToggle,
};
use super::drag::HierDrag;

/// Per-level indent: one caret width, so a child's caret sits directly under
/// its parent's icon and every level has its own column.
///
/// Briefly zero, with each level instead shaded a little lighter: with no
/// step, parent and child rows looked the same, and a shade faint enough not to
/// bring the stripes back was too faint to tell them apart. The indent is the
/// signal every tree uses; what had made the old one look busy was the guide
/// lines drawn through it, which stay off.
pub(crate) const INDENT: f32 = 16.0;
pub(crate) const ROW_H: f32 = 26.0;
pub(crate) const BASE_X: f32 = 4.0;
// Centres a drag line under its level's caret.
const LINE_OFFSET: f32 = INDENT / 2.0 - 1.0;
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
    /// `label_color` was set on this entity, not inherited (it then tints the
    /// icon as well as drawing the edge line).
    pub label_color_own: bool,
    pub is_visible: bool,
    pub is_locked: bool,
    pub is_default_camera: bool,
    /// Authored-asset badges (script / blueprint / material) drawn just left of
    /// the eye/lock toggles.
    pub has_script: bool,
    pub has_blueprint: bool,
    pub has_material: bool,
    /// A sound this entity owns is audible right now — drawn as animated bars
    /// left of the eye. In the hash below because the indicator must appear and
    /// vanish with playback.
    pub is_emitting: bool,
    pub depth: usize,
    pub is_last: bool,
    pub parent_lines: Vec<bool>,
    /// Ancestor entity at each depth (parallel to `parent_lines`), so a connector
    /// line knows which parent it belongs to (drives the drag highlight).
    pub ancestors: Vec<Entity>,
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
        self.has_script.hash(&mut h);
        self.has_blueprint.hash(&mut h);
        self.has_material.hash(&mut h);
        self.is_emitting.hash(&mut h);
        self.label_color.hash(&mut h);
        self.label_color_own.hash(&mut h);
        self.parent_lines.hash(&mut h);
        self.ancestors.hash(&mut h);
        self.is_renaming.hash(&mut h);
        h.finish()
    }
}

/// An entity label colour, softened for the list: mixed a third of the way
/// toward the muted text grey.
///
/// The swatches are saturated so they are easy to tell apart in the picker.
/// Painted at full strength across row fills and names they were the loudest
/// thing in the panel, and several coloured entities together clashed; pulled
/// toward a neutral they still read as their colour without fighting.
pub(crate) fn soft_label([r, g, b]: [u8; 3]) -> Color {
    renzora_ember::theme::mix((r, g, b), renzora_ember::theme::text_muted(), 0.35)
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

/// The parent **group** whose guide lines should highlight for the current drag
/// target. This is the group the cursor is visually inside, which differs from
/// the literal drop target: hovering the body of a *leaf* row (or anywhere in a
/// Before/After zone) resolves to that row's parent, so the whole containing
/// group stays lit as you move across its children. Only the middle of an
/// *expandable* row resolves to that row itself (drop-into-this-group). `None`
/// when no drag is active.
fn drag_highlight_parent(world: &Rx) -> Option<Entity> {
    let drag = world.get_resource::<HierDrag>()?;
    if !drag.active {
        return None;
    }
    let (target, zone) = drag.target?;
    let has_children = world.get::<Children>(target).is_some_and(|c| !c.is_empty());
    // An *open* parent's children sit right below it, so the area below it (the
    // After zone) flows into that parent's group — resolving it to the target
    // too keeps the group lit across the AsChild/After seam just under the
    // parent instead of flickering to the grandparent.
    let expanded = world
        .get_resource::<super::HierExpanded>()
        .is_some_and(|e| e.0.contains(&target));
    match zone {
        TreeDropZone::AsChild if has_children => Some(target),
        TreeDropZone::After if has_children && expanded => Some(target),
        // Leaf AsChild, or a Before/After reorder: the group is the target's parent.
        _ => world.get::<ChildOf>(target).map(|c| c.parent()),
    }
}

/// Show a tree connector line only while a drag makes it mean something: WHITE
/// when its owning ancestor is the highlighted parent group (so the whole
/// group's guide lines light up), invisible otherwise.
///
/// The lines used to be drawn all the time. With the indent, they were most of
/// what made the tree look deep and busy, and the indent alone shows nesting.
/// During a drag they still earn their place: they are the only thing that
/// shows which group a drop would land in. Value-diffed, so it's free on
/// non-drag frames.
fn highlight_line_on_drag(commands: &mut Commands, line: Entity, owner: Entity) {
    commands.entity(line).insert(BackgroundColor(Color::NONE));
    bind_bg(commands, line, move |world: &Rx| {
        if drag_highlight_parent(world) == Some(owner) {
            Color::WHITE
        } else {
            Color::NONE
        }
    });
}

/// Build one row entity (returns it; the keyed list parents it).
pub(crate) fn build_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    s: &RowSnapshot,
) -> Entity {
    let content_x = BASE_X + s.depth as f32 * INDENT;

    // Rows are flat, coloured or not. A label colour used to fill the row's
    // highlight pill and paint the name: the pill is inset from the row above
    // and below, so a coloured subtree showed as a stack of bands with gaps
    // between them, and the saturated names fought each other. The colour is
    // now the edge line below and, on the entity it was set on, the icon.
    let base_bg = Color::NONE;
    let label_color = rgb(text_primary());

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
            // Whole-row hit box for asset drops — wider than the click layer
            // below, which stops short of the eye/lock strip.
            bevy::ui::RelativeCursorPosition::default(),
            HierRowArea { entity: s.entity },
            Name::new("hier-row"),
        ))
        .id();

    let mut kids: Vec<Entity> = Vec::new();

    for (level, &has_more) in s.parent_lines.iter().enumerate() {
        if has_more {
            let x = BASE_X + level as f32 * INDENT + LINE_OFFSET;
            let line = renzora_ember::widgets::tree_vline(commands, x, 0.0, true, 0.0);
            if let Some(&owner) = s.ancestors.get(level) {
                highlight_line_on_drag(commands, line, owner);
            }
            kids.push(line);
        }
    }
    if s.depth > 0 {
        let x = BASE_X + (s.depth - 1) as f32 * INDENT + LINE_OFFSET;
        let vline = renzora_ember::widgets::tree_vline(commands, x, 0.0, !s.is_last, CENTER_Y);
        let hline = renzora_ember::widgets::tree_hline(commands, x, CENTER_Y, 5.0);
        // Both this row's vertical stub and its horizontal tick belong to its
        // direct parent (`ancestors[depth-1]`).
        if let Some(&owner) = s.ancestors.get(s.depth - 1) {
            highlight_line_on_drag(commands, vline, owner);
            highlight_line_on_drag(commands, hline, owner);
        }
        kids.push(vline);
        kids.push(hline);
    }

    // The label colour's edge line: the row's full height, so a coloured
    // entity and its subtree (which inherits the colour) draw one unbroken
    // line down the left of the panel.
    if let Some(c) = s.label_color {
        kids.push(
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(soft_label(c)),
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
                    font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                    font_size: bevy::text::FontSize::Px(12.0),
                    ..default()
                },
                TextColor(rgb(color)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(caret).insert((
            UiInteraction::default(),
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
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(renzora_ember::inspector::LIST_ROW_ICON),
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
                ui_font(&fonts.ui, renzora_ember::inspector::LIST_ROW_TEXT),
                TextColor(label_color),
                bevy::text::TextLayout::no_wrap(),
                Pickable::IGNORE,
                // Not for picking (the click layer covers it) — for the rect the
                // double-click-to-rename gesture is measured against. See
                // [`HierRowLabel`].
                HierRowLabel(s.entity),
            ))
            .id();
        commands.entity(label_box).add_child(label);
        if s.is_default_camera {
            let star = commands
                .spawn((
                    Text::new(glyph_str("star")),
                    TextFont {
                        font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                        font_size: bevy::text::FontSize::Px(10.0),
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

    // Asset badges — a hint that a script / blueprint / material rides on this
    // entity, so you can read it off the row without opening the inspector.
    // Clicking one jumps to that asset's editor (code / blueprint / material).
    // An entity can carry both a script and a blueprint, so these aren't
    // exclusive.
    for (present, glyph, color, kind) in [
        (s.has_script, "code", Color::srgb_u8(120, 170, 255), BadgeKind::Script),
        (s.has_blueprint, "blueprint", Color::srgb_u8(100, 180, 255), BadgeKind::Blueprint),
        (s.has_material, "palette", Color::srgb_u8(0, 200, 130), BadgeKind::Material),
    ] {
        if present {
            kids.push(badge_icon(commands, fonts, glyph, color, s.entity, kind));
        }
    }

    if s.is_emitting {
        kids.push(audio_bars(commands));
    }

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

    // Click layer (covers row except suffix zone). It's spawned after — and so
    // sits above — the label box, so while this row is being inline-renamed it
    // would otherwise swallow clicks meant for the rename field (stealing focus →
    // the edit cancels, and re-selecting the row). Make it ignore picks during
    // rename so clicks reach the field beneath it (moving the caret); clicks off
    // the field then just blur-commit as expected.
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
            UiInteraction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            HierRowClick { entity: s.entity },
            Name::new("hier-row-hit"),
        ))
        .id();
    if s.is_renaming {
        commands.entity(click_layer).insert(Pickable::IGNORE);
    }
    // A name wider than the row ends in "…" before the badges and icons
    // instead of being cut mid-letter by the label box's clip; the full name is
    // the row's tooltip while it is shortened. The click layer is the host
    // because the label itself ignores the pointer.
    if let Some(label) = label_ent {
        renzora_ember::widgets::ellipsize(commands, label, s.name.clone(), Some(click_layer));
    }
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
    // The row's highlight (visual, lowest z): a rounded pill inset from the
    // panel's sides, which carries hover and selection.
    let bg_visual = commands
        .spawn((
            renzora_ember::inspector::list_row_highlight_node(),
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
    bind_bg(commands, bg_visual, move |world: &Rx| {
        // Drop-target tint while dragging onto this row (reparent).
        if let Some(drag) = world.get_resource::<HierDrag>() {
            if drag.active
                && matches!(drag.target, Some((t, TreeDropZone::AsChild)) if t == ent)
            {
                return over(base, rgb(accent()).with_alpha(0.35));
            }
        }
        // Same tint for an asset drag (script / blueprint / material) hovering
        // this row — the thing about to be attached to it.
        if world
            .get_resource::<super::asset_drop::ArmedHierAssetDrop>()
            .is_some_and(|a| a.target() == Some(ent))
        {
            return over(base, rgb(accent()).with_alpha(0.35));
        }
        if world
            .get_resource::<EditorSelection>()
            .is_some_and(|sel| sel.is_selected(ent))
        {
            return over(base, renzora_ember::inspector::list_row_selected());
        }
        let hovered = world
            .get::<UiInteraction>(click)
            .is_some_and(|i| matches!(i, UiInteraction::Hovered | UiInteraction::Pressed));
        if hovered {
            over(base, renzora_ember::inspector::list_row_hover())
        } else {
            base
        }
    });
    if let Some(label) = label_ent {
        bind_text_color(commands, label, move |world: &Rx| {
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

/// A small clickable asset badge (script / blueprint / material) sitting just
/// left of the eye/lock toggles. Clicking it opens the asset in its editor (see
/// [`super::systems::hierarchy_badge_click`]). The glyph child is
/// `Pickable::IGNORE` so the hit lands on the badge button, not the text.
fn badge_icon(
    commands: &mut Commands,
    fonts: &EmberFonts,
    glyph: &str,
    color: Color,
    entity: Entity,
    kind: BadgeKind,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                margin: UiRect::right(Val::Px(3.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            UiInteraction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            HierAssetBadge { entity, kind },
            Name::new("hier-asset-badge"),
        ))
        .id();
    let g = commands
        .spawn((
            Text::new(glyph_str(glyph)),
            TextFont {
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(12.0),
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(btn).add_child(g);
    btn
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
            UiInteraction::default(),
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
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(13.0),
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(btn).add_child(g);
    btn
}


/// One bar of the playing indicator, with the phase that offsets its bounce.
#[derive(Component)]
pub struct AudioBar {
    phase: f32,
}

/// Three little bars that bounce while a sound plays on this entity.
///
/// Animated rather than a static speaker glyph because the question it answers is
/// "is this one making noise *now*", and a still icon answers "this entity has an
/// AudioPlayer" — which the inspector already says, and which stays true when the
/// sound has stopped.
fn audio_bars(commands: &mut Commands) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(1.0),
                ..default()
            },
            // The row is a status light, not a control: a click belongs to the
            // row underneath it, which is what selects the entity.
            Pickable::IGNORE,
            Name::new("hier-audio-bars"),
        ))
        .id();
    // Thirds of a cycle apart, so the three never rise together and the group
    // reads as movement rather than as one blinking block.
    let bars: Vec<Entity> = [0.0f32, 0.33, 0.66]
        .iter()
        .map(|phase| {
            commands
                .spawn((
                    Node {
                        width: Val::Px(2.0),
                        height: Val::Px(4.0),
                        border_radius: BorderRadius::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb_u8(120, 200, 140)),
                    AudioBar { phase: *phase },
                    Pickable::IGNORE,
                ))
                .id()
        })
        .collect();
    commands.entity(row).add_children(&bars);
    row
}

/// Bounce the bars.
///
/// Driven from wall time rather than from real levels: the hierarchy has no
/// meter to read, and a row indicator only has to say "this is playing". Reading
/// per-entity levels would mean a boundary call per row per frame to animate
/// fourteen pixels.
pub fn animate_audio_bars(time: Res<Time>, mut bars: Query<(&AudioBar, &mut Node)>) {
    if bars.is_empty() {
        return;
    }
    let t = time.elapsed_secs();
    for (bar, mut node) in &mut bars {
        // 3 Hz — fast enough to read as audio, slow enough not to strobe.
        let wave = ((t * 3.0 + bar.phase * std::f32::consts::TAU).sin() + 1.0) * 0.5;
        node.height = Val::Px(3.0 + wave * 8.0);
    }
}
