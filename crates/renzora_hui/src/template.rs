//! Runtime: bind a serializable template *path* to a `bevy_hui` `HtmlNode`.
//!
//! `HtmlNode` wraps a `Handle<HtmlTemplate>`, which doesn't round-trip through
//! scene serialization. So the path an entity should display is stored as a
//! plain [`HtmlTemplatePath`] string (which *does* serialize), and an observer
//! turns it into a loaded `HtmlNode`. This means a saved scene re-loads its
//! templates in exported games — not just in the editor.

use bevy::prelude::*;
use bevy_hui::prelude::{HtmlNode, HtmlTemplate};

/// The asset path (project-relative, e.g. `"ui/example_menu.html"`) of the
/// template an entity should display. The source of truth; [`HtmlNode`] is
/// derived from it. Empty = nothing loaded yet.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct HtmlTemplatePath(pub String);

/// Loads the template and (re)inserts [`HtmlNode`] whenever a path is set.
///
/// An observer (not a `Changed<>` system) so it also fires on the reflection
/// inserts performed by `DynamicScene::write_to_world`, which don't propagate
/// change ticks — the same reason `renzora_game_ui` uses insert observers for
/// scene-loaded UI.
fn on_template_path_inserted(
    trigger: On<Insert, HtmlTemplatePath>,
    server: Res<AssetServer>,
    paths: Query<&HtmlTemplatePath>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    let Ok(path) = paths.get(entity) else { return };
    if path.0.is_empty() {
        return;
    }
    let handle: Handle<HtmlTemplate> = server.load(path.0.clone());
    commands.entity(entity).insert(HtmlNode(handle));
}

/// Registers the path component + its binding observer. Runtime (not editor) so
/// scene-authored templates load in shipped games too.
pub fn plugin(app: &mut App) {
    app.register_type::<HtmlTemplatePath>()
        .add_observer(on_template_path_inserted);
}
