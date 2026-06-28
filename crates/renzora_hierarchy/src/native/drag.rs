//! Native hierarchy drag-to-reorder / reparent.
//!
//! Press-drag a row >5px to start dragging it (or the whole selection if the
//! pressed row is part of a multi-select). The row under the cursor is the drop
//! target; the cursor's vertical position within it picks the zone — top third
//! `Before`, bottom third `After`, middle `AsChild` (reparent). On release the
//! move is applied as an undoable `Reorder` compound (mirrors the egui panel).

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::{EditorCommands, EditorSelection, HierarchyOrder, TreeDropZone};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::rgb;
use renzora_undo::{record, CompoundCmd, ReparentCmd, SetHierarchyOrderCmd, UndoCommand, UndoContext};

use super::components::{HierDropEdge, HierRowClick};

#[derive(Component)]
pub(crate) struct HierDragTooltip;
#[derive(Component)]
pub(crate) struct HierDragTooltipText;

/// Live drag state for the hierarchy.
#[derive(Resource, Default)]
pub(crate) struct HierDrag {
    /// Entities being dragged (set once the press promotes to a drag).
    pub entities: Vec<Entity>,
    /// Pending press `(row, cursor)` before the 5px threshold promotes it.
    press: Option<(Entity, Vec2)>,
    /// True once the drag is active (moved past the threshold).
    pub active: bool,
    /// Current drop target row + zone.
    pub target: Option<(Entity, TreeDropZone)>,
}

pub(crate) fn hier_drag(
    mut drag: ResMut<HierDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    selection: Option<Res<EditorSelection>>,
    rows: Query<(&Interaction, &RelativeCursorPosition, &HierRowClick)>,
    mut edges: Query<(&HierDropEdge, &mut Node)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    // Release → apply the drop, then reset.
    if mouse.just_released(MouseButton::Left) {
        if drag.active {
            if let (Some((target, zone)), Some(cmds)) = (drag.target, &cmds) {
                let entities = std::mem::take(&mut drag.entities);
                apply_drop(cmds, entities, target, zone);
            }
        }
        drag.entities.clear();
        drag.press = None;
        drag.active = false;
        drag.target = None;
        for (_, mut node) in &mut edges {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
        return;
    }

    // Press → remember the pressed row.
    if mouse.just_pressed(MouseButton::Left) {
        if let (Some(c), Some((_, _, row))) = (
            cursor,
            rows.iter().find(|(i, _, _)| matches!(i, Interaction::Pressed)),
        ) {
            drag.press = Some((row.entity, c));
        }
    }

    // Promote the press to an active drag once the cursor moves >5px.
    if !drag.active {
        if let (Some((row_e, origin)), Some(c)) = (drag.press, cursor) {
            if c.distance(origin) > 5.0 {
                drag.active = true;
                let multi = selection
                    .as_ref()
                    .is_some_and(|s| s.has_multi_selection() && s.is_selected(row_e));
                drag.entities = if multi {
                    selection
                        .as_ref()
                        .map(|s| s.get_all())
                        .unwrap_or_else(|| vec![row_e])
                } else {
                    vec![row_e]
                };
            }
        }
    }
    if !drag.active {
        return;
    }

    // Drop target = the row under the cursor that isn't being dragged.
    let mut target = None;
    for (_i, rcp, row) in &rows {
        if !rcp.cursor_over || drag.entities.contains(&row.entity) {
            continue;
        }
        let y = rcp.normalized.map(|n| n.y).unwrap_or(0.0); // -0.5 top .. 0.5 bottom
        let zone = if y < -1.0 / 6.0 {
            TreeDropZone::Before
        } else if y > 1.0 / 6.0 {
            TreeDropZone::After
        } else {
            TreeDropZone::AsChild
        };
        target = Some((row.entity, zone));
        break;
    }
    // Keep the last target while the cursor is in a gap (the suffix-toggle column,
    // the empty space below the last row, or a sub-pixel seam between rows) so the
    // drop indicator + parent-line highlight stay put instead of flickering off.
    // Only clears on release (the reset above).
    if target.is_some() {
        drag.target = target;
    }

    // Toggle the per-row Before/After edge indicators.
    let (tgt_e, tgt_z) = match drag.target {
        Some((e, z)) => (Some(e), Some(z)),
        None => (None, None),
    };
    for (edge, mut node) in &mut edges {
        let show = tgt_e == Some(edge.entity)
            && matches!(
                (tgt_z, edge.after),
                (Some(TreeDropZone::Before), false) | (Some(TreeDropZone::After), true)
            );
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}

/// A cursor-following tooltip describing the pending drop ("Move above/below/
/// into {name}" / "Moving N entities"), shown while a drag is active.
pub(crate) fn hier_drag_tooltip(
    mut commands: Commands,
    drag: Res<HierDrag>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    names: Query<&Name>,
    mut root_q: Query<(Entity, &mut Node), With<HierDragTooltip>>,
    mut text_q: Query<&mut Text, With<HierDragTooltipText>>,
) {
    if !drag.active {
        for (e, _) in &root_q {
            commands.entity(e).despawn();
        }
        return;
    }
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let label = |e: Entity| {
        names
            .get(e)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|_| "entity".to_string())
    };
    let count = drag.entities.len();
    let text = match drag.target {
        Some((t, zone)) => {
            let verb = match zone {
                TreeDropZone::Before => "above",
                TreeDropZone::After => "below",
                TreeDropZone::AsChild => "into",
            };
            if count > 1 {
                format!("Move {count} entities {verb} {}", label(t))
            } else {
                format!("Move {verb} {}", label(t))
            }
        }
        None if count > 1 => format!("Moving {count} entities"),
        None => format!(
            "Moving {}",
            drag.entities.first().map(|e| label(*e)).unwrap_or_default()
        ),
    };

    if let Ok((_, mut node)) = root_q.single_mut() {
        node.left = Val::Px(cursor.x + 14.0);
        node.top = Val::Px(cursor.y + 18.0);
        if let Ok(mut t) = text_q.single_mut() {
            if t.0 != text {
                t.0 = text;
            }
        }
        return;
    }

    // Spawn the tooltip (fonts may not be ready on the first frame).
    let Some(fonts) = fonts else {
        return;
    };
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 14.0),
                top: Val::Px(cursor.y + 18.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.11, 0.11, 0.14, 0.94)),
            BorderColor::all(rgb(renzora_ember::theme::accent())),
            GlobalZIndex(10_000),
            Pickable::IGNORE,
            HierDragTooltip,
            Name::new("hier-drag-tooltip"),
        ))
        .id();
    let txt = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(renzora_ember::theme::text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Pickable::IGNORE,
            HierDragTooltipText,
        ))
        .id();
    commands.entity(root).add_child(txt);
}

