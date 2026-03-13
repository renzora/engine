//! Shader code editor panel — syntax-highlighted code editing with multi-language support.

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use egui_phosphor::regular::{
    FLOPPY_DISK, LIGHTNING, PLAY, FILE_PLUS, FOLDER_OPEN,
};

use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_shader::file::ShaderFile;
use renzora_shader::registry::ShaderBackendRegistry;
use renzora_theme::ThemeManager;

use crate::ShaderEditorState;

use std::sync::RwLock;

/// Panel for editing shader source code.
pub struct ShaderCodePanel {
    local_source: RwLock<String>,
    local_language: RwLock<String>,
}

impl Default for ShaderCodePanel {
    fn default() -> Self {
        Self {
            local_source: RwLock::new(String::new()),
            local_language: RwLock::new("WGSL".into()),
        }
    }
}

impl ShaderCodePanel {
    fn syntax_for_language(lang: &str) -> Syntax {
        match lang.to_uppercase().as_str() {
            "WGSL" => Syntax::new("wgsl")
                .with_comment("//")
                .with_comment_multiline(["/*", "*/"])
                .with_keywords([
                    "fn", "let", "var", "const", "struct", "if", "else", "for", "while", "loop",
                    "return", "discard", "switch", "case", "default", "break", "continue",
                    "enable", "override", "alias",
                ])
                .with_types([
                    "bool", "i32", "u32", "f32", "f16",
                    "vec2", "vec3", "vec4", "vec2i", "vec3i", "vec4i",
                    "vec2u", "vec3u", "vec4u", "vec2f", "vec3f", "vec4f",
                    "mat2x2", "mat2x3", "mat2x4", "mat3x2", "mat3x3", "mat3x4",
                    "mat4x2", "mat4x3", "mat4x4",
                    "mat2x2f", "mat3x3f", "mat4x4f",
                    "texture_2d", "texture_3d", "texture_cube",
                    "sampler", "sampler_comparison", "array", "atomic", "ptr",
                ])
                .with_special([
                    "textureSample", "textureSampleLevel", "textureLoad", "textureStore",
                    "dot", "cross", "normalize", "length", "distance",
                    "mix", "clamp", "smoothstep", "step",
                    "abs", "ceil", "floor", "round", "fract",
                    "cos", "sin", "tan", "exp", "log", "pow", "sqrt",
                    "min", "max", "saturate", "select",
                    "transpose", "determinant",
                ]),
            "GLSL" | "SHADERTOY" => Syntax::new("glsl")
                .with_comment("//")
                .with_comment_multiline(["/*", "*/"])
                .with_keywords([
                    "void", "return", "if", "else", "for", "while", "do", "break", "continue",
                    "discard", "switch", "case", "default", "struct", "in", "out", "inout",
                    "uniform", "varying", "layout", "location", "binding", "set",
                    "precision", "highp", "mediump", "lowp", "#version", "#define",
                ])
                .with_types([
                    "float", "int", "uint", "bool", "double",
                    "vec2", "vec3", "vec4", "ivec2", "ivec3", "ivec4",
                    "uvec2", "uvec3", "uvec4", "bvec2", "bvec3", "bvec4",
                    "mat2", "mat3", "mat4", "mat2x2", "mat3x3", "mat4x4",
                    "sampler2D", "sampler3D", "samplerCube",
                ])
                .with_special([
                    "texture", "textureLod", "normalize", "length", "distance",
                    "dot", "cross", "mix", "clamp", "smoothstep", "step",
                    "abs", "ceil", "floor", "round", "fract",
                    "cos", "sin", "tan", "atan", "exp", "log", "pow", "sqrt",
                    "min", "max",
                    // ShaderToy builtins
                    "mainImage", "fragColor", "fragCoord",
                    "iTime", "iResolution", "iMouse", "iTimeDelta", "iFrame",
                ]),
            _ => Syntax::new("glsl"),
        }
    }
}

impl EditorPanel for ShaderCodePanel {
    fn id(&self) -> &str {
        "shader_editor"
    }

    fn title(&self) -> &str {
        "Shader Editor"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::CODE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => &tm.active_theme,
            None => return,
        };
        let muted = theme.text.muted.to_color32();
        let disabled = theme.text.disabled.to_color32();
        let _error_color = theme.semantic.error.to_color32();
        let success_color = theme.semantic.success.to_color32();

