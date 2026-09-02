//! Click-to-select: the raycast, and the granularity walk that turns a hit mesh
//! into the entity a click should actually select.
//!
//! The press does not commit a selection. It arms [`BoxSelectionState`] with a
//! *pending* pick and `box_select.rs` decides on release whether the gesture was
//! a click or a drag — that is what lets a drag-select start on an entity as
//! well as on empty space.

use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{NavOverlayState, ViewportState};
use renzora::SelectionStop;
use renzora_editor_framework::{
    EditorCamera, EditorSettings, HideInHierarchy, SelectionGranularity,
};

use crate::modal_transform;
use crate::types::{BoxSelectionState, GizmoMesh, GizmoMode, GizmoRoot, GizmoState};

pub(crate) fn entity_pick_system(
    gizmo_state: Res<GizmoState>,
    mode: Res<GizmoMode>,
    modal: Res<modal_transform::ModalTransformState>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    viewport: Option<Res<ViewportState>>,
    nav_overlay: Option<Res<NavOverlayState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    settings: Res<EditorSettings>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mesh_ray_cast: MeshRayCast,
    named_entities: Query<(Entity, Has<SelectionStop>), With<Name>>,
    parent_query: Query<&ChildOf>,
    gizmo_meshes: Query<(), Or<(With<GizmoMesh>, With<GizmoRoot>)>>,
    hidden_entities: Query<(), With<HideInHierarchy>>,
    mut box_sel: ResMut<BoxSelectionState>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }
    if modal.active {
        return;
    }
    if gizmo_state.active_axis.is_some() || gizmo_state.hovered_axis.is_some() {
        return;
    }
    // Suspend picking while editing a collider — clicks drive handle drags instead.
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        // If a handle is hovered or being dragged, fully consume the click.
        // Otherwise still suppress to avoid deselecting while the user is tweaking.
        return;
    }
    // GizmoMode::None means a plugin tool is driving — skip picking.
    if *mode == GizmoMode::None {
        return;
    }
    // Don't pick while nav overlay buttons (pan/zoom/orbit) are being dragged
    if let Some(ref nav) = nav_overlay {
        if nav.pan_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav.zoom_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav
                .orbit_dragging
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
    }

    let Some(viewport) = viewport.as_ref() else {
        return;
    };
    if !viewport.hovered {
        return;
    }

    let Ok(window) = window_q.single() else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let vp_local = cursor - viewport.screen_position;
    if vp_local.x < 0.0
        || vp_local.y < 0.0
        || vp_local.x > viewport.screen_size.x
        || vp_local.y > viewport.screen_size.y
    {
        return;
    }

    // The render target may be smaller than the on-screen panel (Half / Quarter
    // resolution), so map the panel-local cursor into render-target pixels
    // before building the pick ray — otherwise clicks land off-target.
    if viewport.screen_size.x <= 0.0 || viewport.screen_size.y <= 0.0 {
        return;
    }
    let render_pos = Vec2::new(
        vp_local.x / viewport.screen_size.x * viewport.current_size.x as f32,
        vp_local.y / viewport.screen_size.y * viewport.current_size.y as f32,
    );

    // Modifiers are read at release time in `box_selection_system` — on
    // press we just arm the gesture.
    let Ok(ray) = camera.viewport_to_world(cam_gt, render_pos) else {
        return;
    };

    // Raycast and find the topmost selectable entity (if any). We do NOT
    // commit the selection yet — we arm `box_sel` with this entity as a
    // pending pick and wait for mouse-up to decide click vs drag.
    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings { ..default() });
    let mut pending: Option<Entity> = None;
    for (entity, _hit) in hits.iter() {
        if gizmo_meshes.get(*entity).is_ok() {
            continue;
        }
        if hidden_entities.get(*entity).is_ok() {
            continue;
        }

        if let Some(target) = resolve_pick(
            *entity,
            settings.selection_granularity,
            &named_entities,
            &parent_query,
            &hidden_entities,
        ) {
            // `resolve_pick` already skips hidden named ancestors, so `target`
            // is a visible row — this guard is a belt-and-braces no-op.
            if hidden_entities.get(target).is_ok() {
                continue;
            }
            pending = Some(target);
            break;
        }
    }

    // Arm the gesture. `box_selection_system` reads these fields each frame
    // and finalises on release. Only arm from tools where box / click
    // selection is meaningful.
    if matches!(
        *mode,
        GizmoMode::Select | GizmoMode::Translate | GizmoMode::Rotate | GizmoMode::Scale
    ) {
        box_sel.active = true;
        box_sel.start_pos = cursor;
        box_sel.current_pos = cursor;
        box_sel.pending_pick = pending;
    }
}

