//! Runtime: bind a serializable template *path* to a `bevy_hui` `HtmlNode`.
//!
//! The path component [`HtmlTemplatePath`] lives in `renzora_game_ui` (so the UI
//! canvas editor and viewport can spawn it without depending on this crate),
//! and `renzora_game_ui` registers it for reflection. This module owns the
//! bevy_hui-specific half.
//!
//! Crucially the `HtmlNode` is placed on a **child** entity, not on the
//! `HtmlTemplatePath` entity itself. bevy_hui builds a template's root node onto
//! whatever entity carries `HtmlNode` and rebuilds it on hot-reload — so if that
//! were the `HtmlTemplatePath` entity, the editor could never give it a stable
//! absolute position (bevy_hui would keep resetting its `Node`). By keeping the
//! markup under a child, the `HtmlTemplatePath` entity stays a plain, stable,
//! scene-saved container the canvas editor can freely position.

use bevy::prelude::*;
use bevy_hui::prelude::{HtmlNode, HtmlTemplate};
pub use renzora_game_ui::HtmlTemplatePath;

/// (Re)spawns the `HtmlNode` markup child whenever a path is set.
///
/// An observer (not a `Changed<>` system) so it also fires on the reflection
/// inserts performed by `DynamicScene::write_to_world`, which don't propagate
/// change ticks. Re-inserting the path (e.g. picking a new template in the
/// inspector) despawns the old markup child and builds the new one.
fn on_template_path_inserted(
    trigger: On<Insert, HtmlTemplatePath>,
    paths: Query<&HtmlTemplatePath>,
    children: Query<&Children>,
    markup: Query<(), With<HtmlNode>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    let Ok(path) = paths.get(entity) else { return };
    if path.0.is_empty() {
        return;
    }

    // Drop any previously-built markup child (and its subtree) so a changed
    // path rebuilds cleanly.
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            if markup.get(child).is_ok() {
                commands.entity(child).despawn();
            }
        }
    }

    let handle: Handle<HtmlTemplate> = server.load(path.0.clone());
    commands.spawn((Name::new("Markup"), HtmlNode(handle), ChildOf(entity)));
}

/// Registers the path→`HtmlNode` binding observer. Runtime (not editor) so
/// scene-authored templates load in shipped games too. The component type
/// itself is registered for reflection by `renzora_game_ui`.
pub fn plugin(app: &mut App) {
    app.add_observer(on_template_path_inserted);
}
