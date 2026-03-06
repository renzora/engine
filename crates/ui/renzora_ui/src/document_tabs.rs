//! Document tab bar — renders between title bar and dock tree.
//!
//! Each open document (scene, script, blueprint, etc.) gets a tab with a
//! type-coded accent color, close button, and modified indicator. Clicking
//! a tab activates it and optionally switches the workspace layout.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use egui_phosphor::regular;
use renzora_theme::Theme;

// ── Constants (matching legacy) ──────────────────────────────────────────────

const TAB_HEIGHT: f32 = 28.0;
const TAB_PADDING: f32 = 8.0;
const TAB_GAP: f32 = 2.0;
const TOP_MARGIN: f32 = 4.0;
const MAX_TAB_WIDTH: f32 = 240.0;
const MIN_TAB_WIDTH: f32 = 60.0;
const ICON_WIDTH: f32 = 18.0;
const CLOSE_BTN_WIDTH: f32 = 20.0;

/// Total height consumed by the document tab bar.
pub const DOC_TAB_BAR_HEIGHT: f32 = TAB_HEIGHT + TOP_MARGIN;

// ── Tab types ────────────────────────────────────────────────────────────────

/// The kind of document a tab represents.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TabKind {
    Scene,
    Script,
    Blueprint,
    Image,
    Video,
    Audio,
    Animation,
    Texture,
    ParticleFX,
    Level,
    Terrain,
    Shader,
}

impl TabKind {
    /// Phosphor icon for this tab type.
    pub fn icon(&self) -> &'static str {
        match self {
            TabKind::Scene => regular::FILM_SCRIPT,
            TabKind::Script => regular::SCROLL,
            TabKind::Blueprint => regular::PALETTE,
            TabKind::Image => regular::IMAGE,
            TabKind::Video => regular::VIDEO,
            TabKind::Audio => regular::MUSIC_NOTES,
            TabKind::Animation => regular::FILM_SCRIPT,
            TabKind::Texture => regular::PAINT_BRUSH,
            TabKind::ParticleFX => regular::SPARKLE,
            TabKind::Level => regular::GAME_CONTROLLER,
            TabKind::Terrain => regular::MOUNTAINS,
            TabKind::Shader => regular::MONITOR,
        }
    }

    /// Accent color for this tab type.
    pub fn accent_color(&self, theme: &Theme) -> Color32 {
        match self {
            TabKind::Scene => theme.semantic.accent.to_color32(),
            TabKind::Script => theme.categories.scripting.accent.to_color32(),
            TabKind::Blueprint => Color32::from_rgb(180, 130, 200),
            TabKind::Image => Color32::from_rgb(166, 217, 140),
            TabKind::Video => Color32::from_rgb(220, 80, 80),
            TabKind::Audio => Color32::from_rgb(180, 100, 220),
            TabKind::Animation => Color32::from_rgb(100, 180, 220),
            TabKind::Texture => Color32::from_rgb(120, 200, 120),
            TabKind::ParticleFX => Color32::from_rgb(255, 180, 50),
            TabKind::Level => Color32::from_rgb(100, 200, 180),
            TabKind::Terrain => Color32::from_rgb(140, 180, 100),
            TabKind::Shader => Color32::from_rgb(180, 130, 255),
        }
    }

    /// Muted version of the accent color for inactive tabs.
    pub fn inactive_color(&self, theme: &Theme) -> Color32 {
        let c = self.accent_color(theme);
        Color32::from_rgb(c.r() / 2 + 50, c.g() / 2 + 50, c.b() / 2 + 55)
    }

    /// The layout name this tab type prefers.
    pub fn preferred_layout(&self) -> &'static str {
        match self {
            TabKind::Scene => "Scene",
            TabKind::Script => "Scripting",
            TabKind::Blueprint => "Blueprints",
            TabKind::Image => "Scene",
            TabKind::Video => "Scene",
            TabKind::Audio => "Scene",
            TabKind::Animation => "Animation",
            TabKind::Texture => "Scene",
            TabKind::ParticleFX => "Particles",
            TabKind::Level => "Level Design",
            TabKind::Terrain => "Terrain",
            TabKind::Shader => "Shaders",
        }
    }

    /// Display name for the + menu.
    pub fn label(&self) -> &'static str {
        match self {
            TabKind::Scene => "Scene",
            TabKind::Script => "Script",
            TabKind::Blueprint => "Blueprint",
            TabKind::Image => "Image",
            TabKind::Video => "Video",
            TabKind::Audio => "Audio",
            TabKind::Animation => "Animation",
            TabKind::Texture => "Texture",
            TabKind::ParticleFX => "Particle FX",
            TabKind::Level => "Level",
            TabKind::Terrain => "Terrain",
            TabKind::Shader => "Shader",
        }
    }
}

