//! Runtime: bind a serializable template *path* to a `bevy_hui` `HtmlNode`.
//!
//! The path component [`HtmlTemplatePath`] lives in `renzora_game_ui` (so the UI
//! canvas editor and viewport can spawn it without depending on this crate),
//! and `renzora_game_ui` registers it for reflection. This module owns the
//! bevy_hui-specific half: an observer that loads the path into an `HtmlNode`
//! whenever it's set, so a saved scene re-loads its templates in exported games
//! too (an `HtmlNode`'s `Handle` doesn't round-trip through scenes).

use bevy::prelude::*;
use bevy_hui::prelude::{HtmlNode, HtmlTemplate};
pub use renzora_game_ui::HtmlTemplatePath;

/// Loads the template and (re)inserts [`HtmlNode`] whenever a path is set.
///
/// An observer (not a `Changed<>` system) so it also fires on the reflection
/// inserts performed by `DynamicScene::write_to_world`, which don't propagate
/// change ticks.
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

/// Registers the path→`HtmlNode` binding observer. Runtime (not editor) so
/// scene-authored templates load in shipped games too. The component type
/// itself is registered for reflection by `renzora_game_ui`.
pub fn plugin(app: &mut App) {
    app.add_observer(on_template_path_inserted);
}
