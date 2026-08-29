//! Scene entities must be named, and one that isn't is despawned.
//!
//! `Name` is not decoration here — three separate systems key off it, and all
//! three are how an entity stays accounted for:
//!
//! - the hierarchy panel lists `(Entity, &Name)`, so a nameless entity is
//!   invisible to the only tool that could show you it exists;
//! - `save_scene` serialises `With<Name>`, so it is never written to the scene;
//! - the scene clear despawns `With<Name>`, so it is never removed either.
//!
//! Those combine into the one genuinely bad state in the engine: an entity that
//! runs, cannot be seen, and survives every scene switch. Load a scene, leave,
//! come back, and you have two of them; do it ten times and you have ten, with
//! nothing in the UI to show for it. It reads as a memory leak in the engine
//! when it is in fact a spawn with a missing component.
//!
//! Nothing legitimate needs that state. A global scene's entities are `Persistent`
//! and *are* listed in the hierarchy (`HierarchyCandidate` excludes
//! `HideInHierarchy`, not `Persistent`). Editor chrome opts out explicitly with
//! `HideInHierarchy`. Those are the two sanctioned ways to be special, and both
//! remain observable — one in the tree, one on an explicit list.
//!
//! ## Why this does not run all the time
//!
//! Because the editor legitimately holds unnamed entities with transforms that
//! are not scene content: the offscreen preview rigs in `renzora_preview` (a
//! camera, a light and a mesh on an isolated render layer, none of them named),
//! and some viewport helpers. Despawning those would break material and model
//! previews to fix a problem they do not have.
//!
//! So the guard runs where new scene content is actually created by game code —
//! while the game is running. In an exported game that is always, and there is
//! no chrome or preview rig to collide with. In the editor it is play mode,
//! which is when a script can spawn. Authoring outside play mode goes through
//! paths that already name what they create (`model_drop`, `spawn_entity`, the
//! scene loader).
//!
//! Bringing the preview rigs in line — `HideInHierarchy`, which is what they
//! are — would let this run unconditionally. That is a worthwhile follow-up and
//! deliberately not bundled in here.

use bevy::prelude::*;
use bevy::platform::collections::HashSet;

use renzora::core::console_log::console_error;
use renzora::core::{HideInHierarchy, Persistent, PlayModeState};

/// Entities the guard considers scene content.
///
/// `With<Transform>` is the definition of "would be in the scene": an entity
/// without one is not placed in the world and is almost always infrastructure —
/// an observer, a system, a window, an asset handle holder. Bevy 0.19 represents
/// several of those as entities, and despawning them would take the app with it.
///
/// `Without<Node>` excludes bevy_ui, which is the editor's own widgets and game
/// UI alike — neither belongs in the scene tree, and ember does not name its
/// nodes.
type SceneContent = (
    Without<Name>,
    With<Transform>,
    Without<HideInHierarchy>,
    Without<Persistent>,
    Without<bevy::ui::Node>,
    Without<bevy::input::gamepad::Gamepad>,
);

/// Despawn scene entities that have no `Name`, and say which and how many.
///
/// **Two-strike, deliberately.** An entity is only acted on if it was already
/// nameless on the previous run. A spawn is free to insert its components across
/// more than one command application — `bsn!` trees and the scene loader both
/// do — and a single-frame check would despawn a perfectly good entity halfway
/// through being built. Waiting one frame costs nothing and removes the race
/// entirely.
pub fn reject_unnamed_entities(
    mut commands: Commands,
    play: Option<Res<PlayModeState>>,
    editor: Option<Res<renzora::core::EditorSession>>,
    q: Query<Entity, SceneContent>,
    mut suspects: Local<HashSet<Entity>>,
    mut reported: Local<u64>,
) {
    // The editor authors outside play mode; a shipped game has no such mode and
    // is always running. `EditorSession` is absent in a runtime build, which is
    // the check for "is this a game?" — a cargo feature cannot answer it, since
    // both binaries come out of one `--workspace` build.
    let enforcing = match (&play, &editor) {
        (Some(p), Some(_)) => p.is_in_play_mode(),
        _ => true,
    };
    if !enforcing {
        suspects.clear();
        return;
    }

    let current: HashSet<Entity> = q.iter().collect();
    let condemned: Vec<Entity> = current.intersection(&suspects).copied().collect();
    *suspects = current;

    if condemned.is_empty() {
        return;
    }
    for entity in &condemned {
        // `try_despawn`: an earlier despawn this frame may already have taken it
        // as part of a subtree.
        commands.entity(*entity).try_despawn();
    }
    *reported += condemned.len() as u64;
    console_error(
        "Scene",
        format!(
            "Despawned {} unnamed entit{} ({} this session). An entity with no Name is \
             invisible to the hierarchy, is not saved with the scene, and is not removed \
             when the scene changes — so it would survive every reload unobserved. Give it \
             a name: `#Label` in a bsn! block, or a `Name` component when spawning.",
            condemned.len(),
            if condemned.len() == 1 { "y" } else { "ies" },
            *reported
        ),
    );
}
