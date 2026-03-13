//! Shader compiler log panel — displays compilation errors and warnings.

use bevy::prelude::*;
use bevy_egui::egui::{self, FontFamily, RichText};
use egui_phosphor::regular::{CHECK_CIRCLE, WARNING};

use renzora_editor::{EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::ShaderEditorState;

pub struct ShaderCompilerLogPanel;

impl EditorPanel for ShaderCompilerLogPanel {
    fn id(&self) -> &str {
        "shader_compiler_log"
    }

    fn title(&self) -> &str {
        "Compiler Log"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::TERMINAL)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => &tm.active_theme,
            None => return,
        };
        let muted = theme.text.muted.to_color32();
        let error_color = theme.semantic.error.to_color32();
        let success_color = theme.semantic.success.to_color32();

        let Some(state) = world.get_resource::<ShaderEditorState>() else { return };

        if state.compile_errors.is_empty() {
            if state.compiled_wgsl.is_some() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(CHECK_CIRCLE).size(12.0).color(success_color));
                    ui.label(RichText::new("Compiled successfully").size(11.0).color(success_color));
                });
            } else {
                ui.label(RichText::new("No compilation output").size(11.0).color(muted));
            }
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for err in &state.compile_errors {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(WARNING).size(12.0).color(error_color));
                    let loc = match (err.line, err.column) {
                        (Some(l), Some(c)) => format!("[{}:{}] ", l, c),
                        (Some(l), None) => format!("[line {}] ", l),
                        _ => String::new(),
                    };
                    ui.label(
                        RichText::new(format!("{}{}", loc, err.message))
                            .size(11.0)
                            .color(error_color)
                            .family(FontFamily::Monospace),
                    );
                });
            }
        });
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }
}
