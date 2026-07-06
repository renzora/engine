//! Parent stacking ("sticky" ancestors). As you scroll the tree, the ancestor
//! chain of the topmost visible row pins to the top of the panel as compact
//! header rows, so you never lose track of where a deep row lives. Toggled by
//! `EditorSettings.hierarchy_parent_stacking` (Settings → Viewport → Hierarchy).
//!
//! The pinned rows live in an absolutely-positioned overlay over the top of the
//! scroll viewport (so they don't scroll). A pinned header reflects the current
//! selection (accent tint + white label, like a real row) and, because a pinned
//! ancestor always has children, clicking it toggles that ancestor's subtree
//! (it carries [`HierCaretToggle`], driven by the shared caret-click system) —
//! so you can collapse/expand the branch straight from the sticky header without
//! it stealing or dropping the selection.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::ScrollPosition;
use bevy::window::SystemCursorIcon;

use renzora_editor_framework::{EditorSelection, EditorSettings};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_glyph, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_bg, bind_text_color};
use renzora_ember::theme::{accent, border, header_bg, rgb, text_primary};

use crate::cache::HierarchyTreeCache;
use crate::state::EntityNode;

use super::components::HierPinClick;
use super::row::{BASE_X, INDENT, ROW_H};
use super::tree::HierFlatCache;
use super::{HierExpanded, HierRevealPending, HierScrollContent};

/// The overlay container + the currently-pinned chain (for change-diffing).
#[derive(Resource)]
pub(crate) struct HierParentStack {
    pub container: Entity,
    /// Pinned ancestor entities, root → … → parent. Empty when not stacking.
    pub current: Vec<Entity>,
}

/// Spawn the (initially hidden, empty) overlay that holds pinned header rows.
/// Added as a child of the scroll *outer* so it overlays the viewport at the top.
pub(crate) fn build_stack_container(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                display: Display::None,
                ..default()
            },
            // Above the scrolling content, below the scrollbar track (ZIndex 50).
            ZIndex(20),
            // The container itself is click-through; each header's click layer opts in.
            Pickable::IGNORE,
            Name::new("hier-parent-stack"),
        ))
        .id()
}

/// Owned snapshot of one pinned header (so we don't hold a borrow into the cache
/// across the despawn/spawn commands).
struct PinnedRow {
    entity: Entity,
    name: String,
    icon: &'static str,
    icon_color: [u8; 3],
    label_color: Option<[u8; 3]>,
    depth: usize,
}

/// Collect the ancestor chain of `target` (root → … → parent, excluding `target`)
/// as references into the cached tree. Returns `true` once `target` is found.
fn ancestor_chain<'a>(
    nodes: &'a [EntityNode],
    target: Entity,
    out: &mut Vec<&'a EntityNode>,
) -> bool {
    for n in nodes {
        out.push(n);
        if n.entity == target {
            out.pop(); // exclude the target itself — only its ancestors pin
            return true;
        }
        if ancestor_chain(&n.children, target, out) {
            return true;
        }
        out.pop();
    }
    false
}

/// Recompute the pinned ancestor chain from the current scroll offset and, when
/// it changes, rebuild the overlay's header rows. Runs every frame (cheap — it
/// early-returns unless the chain actually changed).
#[allow(clippy::too_many_arguments)]
pub(crate) fn hierarchy_parent_stack(
    mut commands: Commands,
    settings: Res<EditorSettings>,
    cache: Res<HierarchyTreeCache>,
    flat: Res<HierFlatCache>,
    fonts: Res<EmberFonts>,
    stack: Option<ResMut<HierParentStack>>,
    content: Query<Entity, With<HierScrollContent>>,
    parents: Query<&ChildOf>,
    scroll_pos: Query<&ScrollPosition>,
    children_q: Query<&Children>,
    mut node_q: Query<&mut Node>,
) {
    let Some(mut stack) = stack else {
        return;
    };

    // Scroll offset of this panel's own viewport (parent of the marked content).
    let offset = content
        .iter()
        .next()
        .and_then(|list| parents.get(list).ok().map(|c| c.parent()))
        .and_then(|vp| scroll_pos.get(vp).ok())
        .map(|sp| sp.y)
        .unwrap_or(0.0);

    // The pinned chain: nothing when disabled or scrolled to the very top.
    let pinned: Vec<PinnedRow> = if !settings.hierarchy_parent_stacking || offset <= 0.5 {
        Vec::new()
    } else {
        let order = &flat.order;
        let top_idx = (offset / ROW_H).floor() as usize;
        match order.get(top_idx).copied() {
            Some(top) => {
                let mut chain = Vec::new();
                ancestor_chain(&cache.nodes, top, &mut chain);
                chain
                    .iter()
                    .enumerate()
                    .map(|(depth, n)| PinnedRow {
                        entity: n.entity,
                        name: n.name.clone(),
                        icon: n.icon,
                        icon_color: n.icon_color,
                        label_color: n.label_color,
                        depth,
                    })
                    .collect()
            }
            None => Vec::new(),
        }
    };

    let new_entities: Vec<Entity> = pinned.iter().map(|p| p.entity).collect();
    if new_entities == stack.current {
        return; // unchanged — leave the existing header rows in place
    }
    stack.current = new_entities;

    // Clear old header rows (despawn is recursive in Bevy 0.18).
    if let Ok(kids) = children_q.get(stack.container) {
        for child in kids.iter() {
            commands.entity(child).despawn();
        }
    }

    if pinned.is_empty() {
        if let Ok(mut node) = node_q.get_mut(stack.container) {
            node.display = Display::None;
        }
        return;
    }

    if let Ok(mut node) = node_q.get_mut(stack.container) {
        node.display = Display::Flex;
    }

    let mut rows: Vec<Entity> = pinned
        .iter()
        .map(|p| build_pinned_row(&mut commands, &fonts, p))
        .collect();
    // A 1px divider under the stack to separate it from the scrolling content.
    rows.push(
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(rgb(border())),
                Pickable::IGNORE,
            ))
            .id(),
    );
    commands.entity(stack.container).add_children(&rows);
}

