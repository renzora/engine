//! Document tab bar — renders between title bar and dock tree.
//!
//! Each tab represents an open scene. The "+" button creates a new empty scene tab.
//! Workspace layouts are switched independently via the layout/workspace system.

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

// ── Document tab ─────────────────────────────────────────────────────────────

/// What type of asset a document tab represents. The layout that should be
/// active when the tab is focused, and the icon used in its tab header, both
/// follow from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocTabKind {
    #[default]
    Scene,
    Material,
    Particle,
    Blueprint,
    Script,
    Shader,
    Other,
}

impl DocTabKind {
    /// True if this tab represents a single asset file (vs a scene). Asset
    /// tabs put the editor into Asset mode where panels load directly from
    /// the file path and entity selection is irrelevant.
    pub fn is_asset(self) -> bool {
        !matches!(self, DocTabKind::Scene | DocTabKind::Other)
    }

    /// Named workspace layout that should activate when this tab is focused
    /// in **Scene mode** (i.e. only meaningful for `Scene`). `None` means
    /// keep whatever layout is currently active.
    pub fn layout_name(self) -> Option<&'static str> {
        match self {
            DocTabKind::Scene => Some("Scene"),
            DocTabKind::Material => Some("Materials"),
            DocTabKind::Particle => Some("Particles"),
            DocTabKind::Blueprint => Some("Blueprints"),
            DocTabKind::Script => Some("Scripting"),
            DocTabKind::Shader => Some("Shaders"),
            DocTabKind::Other => None,
        }
    }

    /// Hidden workspace layout used for **Asset mode** — when a single asset
    /// file is open. These layouts drop the hierarchy/outline panels so the
    /// editor doesn't nag for an entity selection that doesn't apply.
    pub fn asset_layout_name(self) -> Option<&'static str> {
        match self {
            DocTabKind::Material => Some("Materials-Asset"),
            DocTabKind::Script | DocTabKind::Shader => Some("Scripting-Asset"),
            DocTabKind::Blueprint => Some("Blueprints-Asset"),
            DocTabKind::Particle => Some("Particles-Asset"),
            DocTabKind::Scene | DocTabKind::Other => None,
        }
    }

    /// Phosphor icon glyph for this tab kind.
    pub fn icon(self) -> &'static str {
        match self {
            DocTabKind::Scene => regular::FILM_SCRIPT,
            DocTabKind::Material => regular::PALETTE,
            DocTabKind::Particle => regular::SPARKLE,
            DocTabKind::Blueprint => regular::BLUEPRINT,
            DocTabKind::Script => regular::CODE,
            DocTabKind::Shader => regular::GRAPHICS_CARD,
            DocTabKind::Other => regular::FILE,
        }
    }
}

/// A single open document tab. Historically these were always scenes; they
/// now also cover other asset types (materials, particles, blueprints,
/// scripts, shaders) opened via double-click in the asset browser. The
/// `scene_path` field stores the file path regardless of kind — kept under
/// the legacy name so existing scene-tab call sites continue to compile.
#[derive(Debug, Clone)]
pub struct DocumentTab {
    /// Unique id for this tab instance.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Path to the file on disk, project-relative (None for unsaved tabs).
    pub scene_path: Option<String>,
    /// What kind of asset this tab represents.
    pub kind: DocTabKind,
    /// Whether the document has unsaved changes.
    pub is_modified: bool,
}

// ── Editor context ──────────────────────────────────────────────────────────

/// What the editor is currently focused on. Derived from the active document
/// tab — Scene mode means panels follow `EditorSelection`; Asset mode means
/// panels load directly from the asset file path and ignore entity selection.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum EditorContext {
    #[default]
    Scene,
    Asset {
        /// Project-relative path to the file being edited.
        path: String,
        /// What kind of asset this is.
        kind: DocTabKind,
    },
}

