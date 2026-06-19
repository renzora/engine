//! Hierarchy panel — shows the scene entity tree.

mod cache;
pub mod native;
mod state;

use bevy::prelude::*;
use renzora_editor_framework::{
    AppEditorExt, AutoSelectFirstHierarchyEntity, EditorSelection, SceneStarter, SpawnRegistry,
};

use cache::{HierarchyDirty, HierarchyTreeCache};

/// Label color presets: ([r, g, b], name).
pub const LABEL_COLORS: &[([u8; 3], &str)] = &[
    ([220, 70, 70], "Red"),
    ([210, 120, 80], "Coral"),
    ([220, 140, 60], "Orange"),
    ([210, 175, 55], "Amber"),
    ([210, 195, 60], "Yellow"),
    ([160, 210, 60], "Lime"),
    ([70, 190, 100], "Green"),
    ([55, 185, 155], "Teal"),
    ([60, 200, 200], "Cyan"),
    ([70, 170, 220], "Sky"),
    ([80, 140, 220], "Blue"),
    ([90, 100, 220], "Indigo"),
    ([155, 80, 220], "Purple"),
    ([190, 70, 200], "Violet"),
    ([220, 80, 180], "Pink"),
    ([220, 80, 120], "Rose"),
    ([160, 110, 75], "Brown"),
    ([130, 130, 140], "Gray"),
    ([200, 200, 200], "White"),
];

/// Plugin that registers the native hierarchy panel and built-in entity presets.
/// One-shot consumer of `AutoSelectFirstHierarchyEntity`. After scene load,
/// the scene crate sets the flag; once the cache rebuild lands a non-empty
/// tree on the next frame, we select its first root entity (skipping if
/// the user already manually selected something while the scene was loading).
fn auto_select_first_hierarchy_entity(
    mut pending: ResMut<AutoSelectFirstHierarchyEntity>,
    cache: Res<HierarchyTreeCache>,
    selection: Res<EditorSelection>,
) {
    if !pending.0 {
        return;
    }
    if cache.nodes.is_empty() {
        return;
    }
    if selection.get().is_none() {
        if let Some(top) = cache.nodes.first() {
            selection.set(Some(top.entity));
        }
    }
    pending.0 = false;
}

#[derive(Default)]
pub struct HierarchyPanelPlugin;

impl Plugin for HierarchyPanelPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HierarchyPanelPlugin");
        // Bevy-native (ember) hierarchy panel for the bevy_ui shell. Reads the
        // shared HierarchyTreeCache + EditorSelection.
        native::register_native_hierarchy(app);
        app.init_resource::<RenameRequest>();
        app.init_resource::<HierarchyTreeCache>();
        app.init_resource::<HierarchyDirty>();
        app.add_systems(
            bevy::prelude::Update,
            (
                detect_selection_keybindings,
                cache::mark_hierarchy_dirty,
                cache::update_hierarchy_cache.after(cache::mark_hierarchy_dirty),
                auto_select_first_hierarchy_entity.after(cache::update_hierarchy_cache),
            ),
        );

        // Spawn presets are now self-registered by their owning crates:
        // - Bevy types (Empty, lights, camera): renzora_editor_framework::bevy_inspectors
        // - Physics: renzora_physics::inspector (editor feature)
        // - Terrain: renzora_terrain (editor feature)
        // - World Environment/Sun: renzora_level_presets
        app.init_resource::<SpawnRegistry>();

        // Scene starters shown on the empty-hierarchy picker. Feature-specific
        // starters (Environment, UI Canvas, Physics Arena) are registered by
        // their owning crates.
        app.register_scene_starter(SceneStarter {
            id: "empty_scene",
            title: "Empty Scene",
            description: "Start with just a camera",
            icon: "circle-dashed",
            spawn_fn: |world: &mut World| {
                use renzora::core::SceneCamera;
                world.spawn((
                    Name::new("Camera"),
                    SceneCamera,
                    Camera3d::default(),
                    Camera {
                        is_active: false,
                        ..default()
                    },
                    Transform::from_xyz(5.0, 4.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
                ));
            },
        });

        // 2D scene starter — spawns a Camera 2D so the project is
        // immediately playable. The Camera2d observer in
        // `renzora_engine::camera` sets `viewport_origin = (0, 1)` on
        // insert, so the camera at world origin renders the
        // Godot-style "window starts at (0, 0)" view that all the
        // editor outlines and rulers expect. Also flips the viewport
        // to 2D mode so the user lands in the right authoring space.
        app.register_scene_starter(SceneStarter {
            id: "scene_2d",
            title: "2D Scene",
            description: "Start with a Camera 2D — sprites, UI, retro pixel art",
            icon: "image-square",
            spawn_fn: |world: &mut World| {
                use renzora::core::viewport_types::{ViewportSettings, ViewportView};
                use renzora::core::{DefaultCamera, SceneCamera};
                world.spawn((
                    Name::new("Camera 2D"),
                    SceneCamera,
                    DefaultCamera,
                    Camera2d,
                    Camera {
                        is_active: false,
                        ..default()
                    },
                    Transform::default(),
                ));
                if let Some(mut settings) = world.get_resource_mut::<ViewportSettings>() {
                    settings.viewport_view = ViewportView::Two;
                }
            },
        });
    }
}

