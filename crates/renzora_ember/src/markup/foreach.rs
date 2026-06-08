//! `<for tag="enemy"> ... </for>` — repeat a loop body per matching entity.
//!
//! The loader stamps a [`ForEach`] on the `<for>` container (a styled flex
//! node) and skips spawning its children. Each frame [`update_foreach`] finds
//! every entity carrying a matching `EntityTag`, and — when that set changes —
//! rebuilds the container's children: one copy of the loop body per matched
//! entity, with that entity as the binding host. So inside the body,
//! `{{ Health.current }}` reads the entity this row was spawned for, and
//! `{{ Name }}` its name.
//!
//! Reconciliation is coarse (rebuild the whole list when the matched *set*
//! changes). Per-entity field changes don't need a rebuild — the body's
//! `{{ }}` bindings already re-read their host's components every frame.

use bevy::prelude::*;
use bevy_hui::prelude::HtmlTemplate;
use renzora::EntityTag;

use crate::markup::loader::build_for_children;

/// Stamped on a `<for>` container. Remembers where its loop body lives in the
/// source template (handle + node path) and which tag to match.
#[derive(Component)]
pub struct ForEach {
    template: Handle<HtmlTemplate>,
    node_path: Vec<u32>,
    tag: String,
    /// Last matched entity set (sorted), to detect changes.
    last: Vec<Entity>,
    /// Forces a rebuild on the first run even if the set is empty.
    built_once: bool,
}

impl ForEach {
    pub fn new(template: Handle<HtmlTemplate>, node_path: Vec<u32>, tag: String) -> Self {
        Self {
            template,
            node_path,
            tag,
            last: Vec::new(),
            built_once: false,
        }
    }
}

fn update_foreach(
    mut commands: Commands,
    server: Res<AssetServer>,
    templates: Res<Assets<HtmlTemplate>>,
    tagged: Query<(Entity, &EntityTag)>,
    mut fors: Query<(Entity, &mut ForEach)>,
    children_q: Query<&Children>,
    has_node: Query<(), With<Node>>,
) {
    for (container, mut fe) in &mut fors {
        if fe.tag.is_empty() {
            continue;
        }

        let mut matches: Vec<Entity> = tagged
            .iter()
            .filter(|(_, t)| t.tag == fe.tag)
            .map(|(e, _)| e)
            .collect();
        matches.sort_by_key(|e| e.to_bits());

        if fe.built_once && matches == fe.last {
            continue;
        }

        // Set changed (or first run): clear the container's item UI and
        // rebuild one body per matched entity. Only despawn `Node`-bearing
        // children so anything non-UI under the container is left alone.
        if let Ok(kids) = children_q.get(container) {
            for child in kids.iter() {
                if has_node.get(child).is_ok() {
                    commands.entity(child).despawn();
                }
            }
        }
        for &matched in &matches {
            build_for_children(
                &mut commands,
                &server,
                &templates,
                &fe.template,
                &fe.node_path,
                matched,
                container,
            );
        }
        fe.last = matches;
        fe.built_once = true;
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, update_foreach);
}