/// Would re-parenting `entity` under `new_parent` create a cycle? True if
/// `new_parent` is `entity` itself or sits inside `entity`'s own subtree.
/// (Reparenting onto a descendant produces an A→B→A loop that breaks Bevy's
/// transform propagation — and with it all rendering, including the editor UI.)
fn would_cycle(world: &World, entity: Entity, new_parent: Entity) -> bool {
    let mut cur = new_parent;
    loop {
        if cur == entity {
            return true;
        }
        match world.get::<ChildOf>(cur) {
            Some(c) => cur = c.parent(),
            None => return false,
        }
    }
}

/// Apply a drop — reparent (`AsChild`) or reorder (`Before`/`After`), recording
/// an undoable `Reorder` compound. Guarded against cycles and editor-UI nodes.
fn apply_drop(
    cmds: &EditorCommands,
    drag_entities: Vec<Entity>,
    target: Entity,
    zone: TreeDropZone,
) {
    cmds.push(move |world: &mut World| {
        // Snapshot old parents + every root order before mutating.
        let old_parents: Vec<(Entity, Option<Entity>)> = drag_entities
            .iter()
            .map(|e| (*e, world.get::<ChildOf>(*e).map(|c| c.parent())))
            .collect();
        let mut old_orders: Vec<(Entity, Option<u32>)> = Vec::new();
        for archetype in world.archetypes().iter() {
            for arch_entity in archetype.entities() {
                let e = arch_entity.id();
                if world.get::<Name>(e).is_none() {
                    continue;
                }
                if world.get::<renzora::core::HideInHierarchy>(e).is_some() {
                    continue;
                }
                // Never touch editor chrome (bevy_ui nodes).
                if world.get::<bevy::ui::Node>(e).is_some() {
                    continue;
                }
                old_orders.push((e, world.get::<HierarchyOrder>(e).map(|h| h.0)));
            }
        }

        for entity in &drag_entities {
            if *entity == target {
                continue;
            }
            match zone {
                TreeDropZone::AsChild => {
                    if would_cycle(world, *entity, target) {
                        continue;
                    }
                    world.entity_mut(*entity).set_parent_in_place(target);
                }
                TreeDropZone::Before | TreeDropZone::After => {
                    let parent = world.get::<ChildOf>(target).map(|c| c.parent());
                    if let Some(p) = parent {
                        // Reparenting under a descendant of the dragged entity
                        // would form a cycle — skip.
                        if would_cycle(world, *entity, p) {
                            continue;
                        }
                        let target_idx = world
                            .get::<Children>(p)
                            .and_then(|children| children.iter().position(|c| c == target));
                        world.entity_mut(*entity).remove_parent_in_place();
                        if let Some(idx) = target_idx {
                            let new_target_idx = world
                                .get::<Children>(p)
                                .and_then(|children| children.iter().position(|c| c == target));
                            let mut final_idx = if let Some(nti) = new_target_idx {
                                if matches!(zone, TreeDropZone::After) {
                                    nti + 1
                                } else {
                                    nti
                                }
                            } else if matches!(zone, TreeDropZone::After) {
                                idx + 1
                            } else {
                                idx
                            };
                            // Clamp so insert_child can't index past the end.
                            let len = world.get::<Children>(p).map(|c| c.len()).unwrap_or(0);
                            final_idx = final_idx.min(len);
                            world.entity_mut(p).insert_child(final_idx, *entity);
                        } else {
                            world.entity_mut(*entity).set_parent_in_place(p);
                        }
                    } else {
                        // Root-level reorder: rewrite HierarchyOrder on all roots.
                        world.entity_mut(*entity).remove_parent_in_place();
                        let mut roots: Vec<(Entity, u32)> = Vec::new();
                        for archetype in world.archetypes().iter() {
                            for arch_entity in archetype.entities() {
                                let e = arch_entity.id();
                                if world.get::<Name>(e).is_none() {
                                    continue;
                                }
                                if world.get::<ChildOf>(e).is_some() {
                                    continue;
                                }
                                if world.get::<renzora::core::HideInHierarchy>(e).is_some() {
                                    continue;
                                }
                                if world.get::<bevy::ui::Node>(e).is_some() {
                                    continue;
                                }
                                let order =
                                    world.get::<HierarchyOrder>(e).map(|h| h.0).unwrap_or(u32::MAX);
                                roots.push((e, order));
                            }
                        }
                        roots.sort_by_key(|&(_, o)| o);
                        roots.retain(|&(e, _)| e != *entity);
                        let target_pos =
                            roots.iter().position(|&(e, _)| e == target).unwrap_or(0);
                        let insert_pos = if matches!(zone, TreeDropZone::After) {
                            target_pos + 1
                        } else {
                            target_pos
                        };
                        roots.insert(insert_pos, (*entity, 0));
                        for (i, &(e, _)) in roots.iter().enumerate() {
                            world.entity_mut(e).insert(HierarchyOrder(i as u32));
                        }
                    }
                }
            }
        }

        // Diff parents + orders into an undoable compound.
        let mut undo: Vec<Box<dyn UndoCommand>> = Vec::new();
        for (entity, old_parent) in old_parents {
            let new_parent = world.get::<ChildOf>(entity).map(|c| c.parent());
            if old_parent != new_parent {
                undo.push(Box::new(ReparentCmd {
                    entity,
                    old_parent,
                    new_parent,
                }));
            }
        }
        for (entity, old) in old_orders {
            let new = world.get::<HierarchyOrder>(entity).map(|h| h.0);
            if old != new {
                undo.push(Box::new(SetHierarchyOrderCmd { entity, old, new }));
            }
        }
        if !undo.is_empty() {
            record(
                world,
                UndoContext::Scene,
                Box::new(CompoundCmd {
                    label: "Reorder".into(),
                    cmds: undo,
                }),
            );
        }
    });
}
