//! Click-to-select: the raycast, and the walk that turns a hit mesh into the
//! entity a click should actually select.
//!
//! **What a click selects.** The mesh it hits, unless something above that mesh
//! is marked to pick as one object ([`renzora::PickAsUnit`], the user's choice
//! from the hierarchy menu, or [`renzora::SelectionStop`], which code puts on
//! things like a terrain whose chunks are its own machinery); then the outermost
//! such ancestor. A locked entity, or anything under one, is passed through:
//! the click selects whatever is behind it. The rule is the same for a click,
//! a box selection and the right-click pick list ([`PickResolver`]).
//!
//! This replaced a global setting whose default picked "the topmost named
//! ancestor below the model root". That depended on every exporter's wrapper
//! nodes being recognised and hidden, and each one that was not (`scene0`,
//! Sketchfab's `root`) swallowed every click on the model: a car's wheel
//! selected the wrapper. Picking the hit mesh needs no knowledge of wrappers,
//! and grouping became something the user says per model.
//!
//! The press does not commit a selection. It arms [`BoxSelectionState`] with a
//! *pending* pick and `box_select.rs` decides on release whether the gesture was
//! a click or a drag — that is what lets a drag-select start on an entity as
//! well as on empty space. A press on an entity that is already selected arms a
//! move instead of a box (`drag.rs`).

use bevy::camera::visibility::{RenderLayers, VisibilityRange};
use bevy::ecs::system::SystemParam;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{NavOverlayState, ViewportState};
use renzora::{EditorLocked, PickAsUnit, SelectionStop};
use renzora_editor_framework::{EditorCamera, EditorSelection, HideInHierarchy};

use crate::modal_transform;
use crate::types::{BoxSelectionState, GizmoMesh, GizmoMode, GizmoRoot, GizmoState};

/// What [`SceneRay`] reads of a mesh with a distance band.
type RangedMesh = (
    &'static VisibilityRange,
    &'static GlobalTransform,
    Option<&'static bevy::camera::primitives::Aabb>,
);

/// The transform gizmo's own meshes, which a scene pick never counts.
type GizmoParts = Or<(With<GizmoMesh>, With<GizmoRoot>)>;

/// The scene meshes along a ray, nearest first, as a click can reach them.
#[derive(SystemParam)]
pub(crate) struct SceneRay<'w, 's> {
    ray_cast: MeshRayCast<'w, 's>,
    /// For keeping the ray to the scene's render layer.
    layers: Query<'w, 's, &'static RenderLayers>,
    /// For skipping detail levels the camera is too near or too far to show.
    ranges: Query<'w, 's, RangedMesh>,
    gizmo_meshes: Query<'w, 's, (), GizmoParts>,
    hidden: Query<'w, 's, (), With<HideInHierarchy>>,
}

impl SceneRay<'_, '_> {
    /// Every scene mesh `ray` passes through, nearest first, with its
    /// distance. Not gizmo handles, not hidden helpers.
    ///
    /// Anything shown in the scene, not what the camera saw last frame. Bevy's
    /// default (`VisibleInView`) asks the camera's last visibility pass, and a
    /// viewport idling under render-on-change runs none, so every mesh read as
    /// unseen: a click on an idle viewport hit nothing. The layer test is one
    /// part of `VisibleInView` worth keeping: offscreen preview rigs on their
    /// own render layers are never under the cursor.
    ///
    /// The distance band is the other. A model with baked LODs carries every
    /// detail level as a sibling subtree in the same place, each shown only
    /// within its `VisibilityRange` of the camera; without the band test the
    /// ray would hit the copy the camera is not drawing. Measured the way Bevy
    /// measures it (`check_visibility_ranges`): from the camera to the mesh's
    /// bounds centre when the range asks for that, else its origin.
    pub(crate) fn hits(&mut self, ray: Ray3d, cam_pos: Vec3) -> Vec<(Entity, f32)> {
        let layers = &self.layers;
        let ranges = &self.ranges;
        let shown = |entity: Entity| {
            let on_layer = match layers.get(entity) {
                Ok(l) => l.intersects(&RenderLayers::layer(0)),
                // No `RenderLayers` is layer 0.
                Err(_) => true,
            };
            let in_range = match ranges.get(entity) {
                Ok((range, gt, aabb)) => {
                    let at = match (range.use_aabb, aabb) {
                        (true, Some(aabb)) => gt.transform_point(aabb.center.into()),
                        _ => gt.translation(),
                    };
                    range.is_visible_at_all(cam_pos.distance(at))
                }
                // No band: shown at every distance.
                Err(_) => true,
            };
            on_layer && in_range
        };
        let settings = MeshRayCastSettings::default()
            .with_visibility(RayCastVisibility::Visible)
            .with_filter(&shown)
            .never_early_exit();
        let hits: Vec<(Entity, f32)> = self
            .ray_cast
            .cast_ray(ray, &settings)
            .iter()
            .map(|(e, hit)| (*e, hit.distance))
            .collect();
        hits.into_iter()
            .filter(|(e, _)| !self.gizmo_meshes.contains(*e) && !self.hidden.contains(*e))
            .collect()
    }
}

