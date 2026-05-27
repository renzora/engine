//! Editor integration: surface `bevy_hui` template entities in
//! `renzora_game_ui`'s canvas preview so they can be selected and dragged, and
//! persist drag positions as a per-`id` overlay on the (scene-saved) `HtmlNode`
//! root. The overlay is re-applied after each hot-reload, so layout tweaks
//! survive template edits — the `.html` file itself is never rewritten.
//!
//! Workflow: the user spawns an entity with an [`HtmlNode`] (a template path)
//! under a UI Canvas. When `bevy_hui` finishes building the node tree, every
//! node is tagged with [`UiWidget`] so the existing canvas treats it as a
//! draggable widget. Dragging a node whose markup gave it an `id` records its
//! new position; the next rebuild restores it.
//!
//! Gated behind the crate's `editor` feature: tagging inserts `UiWidget`, which
//! drives `renzora_game_ui`'s layout/canvas systems — desirable in the editor,
//! but it must not run in shipped games where `bevy_hui` owns layout.

use bevy::prelude::*;
use bevy::ui::Val;
use bevy_hui::prelude::{HtmlNode, Tags, UiId};
use renzora_game_ui::{UiThemed, UiWidget};

/// Marker on every `bevy_hui` node we've tagged into the canvas, recording the
/// `left`/`top` values `bevy_hui` built it with. A drag is detected as the live
/// position drifting away from this baseline.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct HuiManaged {
    pub base_left: Val,
    pub base_top: Val,
}

/// One saved drag position, keyed by the node's markup `id`.
#[derive(Reflect, Clone, Default)]
pub struct HuiOverride {
    pub id: String,
    pub left: Val,
    pub top: Val,
}

/// Per-`id` drag overrides, stored on the `HtmlNode` root entity so they save
/// with the scene and survive template hot-reload. Only nodes with a markup
/// `id` can persist a drag.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct HuiLayoutOverrides(pub Vec<HuiOverride>);

pub struct HuiEditorPlugin;

impl Plugin for HuiEditorPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<HuiManaged>()
            .register_type::<HuiOverride>()
            .register_type::<HuiLayoutOverrides>()
            // Tag/restore must apply before we look for drags to capture.
            .add_systems(Update, (tag_and_restore, capture_drags).chain());
    }
}

/// As each template node is built (`Tags` is inserted on every node), tag it as
/// a draggable widget and re-apply any saved drag override for its markup `id`.
/// Restoring *before* recording the baseline means the restored position becomes
/// the new baseline and isn't later mistaken for a fresh drag.
fn tag_and_restore(
    mut commands: Commands,
    mut built: Query<(Entity, &mut Node, Option<&UiId>), Added<Tags>>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<HtmlNode>>,
    overrides: Query<&HuiLayoutOverrides>,
) {
    for (entity, mut node, ui_id) in &mut built {
        let (mut left, mut top) = (node.left, node.top);

        if let Some(ui_id) = ui_id {
            if let Some(root) = find_template_root(entity, &parents, &roots) {
                if let Ok(ov) = overrides.get(root) {
                    if let Some(o) = ov.0.iter().find(|o| o.id == *ui_id.id()) {
                        left = o.left;
                        top = o.top;
                    }
                }
            }
        }

        node.left = left;
        node.top = top;
        commands.entity(entity).insert((
            UiWidget::default(),
            UiThemed,
            HuiManaged {
                base_left: left,
                base_top: top,
            },
        ));
    }
}

/// Detect a user drag (live `left`/`top` drifted from the `bevy_hui` baseline)
/// on a managed node carrying a markup `id`, and upsert the override onto its
/// owning `HtmlNode` root entity.
fn capture_drags(
    mut commands: Commands,
    moved: Query<(Entity, &Node, &HuiManaged, &UiId), Changed<Node>>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<HtmlNode>>,
    mut overrides: Query<&mut HuiLayoutOverrides>,
) {
    for (entity, node, managed, ui_id) in &moved {
        // React only to position drift — not the size/color animations
        // `bevy_hui` runs on hover, which leave left/top untouched.
        if node.left == managed.base_left && node.top == managed.base_top {
            continue;
        }
        let Some(root) = find_template_root(entity, &parents, &roots) else {
            continue;
        };
        let entry = HuiOverride {
            id: ui_id.id().clone(),
            left: node.left,
            top: node.top,
        };
        if let Ok(mut ov) = overrides.get_mut(root) {
            match ov.0.iter_mut().find(|o| o.id == entry.id) {
                Some(existing) => *existing = entry,
                None => ov.0.push(entry),
            }
        } else {
            commands
                .entity(root)
                .insert(HuiLayoutOverrides(vec![entry]));
        }
    }
}

/// Walk ancestors until the entity carrying the [`HtmlNode`] (template root) is
/// found. Returns `None` if the node isn't part of a template tree.
fn find_template_root(
    start: Entity,
    parents: &Query<&ChildOf>,
    roots: &Query<(), With<HtmlNode>>,
) -> Option<Entity> {
    let mut entity = start;
    loop {
        if roots.get(entity).is_ok() {
            return Some(entity);
        }
        match parents.get(entity) {
            Ok(child_of) => entity = child_of.parent(),
            Err(_) => return None,
        }
    }
}
