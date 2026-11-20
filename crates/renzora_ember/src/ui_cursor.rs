//! Where the mouse is relative to a UI node, from Bevy's picking.
//!
//! Bevy removed `bevy_ui::RelativeCursorPosition` (after deprecating it in
//! 0.20) along with the focus system that wrote it, and pointed at picking
//! instead. Picking already knows everything that component held: the UI
//! backend's hit for a node carries the cursor's position in that node,
//! normalized exactly as the old component was, and a node is in the hover map
//! only when nothing above it blocks the pointer. A node whose descendant was
//! hit counts as under the mouse too (see [`UiCursor::cursor_over`]).
//! [`UiCursor`] is that, kept on
//! the node so the editor's many "is the mouse over this panel, and where"
//! reads stay one component away, in the way [`crate::UiInteraction`] keeps
//! hover and press.
//!
//! Hit-testing is Bevy's; nothing here decides what is under the cursor. The
//! one thing picking cannot say is where the cursor is relative to a node it is
//! *not* over, which a drag that runs past a lane's edge still needs; that
//! comes from Bevy's own node geometry (`ComputedNode::normalize_point`), in
//! the same viewport space the picking backend measures in.
//!
//! A node opts in by carrying `UiCursor`.

use bevy::camera::{NormalizedRenderTarget, RenderTarget};
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::picking::PickingSystems;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::ui::{ComputedUiTargetCamera, UiGlobalTransform, UiSystems};
use bevy::window::PrimaryWindow;

/// The mouse relative to this node. See the module docs.
#[derive(Component, Copy, Clone, Default, PartialEq, Debug, Reflect)]
#[reflect(Component, Default, PartialEq, Debug, Clone)]
pub struct UiCursor {
    /// The mouse is over this node or one of its descendants, and nothing
    /// unrelated above it blocks the pointer: picking hit the node or
    /// something inside it.
    ///
    /// Descendants count, as in CSS `:hover` and picking's own `Hovered`.
    /// Without them a button that blocks the pointer would take its panel out
    /// of the hover map, and every panel-level wheel or drag handler that
    /// gates on its content node would go dead over the panel's own buttons.
    pub cursor_over: bool,
    /// The mouse in this node's space: `(0, 0)` its centre, `(-0.5, -0.5)` its
    /// top-left corner and `(0.5, 0.5)` its bottom-right, going past those when
    /// the mouse is outside it. `None` when the mouse is not over the window
    /// the node is drawn in, or the node has no size.
    pub normalized: Option<Vec2>,
}

impl UiCursor {
    /// Whether the mouse is over this node; see [`Self::cursor_over`].
    pub fn cursor_over(&self) -> bool {
        self.cursor_over
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.register_type::<UiCursor>().add_systems(
        PreUpdate,
        update_ui_cursors
            .in_set(UiSystems::Focus)
            .after(PickingSystems::Last),
    );
}

/// Refresh every [`UiCursor`] from picking's hover map and the mouse.
///
/// `set_if_neq`, so a still mouse leaves every node unchanged.
fn update_ui_cursors(
    hover_map: Option<Res<HoverMap>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    cameras: Query<(Entity, &Camera, &RenderTarget)>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    parents: Query<&ChildOf>,
    mut nodes: Query<(
        Entity,
        &mut UiCursor,
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiTargetCamera,
        Option<&InheritedVisibility>,
    )>,
) {
    let hits = hover_map.as_deref().and_then(|m| m.get(&PointerId::Mouse));

    // Every ancestor of a hit, so a node counts as under the mouse when
    // something inside it was hit (see `UiCursor::cursor_over`). The hover map
    // holds the handful of entities under the pointer, so this is a few short
    // walks up the tree.
    let mut containing: HashSet<Entity> = HashSet::default();
    for &hit in hits.iter().flat_map(|h| h.keys()) {
        let mut at = hit;
        while let Ok(parent) = parents.get(at) {
            at = parent.parent();
            if !containing.insert(at) {
                break;
            }
        }
    }

    // The mouse in each camera's viewport space (physical pixels from the
    // viewport's corner), measured the way Bevy's UI picking backend measures
    // it, so a position from geometry and one from a hit agree.
    let mouse = pointers
        .iter()
        .find(|(id, _)| **id == PointerId::Mouse)
        .and_then(|(_, loc)| loc.location().cloned());
    let primary = primary_window.single().ok();
    let mut by_camera: HashMap<Entity, Vec2> = HashMap::default();
    if let Some(mouse) = &mouse {
        for (entity, camera, target) in &cameras {
            let on_target = target
                .normalize(primary)
                .is_some_and(|t| t == mouse.target && matches!(t, NormalizedRenderTarget::Window(_)));
            if !on_target {
                continue;
            }
            let at = mouse.position * camera.target_scaling_factor().unwrap_or(1.0);
            let origin = camera
                .physical_viewport_rect()
                .map(|r| r.min.as_vec2())
                .unwrap_or_default();
            by_camera.insert(entity, at - origin);
        }
    }

    for (entity, mut cursor, node, transform, target, visibility) in &mut nodes {
        let shown = visibility.is_none_or(|v| v.get());
        let hit = hits.and_then(|h| h.get(&entity)).filter(|_| shown);
        // Where the mouse is in this node by its geometry, for a node picking
        // did not hit itself (one of its descendants was, or nothing was).
        let geometric = || {
            target
                .get()
                .and_then(|camera| by_camera.get(&camera))
                .and_then(|&at| node.normalize_point(*transform, at))
        };
        let next = match hit {
            Some(hit) => UiCursor {
                cursor_over: true,
                normalized: hit.position.map(|p| p.truncate()).or_else(geometric),
            },
            None => UiCursor {
                cursor_over: shown && containing.contains(&entity),
                normalized: geometric(),
            },
        };
        cursor.set_if_neq(next);
    }
}
