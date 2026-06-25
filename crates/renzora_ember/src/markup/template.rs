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
//! Hot-reload (Phase C): `hot_reload_templates` watches `AssetEvent::Modified`
//! and re-queues the holders, so `finalize_pending_templates` despawns the old
//! children and re-runs the loader. Saving a template in the code editor forces
//! the asset re-read that drives this.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_hui::prelude::HtmlTemplate;
pub use renzora_game_ui::HtmlTemplatePath;

use crate::markup::loader::{build_template_onto, template_deps_ready, TemplateHandles};

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
#[allow(clippy::too_many_arguments)]
fn finalize_pending_templates(
    server: Res<AssetServer>,
    templates: Res<Assets<HtmlTemplate>>,
    mut keeper: ResMut<TemplateHandles>,
    pending: Query<(Entity, &PendingTemplate)>,
    has_node: Query<(), With<Node>>,
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
        // Make sure every `template="path"` reference (recursively) is loaded
        // before we build. Skipping a frame is fine — the entity keeps its
        // `PendingTemplate` and we retry next tick. Avoids the half-built
        // state where a node has `template="x.html"` but x.html hasn't loaded
        // yet, leading to a bare placeholder in the output.
        if !template_deps_ready(template, &server, &templates, &mut keeper) {
            continue;
        }

        // Model: the `HtmlTemplatePath` holder is expected to be a `UiCanvas`
        // root, and the markup tree builds as a **child** inside it. The canvas
        // keeps its own full-size root node; the template is content within it.
        // To read another entity's data, name it in the binding:
        // `{{ World Environment.Sun.azimuth }}`.
        //
        // Clear the previous build first by despawning the holder's
        // `Node`-bearing children (the old template root + stale scene builds);
        // non-UI children are left intact.
        if let Ok(kids) = children_q.get(entity) {
            for child in kids.iter() {
                if has_node.get(child).is_ok() {
                    commands.entity(child).despawn();
                }
            }
        }

        let child = commands.spawn_empty().id();
        commands.entity(entity).add_child(child);

        info!(
            "renzora_hui: building template `{}` under canvas {entity} (root {child})",
            server
                .get_path(&marker.0)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "<no path>".into())
        );
        build_template_onto(
            &mut commands,
            &server,
            entity, // host = the canvas holder (binding source for bare paths)
            child,  // build target = the template root, a child of the canvas
            template,
            marker.0.clone(),
            &templates,
            &no_overrides,
            None,
        );
        // Keep the root template's asset alive past this build so editing the
        // file emits `AssetEvent::Modified` and hot-reload can react. Without an
        // owner the handle drops with `PendingTemplate` below and the asset is
        // GC'd.
        if let Some(path) = server.get_path(&marker.0) {
            keeper.keep(path.to_string(), marker.0.clone());
        }
        commands.entity(entity).remove::<PendingTemplate>();
    }
}

/// Template asset ids the editor has explicitly asked to hot-reload after a
/// save. Only `Modified` events for ids in here rebuild the UI.
///
/// The gate matters because the inspector's attribute writeback *also* dirties
/// the asset — it calls `Assets::get_mut` (emits `Modified`) and rewrites the
/// `.html` (the file watcher emits another `Modified`) — yet it already updated
/// the live entity in place and must NOT trigger a rebuild, which would despawn
/// the node the user has selected and is mid-edit on. So instead of reacting to
/// every `Modified`, we react only to saves that registered here. External edits
/// (a different editor) are intentionally out of scope.
#[derive(Resource, Default)]
pub struct TemplateReloadRequests(pub bevy::platform::collections::HashSet<AssetId<HtmlTemplate>>);

impl TemplateReloadRequests {
    /// Register `path`'s template asset so the next `Modified` for it triggers a
    /// rebuild. Resolving the handle here (instead of in the caller) keeps
    /// `HtmlTemplate` out of the editor crate's dependency surface.
    pub fn request(&mut self, server: &AssetServer, path: &str) {
        let handle: Handle<HtmlTemplate> = server.load(path.to_string());
        self.0.insert(handle.id());
    }
}

/// Hot-reload (Phase C): when a template the editor saved has finished
/// re-reading from disk, rebuild every canvas that uses it.
///
/// We re-queue **all** `HtmlTemplatePath` holders rather than tracking which one
/// depends on the changed asset: a top-level template can pull in others via
/// `template="..."`, so a saved sub-template should still refresh the canvases
/// that embed it. There are only a handful of canvases, so the blanket rebuild
/// is cheap and avoids a brittle dependency graph. Re-inserting `PendingTemplate`
/// is all it takes — `finalize_pending_templates` does the despawn-and-rebuild
/// next tick, exactly as for a fresh path insert.
fn hot_reload_templates(
    mut events: MessageReader<AssetEvent<HtmlTemplate>>,
    mut requests: ResMut<TemplateReloadRequests>,
    server: Res<AssetServer>,
    holders: Query<(Entity, &HtmlTemplatePath)>,
    mut commands: Commands,
) {
    // A save can surface as both a `reload()`-driven and a file-watcher-driven
    // `Modified` for the same id; consuming the id on the first means the second
    // is a no-op, so we rebuild once.
    let mut rebuild = false;
    for ev in events.read() {
        if let AssetEvent::Modified { id } = ev {
            if requests.0.remove(id) {
                rebuild = true;
            }
        }
    }
    if !rebuild {
        return;
    }
    for (entity, path) in &holders {
        if path.0.is_empty() {
            continue;
        }
        let handle: Handle<HtmlTemplate> = server.load(path.0.clone());
        commands.entity(entity).insert(PendingTemplate(handle));
    }
}

/// Runtime plugin: the path→tree binding. Adds **only** what's needed for
/// async load + finalize; bevy_hui's `BuildPlugin`/`TransitionPlugin`/etc. are
/// no longer registered in this crate's plugin.
pub fn plugin(app: &mut App) {
    app.init_resource::<TemplateHandles>()
        .init_resource::<TemplateReloadRequests>()
        .add_observer(on_template_path_inserted)
        .add_systems(Update, (hot_reload_templates, finalize_pending_templates));
}
