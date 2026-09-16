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
//!
//! ## Why a code-first project is named instead of despawned
//!
//! Everything above assumes the spawn was written against this engine and that
//! the missing `Name` is a mistake. A [`renzora::ProjectKind::Bevy`] project breaks that
//! assumption completely: it is a Bevy crate someone wrote before they had ever
//! heard of Renzora, its `Startup` systems spawn a camera, a sun and several
//! hundred pieces of scenery, and not one of them calls `Name::new` because
//! nothing in Bevy has ever asked it to. Measured on the project this was built
//! against, the guard as written despawned the camera, the player and the entire
//! level on the second frame and reported it as the user's error.
//!
//! So for those projects the remedy inverts. The condition the guard exists to
//! prevent is an entity that runs and cannot be seen, and naming it fixes that
//! just as completely as despawning it, while also being the thing the user
//! actually wanted, which is their world showing up in the hierarchy. The label
//! comes from the entity's own components and prefers one the *project* defined
//! (`Player`, `Collectible`) over one Bevy did, because `Player` is worth
//! reading in a tree and `Transform` is not.

use bevy::ecs::archetype::Archetype;
use bevy::ecs::component::{ComponentId, Components};
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use renzora::core::console_log::console_error;
use renzora::core::bevy_project::{AuthoredByEditor, FromProjectCode, ProjectComponentLabels};
use renzora::core::{CurrentProject, HideInHierarchy, Persistent, PlayModeState};

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
/// Scene content in a code-first project whose origin is not yet recorded.
///
/// The two markers are mutually exclusive and one of them is always inserted, so
/// an entity matches this exactly once and then never again, which is what
/// makes a query with no `Added` filter cheap.
type UnclaimedContent = (
    With<Name>,
    With<Transform>,
    Without<FromProjectCode>,
    Without<AuthoredByEditor>,
    Without<HideInHierarchy>,
    Without<Persistent>,
    Without<bevy::ui::Node>,
);

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
    project: Option<Res<CurrentProject>>,
    // Written by the generated crate root of a Bevy project, and absent in every
    // other build, hence `Option`, which also makes "no labels" the same code
    // path as "a project whose label table did not compile".
    labels: Option<Res<ProjectComponentLabels>>,
    components: &Components,
    q: Query<Entity, SceneContent>,
    // A second view of the same set, for the naming path only. `EntityRef` reads
    // every component on the entity, which is what reaching its archetype costs;
    // the despawn path needs none of that and keeps the cheaper `Entity` query
    // it has always had.
    refs: Query<EntityRef, SceneContent>,
    // Scene content in a code-first project that has not been classified yet.
    // An entity reaches this query exactly once in its life, because both arms
    // of [`claim_for_the_editor`] insert one of the two markers.
    unclaimed: Query<Entity, UnclaimedContent>,
    mut suspects: Local<HashSet<Entity>>,
    mut reported: Local<u64>,
    mut counters: Local<HashMap<String, u32>>,
) {
    // A code-first project is named rather than emptied. Checked before the
    // play-mode gate and not after: a Bevy project's `Startup` systems run at
    // load, long before anyone presses Play, and its entities should be in the
    // hierarchy from the moment they exist rather than from the moment the user
    // happens to start the game.
    let code_first =
        project.as_ref().is_some_and(|p| p.config.kind.is_code_first());
    if code_first {
        name_unnamed_entities(
            &mut commands,
            components,
            labels.as_deref(),
            &refs,
            &mut counters,
        );
        // No two-strike wait here, and no `suspects` bookkeeping. Naming is not
        // destructive, so there is nothing to be careful about: an entity caught
        // mid-build simply gets its name one frame early, and a later `Name` the
        // spawn inserts itself overwrites it.
        claim_for_the_editor(&mut commands, &unclaimed);
        return;
    }

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

/// Give every unnamed entity a readable `Name` derived from what it is.
///
/// The code-first half of [`reject_unnamed_entities`]. Self-limiting by
/// construction: an entity named on this pass no longer matches
/// [`SceneContent`], so a project that spawns its world once at `Startup` pays
/// for one pass and then queries an empty set every frame after it.
fn name_unnamed_entities(
    commands: &mut Commands,
    components: &Components,
    labels: Option<&ProjectComponentLabels>,
    q: &Query<EntityRef, SceneContent>,
    counters: &mut HashMap<String, u32>,
) {
    let empty = ProjectComponentLabels::default();
    let labels = labels.unwrap_or(&empty);
    for entity_ref in q.iter() {
        let entity = entity_ref.id();
        let base = label_for(entity_ref.archetype(), components, labels)
            .unwrap_or_else(|| "Entity".to_string());
        // Counted per label rather than globally, so a grove of 300 trees reads
        // `Tree 1..300` and the four things that are not trees keep their own
        // short numbers instead of being `Player 212`.
        let n = counters.entry(base.clone()).or_insert(0);
        *n += 1;
        let name = if *n == 1 { base } else { format!("{base} {n}") };
        // `try_insert`: the entity may have been despawned between the query and
        // the command applying: a `Startup` system that spawns and immediately
        // cleans up is unusual but not wrong.
        //
        // `FromProjectCode` rides along because this is the one place every
        // code-spawned entity passes through, and the write-back needs to know
        // which entities it must NOT generate Rust for: the project already
        // spawns these, and writing them out too would build the world twice on
        // the next run. See that component for the rule and its known edge.
        commands
            .entity(entity)
            .try_insert((Name::new(name), FromProjectCode));
    }
}

/// The best short label for an entity, read off its components.
///
/// Preference order, and the order is the whole value of this function:
///
/// 1. a component the **project** defined, because `Player` and `Collectible`
///    are what the person reading the hierarchy is looking for, and they are
///    exactly the components Bevy did not define;
/// 2. a Bevy component that says what the entity *is* rather than where it is
///    ([`TELLING`]), so a camera reads `Camera3d` and not `Transform`;
/// 3. nothing, and the caller falls back to `Entity`.
///
/// Ubiquitous components are never a label. Every entity here has a `Transform`
/// (that is what [`SceneContent`] selects on) and most have a `Visibility`, so
/// either would name the entire world the same thing.
///
/// # Where the project's names come from
///
/// Not from `ComponentInfo::name()`. Bevy keeps component type names behind its
/// `debug` feature, which this engine deliberately builds **without** (it is
/// listed in the root manifest beside `bevy_ui_debug` and the `glam_assert`s as
/// something a shipped binary does not carry), so that call returns the same
/// placeholder for every component in the process. The project's names come from
/// [`ProjectComponentLabels`], a table the generated crate root fills in by
/// registering each of the project's own component types. See
/// `renzora_bevy_project::entry`. Bevy's own names are matched by `ComponentId`
/// against types this crate can name because it links them.
fn label_for(
    archetype: &Archetype,
    components: &Components,
    project: &ProjectComponentLabels,
) -> Option<String> {
    // Bevy components worth reading in a tree, best first, resolved to ids once.
    // Matched by `ComponentId` rather than by name because a name is not
    // available (see the note above): this crate links Bevy, so it can ask what
    // id each of these types was given and compare.
    //
    // Deliberately short: a long list becomes a ranking nobody maintains, and
    // anything missing from it still gets `Entity`, which is no worse than the
    // nothing it had before.
    let telling_types: [(Option<ComponentId>, &str); 7] = [
        (components.component_id::<Camera3d>(), "Camera3d"),
        (components.component_id::<Camera2d>(), "Camera2d"),
        (components.component_id::<DirectionalLight>(), "DirectionalLight"),
        (components.component_id::<PointLight>(), "PointLight"),
        (components.component_id::<SpotLight>(), "SpotLight"),
        (components.component_id::<Sprite>(), "Sprite"),
        // Last on purpose, and the weakest of the seven: nearly everything
        // visible in a 3D scene has one, so it is the label of last resort
        // before `Entity`.
        (components.component_id::<Mesh3d>(), "Mesh3d"),
    ];

    let mut telling: Option<(usize, &str)> = None;

    for id in archetype.components() {
        // The project's own components win outright, and the first one on the
        // archetype wins among those: archetype order is insertion order, which
        // for `spawn((Player, Transform, …))` is the order the author wrote,
        // and an author puts the marker first.
        if let Some(label) = project.get(*id) {
            return Some(label.to_string());
        }
        if let Some(rank) = telling_types
            .iter()
            .position(|(known, _)| *known == Some(*id))
        {
            if telling.is_none_or(|(best, _)| rank < best) {
                telling = Some((rank, telling_types[rank].1));
            }
        }
    }

    telling.map(|(_, name)| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::core::{ProjectConfig, ProjectKind};

    /// A component the *project* defined, as far as `label_for` is concerned:
    /// its type path does not start with `bevy`.
    #[derive(Component)]
    struct Player;

    #[derive(Component)]
    struct Collectible;

    fn app(kind: ProjectKind) -> App {
        let mut app = App::new();
        app.insert_resource(CurrentProject {
            path: std::path::PathBuf::from("/tmp/project"),
            config: ProjectConfig {
                kind,
                ..Default::default()
            },
        });
        app.add_systems(Update, reject_unnamed_entities);
        app
    }

    /// Stand in for the generated crate root, which registers each of the
    /// project's component types and records the id against the author's name.
    /// Doing it the same way here is the point: the labels cannot be read off
    /// the components at runtime (Bevy's `debug` feature is off), so a test that
    /// faked the table would not be testing the mechanism that has to work.
    fn register_labels(app: &mut App) {
        let player = app.world_mut().register_component::<Player>();
        let collectible = app.world_mut().register_component::<Collectible>();
        let mut labels = ProjectComponentLabels::default();
        labels.insert(player, "Player", "src/lib.rs", 1);
        labels.insert(collectible, "Collectible", "src/lib.rs", 2);
        app.insert_resource(labels);
    }

    /// The reason this exists at all: a Bevy crate spawns hundreds of entities
    /// and names none of them, and the guard as written despawned every one.
    #[test]
    fn a_code_first_project_has_its_entities_named_not_despawned() {
        let mut app = app(ProjectKind::Bevy);
        register_labels(&mut app);
        let player = app.world_mut().spawn((Player, Transform::default())).id();
        let camera = app.world_mut().spawn((Camera3d::default(), Transform::default())).id();
        let star = app.world_mut().spawn((Collectible, Transform::default())).id();
        let star2 = app.world_mut().spawn((Collectible, Transform::default())).id();

        // Twice, because the despawning path is two-strike and this one must not
        // be: an entity that survives one frame must survive the next as well.
        app.update();
        app.update();

        let name = |e| app.world().get::<Name>(e).map(|n| n.as_str().to_string());
        assert_eq!(name(player).as_deref(), Some("Player"));
        assert_eq!(name(star).as_deref(), Some("Collectible"));
        // Counted per label, so the second star is 2 rather than a global index.
        assert_eq!(name(star2).as_deref(), Some("Collectible 2"));
        // Nothing in the project defined this one, so the label comes from the
        // Bevy component that says what it is, not from `Transform`, which
        // every entity here has.
        assert_eq!(name(camera).as_deref(), Some("Camera3d"));
    }

    /// An entity that named itself keeps its name. Auto-naming is a floor, not
    /// an override: `Star (tower top)` is worth more than `Collectible 14`.
    #[test]
    fn an_entity_that_already_has_a_name_is_left_alone() {
        let mut app = app(ProjectKind::Bevy);
        register_labels(&mut app);
        let e = app
            .world_mut()
            .spawn((Player, Transform::default(), Name::new("Hero")))
            .id();
        app.update();
        assert_eq!(app.world().get::<Name>(e).map(|n| n.as_str()), Some("Hero"));
    }

    /// The historical behaviour, unchanged. An authored project outside play
    /// mode touches nothing: `PlayModeState` is absent here, but so is
    /// `EditorSession`, which is the "this is a shipped game" case where the
    /// guard enforces.
    #[test]
    fn an_authored_project_still_despawns_rather_than_naming() {
        let mut app = app(ProjectKind::Authored);
        let e = app.world_mut().spawn((Player, Transform::default())).id();
        // Two-strike: seen on the first pass, taken on the second.
        app.update();
        assert!(app.world().get_entity(e).is_ok());
        app.update();
        assert!(app.world().get_entity(e).is_err());
        assert!(app.world().get::<Name>(e).is_none());
    }

    /// `HideInHierarchy` is how editor chrome opts out, and it has to keep
    /// opting out of the naming path too, or every preview rig and
    /// gizmo helper appears in a Bevy project's hierarchy.
    #[test]
    fn hidden_chrome_is_neither_named_nor_despawned() {
        let mut app = app(ProjectKind::Bevy);
        let e = app
            .world_mut()
            .spawn((Transform::default(), HideInHierarchy))
            .id();
        app.update();
        app.update();
        assert!(app.world().get_entity(e).is_ok());
        assert!(app.world().get::<Name>(e).is_none());
    }
}

/// Mark a code-first project's named, unclassified entities as the editor's.
///
/// The other half of [`FromProjectCode`]. Anything that arrives already carrying
/// a `Name` was put there by the editor (Add Entity, a model dropped into the
/// viewport, a paste) because the project's own spawns do not name anything and
/// are tagged as they are named.
///
/// Running a frame *after* the naming pass is what makes that true rather than
/// merely likely: a code-spawned entity gets `Name` and `FromProjectCode` in the
/// same command, so by the time it could match this query it no longer does.
///
/// This is what the write-back generates Rust from, which is why it is a stored
/// marker and not a query written at save time: by then the evidence for how an
/// entity arrived is gone.
fn claim_for_the_editor(commands: &mut Commands, unclaimed: &Query<Entity, UnclaimedContent>) {
    for entity in unclaimed.iter() {
        commands.entity(entity).try_insert(AuthoredByEditor);
    }
}
