//! The hierarchy's eye (`EditorHidden`), applied on top of `Visibility`.
//!
//! An entity hidden with the eye is hidden in the editor always, and in the
//! game only when the project turns on `editor_hidden_in_game`. Either way the
//! entity's own `Visibility` is left alone: that is what the game draws by, it
//! is what the inspector and scripts set, and the eye writing it made the
//! eye's state leak into the game and get overwritten by anything else that
//! owns `Visibility` (issue #75: a UI canvas's visibility mode turned a hidden
//! canvas visible again on every load and every undo).
//!
//! So this works one step later, on `InheritedVisibility`, which Bevy derives
//! from `Visibility` and which everything that draws or picks actually reads
//! (meshes, sprites and lights through `CheckVisibility`, UI rendering and UI
//! picking directly). It runs right after Bevy's propagation and turns the
//! marked entities and everything under them off. Bevy only recomputes an
//! entity whose `Visibility` or parent changed, so a forced value stays put
//! until something moves it, and this pass puts it back the same frame when
//! something does.
//!
//! Undoing the forcing is the other half: when the marker goes, or the setting
//! is switched off in a game, touching `Visibility` makes Bevy recompute the
//! subtree from the real values on the next frame. Here rather than in the
//! editor because the game applies the same marker, and the setting is the
//! game's to read (a settings menu can flip it while the game runs).

use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use renzora_project_api::{CurrentProject, EditorSession};
use renzora_scene_api::EditorHidden;

pub fn register(app: &mut App) {
    app.add_systems(
        PostUpdate,
        apply_editor_hidden
            .after(VisibilitySystems::VisibilityPropagate)
            .before(VisibilitySystems::CheckVisibility),
    );
}

/// Whether eye-hidden entities are hidden in this process: always while
/// editing, and in a game only when the project says so.
fn hides(is_editor: bool, hidden_in_game: bool) -> bool {
    is_editor || hidden_in_game
}

#[allow(clippy::too_many_arguments)]
fn apply_editor_hidden(
    session: Option<Res<EditorSession>>,
    project: Option<Res<CurrentProject>>,
    mut was_hiding: Local<bool>,
    marked: Query<Entity, With<EditorHidden>>,
    mut unmarked: RemovedComponents<EditorHidden>,
    children: Query<&Children>,
    mut inherited: Query<&mut InheritedVisibility>,
    mut visibility: Query<&mut Visibility>,
) {
    let hiding = hides(
        session.is_some_and(|s| s.0),
        project.is_some_and(|p| p.config.editor_hidden_in_game),
    );

    // Entities that stop being hidden get their real visibility back from
    // Bevy's next propagation. Read the removals every frame, hiding or not,
    // so a stale one never surfaces later.
    let mut released: Vec<Entity> = unmarked.read().collect();
    if *was_hiding && !hiding {
        released.extend(marked.iter());
    }
    *was_hiding = hiding;
    for entity in released {
        if let Ok(mut v) = visibility.get_mut(entity) {
            v.set_changed();
        }
    }
    if !hiding {
        return;
    }

    let mut stack: Vec<Entity> = marked.iter().collect();
    while let Some(entity) = stack.pop() {
        // Compare first: writing an unchanged value would still mark it
        // changed, and the UI extraction re-sends every node that is.
        if let Ok(mut iv) = inherited.get_mut(entity) {
            if iv.get() {
                *iv = InheritedVisibility::HIDDEN;
            }
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(is_editor: bool) -> App {
        let mut app = App::new();
        app.insert_resource(EditorSession(is_editor));
        app.add_systems(Update, apply_editor_hidden);
        app
    }

    fn visible(app: &App, e: Entity) -> bool {
        app.world().get::<InheritedVisibility>(e).is_some_and(|v| v.get())
    }

    #[test]
    fn the_editor_always_hides_and_a_game_only_when_asked() {
        assert!(hides(true, false));
        assert!(hides(true, true));
        assert!(!hides(false, false));
        assert!(hides(false, true));
    }

    #[test]
    fn a_hidden_entity_and_everything_under_it_is_hidden_in_the_editor() {
        let mut app = app(true);
        let parent = app
            .world_mut()
            .spawn((EditorHidden, Visibility::Inherited, InheritedVisibility::VISIBLE))
            .id();
        let child = app
            .world_mut()
            .spawn((Visibility::Inherited, InheritedVisibility::VISIBLE, ChildOf(parent)))
            .id();
        let other = app
            .world_mut()
            .spawn((Visibility::Inherited, InheritedVisibility::VISIBLE))
            .id();
        app.update();
        assert!(!visible(&app, parent));
        assert!(!visible(&app, child));
        assert!(visible(&app, other), "an unmarked entity is left alone");
        assert_eq!(
            app.world().get::<Visibility>(parent),
            Some(&Visibility::Inherited),
            "the eye never writes the Visibility the game draws by"
        );
    }

    #[test]
    fn a_game_ignores_the_marker_by_default() {
        let mut app = app(false);
        let e = app
            .world_mut()
            .spawn((EditorHidden, Visibility::Inherited, InheritedVisibility::VISIBLE))
            .id();
        app.update();
        assert!(visible(&app, e));
    }

    #[test]
    fn removing_the_marker_asks_bevy_to_recompute() {
        let mut app = app(true);
        let e = app
            .world_mut()
            .spawn((EditorHidden, Visibility::Inherited, InheritedVisibility::VISIBLE))
            .id();
        app.update();
        app.world_mut().entity_mut(e).remove::<EditorHidden>();
        let before = app.world().read_change_tick();
        app.update();
        let ticks = app
            .world()
            .entity(e)
            .get_change_ticks::<Visibility>()
            .expect("visibility");
        assert!(
            ticks.is_changed(before, app.world().read_change_tick()),
            "Visibility is touched so the next propagation restores the real value"
        );
    }
}