/// The hierarchy rules a pick goes through: which entity a hit mesh stands for,
/// what is grouped, what is locked. Shared by the click, the box selection and
/// the pick list so the three agree.
#[derive(SystemParam)]
pub(crate) struct PickResolver<'w, 's> {
    named: Query<'w, 's, (Has<SelectionStop>, Has<PickAsUnit>), With<Name>>,
    parents: Query<'w, 's, &'static ChildOf>,
    hidden: Query<'w, 's, (), With<HideInHierarchy>>,
    locked: Query<'w, 's, (), With<EditorLocked>>,
}

impl PickResolver<'_, '_> {
    /// The nearest entity at or above `entity` that the hierarchy shows: the
    /// hit mesh itself, or its nearest named, visible ancestor when the mesh
    /// is an unnamed or hidden helper.
    pub(crate) fn leaf(&self, entity: Entity) -> Option<Entity> {
        self.chain(entity).find(|&e| self.shown(e))
    }

    /// The outermost entity at or above `entity` that picks as one object.
    pub(crate) fn unit(&self, entity: Entity) -> Option<Entity> {
        self.chain(entity)
            .filter(|&e| self.shown(e))
            .filter(|&e| self.named.get(e).is_ok_and(|(stop, unit)| stop || unit))
            .last()
    }

    /// What a click on `entity` selects: its unit if it is in one, else the
    /// leaf.
    pub(crate) fn target(&self, entity: Entity) -> Option<Entity> {
        self.unit(entity).or_else(|| self.leaf(entity))
    }

    /// Whether `entity` or anything above it is locked. A lock on a model
    /// locks its parts, which is what locking a model in the hierarchy means.
    pub(crate) fn is_locked(&self, entity: Entity) -> bool {
        self.chain(entity).any(|e| self.locked.contains(e))
    }

    /// The shown entities above `entity`, nearest first, not counting itself.
    pub(crate) fn ancestors(&self, entity: Entity) -> Vec<Entity> {
        self.chain(entity).skip(1).filter(|&e| self.shown(e)).collect()
    }

    fn shown(&self, entity: Entity) -> bool {
        self.named.contains(entity) && !self.hidden.contains(entity)
    }