/// A request from the keybinding system to start renaming an entity in the
/// hierarchy panel. Consumed by the panel UI next frame.
#[derive(Resource, Default)]
pub struct RenameRequest(pub Option<Entity>);

/// Watches the new editor keybindings (SelectAll / Rename / Hide / Isolate)
/// and applies their effects.
fn detect_selection_keybindings(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<renzora::core::keybindings::KeyBindings>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    input_focus: Res<renzora::core::InputFocusState>,
    select_all_claimed: Option<Res<renzora::core::SelectAllClaimed>>,
    selection: Res<EditorSelection>,
    entities_q: Query<(
        Entity,
        Option<&bevy::prelude::Name>,
        Option<&renzora::core::HideInHierarchy>,
    )>,
    mut vis_q: Query<&mut bevy::prelude::Visibility>,
    mut rename_req: ResMut<RenameRequest>,
) {
    use renzora::core::keybindings::EditorAction;
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.egui_wants_keyboard {
        return;
    }

    // SelectAll: pick every named, non-hidden-in-hierarchy entity — unless the
    // pointer is over a panel that owns Ctrl+A for its own selection (the asset
    // browser's file grid), which handles the key itself.
    let claimed = select_all_claimed.as_ref().is_some_and(|c| c.0);
    if !claimed && keybindings.just_pressed(EditorAction::SelectAll, &keyboard) {
        let mut all = Vec::new();
        for (e, name, hide) in entities_q.iter() {
            if name.is_some() && hide.is_none() {
                all.push(e);
            }
        }
        selection.set_multiple(all);
    }

    // Rename: start rename on the current primary selection.
    if keybindings.just_pressed(EditorAction::Rename, &keyboard) {
        if let Some(e) = selection.get() {
            rename_req.0 = Some(e);
        }
    }

    // HideSelected: toggle Visibility::Hidden on every selected entity.
    if keybindings.just_pressed(EditorAction::HideSelected, &keyboard) {
        for e in selection.get_all() {
            if let Ok(mut v) = vis_q.get_mut(e) {
                *v = match *v {
                    Visibility::Hidden => Visibility::Visible,
                    _ => Visibility::Hidden,
                };
            }
        }
    }

    // IsolateSelected: hide everything except the current selection (and its
    // ancestors, so the tree stays navigable).
    if keybindings.just_pressed(EditorAction::IsolateSelected, &keyboard) {
        let sel: std::collections::HashSet<Entity> = selection.get_all().into_iter().collect();
        if !sel.is_empty() {
            for (e, name, hide) in entities_q.iter() {
                if name.is_none() || hide.is_some() {
                    continue;
                }
                if let Ok(mut v) = vis_q.get_mut(e) {
                    *v = if sel.contains(&e) {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
    }
}

renzora::add!(HierarchyPanelPlugin, Editor);
