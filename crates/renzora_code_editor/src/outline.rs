//! Outline panel — lists functions in the active editor tab and jumps the
//! editor to them on click. Reads `CodeEditorState`; click writes
//! `pending_goto_line` via `EditorCommands`.

use bevy::prelude::*;
use bevy_egui::egui::{self, CursorIcon, FontId, RichText, Sense, Vec2};
use egui_phosphor::regular::{LIST_BULLETS, X};
use renzora_editor_framework::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::highlight::Language;
use crate::state::CodeEditorState;

#[derive(Debug, Clone)]
pub struct OutlineSymbol {
    pub name: String,
    pub line: usize, // 0-based
    pub kind: SymbolKind,
}

#[derive(Debug, Clone, Copy)]
pub enum SymbolKind {
    Function,
    Class,
}

pub fn extract_symbols(content: &str, lang: Language) -> Vec<OutlineSymbol> {
    let mut out = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        match lang {
            Language::Lua => {
                if let Some(name) = parse_lua_function(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Function,
                    });
                }
            }
            Language::Rust | Language::Rhai | Language::Wgsl => {
                if let Some(name) = parse_c_style_fn(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Function,
                    });
                }
                if matches!(lang, Language::Rust) {
                    if let Some(name) = parse_rust_struct_or_enum(trimmed) {
                        out.push(OutlineSymbol {
                            name,
                            line: line_idx,
                            kind: SymbolKind::Class,
                        });
                    }
                }
            }
            Language::Python => {
                if let Some(name) = parse_python_def(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Function,
                    });
                }
                if let Some(name) = parse_python_class(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Class,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_lua_function(line: &str) -> Option<String> {
    let s = line.strip_prefix("local ").unwrap_or(line);
    let s = s.strip_prefix("function ")?;
    let end = s
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_c_style_fn(line: &str) -> Option<String> {
    let s = line.strip_prefix("pub ").unwrap_or(line);
    let s = s.strip_prefix("async ").unwrap_or(s);
    let s = s.strip_prefix("fn ")?;
    let end = s
        .find(|c: char| c == '(' || c == '<' || c.is_whitespace())
        .unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_rust_struct_or_enum(line: &str) -> Option<String> {
    let s = line.strip_prefix("pub ").unwrap_or(line);
    for prefix in ["struct ", "enum ", "trait ", "impl "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let end = rest
                .find(|c: char| c == '<' || c == '{' || c == '(' || c.is_whitespace())
                .unwrap_or(rest.len());
            let name = rest[..end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn parse_python_def(line: &str) -> Option<String> {
    let s = line.strip_prefix("def ")?;
    let end = s.find('(').unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_python_class(line: &str) -> Option<String> {
    let s = line.strip_prefix("class ")?;
    let end = s
        .find(|c: char| c == '(' || c == ':' || c.is_whitespace())
        .unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub struct OutlinePanel;

impl EditorPanel for OutlinePanel {
    fn id(&self) -> &str {
        "outline"
    }
    fn title(&self) -> &str {
        "Outline"
    }
    fn icon(&self) -> Option<&str> {
        Some(LIST_BULLETS)
    }
    fn closable(&self) -> bool {
        true
    }
    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let muted = theme.text.muted.to_color32();
        let primary = theme.text.primary.to_color32();
        let accent = theme.semantic.accent.to_color32();

        let Some(state) = world.get_resource::<CodeEditorState>() else {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No file open").size(11.0).color(muted));
            });
            return;
        };

        let Some(active_idx) = state.active_tab else {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No file open").size(11.0).color(muted));
            });
            return;
        };
        let Some(file) = state.open_files.get(active_idx) else { return };

        let lang = file
            .path
            .extension()
            .and_then(|e| e.to_str())
            .map(Language::from_extension)
            .unwrap_or(Language::PlainText);

        let symbols = extract_symbols(&file.content, lang);

        if symbols.is_empty() {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No symbols in this file")
                        .size(11.0)
                        .color(muted),
                );
            });
            return;
        }

        let cmds = world.get_resource::<EditorCommands>();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;
                for sym in &symbols {
                    let row_height = 22.0;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            theme.widgets.hovered_bg.to_color32(),
                        );
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let icon = match sym.kind {
                        SymbolKind::Function => "ƒ",
                        SymbolKind::Class => "C",
                    };
                    let icon_color = match sym.kind {
                        SymbolKind::Function => accent,
                        SymbolKind::Class => muted,
                    };
                    ui.painter().text(
                        egui::Pos2::new(rect.min.x + 8.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        icon,
                        FontId::monospace(13.0),
                        icon_color,
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.min.x + 26.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &sym.name,
                        FontId::proportional(11.5),
                        primary,
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.max.x - 8.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        format!("{}", sym.line + 1),
                        FontId::monospace(10.5),
                        muted,
                    );

                    if resp.clicked() {
                        let line_1based = sym.line + 1;
                        if let Some(c) = cmds {
                            c.push(move |world: &mut World| {
                                if let Some(mut s) =
                                    world.get_resource_mut::<CodeEditorState>()
                                {
                                    s.pending_goto_line = Some(line_1based);
                                    s.goto_line_open = false;
                                }
                            });
                        }
                    }
                }
            });

        let _ = X;
    }
}
