//! Editor half of `renzora_terrain_graph`: the Terrain Graph editor.
//!
//! A `.tgraph` opens (double-click it in the Assets panel, or Edit Graph on a
//! terrain's generator) as a document in the **Terrain** workspace:
//!
//! - [`graph_panel`]: the graph on ember's node graph view, the same widget
//!   and conventions as the material and effect graphs, each node showing a
//!   shaded preview of its result;
//! - [`inspector_panel`]: the selected node's parameters, the graph's
//!   exposed parameters, and Build for the terrain it targets;
//! - [`preview`]: the background re-evaluation behind the node previews;
//! - [`build`]: full-resolution builds in the background, committed as one
//!   undo step that keeps the terrain's sculpting;
//! - [`generator_inspector`]: the parsed graphs the Terrain component's
//!   generator card (its Manage tab) reads.
//!
//! New graphs come from the Assets panel's create menu, one entry per shipped
//! preset (`renzora_terrain_graph::presets`).

pub mod build;
pub mod generator_inspector;
pub mod graph_panel;
pub mod inspector_panel;
pub mod preview;
pub mod state;

use bevy::prelude::*;
use renzora::core::keybindings::KeyBinding;
use renzora::create_menu::{CreateMenuItem, RegisterCreateMenuItem};
use renzora::RenzoraShellExt;
use renzora_editor_framework::{AppEditorExt, ShortcutEntry, SplashState};
use renzora_ember::dock::{panel_active, Dock, DockTree, DockWindows, FixedDock};
use renzora_ember::workspace::RegisterWorkspace;

pub use build::{cancel_build, request_build};

/// The Terrain workspace: the graph, with the viewport (where a build lands)
/// over the inspector beside it.
pub fn workspace_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf(graph_panel::PANEL),
        DockTree::vertical(DockTree::leaf("viewport"), DockTree::leaf(inspector_panel::PANEL), 0.45),
        0.64,
    )
}

#[derive(Default)]
pub struct TerrainGraphEditorPlugin;

impl Plugin for TerrainGraphEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TerrainGraphEditorPlugin");
        app.init_resource::<state::TerrainGraphEditorState>()
            .init_resource::<state::GraphShadow>()
            .init_resource::<preview::PreviewImages>()
            .init_resource::<preview::PreviewJob>()
            .init_resource::<build::BuildJobs>()
            .init_resource::<generator_inspector::GraphFiles>()
            .register_shell_panel(graph_panel::PANEL, "Terrain Graph", "graph", "Terrain")
            .register_shell_panel(inspector_panel::PANEL, "Terrain Graph Inspector", "sliders-horizontal", "Terrain")
            .register_workspace("Terrain", workspace_layout())
            .add_plugins((graph_panel::TerrainGraphPanel, inspector_panel::TerrainGraphInspectorPanel));

        app.register_shortcut(ShortcutEntry::new(
            "terrain_graph.save",
            "Save Terrain Graph",
            "File",
            KeyBinding::new(KeyCode::KeyS).ctrl(),
            |w| {
                let visible = [graph_panel::PANEL, inspector_panel::PANEL].iter().any(|id| {
                    renzora_ember::dock::panel_visible_anywhere(id, w.get_resource::<Dock>(), w.get_resource::<FixedDock>(), w.get_resource::<DockWindows>())
                });
                if visible && w.resource::<state::TerrainGraphEditorState>().dirty {
                    state::save_document(w);
                }
            },
        ));

        for (id, name, description, build) in renzora_terrain_graph::presets::PRESETS {
            let label = if *id == "empty" { "Terrain Graph".to_string() } else { format!("Terrain Graph: {name}") };
            let stem = if *id == "empty" { "terrain_graph".to_string() } else { (*id).to_string() };
            let build = *build;
            app.register_create_menu_item(
                CreateMenuItem::new(format!("terrain_graph_{id}"), stem, renzora_terrain_graph::EXTENSION, move |_| build().to_json())
                    .label(format!("assets.new.terrain_graph_{id}"), label)
                    .subtitle(format!("assets.new.terrain_graph_{id}_sub"), *description)
                    .icon("mountains")
                    .folder("terrain"),
            );
        }

        // Document orchestration runs whichever panel is showing, as the
        // effect editor's does: opening a graph from the Assets panel must
        // load it before either panel has been built.
        app.add_systems(
            Update,
            (state::load_active_document, state::observe_edits, generator_inspector::refresh_graph_files)
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );
        // Previews only while the graph is on screen: nobody sees them
        // otherwise, and an evaluation per edit is not free.
        app.add_systems(
            Update,
            (preview::schedule_previews, preview::poll_previews)
                .chain()
                .after(state::observe_edits)
                .run_if(in_state(SplashState::Editor))
                .run_if(panel_active(graph_panel::PANEL)),
        );
        // Builds run to the end whatever is showing: a build started from the
        // entity inspector must land with the Terrain workspace closed.
        app.add_systems(Update, (build::start_builds, build::poll_builds).chain().run_if(in_state(SplashState::Editor)));
    }
}

renzora::add!(TerrainGraphEditorPlugin, Editor);