// ── Document tab ─────────────────────────────────────────────────────────────

/// A single open document tab.
#[derive(Debug, Clone)]
pub struct DocumentTab {
    /// Unique id for this tab instance.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Document type.
    pub kind: TabKind,
    /// Whether the document has unsaved changes.
    pub is_modified: bool,
}

// ── State resource ───────────────────────────────────────────────────────────

/// Resource managing all open document tabs.
#[derive(Resource, Clone)]
pub struct DocumentTabState {
    /// All open tabs in display order.
    pub tabs: Vec<DocumentTab>,
    /// Index of the currently active tab.
    pub active_tab: usize,
    /// Auto-incrementing ID counter.
    next_id: u64,
}

impl Default for DocumentTabState {
    fn default() -> Self {
        let mut state = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_id: 1,
        };
        // Start with one default scene tab
        state.add_tab("Untitled Scene".into(), TabKind::Scene);
        state
    }
}

impl DocumentTabState {
    /// Add a new tab and return its index.
    pub fn add_tab(&mut self, name: String, kind: TabKind) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(DocumentTab {
            id,
            name,
            kind,
            is_modified: false,
        });
        self.tabs.len() - 1
    }

    /// Close a tab by index. Returns the preferred layout of the newly active tab, if any.
    pub fn close_tab(&mut self, index: usize) -> Option<&'static str> {
        if index >= self.tabs.len() {
            return None;
        }
        // Don't close the last scene tab
        let is_last_scene = self.tabs[index].kind == TabKind::Scene
            && self.tabs.iter().filter(|t| t.kind == TabKind::Scene).count() <= 1;
        if is_last_scene {
            return None;
        }

        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.add_tab("Untitled Scene".into(), TabKind::Scene);
            self.active_tab = 0;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > index {
            self.active_tab -= 1;
        }
        Some(self.tabs[self.active_tab].kind.preferred_layout())
    }

    /// Activate a tab by index. Returns the preferred layout name.
    pub fn activate_tab(&mut self, index: usize) -> Option<&'static str> {
        if index < self.tabs.len() {
            self.active_tab = index;
            Some(self.tabs[index].kind.preferred_layout())
        } else {
            None
        }
    }

    /// Reorder: move tab from `from` to `to` index.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to > self.tabs.len() || from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        let insert_at = if to > from { to - 1 } else { to };
        let insert_at = insert_at.min(self.tabs.len());
        self.tabs.insert(insert_at, tab);

        // Update active_tab to follow the moved tab if it was active
        if self.active_tab == from {
            self.active_tab = insert_at;
        } else if from < self.active_tab && self.active_tab <= insert_at {
            self.active_tab -= 1;
        } else if insert_at <= self.active_tab && self.active_tab < from {
            self.active_tab += 1;
        }
    }
}

// ── Actions ──────────────────────────────────────────────────────────────────