    /// `entity` and every ancestor up to the scene root.
    fn chain(&self, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
        std::iter::successors(Some(entity), |&e| self.parents.get(e).ok().map(|c| c.parent()))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn entity_pick_system(
    gizmo_state: Res<GizmoState>,
    mode: Res<GizmoMode>,
    modal: Res<modal_transform::ModalTransformState>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    viewport: Option<Res<ViewportState>>,
    nav_overlay: Option<Res<NavOverlayState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform, &Projection), With<EditorCamera>>,
    mut scene_ray: SceneRay,
    resolver: PickResolver,
    pick_proxies: Query<(Entity, &GlobalTransform, &renzora::PickProxy)>,
    drawn_bones: Res<crate::skeleton_gizmo::DrawnBones>,
    selection: Res<EditorSelection>,
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
    let Ok((camera, cam_gt, projection)) = camera_q.single() else {
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

    // A drawn skeleton bone under the cursor wins outright: bones are drawn
    // over the mesh, so the one you can see is the one you clicked, even
    // though the skin is nearer along the ray. Within a few pixels of the
    // segment, the nearest along the ray.
    let vh = viewport.current_size.y.max(1) as f32;
    let bone_hit = drawn_bones
        .segments
        .iter()
        .filter_map(|&(bone, head, tail)| {
            let mid = (head + tail) * 0.5;
            let threshold = crate::ray::pick_threshold(cam_gt, mid, projection, vh) * 0.6;
            let miss = if (tail - head).length_squared() < 1e-10 {
                let to = head - ray.origin;
                let along = to.dot(*ray.direction);
                (along > 0.0).then(|| (to - *ray.direction * along).length())?
            } else {
                crate::ray::closest_distance_ray_segment(&ray, head, tail)?
            };
            (miss <= threshold).then(|| (bone, (mid - ray.origin).length()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(bone, _)| bone);
    if let Some(bone) = bone_hit {
        if matches!(*mode, GizmoMode::Select | GizmoMode::Translate | GizmoMode::Rotate | GizmoMode::Scale) {
            box_sel.active = true;
            box_sel.start_pos = cursor;
            box_sel.current_pos = cursor;
            box_sel.pending_pick = Some(bone);
            box_sel.move_on_drag = false;
        }
        return;
    }

    // Raycast and find the nearest pickable entity (if any). We do NOT commit
    // the selection yet: we arm `box_sel` with this entity as a pending pick and
    // wait for mouse-up to decide click vs drag. A locked hit is passed
    // through, so the click selects what is behind it.
    let cam_pos = cam_gt.translation();
    let mut pending: Option<Entity> = None;
    let mut pending_dist = f32::INFINITY;
    for (entity, distance) in scene_ray.hits(ray, cam_pos) {
        if resolver.is_locked(entity) {
            continue;
        }
        if let Some(target) = resolver.target(entity) {
            pending = Some(target);
            pending_dist = distance;
            break;
        }
    }

    // Entities with nothing to raycast stand in as a sphere, and the nearer of
    // the two wins, so a particle system in front of a wall is what a click on
    // it selects. The radius never drops below a few pixels' worth at the hit
    // distance, so a small effect far away is still something you can hit.
    for (entity, gt, proxy) in &pick_proxies {
        if resolver.is_locked(entity) {
            continue;
        }
        let Some(target) = resolver.target(entity) else {
            continue;
        };
        let centre = gt.translation();
        let to = centre - ray.origin;
        let along = to.dot(*ray.direction);
        if along <= 0.0 {
            continue;
        }
        let radius = proxy.radius.max(along * 0.025);
        let miss = (to - *ray.direction * along).length();
        if miss <= radius && along < pending_dist {
            pending = Some(target);
            pending_dist = along;
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
        // Pressing on something already selected and dragging moves it, rather
        // than starting a box: the same gesture as dragging a handle, without
        // having to find the handle (`drag::gizmo_drag`). Translate mode only,
        // which is the mode a selection switches to.
        box_sel.move_on_drag = *mode == GizmoMode::Translate
            && pending.is_some_and(|e| selection.is_selected(e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn target(world: &mut World, start: Entity) -> Option<Entity> {
        world.run_system_once(move |r: PickResolver| r.target(start)).unwrap()
    }

    fn locked(world: &mut World, start: Entity) -> bool {
        world.run_system_once(move |r: PickResolver| r.is_locked(start)).unwrap()
    }

    /// The car case this design exists for: a click on a part selects the part,
    /// however many named nodes sit above it.
    #[test]
    fn a_click_selects_the_part_it_hits() {
        let mut world = World::new();
        let car = world.spawn(Name::new("Car")).id();
        let wrapper = world.spawn((Name::new("root"), ChildOf(car))).id();
        let wheel = world.spawn((Name::new("Wheel"), ChildOf(wrapper))).id();
        let tyre = world.spawn((Name::new("Tyre"), ChildOf(wheel))).id();
        assert_eq!(target(&mut world, tyre), Some(tyre));
        assert_eq!(target(&mut world, wheel), Some(wheel));

        // An unnamed mesh stands for its nearest named ancestor.
        let unnamed = world.spawn(ChildOf(tyre)).id();
        assert_eq!(target(&mut world, unnamed), Some(tyre));
    }

    /// A model marked to pick as one object selects the model from any part;
    /// the outermost mark wins, so a group of grouped models picks as the
    /// group.
    #[test]
    fn a_unit_picks_as_one_object() {
        let mut world = World::new();
        let car = world.spawn((Name::new("Car"), PickAsUnit)).id();
        let wheel = world.spawn((Name::new("Wheel"), ChildOf(car))).id();
        assert_eq!(target(&mut world, wheel), Some(car));

        let fleet = world.spawn((Name::new("Fleet"), PickAsUnit)).id();
        world.entity_mut(car).insert(ChildOf(fleet));
        assert_eq!(target(&mut world, wheel), Some(fleet));
    }

    /// Code-made compounds (a terrain and its chunks) pick as one object
    /// without the user choosing it.
    #[test]
    fn a_selection_stop_picks_as_one_object() {
        let mut world = World::new();
        let terrain = world.spawn((Name::new("Terrain"), SelectionStop)).id();
        let chunk = world.spawn((Name::new("Chunk"), ChildOf(terrain))).id();
        assert_eq!(target(&mut world, chunk), Some(terrain));
    }

    /// Hidden wrappers are transparent, as they are in the hierarchy panel:
    /// a hidden unit is not a unit, and a hidden mesh stands for the nearest
    /// shown ancestor.
    #[test]
    fn hidden_entities_are_transparent() {
        let mut world = World::new();
        let model = world.spawn(Name::new("Model")).id();
        let wrapper = world
            .spawn((Name::new("RootNode"), HideInHierarchy, PickAsUnit, ChildOf(model)))
            .id();
        let wall = world.spawn((Name::new("Wall"), ChildOf(wrapper))).id();
        assert_eq!(target(&mut world, wall), Some(wall));
        let helper = world.spawn((Name::new("Helper"), HideInHierarchy, ChildOf(wall))).id();
        assert_eq!(target(&mut world, helper), Some(wall));
    }

    /// A lock on a model locks its parts.
    #[test]
    fn a_lock_covers_everything_under_it() {
        let mut world = World::new();
        let car = world.spawn(Name::new("Car")).id();
        let wheel = world.spawn((Name::new("Wheel"), ChildOf(car))).id();
        assert!(!locked(&mut world, wheel));
        world.entity_mut(car).insert(EditorLocked);
        assert!(locked(&mut world, wheel));
    }

    #[test]
    fn an_unnamed_chain_selects_nothing() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let child = world.spawn(ChildOf(root)).id();
        assert_eq!(target(&mut world, child), None);
    }
}