/// Build one compact pinned header row: opaque background, indent, type icon,
/// label, and a transparent click layer that selects the ancestor.
fn build_pinned_row(commands: &mut Commands, fonts: &EmberFonts, p: &PinnedRow) -> Entity {
    let content_x = BASE_X + p.depth as f32 * INDENT;
    let label_color = p
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
            Name::new("hier-pin-row"),
        ))
        .id();

    // Opaque background so the scrolling content doesn't bleed through.
    let bg = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            Pickable::IGNORE,
        ))
        .id();

    // Selection tint over the opaque background — value-diffed, so it only
    // repaints when this ancestor's selected-ness changes.
    let sel_tint = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
        ))
        .id();
    let ent = p.entity;
    bind_bg(commands, sel_tint, move |world: &World| {
        if world
            .get_resource::<EditorSelection>()
            .is_some_and(|sel| sel.is_selected(ent))
        {
            rgb(accent()).with_alpha(0.63)
        } else {
            Color::NONE
        }
    });

    let spacer = commands
        .spawn((
            Node {
                width: Val::Px(content_x + 16.0),
                height: Val::Px(ROW_H),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();

    let icon = commands
        .spawn((
            Text::new(icon_glyph(p.icon).unwrap_or('\u{E4C6}').to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(14.0),
                ..default()
            },
            TextColor(Color::srgb_u8(p.icon_color[0], p.icon_color[1], p.icon_color[2])),
            Node {
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();

    let label = commands
        .spawn((
            Text::new(&p.name),
            ui_font(&fonts.ui, 13.0),
            TextColor(label_color),
            bevy::text::TextLayout::no_wrap(),
            Node {
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
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

    // Transparent click layer on top → collapses this ancestor's branch, keeps it
    // selected, and scrolls back to its real row (see `hierarchy_pin_click`).
    let click = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            HierPinClick { entity: p.entity },
            Name::new("hier-pin-hit"),
        ))
        .id();

    commands
        .entity(row)
        .add_children(&[bg, sel_tint, spacer, icon, label, click]);
    row
}

/// Click a sticky parent-stack header → collapse that branch, deselect anything
/// inside it, and scroll the tree back to its real row.
///
/// A pinned ancestor is only ever shown because it's expanded, so the toggle
/// closes it. Collapsing hides the whole branch, so if the current selection is
/// that header *or one of its descendants* it's no longer visible — we clear it,
/// which is what makes both "the header itself was selected" and "a child was
/// selected" end up deselected. A selection elsewhere in the tree is left alone.
///
/// We arm the reveal directly rather than relying on the selection change: after
/// a deselect there's nothing for `hierarchy_reveal_selection` to reveal (and it
/// deliberately won't stomp this), so `hierarchy_scroll_to_selection` brings the
/// collapsed branch root back into view — which also scrolls it out from under
/// the pinned overlay (un-pinning it).
pub(crate) fn hierarchy_pin_click(
    q: Query<(&Interaction, &HierPinClick), Changed<Interaction>>,
    selection: Option<Res<EditorSelection>>,
    cache: Res<HierarchyTreeCache>,
    mut expanded: ResMut<HierExpanded>,
    mut pending: ResMut<HierRevealPending>,
) {
    let Some(selection) = selection else {
        return;
    };
    for (interaction, pin) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        expanded.0.remove(&pin.entity);

        // Deselect if the selection lives inside the branch we just hid.
        if let Some(sel) = selection.get() {
            let in_branch = sel == pin.entity || {
                let mut chain = Vec::new();
                ancestor_chain(&cache.nodes, sel, &mut chain);
                chain.iter().any(|n| n.entity == pin.entity)
            };
            if in_branch {
                selection.set(None);
            }
        }

        pending.entity = Some(pin.entity);
        pending.frames = 0;
        pending.decided = false;
        pending.scroll = false;
    }
}
