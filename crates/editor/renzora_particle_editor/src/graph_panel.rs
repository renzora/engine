//! Node Graph Particle Editor Panel — Niagara-style visual node editor.

use std::cell::RefCell;

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular::FLOW_ARROW;

use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::{Theme, ThemeManager};

use renzora_hanabi::{ParticleEditorState, node_graph::ParticleNodeGraph};

use crate::graph_editor::{self, GraphEditorState};

thread_local! {
    static GRAPH_EDITOR_STATE: RefCell<GraphEditorState> = RefCell::new(GraphEditorState::default());
}

pub struct ParticleGraphPanel;

impl EditorPanel for ParticleGraphPanel {
    fn id(&self) -> &str {
        "particle_graph"
    }

    fn title(&self) -> &str {
        "Particle Graph"
    }

    fn icon(&self) -> Option<&str> {
        Some(FLOW_ARROW)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let cmds = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let Some(editor_state) = world.get_resource::<ParticleEditorState>() else {
            return;
        };

        // If no effect is loaded, show a message
        if editor_state.current_effect.is_none() {
            let text_muted = theme.text.muted.to_color32();
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);
                ui.label(RichText::new("Particle Graph Editor").size(20.0).color(text_muted));
                ui.add_space(16.0);
                ui.label(RichText::new("Load an effect in the Particle Editor panel first").size(12.0).color(text_muted));
            });
            return;
        }

        let effect_name = editor_state.current_effect.as_ref()
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "New Effect".to_string());

        // Initialize graph if needed (generate from current effect)
        let mut node_graph = editor_state.node_graph.clone()
            .unwrap_or_else(|| {
                if let Some(ref effect) = editor_state.current_effect {
                    ParticleNodeGraph::from_effect(effect)
                } else {
                    ParticleNodeGraph::new(&effect_name)
                }
            });

        let (graph_modified, selected) = GRAPH_EDITOR_STATE.with(|cell| {
            let mut graph_state = cell.borrow_mut();
            let m = graph_editor::render_graph_editor(ui, &mut node_graph, &mut graph_state, &theme);
            let sel = if graph_state.widget_state.selected.len() == 1 {
                Some(graph_state.widget_state.selected[0])
            } else {
                None
            };
            (m, sel)
        });

        // Always sync selected node to editor state
        let prev_selected = editor_state.selected_node;
        if selected != prev_selected {
            let sel = selected;
            cmds.push(move |world: &mut World| {
                world.resource_mut::<ParticleEditorState>().selected_node = sel;
            });
        }

        if graph_modified {
            cmds.push(move |world: &mut World| {
                let mut state = world.resource_mut::<ParticleEditorState>();
                state.node_graph = Some(node_graph);
                state.is_modified = true;
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}