/// Actions returned from the document tab bar.
pub enum DocTabAction {
    None,
    /// Activate tab at index — includes preferred layout name.
    Activate(usize, &'static str),
    /// Close tab at index.
    Close(usize),
    /// Reorder tab from index to index.
    Reorder(usize, usize),
    /// Add a new tab of the given kind.
    AddNew(TabKind),
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Render the document tab bar. Returns an action to handle.
pub fn render_document_tabs(
    ctx: &egui::Context,
    tab_state: &DocumentTabState,
    theme: &Theme,
) -> DocTabAction {
    let mut action = DocTabAction::None;

    let bg_color = theme.surfaces.extreme.to_color32();

    egui::TopBottomPanel::top("renzora_document_tabs")
        .exact_height(DOC_TAB_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {
            let panel_rect = ui.available_rect_before_wrap();
            let top_y = panel_rect.min.y;

            let tab_bg = theme.widgets.inactive_bg.to_color32();
            let tab_active_bg = theme.widgets.active_bg.to_color32();
            let tab_hover_bg = theme.widgets.hovered_bg.to_color32();
            let text_color = theme.text.secondary.to_color32();
            let text_active_color = theme.text.primary.to_color32();
            let drop_indicator_color = theme.semantic.accent.to_color32();

            // Bottom border
            let bar_bottom = panel_rect.min.y + DOC_TAB_BAR_HEIGHT;
            ui.painter().line_segment(
                [
                    Pos2::new(panel_rect.min.x, bar_bottom),
                    Pos2::new(panel_rect.max.x, bar_bottom),
                ],
                Stroke::new(1.0, theme.widgets.border.to_color32()),
            );

            // Drag state in egui memory
            let drag_id = egui::Id::new("doc_tab_drag");
            let mut dragging: Option<usize> = ui.memory(|m| m.data.get_temp(drag_id));
            let mut drop_target_idx: Option<usize> = None;

            let mut x_offset = panel_rect.min.x + 8.0;

            // Collect tab rects for drop detection
            let mut tab_rects: Vec<(usize, Rect)> = Vec::new();

            let scene_count = tab_state.tabs.iter().filter(|t| t.kind == TabKind::Scene).count();

            for (order_idx, tab) in tab_state.tabs.iter().enumerate() {
                let is_active = order_idx == tab_state.active_tab;
                let is_being_dragged = dragging == Some(order_idx);
                let accent = tab.kind.accent_color(theme);
                let icon_inactive = tab.kind.inactive_color(theme);

                // Build display text
                let tab_text = if tab.is_modified {
                    format!("{}*", tab.name)
                } else {
                    tab.name.clone()
                };

                // Calculate tab width based on text
                let font_id = egui::FontId::proportional(12.0);
                let base_width = TAB_PADDING * 2.0 + ICON_WIDTH + CLOSE_BTN_WIDTH;
                let max_text_width = MAX_TAB_WIDTH - base_width;

                // Measure actual text width
                let text_width = ui.fonts_mut(|f| {
                    f.layout_no_wrap(tab_text.clone(), font_id.clone(), Color32::WHITE).size().x
                });

                // Truncate text if needed
                let (display_text, display_text_width) = if text_width > max_text_width {
                    let ellipsis_width = ui.fonts_mut(|f| {
                        f.layout_no_wrap("...".to_string(), font_id.clone(), Color32::WHITE).size().x
                    });
                    let available_for_text = max_text_width - ellipsis_width;

                    let mut truncated = String::new();
                    for ch in tab_text.chars() {
                        let test = format!("{}{}", truncated, ch);
                        let test_width = ui.fonts_mut(|f| {
                            f.layout_no_wrap(test.clone(), font_id.clone(), Color32::WHITE).size().x
                        });
                        if test_width > available_for_text {
                            break;
                        }
                        truncated.push(ch);
                    }
                    let final_text = format!("{}...", truncated);
                    let final_width = ui.fonts_mut(|f| {
                        f.layout_no_wrap(final_text.clone(), font_id.clone(), Color32::WHITE).size().x
                    });
                    (final_text, final_width)
                } else {
                    (tab_text, text_width)
                };

                // Calculate final tab width
                let tab_width = (display_text_width + base_width).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);

                let tab_rect = Rect::from_min_size(
                    Pos2::new(x_offset, top_y + TOP_MARGIN + 2.0),
                    Vec2::new(tab_width, TAB_HEIGHT - 2.0),
                );

                tab_rects.push((order_idx, tab_rect));

                // Tab interaction
                let tab_response = ui.allocate_rect(tab_rect, Sense::click_and_drag());
                let is_hovered = tab_response.hovered();

                // Cursor
                if is_being_dragged {
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                } else if is_hovered {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }

                // Start drag
                if tab_response.drag_started() {
                    dragging = Some(order_idx);
                }

                // Tab background
                let bg = if is_being_dragged {
                    Color32::from_rgb(60, 60, 75)
                } else if is_active {
                    tab_active_bg
                } else if is_hovered {
                    tab_hover_bg
                } else {
                    tab_bg
                };

                ui.painter().rect(
                    tab_rect,
                    CornerRadius::ZERO,
                    bg,
                    Stroke::NONE,
                    StrokeKind::Outside,
                );

                // Active indicator line at top
                if is_active && !is_being_dragged {
                    ui.painter().line_segment(
                        [
                            Pos2::new(tab_rect.min.x, tab_rect.min.y),
                            Pos2::new(tab_rect.max.x, tab_rect.min.y),
                        ],
                        Stroke::new(2.0, accent),
                    );
                }

                // Tab icon
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 8.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    tab.kind.icon(),
                    egui::FontId::proportional(12.0),
                    if is_active { accent } else { icon_inactive },
                );

                // Tab text
                let txt_color = if is_active { text_active_color } else { text_color };
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 24.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &display_text,
                    egui::FontId::proportional(12.0),
                    txt_color,
                );

                // Close button
                let close_rect = Rect::from_min_size(
                    Pos2::new(tab_rect.max.x - 20.0, tab_rect.min.y + 6.0),
                    Vec2::new(14.0, 14.0),
                );

                let close_response = ui.allocate_rect(close_rect, Sense::click());
                let close_hovered = close_response.hovered();

                if close_hovered {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }

                // Only show close for scenes if more than one scene exists
                let can_close = match tab.kind {
                    TabKind::Scene => scene_count > 1,
                    _ => true,
                };

                if can_close {
                    let close_color = if close_hovered {
                        theme.semantic.error.to_color32()
                    } else if is_hovered || is_active {
                        theme.text.muted.to_color32()
                    } else {
                        theme.text.disabled.to_color32()
                    };

                    // Draw X
                    let x_center = close_rect.center();
                    let x_size = 4.0;
                    ui.painter().line_segment(
                        [
                            Pos2::new(x_center.x - x_size, x_center.y - x_size),
                            Pos2::new(x_center.x + x_size, x_center.y + x_size),
                        ],
                        Stroke::new(1.5, close_color),
                    );
                    ui.painter().line_segment(
                        [
                            Pos2::new(x_center.x + x_size, x_center.y - x_size),
                            Pos2::new(x_center.x - x_size, x_center.y + x_size),
                        ],
                        Stroke::new(1.5, close_color),
                    );
                }

                // Handle clicks
                if close_response.clicked() && can_close {
                    action = DocTabAction::Close(order_idx);
                } else if tab_response.clicked() && !tab_response.dragged() && !is_active {
                    let layout = tab.kind.preferred_layout();
                    action = DocTabAction::Activate(order_idx, layout);
                }

                x_offset += tab_width + TAB_GAP;
            }

            // + button — flush with the last tab, same height and Y
            let tab_area_y = top_y + TOP_MARGIN + 2.0;
            let tab_area_h = TAB_HEIGHT - 2.0;
            let add_btn_size = Vec2::new(24.0, tab_area_h);
            let add_btn_pos = Pos2::new(x_offset, tab_area_y);
            let add_btn_rect = Rect::from_min_size(add_btn_pos, add_btn_size);

            // Render + button with menu styling
            #[allow(deprecated)]
            ui.allocate_ui_at_rect(add_btn_rect, |ui| {
                ui.style_mut().visuals.widgets.inactive.weak_bg_fill = tab_bg;
                ui.style_mut().visuals.widgets.hovered.weak_bg_fill = tab_hover_bg;
                ui.style_mut().visuals.widgets.active.weak_bg_fill = tab_hover_bg;

                #[allow(deprecated)]
                let _menu_response = egui::menu::menu_button(ui, "+", |ui| {
                    ui.set_min_width(140.0);

                    let doc_types = [
                        TabKind::Scene,
                        TabKind::Blueprint,
                        TabKind::Script,
                        TabKind::Shader,
                    ];

                    for kind in &doc_types {
                        let label = format!("{} {}", kind.icon(), kind.label());
                        if ui.button(label).clicked() {
                            action = DocTabAction::AddNew(kind.clone());
                            #[allow(deprecated)]
                            ui.close_menu();
                        }
                    }
                });
            });

            // Handle drag-drop for reordering
            if dragging.is_some() {
                let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                if let Some(pos) = pointer_pos {
                    // Find drop target
                    drop_target_idx = None;
                    for (order_idx, rect) in &tab_rects {
                        if pos.x < rect.center().x {
                            drop_target_idx = Some(*order_idx);
                            break;
                        }
                    }
                    // If past all tabs, drop at end
                    if drop_target_idx.is_none() && !tab_rects.is_empty() {
                        drop_target_idx = Some(tab_rects.len());
                    }

                    // Draw drop indicator
                    if let (Some(from_idx), Some(drop_idx)) = (dragging, drop_target_idx) {
                        if from_idx != drop_idx && from_idx + 1 != drop_idx {
                            let indicator_x = if drop_idx < tab_rects.len() {
                                tab_rects[drop_idx].1.min.x - 1.0
                            } else if let Some((_, last_rect)) = tab_rects.last() {
                                last_rect.max.x + 1.0
                            } else {
                                panel_rect.min.x + 8.0
                            };

                            ui.painter().line_segment(
                                [
                                    Pos2::new(indicator_x, top_y + TOP_MARGIN + 4.0),
                                    Pos2::new(indicator_x, top_y + TOP_MARGIN + TAB_HEIGHT - 4.0),
                                ],
                                Stroke::new(2.0, drop_indicator_color),
                            );
                        }
                    }
                }

                // End drag on release
                if ui.input(|i| i.pointer.any_released()) {
                    if let (Some(from_idx), Some(to_idx)) = (dragging, drop_target_idx) {
                        if from_idx != to_idx && from_idx + 1 != to_idx {
                            action = DocTabAction::Reorder(from_idx, to_idx);
                        }
                    }
                    dragging = None;
                }
            }

            // Store drag state back to memory
            ui.memory_mut(|m| {
                if let Some(idx) = dragging {
                    m.data.insert_temp(drag_id, idx);
                } else {
                    m.data.remove::<usize>(drag_id);
                }
            });
        });

    action
}
