//! Runtime: bind a serializable template *path* to the entity tree the
//! renzora loader builds from the `.html` file.
//!
//! `HtmlTemplatePath` lives in `renzora_game_ui` (so the canvas/viewport can
//! spawn it without depending on this crate), and `renzora_game_ui` registers
//! it for reflection. This module owns the loader trigger.
//!
//! **Single entity model.** `HtmlTemplatePath` sits on the entity the user
//! spawned. As soon as it's set, an observer waits for the parsed
//! `HtmlTemplate` asset to become available, then `build_template_onto` walks
//! the AST and attaches the markup's components/children directly to that same
//! entity. There's no opaque "scope root" wrapper, no `HtmlNode`, no
//! `HtmlStyle` — just the components the loader writes.
//!
//! Hot-reload (Phase C) will despawn the children and re-run the loader; for
//! now we only handle the initial path-insert path.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_hui::prelude::HtmlTemplate;
pub use renzora_game_ui::HtmlTemplatePath;

use crate::components::ComponentRegistry;
use crate::loader::build_template_onto;

/// When `HtmlTemplatePath` is set, kick off async loading of the `.html` and
/// queue a one-shot system that builds it onto the entity once the asset is
/// ready. Stores the load handle in `PendingTemplate` so the build can find
/// the template asset later.
#[derive(Component)]
struct PendingTemplate(Handle<HtmlTemplate>);

fn on_template_path_inserted(
    trigger: On<Insert, HtmlTemplatePath>,
    paths: Query<&HtmlTemplatePath>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    let Ok(path) = paths.get(entity) else { return };
    if path.0.is_empty() {
        return;
    }
    let handle: Handle<HtmlTemplate> = server.load(path.0.clone());
    commands.entity(entity).insert(PendingTemplate(handle));
}

/// Each frame, finish loading any `PendingTemplate` entities whose asset has
/// arrived: clear any pre-existing children, then build the markup tree onto
/// them and drop the marker.
///
/// The children wipe matters for three independent cases:
///   * **Scene reload** — saved scenes also captured the previously-built
///     children. On load, the scene loader spawns the parent + all those
///     children, then we'd build on top, doubling the tree. The Insert observer
///     can't help here: it fires before the scene loader has finished parenting
///     the children, so it would see an empty `Children` and clear nothing.
///   * **Hot-reload** of the `.html` asset (Phase C — re-inserts `PendingTemplate`
///     against the same entity).
///   * **Path edit** in the inspector (`HtmlTemplatePath` reinserted with a new
///     value; the observer re-queues `PendingTemplate`).
fn finalize_pending_templates(
    server: Res<AssetServer>,
    templates: Res<Assets<HtmlTemplate>>,
    registry: Res<ComponentRegistry>,
    pending: Query<(Entity, &PendingTemplate)>,
    children_q: Query<&Children>,
    mut commands: Commands,
) {
    let no_overrides: HashMap<String, String> = HashMap::default();
    for (entity, marker) in &pending {
        // Just check the asset is available — `is_loaded_with_dependencies` can
        // gate on transitive deps that never resolve for self-contained
        // templates and leave us in pending forever.
        let Some(template) = templates.get(&marker.0) else {
            continue;
        };
        if let Ok(kids) = children_q.get(entity) {
            for child in kids.iter() {
                commands.entity(child).despawn();
            }
        }
        info!(
            "renzora_hui: building template `{}` onto {entity}",
            server
                .get_path(&marker.0)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "<no path>".into())
        );
        build_template_onto(
            &mut commands,
            &server,
            entity,
            template,
            marker.0.clone(),
            &registry,
            &templates,
            &no_overrides,
            None,
        );
        commands.entity(entity).remove::<PendingTemplate>();
    }
}

/// Runtime plugin: the path→tree binding. Adds **only** what's needed for
/// async load + finalize; bevy_hui's `BuildPlugin`/`TransitionPlugin`/etc. are
/// no longer registered in this crate's plugin.
pub fn plugin(app: &mut App) {
    app.add_observer(on_template_path_inserted)
        .add_systems(Update, finalize_pending_templates);
}