impl EditorContext {
    pub fn is_scene(&self) -> bool {
        matches!(self, Self::Scene)
    }

    pub fn is_asset(&self) -> bool {
        matches!(self, Self::Asset { .. })
    }

    pub fn asset_path(&self) -> Option<&str> {
        if let Self::Asset { path, .. } = self {
            Some(path.as_str())
        } else {
            None
        }
    }

    pub fn asset_kind(&self) -> Option<DocTabKind> {
        if let Self::Asset { kind, .. } = self {
            Some(*kind)
        } else {
            None
        }
    }

    /// Build a context from a document tab.
    pub fn from_tab(tab: &DocumentTab) -> Self {
        match (tab.kind, tab.scene_path.as_ref()) {
            (DocTabKind::Scene, _) | (DocTabKind::Other, _) => Self::Scene,
            (kind, Some(path)) => Self::Asset {
                path: path.clone(),
                kind,
            },
            // Asset-kind tab with no path (unsaved): treat as scene-mode so
            // panels don't try to load `None`.
            (_, None) => Self::Scene,
        }
    }
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
    /// Most-recently-used stack of **scene** tab IDs. Top of stack is the
    /// scene to return to when leaving an asset tab via the title-bar layout
    /// switcher. IDs of closed tabs stay until pruned by `find_mru_scene_tab`.
    scene_mru: Vec<u64>,
}

impl Default for DocumentTabState {
    fn default() -> Self {
        let mut state = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_id: 1,
            scene_mru: Vec::new(),
        };
        // Start with one default scene tab
        state.add_tab("Untitled Scene".into(), None);
        // Seed MRU with the initial scene tab.
        if let Some(id) = state.tabs.first().map(|t| t.id) {
            state.scene_mru.push(id);
        }
        state
    }
}

impl DocumentTabState {
    /// Add a new scene tab and return its index.
    pub fn add_tab(&mut self, name: String, scene_path: Option<String>) -> usize {
        self.add_tab_of_kind(name, scene_path, DocTabKind::Scene)
    }

    /// Add a tab of the given kind and return its index.
    pub fn add_tab_of_kind(&mut self, name: String, path: Option<String>, kind: DocTabKind) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(DocumentTab {
            id,
            name,
            scene_path: path,
            kind,
            is_modified: false,
        });
        self.tabs.len() - 1
    }

    /// Find an existing tab for the given project-relative path and kind.
    pub fn find_by_path(&self, path: &str, kind: DocTabKind) -> Option<usize> {
        self.tabs.iter().position(|t| {
            t.kind == kind && t.scene_path.as_deref() == Some(path)
        })
    }

    /// Close a tab by index. Returns the closed tab's id for buffer cleanup,
    /// or `None` if the close was denied (last tab overall, or last remaining
    /// scene tab — at least one scene must always be open).
    pub fn close_tab(&mut self, index: usize) -> Option<u64> {
        if index >= self.tabs.len() {
            return None;
        }
        // Don't close the last tab
        if self.tabs.len() <= 1 {
            return None;
        }
        // Don't close the last scene tab — Asset mode requires a scene to
        // return to when the user leaves Asset mode via the title bar.
        if self.tabs[index].kind == DocTabKind::Scene {
            let scene_count = self.tabs.iter().filter(|t| t.kind == DocTabKind::Scene).count();
            if scene_count <= 1 {
                return None;
            }
        }

        let closed_id = self.tabs[index].id;
        self.tabs.remove(index);
        // Drop the closed tab from the MRU.
        self.scene_mru.retain(|id| *id != closed_id);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > index {
            self.active_tab -= 1;
        }
        Some(closed_id)
    }

    /// Activate a tab by index. Returns (old_id, new_id) if the tab changed.
    /// Side effect: if the activated tab is a scene, its id is pushed onto
    /// the MRU stack so the title-bar layout switcher knows where to return.
    pub fn activate_tab(&mut self, index: usize) -> Option<(u64, u64)> {
        if index < self.tabs.len() && index != self.active_tab {
            let old_id = self.tabs[self.active_tab].id;
            self.active_tab = index;
            let new_id = self.tabs[index].id;
            self.touch_scene_mru(index);
            Some((old_id, new_id))
        } else {
            None
        }
    }

    /// Push a scene tab to the top of the MRU stack. No-op for non-scene tabs.
    pub fn touch_scene_mru(&mut self, index: usize) {
        let Some(tab) = self.tabs.get(index) else { return };
        if tab.kind != DocTabKind::Scene {
            return;
        }
        let id = tab.id;
        self.scene_mru.retain(|other| *other != id);
        self.scene_mru.push(id);
    }

    /// Find the most-recently-used scene tab still open. Walks the MRU
    /// top-down, pruning ids of closed tabs along the way. Falls back to the
    /// first scene tab in display order if MRU is empty.
    pub fn find_mru_scene_tab(&mut self) -> Option<usize> {
        while let Some(&id) = self.scene_mru.last() {
            if let Some(idx) = self.tabs.iter().position(|t| t.id == id && t.kind == DocTabKind::Scene) {
                return Some(idx);
            }
            self.scene_mru.pop();
        }
        // Fallback: the first scene tab in display order.
        self.tabs.iter().position(|t| t.kind == DocTabKind::Scene)
    }

    /// Get the active tab as a reference.
    pub fn active_tab(&self) -> Option<&DocumentTab> {
        self.tabs.get(self.active_tab)
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

    /// Get the active tab's id.
    pub fn active_tab_id(&self) -> Option<u64> {
        self.tabs.get(self.active_tab).map(|t| t.id)
    }
}

