//! Right-click in the viewport, without dragging, for a list of everything
//! under the cursor.
//!
//! A click selects one thing: the nearest mesh, or the object it belongs to
//! when that picks as a unit. The list is for everything else. It names every
//! mesh along the ray, nearest first, so a part behind another, inside a
//! grouped model, or locked can still be picked from the viewport; and the
//! named ancestors of the nearest one, so a car's wheel assembly is one click
//! away from its tyre. Choosing a row selects exactly that entity, whatever
//! the grouping or lock says, because choosing from the list is a deliberate
//! act on something already named.
//!
//! **Why a still right-click.** Right-drag is the camera's look and fly, and
//! Alt with the left button orbits, so neither combination is free. A right
//! press that ends without the mouse moving and without a key being held did
//! nothing before: the camera only acts on motion. Releasing promptly is
//! required too, so holding the button to look around and letting go never
//! opens it.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora_editor_framework::{EditorCamera, EditorSelection};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{menu_header, menu_item, menu_sep, screen_menu_flip};

use crate::modal_transform::ModalTransformState;
use crate::picking::{PickResolver, SceneRay};
use crate::ray::viewport_ray_at;
use crate::types::GizmoMode;

/// Pixels of mouse travel (summed over the hold) under which a right press is
/// a click. The camera's look moves with the first pixel, but a hand on a
/// mouse rarely stays perfectly still.
const STILL_TRAVEL: f32 = 4.0;

/// Seconds a right press may last and still count as a click. Longer is a
/// look held still, and its release should not open a menu.
const CLICK_SECONDS: f64 = 0.4;

/// Rows of meshes under the cursor at most; a ray through a dense scene can
/// cross hundreds.
const MAX_HITS: usize = 12;

/// The right press being watched.
#[derive(Default)]
pub(crate) struct RightPress {
    /// Where and when it went down, while it counts as a candidate click.
    at: Option<(Vec2, f64)>,
    travel: f32,
    /// A key was held during it, so it was a fly, not a click.
    keyed: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn pick_list_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    time: Res<Time>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<ViewportState>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    (modal, mode): (Res<ModalTransformState>, Res<GizmoMode>),
    mut scene_ray: SceneRay,
    resolver: PickResolver,
    names: Query<&Name>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
    mut press: Local<RightPress>,
) {
    let travelled: f32 = motion.read().map(|m| m.delta.length()).sum();

    if mouse_button.just_pressed(MouseButton::Right) {
        let cursor = window_q.single().ok().and_then(|w| w.cursor_position());
        // Not while a modal transform runs (a right press cancels it), and not
        // while a plugin tool owns the mouse (`GizmoMode::None`).
        let eligible = viewport.as_ref().is_some_and(|v| v.hovered)
            && !modal.active
            && *mode != GizmoMode::None;
        *press = RightPress {
            at: cursor.filter(|_| eligible).map(|c| (c, time.elapsed_secs_f64())),
            travel: 0.0,
            keyed: false,
        };
        return;
    }
    if press.at.is_none() {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        press.travel += travelled;
        press.keyed |= keyboard.get_pressed().next().is_some();
        return;
    }
    if !mouse_button.just_released(MouseButton::Right) {
        press.at = None;
        return;
    }
    let Some((at, when)) = press.at.take() else {
        return;
    };
    if press.travel + travelled > STILL_TRAVEL
        || press.keyed
        || time.elapsed_secs_f64() - when > CLICK_SECONDS
    {
        return;
    }

    let (Some(viewport), Some(fonts)) = (viewport.as_ref(), fonts) else {
        return;
    };
    let Ok((cam_gt, projection)) = camera_q.single() else {
        return;
    };
    let Some(ray) = viewport_ray_at(at, viewport, cam_gt, projection) else {
        return;
    };

    // One row per shown entity, nearest first: a model's mesh primitives
    // often stand for one named part, which should not be listed twice.
    let mut under: Vec<Entity> = Vec::new();
    for (hit, _) in scene_ray.hits(ray, cam_gt.translation()) {
        if let Some(leaf) = resolver.leaf(hit) {
            if !under.contains(&leaf) {
                under.push(leaf);
                if under.len() == MAX_HITS {
                    break;
                }
            }
        }
    }
    let Some(&nearest) = under.first() else {
        return;
    };
    let parents = resolver.ancestors(nearest);

    let name_of = |e: Entity| names.get(e).map_or_else(|_| format!("{e}"), |n| n.to_string());
    let win_h = window_q.single().map_or(f32::MAX, |w| w.height());
    let menu = screen_menu_flip(&mut commands, at.x, at.y, win_h);
    let mut rows = vec![menu_header(
        &mut commands,
        &fonts,
        &renzora::lang::t("viewport.pick_list.under_cursor"),
    )];
    for &e in &under {
        let mut label = name_of(e);
        // Say why a plain click would not have selected this one.
        if resolver.is_locked(e) {
            label = renzora::lang::t_args("viewport.pick_list.locked", &[("name", &label)]);
        } else if let Some(unit) = resolver.unit(e).filter(|&u| u != e) {
            label = renzora::lang::t_args(
                "viewport.pick_list.in_unit",
                &[("name", &label), ("unit", &name_of(unit))],
            );
        }
        let icon = if resolver.is_locked(e) { "lock-simple" } else { "cube" };
        rows.push(menu_item(&mut commands, &fonts, icon, &label, move |w| select(w, e)));
    }
    if !parents.is_empty() {
        rows.push(menu_sep(&mut commands));
        rows.push(menu_header(
            &mut commands,
            &fonts,
            &renzora::lang::t_args("viewport.pick_list.parents", &[("name", &name_of(nearest))]),
        ));
        for &e in &parents {
            let label = name_of(e);
            rows.push(menu_item(&mut commands, &fonts, "tree-structure", &label, move |w| {
                select(w, e)
            }));
        }
    }
    commands.entity(menu).add_children(&rows);
}

/// Select exactly `entity`, as a click on a hierarchy row does.
fn select(world: &mut World, entity: Entity) {
    if world.get_entity(entity).is_err() {
        return;
    }
    if let Some(selection) = world.get_resource::<EditorSelection>() {
        selection.set(Some(entity));
    }
}