        let Some(state) = world.get_resource::<ShaderEditorState>() else { return };

        // Sync local source from state
        if let Ok(mut local) = self.local_source.write() {
            if local.is_empty() && !state.shader_file.shader_source.is_empty() {
                *local = state.shader_file.shader_source.clone();
            }
        }
        if let Ok(mut local_lang) = self.local_language.write() {
            if *local_lang != state.shader_file.language {
                *local_lang = state.shader_file.language.clone();
            }
        }

        let available_languages = world
            .get_resource::<ShaderBackendRegistry>()
            .map(|r| r.languages())
            .unwrap_or_default();

        // ── Toolbar ──
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    // Language dropdown
                    let current_lang = self.local_language.read().map(|l| l.clone()).unwrap_or("WGSL".into());
                    egui::ComboBox::from_id_salt("shader_lang")
                        .selected_text(&current_lang)
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for lang in &available_languages {
                                let lang_str = lang.to_string();
                                if ui.selectable_label(current_lang == lang_str, &lang_str).clicked() {
                                    if let Ok(mut local_lang) = self.local_language.write() {
                                        *local_lang = lang_str.clone();
                                    }
                                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                        let l = lang_str;
                                        cmds.push(move |world: &mut World| {
                                            if let Some(mut s) = world.get_resource_mut::<ShaderEditorState>() {
                                                s.shader_file.language = l;
                                                s.is_modified = true;
                                            }
                                        });
                                    }
                                }
                            }
                        });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Compile button
                    if ui.button(RichText::new(format!("{} Compile", PLAY)).size(12.0)).clicked() {
                        if let Ok(source) = self.local_source.read() {
                            let src = source.clone();
                            let lang = current_lang.clone();
                            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                cmds.push(move |world: &mut World| {
                                    compile_shader(world, &lang, &src);
                                });
                            }
                        }
                    }

                    // Auto-compile toggle
                    let auto_label = if state.auto_compile {
                        RichText::new(format!("{} Auto", LIGHTNING)).size(12.0).color(success_color)
                    } else {
                        RichText::new(format!("{} Auto", LIGHTNING)).size(12.0).color(muted)
                    };
                    if ui.button(auto_label).clicked() {
                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                            let new_val = !state.auto_compile;
                            cmds.push(move |world: &mut World| {
                                if let Some(mut s) = world.get_resource_mut::<ShaderEditorState>() {
                                    s.auto_compile = new_val;
                                }
                            });
                        }
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // New
                    if ui.button(RichText::new(format!("{}", FILE_PLUS)).size(12.0)).clicked() {
                        if let Ok(mut local) = self.local_source.write() {
                            let default_file = ShaderFile::default();
                            *local = default_file.shader_source.clone();
                        }
                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                            cmds.push(|world: &mut World| {
                                if let Some(mut s) = world.get_resource_mut::<ShaderEditorState>() {
                                    *s = ShaderEditorState::default();
                                }
                            });
                        }
                    }

                    // Open
                    if ui.button(RichText::new(format!("{}", FOLDER_OPEN)).size(12.0)).clicked() {
                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                            cmds.push(|world: &mut World| {
                                open_shader_file(world);
                            });
                        }
                    }

                    // Save
                    let save_label = if state.is_modified {
                        RichText::new(format!("{}", FLOPPY_DISK)).size(12.0).color(egui::Color32::from_rgb(255, 200, 80))
                    } else {
                        RichText::new(format!("{}", FLOPPY_DISK)).size(12.0).color(muted)
                    };
                    if ui.button(save_label).clicked() {
                        if let Ok(source) = self.local_source.read() {
                            let src = source.clone();
                            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                cmds.push(move |world: &mut World| {
                                    save_shader_file(world, &src);
                                });
                            }
                        }
                    }

                    // Status: file path or "unsaved"
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(ref path) = state.file_path {
                            ui.label(RichText::new(path).size(10.0).color(disabled));
                        } else {
                            ui.label(RichText::new("unsaved").size(10.0).color(disabled));
                        }
                    });
                });
            });

        ui.separator();

        // ── Code Editor ──
        let syntax = {
            let lang = self.local_language.read().map(|l| l.clone()).unwrap_or("WGSL".into());
            Self::syntax_for_language(&lang)
        };

        let mut source_changed = false;
        if let Ok(mut local_source) = self.local_source.write() {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let response = CodeEditor::default()
                    .id_source("shader_code_editor")
                    .with_syntax(syntax)
                    .with_fontsize(14.0)
                    .with_theme(ColorTheme::GRUVBOX)
                    .with_numlines(true)
                    .show(ui, &mut *local_source);

                if response.response.changed() {
                    source_changed = true;
                }
            });
        }

        // Push source changes back to state
        if source_changed {
            if let Ok(source) = self.local_source.read() {
                let src = source.clone();
                let auto_compile = state.auto_compile;
                let lang = self.local_language.read().map(|l| l.clone()).unwrap_or("WGSL".into());

                if let Some(cmds) = world.get_resource::<EditorCommands>() {
                    cmds.push(move |world: &mut World| {
                        if let Some(mut s) = world.get_resource_mut::<ShaderEditorState>() {
                            s.shader_file.shader_source = src.clone();
                            s.is_modified = true;
                        }
                        if auto_compile {
                            compile_shader(world, &lang, &src);
                        }
                    });
                }
            }
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn min_size(&self) -> [f32; 2] {
        [300.0, 200.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Helper functions ─────────────────────────────────────────────────────────

fn compile_shader(world: &mut World, language: &str, source: &str) {
    // Extract @param annotations from source before compilation
    let extracted_params = renzora_shader::file::extract_params(source);

    let result = {
        let registry = world.resource::<ShaderBackendRegistry>();
        registry.compile(language, source)
    };

    match result {
        Ok(wgsl) => {
            let mut state = world.resource_mut::<ShaderEditorState>();
            state.compiled_wgsl = Some(wgsl);
            state.compile_errors.clear();

            // Merge extracted params — keep user-edited values for existing params,
            // add new ones, remove params no longer in source
            let old_params = std::mem::take(&mut state.shader_file.params);
            for (name, mut param) in extracted_params {
                // Preserve user-edited value if the param already existed with same type
                if let Some(old) = old_params.get(&name) {
                    if old.param_type == param.param_type {
                        param.default_value = old.default_value.clone();
                    }
                }
                state.shader_file.params.insert(name, param);
            }
        }
        Err(err) => {
            let mut state = world.resource_mut::<ShaderEditorState>();
            state.compiled_wgsl = None;
            state.compile_errors = vec![err];
        }
    }
}

fn open_shader_file(world: &mut World) {
    let file = rfd::FileDialog::new()
        .add_filter("Shader", &["shader"])
        .pick_file();

    if let Some(path) = file {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<ShaderFile>(&content) {
                    Ok(shader_file) => {
                        let mut state = world.resource_mut::<ShaderEditorState>();
                        state.shader_file = shader_file;
                        state.file_path = Some(path.display().to_string());
                        state.is_modified = false;
                        state.compiled_wgsl = None;
                        state.compile_errors.clear();
                    }
                    Err(e) => {
                        bevy::log::error!("Failed to parse .shader file: {}", e);
                    }
                }
            }
            Err(e) => {
                bevy::log::error!("Failed to read file: {}", e);
            }
        }
    }
}

fn save_shader_file(world: &mut World, source: &str) {
    let mut state = world.resource_mut::<ShaderEditorState>();
    state.shader_file.shader_source = source.to_string();

    let path = if let Some(ref p) = state.file_path {
        std::path::PathBuf::from(p)
    } else {
        // Save-as dialog
        let file = rfd::FileDialog::new()
            .add_filter("Shader", &["shader"])
            .set_file_name("new_shader.shader")
            .save_file();
        match file {
            Some(p) => {
                state.file_path = Some(p.display().to_string());
                p
            }
            None => return,
        }
    };

    let json = serde_json::to_string_pretty(&state.shader_file as &ShaderFile).unwrap_or_default();
    drop(state);

    match std::fs::write(&path, &json) {
        Ok(_) => {
            let mut state = world.resource_mut::<ShaderEditorState>();
            state.is_modified = false;
            bevy::log::info!("Saved shader: {}", path.display());
        }
        Err(e) => {
            bevy::log::error!("Failed to save shader: {}", e);
        }
    }
}