/// Resolve a raycast-hit entity to the entity a click should select, per the
/// configured [`SelectionGranularity`].
///
/// Walking up from the hit mesh toward the scene root, three candidates fall
/// out of a single pass:
///   - `leaf`  — the nearest named ancestor (the clicked mesh itself)
///   - `group` — the topmost named ancestor still *below* a `SelectionStop`
///     boundary (the clicked mesh's own sub-root within an imported model)
///   - `root`  — the `SelectionStop` bearer (the whole imported model), or the
///     topmost named ancestor when the chain has no stop.
///
/// `SelectionStop` marks a compound boundary (an imported model root, terrain
/// root, etc.) whose internals are selected as a unit under `EntireRoot`.
///
/// Entities tagged [`HideInHierarchy`] are transparent here: an imported model
/// often carries a named-but-hidden GLTF wrapper (`RootNode`, `Scene`, …) that
/// `hide_gltf_wrappers` flagged when flatten couldn't collapse it. The hierarchy
/// panel hides those rows and re-parents their children to the nearest visible
/// ancestor, so the click resolution must mirror that — otherwise `MeshRoot`
/// could land on a hidden wrapper and the caller would reject it, selecting
/// nothing.
pub(crate) fn resolve_pick(
    entity: Entity,
    granularity: SelectionGranularity,
    named: &Query<(Entity, Has<SelectionStop>), With<Name>>,
    parents: &Query<&ChildOf>,
    hidden: &Query<(), With<HideInHierarchy>>,
) -> Option<Entity> {
    let mut leaf: Option<Entity> = None;
    let mut group: Option<Entity> = None;
    let mut root: Option<Entity> = None;
    let mut current = entity;
    loop {
        if let Ok((e, stop)) = named.get(current) {
            let visible = hidden.get(e).is_err();
            if visible {
                if leaf.is_none() {
                    leaf = Some(e);
                }
                if !stop {
                    group = Some(e);
                }
            }
            if stop {
                // A `SelectionStop` is a boundary even if the bearer is hidden,
                // but only a *visible* stop is a valid `EntireRoot` target.
                if visible {
                    root = Some(e);
                }
                break;
            }
        }
        match parents.get(current) {
            Ok(child_of) => current = child_of.parent(),
            Err(_) => break,
        }
    }
    // No `SelectionStop` in the chain: the whole-model root is just the topmost
    // visible named ancestor, which is exactly `group`.
    let root = root.or(group);
    match granularity {
        SelectionGranularity::Mesh => leaf,
        SelectionGranularity::MeshRoot => group.or(leaf),
        SelectionGranularity::EntireRoot => root.or(leaf),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn run_resolve_pick(
        world: &mut World,
        start: Entity,
        g: SelectionGranularity,
    ) -> Option<Entity> {
        world
            .run_system_once(
                move |named: Query<(Entity, Has<SelectionStop>), With<Name>>,
                      parents: Query<&ChildOf>,
                      hidden: Query<(), With<HideInHierarchy>>| {
                    resolve_pick(start, g, &named, &parents, &hidden)
                },
            )
            .unwrap()
    }

    #[test]
    fn resolve_pick_no_stop_returns_leaf_or_topmost() {
        let mut world = World::new();
        let root = world.spawn(Name::new("Root")).id();
        let mesh = world.spawn((Name::new("Mesh"), ChildOf(root))).id();
        use SelectionGranularity::*;
        // Without a SelectionStop boundary, Mesh = the clicked mesh; MeshRoot
        // and EntireRoot both bubble to the topmost named ancestor.
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(root));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(root));

        // An unnamed child resolves to its nearest named ancestor.
        let unnamed = world.spawn(ChildOf(mesh)).id();
        assert_eq!(run_resolve_pick(&mut world, unnamed, Mesh), Some(mesh));
    }

    #[test]
    fn resolve_pick_distinguishes_granularity_at_stop_boundary() {
        // model (SelectionStop) → group → mesh
        let mut world = World::new();
        let model = world.spawn((Name::new("Model"), SelectionStop)).id();
        let group = world.spawn((Name::new("Building"), ChildOf(model))).id();
        let mesh = world.spawn((Name::new("Wall"), ChildOf(group))).id();
        use SelectionGranularity::*;
        // Mesh = clicked leaf; MeshRoot = topmost named below the stop (the
        // sub-object); EntireRoot = the whole model at the stop.
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(group));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(model));
    }

    #[test]
    fn resolve_pick_flat_model_meshroot_is_mesh() {
        // model (SelectionStop) → mesh directly. MeshRoot collapses to the mesh
        // since there is no intermediate group below the stop.
        let mut world = World::new();
        let model = world.spawn((Name::new("Car"), SelectionStop)).id();
        let mesh = world.spawn((Name::new("Wheel"), ChildOf(model))).id();
        use SelectionGranularity::*;
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(model));
    }

    #[test]
    fn resolve_pick_skips_hidden_wrapper_between_root_and_mesh() {
        // model (SelectionStop) → RootNode (named + HideInHierarchy) → mesh.
        // The hidden wrapper must be transparent: MeshRoot resolves to the mesh
        // (the topmost VISIBLE named below the stop), not the hidden wrapper —
        // otherwise the caller rejects the hidden target and nothing selects.
        let mut world = World::new();
        let model = world.spawn((Name::new("Model"), SelectionStop)).id();
        let wrapper = world
            .spawn((Name::new("RootNode"), HideInHierarchy, ChildOf(model)))
            .id();
        let mesh = world.spawn((Name::new("Wall"), ChildOf(wrapper))).id();
        use SelectionGranularity::*;
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(model));
    }

    #[test]
    fn resolve_pick_unnamed_chain_returns_none() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let child = world.spawn(ChildOf(root)).id();
        assert_eq!(
            run_resolve_pick(&mut world, child, SelectionGranularity::MeshRoot),
            None
        );
    }
}