// ── Actions ──────────────────────────────────────────────────────────────────

/// Actions returned from the document tab bar.
pub enum DocTabAction {
    None,
    /// Activate tab at index.
    Activate(usize),
    /// Close tab at index.
    Close(usize),
    /// Reorder tab from index to index.
    Reorder(usize, usize),
    /// Add a new scene tab.
    AddNew,
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
    let accent = theme.semantic.accent.to_color32();

    egui::TopBottomPanel::top("renzora_document_tabs")
        .exact_height(DOC_TAB_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {
            let panel_rect = ui.available_rect_before_wrap();
            let top_y = panel_rect.min.y;

            let tab_bg = theme.widgets.inactive_bg.to_color32();
            let tab_active_bg = Color32::from_rgb(
                (tab_bg.r() as f32 * 0.7) as u8,
                (tab_bg.g() as f32 * 0.7) as u8,
                (tab_bg.b() as f32 * 0.7) as u8,
            );
            let tab_hover_bg = theme.widgets.hovered_bg.to_color32();
            let text_color = theme.text.secondary.to_color32();
            let text_active_color = theme.text.primary.to_color32();
            let drop_indicator_color = accent;

            // Inactive icon color (muted accent)
            let icon_inactive = {
                let c = accent;
                Color32::from_rgb(c.r() / 2 + 50, c.g() / 2 + 50, c.b() / 2 + 55)
            };

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

            // ── Pre-compute display text + width for every tab ───────────
            let font_id = egui::FontId::proportional(12.0);
            let base_width = TAB_PADDING * 2.0 + ICON_WIDTH + CLOSE_BTN_WIDTH;
            let max_text_width = MAX_TAB_WIDTH - base_width;
            let layouts: Vec<(String, f32)> = tab_state
                .tabs
                .iter()
                .map(|tab| {
                    let tab_text = if tab.is_modified {
                        format!("{}*", tab.name)
                    } else {
                        tab.name.clone()
                    };
                    let text_width = ui.fonts_mut(|f| {
                        f.layout_no_wrap(tab_text.clone(), font_id.clone(), Color32::WHITE).size().x
                    });
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
                    let tab_width = (display_text_width + base_width).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
                    (display_text, tab_width)
                })
                .collect();

            // ── Decide visible vs overflow ───────────────────────────────
            // The "+" button is always visible, pinned to the right. If the
            // remaining tabs don't all fit, we also reserve room for an
            // overflow chevron and route the rest into a dropdown.
            let bar_left = panel_rect.min.x + 8.0;
            let bar_right = panel_rect.max.x - 8.0;
            let add_btn_w = 24.0;
            let overflow_btn_w = 28.0;
            let right_inner_gap = 4.0;

            let fit_count = |max_x: f32| -> usize {
                let mut x = bar_left;
                let mut count = 0usize;
                for (i, (_, w)) in layouts.iter().enumerate() {
                    let next_x = x + w + if i > 0 { TAB_GAP } else { 0.0 };
                    if next_x <= max_x {
                        x = next_x;
                        count += 1;
                    } else {
                        break;
                    }
                }
                count
            };

            let max_x_no_overflow = bar_right - add_btn_w - right_inner_gap;
            let visible_count_no_overflow = fit_count(max_x_no_overflow);
            let needs_overflow = visible_count_no_overflow < layouts.len();
            let visible_count = if needs_overflow {
                fit_count(max_x_no_overflow - overflow_btn_w - right_inner_gap)
            } else {
                visible_count_no_overflow
            };

            let mut x_offset = bar_left;

            // Collect visible tab rects for drop detection
            let mut tab_rects: Vec<(usize, Rect)> = Vec::new();

            for order_idx in 0..visible_count {
                let tab = &tab_state.tabs[order_idx];
                let display_text = layouts[order_idx].0.clone();
                let tab_width = layouts[order_idx].1;
                let is_active = order_idx == tab_state.active_tab;
                let is_being_dragged = dragging == Some(order_idx);

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

                // Tab icon — based on asset kind
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

                // Only show close if more than one tab
                let can_close = tab_state.tabs.len() > 1;

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
                    action = DocTabAction::Activate(order_idx);
                }

                x_offset += tab_width + TAB_GAP;
            }

            // ── Right-side controls ──────────────────────────────────────
            // Both controls flow right after the last visible tab, with the
            // chevron (only shown when tabs overflow) coming first and the
            // "+" button last so it visually closes the strip.
            let tab_area_y = top_y + TOP_MARGIN + 2.0;
            let tab_area_h = TAB_HEIGHT - 2.0;

            // Overflow chevron + popup, only when some tabs were hidden.
            // Painted directly (not via `ui.button`) so its content can't
            // overflow its rect and push the tab bar's height around.
            if needs_overflow {
                let chevron_x = x_offset;
                let chevron_rect = Rect::from_min_size(
                    Pos2::new(chevron_x, tab_area_y),
                    Vec2::new(overflow_btn_w, tab_area_h),
                );
                let active_in_overflow = tab_state.active_tab >= visible_count;
                let hidden_count = layouts.len() - visible_count;
                let popup_id = egui::Id::new("renzora_doc_tab_overflow");

                let chevron_response = ui.allocate_rect(chevron_rect, Sense::click());
                let is_hovered = chevron_response.hovered();
                if is_hovered {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }

                let bg = if is_hovered { tab_hover_bg } else { tab_bg };
                ui.painter().rect(
                    chevron_rect,
                    CornerRadius::ZERO,
                    bg,
                    Stroke::NONE,
                    StrokeKind::Outside,
                );

                let _ = active_in_overflow; // visually flagged via popup highlight, not chevron color
                let label = format!("{} {}", regular::CARET_DOWN, hidden_count);
                ui.painter().text(
                    chevron_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &label,
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );

                if chevron_response.clicked() {
                    #[allow(deprecated)]
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }

                #[allow(deprecated)]
                egui::popup_below_widget(
                    ui,
                    popup_id,
                    &chevron_response,
                    egui::PopupCloseBehavior::CloseOnClick,
                    |ui| {
                        ui.set_min_width(200.0);

                        for hidden_idx in visible_count..tab_state.tabs.len() {
                            let tab = &tab_state.tabs[hidden_idx];
                            let is_active = hidden_idx == tab_state.active_tab;
                            let label_text = if tab.is_modified {
                                format!("{}  {}*", tab.kind.icon(), tab.name)
                            } else {
                                format!("{}  {}", tab.kind.icon(), tab.name)
                            };
                            let label_color = if is_active { accent } else { text_color };

                            // Manual row paint so text is left-aligned (egui
                            // Buttons center-align by default and there's no
                            // built-in flag to change that).
                            let row_h = 22.0;
                            let (row_rect, row_response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), row_h),
                                Sense::click(),
                            );
                            let row_hovered = row_response.hovered();

                            // Inset close (X) on the right of the row, sized
                            // and styled like the tab-bar close button.
                            let can_close = tab_state.tabs.len() > 1
                                && !(tab.kind == DocTabKind::Scene
                                    && tab_state
                                        .tabs
                                        .iter()
                                        .filter(|t| t.kind == DocTabKind::Scene)
                                        .count()
                                        <= 1);
                            let close_rect = Rect::from_min_size(
                                Pos2::new(row_rect.max.x - 22.0, row_rect.min.y + 4.0),
                                Vec2::new(14.0, 14.0),
                            );
                            let close_response = ui.allocate_rect(close_rect, Sense::click());
                            let close_hovered = close_response.hovered();

                            if close_hovered || row_hovered {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }

                            let bg = if row_hovered {
                                tab_hover_bg
                            } else if is_active {
                                tab_active_bg
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(row_rect, CornerRadius::ZERO, bg);

                            ui.painter().text(
                                egui::pos2(row_rect.min.x + 8.0, row_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &label_text,
                                egui::FontId::proportional(12.0),
                                label_color,
                            );

                            if can_close {
                                let close_color = if close_hovered {
                                    theme.semantic.error.to_color32()
                                } else if row_hovered || is_active {
                                    theme.text.muted.to_color32()
                                } else {
                                    theme.text.disabled.to_color32()
                                };
                                let cx = close_rect.center();
                                let s = 4.0;
                                ui.painter().line_segment(
                                    [Pos2::new(cx.x - s, cx.y - s), Pos2::new(cx.x + s, cx.y + s)],
                                    Stroke::new(1.5, close_color),
                                );
                                ui.painter().line_segment(
                                    [Pos2::new(cx.x + s, cx.y - s), Pos2::new(cx.x - s, cx.y + s)],
                                    Stroke::new(1.5, close_color),
                                );
                            }

                            // Close click takes precedence over activate.
                            if can_close && close_response.clicked() {
                                action = DocTabAction::Close(hidden_idx);
                            } else if row_response.clicked() {
                                action = DocTabAction::Activate(hidden_idx);
                            }
                        }
                    },
                );
            }

            // "+" button — sits after the chevron when overflowing, or
            // right after the last visible tab otherwise.
            let add_btn_x = if needs_overflow {
                x_offset + overflow_btn_w + right_inner_gap
            } else {
                x_offset
            };
            let add_btn_rect = Rect::from_min_size(
                Pos2::new(add_btn_x, tab_area_y),
                Vec2::new(add_btn_w, tab_area_h),
            );

            #[allow(deprecated)]
            ui.allocate_ui_at_rect(add_btn_rect, |ui| {
                ui.style_mut().visuals.widgets.inactive.weak_bg_fill = tab_bg;
                ui.style_mut().visuals.widgets.hovered.weak_bg_fill = tab_hover_bg;
                ui.style_mut().visuals.widgets.active.weak_bg_fill = tab_hover_bg;

                if ui.button("+").clicked() {
                    action = DocTabAction::AddNew;
                }
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
