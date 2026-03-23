use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Pos2, Stroke, StrokeKind, Vec2};

use crate::blueprint::BlueprintEditorState;
use crate::core::{DockingState, SceneManagerState, SceneTab, TabKind};
use renzora_theme::Theme;

use egui_phosphor::regular::{
    FILM_SCRIPT, SCROLL, CUBE, TREE_STRUCTURE, CODE, PALETTE, IMAGE,
    VIDEO, MUSIC_NOTES, SPARKLE, PAINT_BRUSH, GAME_CONTROLLER, MOUNTAINS,
};

const TAB_HEIGHT: f32 = 28.0;
const TAB_PADDING: f32 = 8.0;
const TAB_GAP: f32 = 2.0;
const TOP_MARGIN: f32 = 4.0;
const MAX_TAB_WIDTH: f32 = 240.0;
const MIN_TAB_WIDTH: f32 = 60.0;
const ICON_WIDTH: f32 = 18.0;
const CLOSE_BTN_WIDTH: f32 = 20.0;

/// Types of documents that can be created
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewDocumentType {
    Scene,
    Blueprint,
    Script,
    Material,
    Shader,
}

impl NewDocumentType {
    pub fn label(&self) -> &'static str {
        match self {
            NewDocumentType::Scene => "Scene",
            NewDocumentType::Blueprint => "Blueprint",
            NewDocumentType::Script => "Script",
            NewDocumentType::Material => "Material",
            NewDocumentType::Shader => "Shader",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            NewDocumentType::Scene => FILM_SCRIPT,
            NewDocumentType::Blueprint => TREE_STRUCTURE,
            NewDocumentType::Script => SCROLL,
            NewDocumentType::Material => CUBE,
            NewDocumentType::Shader => CODE,
        }
    }

    /// Layout to switch to when this document type is activated
    #[allow(dead_code)]
    pub fn layout(&self) -> &'static str {
        match self {
            NewDocumentType::Scene => "Scene",
            NewDocumentType::Blueprint => "Blueprints",
            NewDocumentType::Script => "Scripting",
            NewDocumentType::Material => "Blueprints",
            NewDocumentType::Shader => "Scripting",
        }
    }
}

/// Drag state for tab reordering
#[derive(Clone)]
struct TabDragState {
    dragging: Option<usize>, // Index in tab_order being dragged
    drop_target: Option<usize>, // Index where to drop
}

pub fn render_document_tabs(
    ctx: &egui::Context,
    scene_state: &mut SceneManagerState,
    blueprint_editor: &mut BlueprintEditorState,
    docking_state: &mut DockingState,
    left_panel_width: f32,
    right_panel_width: f32,
    top_y: f32,
    theme: &Theme,
) -> f32 {
    let screen_rect = ctx.content_rect();
    let available_width = screen_rect.width() - left_panel_width - right_panel_width;

    let tab_bar_rect = egui::Rect::from_min_size(
        Pos2::new(left_panel_width, top_y),
        Vec2::new(available_width, TAB_HEIGHT + TOP_MARGIN),
    );

    let bg_color = theme.surfaces.extreme.to_color32();
    let tab_bg = theme.widgets.inactive_bg.to_color32();
    let tab_active_bg = theme.widgets.active_bg.to_color32();
    let tab_hover_bg = theme.widgets.hovered_bg.to_color32();
    let text_color = theme.text.secondary.to_color32();
    let text_active_color = theme.text.primary.to_color32();
    let scene_accent_color = theme.semantic.accent.to_color32();
    let script_accent_color = theme.categories.scripting.accent.to_color32();
    let drop_indicator_color = theme.semantic.accent.to_color32();

    let mut layout_to_switch: Option<&'static str> = None;

    // Ensure tab_order is in sync with actual tabs
    sync_tab_order(scene_state, blueprint_editor);

    // Get drag state from egui memory
    let drag_id = egui::Id::new("document_tab_drag");
    let mut drag_state = ctx.memory(|mem| {
        mem.data.get_temp::<TabDragState>(drag_id).unwrap_or(TabDragState {
            dragging: None,
            drop_target: None,
        })
    });

    egui::Area::new(egui::Id::new("document_tabs_area"))
        .fixed_pos(tab_bar_rect.min)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            // Draw background
            ui.painter().rect_filled(tab_bar_rect, CornerRadius::ZERO, bg_color);

            // Bottom border (dark)
            ui.painter().line_segment(
                [
                    Pos2::new(tab_bar_rect.min.x, tab_bar_rect.max.y),
                    Pos2::new(tab_bar_rect.max.x, tab_bar_rect.max.y),
                ],
                Stroke::new(1.0, theme.widgets.border.to_color32()),
            );

            let mut x_offset = left_panel_width + 8.0;
            let mut tab_to_close: Option<TabKind> = None;
            let mut tab_to_activate: Option<TabKind> = None;

            // Store tab rects for drag-drop detection
            let mut tab_rects: Vec<(usize, egui::Rect)> = Vec::new();

            // Render all tabs in unified order
            for (order_idx, tab_kind) in scene_state.tab_order.iter().enumerate() {
                let blueprint_accent_color = Color32::from_rgb(180, 130, 200);

                // Use the unified active_document check for all tab types
                let is_active = scene_state.is_active_document(tab_kind);

                let (tab_text, icon, accent_color, icon_inactive_color) = match tab_kind {
                    TabKind::Scene(idx) => {
                        let tab = &scene_state.scene_tabs[*idx];
                        let text = if tab.is_modified {
                            format!("{}*", tab.name)
                        } else {
                            tab.name.clone()
                        };
                        // Muted version of scene accent
                        let [r, g, b, _] = scene_accent_color.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 75);
                        (text, FILM_SCRIPT, scene_accent_color, inactive)
                    }
                    TabKind::Script(idx) => {
                        let script = &scene_state.open_scripts[*idx];
                        let text = if script.is_modified {
                            format!("{}*", script.name)
                        } else {
                            script.name.clone()
                        };
                        // Muted version of script accent
                        let [r, g, b, _] = script_accent_color.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, SCROLL, script_accent_color, inactive)
                    }
                    TabKind::Blueprint(path) => {
                        let name = path
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or(path)
                            .trim_end_matches(".material_bp")
                            .trim_end_matches(".blueprint")
                            .to_string();
                        // Muted version of blueprint accent
                        let [r, g, b, _] = blueprint_accent_color.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (name, PALETTE, blueprint_accent_color, inactive)
                    }
                    TabKind::Image(idx) => {
                        let image = &scene_state.open_images[*idx];
                        let text = image.name.clone();
                        let image_accent_color = Color32::from_rgb(166, 217, 140); // Green for images
                        let [r, g, b, _] = image_accent_color.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, IMAGE, image_accent_color, inactive)
                    }
                    TabKind::Video(idx) => {
                        let video = &scene_state.open_videos[*idx];
                        let text = if video.is_modified { format!("{}*", video.name) } else { video.name.clone() };
                        let accent = Color32::from_rgb(220, 80, 80); // Red for video
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, VIDEO, accent, inactive)
                    }
                    TabKind::Audio(idx) => {
                        let audio = &scene_state.open_audios[*idx];
                        let text = if audio.is_modified { format!("{}*", audio.name) } else { audio.name.clone() };
                        let accent = Color32::from_rgb(180, 100, 220); // Purple for audio
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, MUSIC_NOTES, accent, inactive)
                    }
                    TabKind::Animation(idx) => {
                        let anim = &scene_state.open_animations[*idx];
                        let text = if anim.is_modified { format!("{}*", anim.name) } else { anim.name.clone() };
                        let accent = Color32::from_rgb(100, 180, 220); // Light blue for animation
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, FILM_SCRIPT, accent, inactive)
                    }
                    TabKind::Texture(idx) => {
                        let tex = &scene_state.open_textures[*idx];
                        let text = if tex.is_modified { format!("{}*", tex.name) } else { tex.name.clone() };
                        let accent = Color32::from_rgb(120, 200, 120); // Green for textures
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, PAINT_BRUSH, accent, inactive)
                    }
                    TabKind::ParticleFX(idx) => {
                        let particle = &scene_state.open_particles[*idx];
                        let text = if particle.is_modified { format!("{}*", particle.name) } else { particle.name.clone() };
                        let accent = Color32::from_rgb(255, 180, 50); // Orange/gold for particles
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, SPARKLE, accent, inactive)
                    }
                    TabKind::Level(idx) => {
                        let level = &scene_state.open_levels[*idx];
                        let text = if level.is_modified { format!("{}*", level.name) } else { level.name.clone() };
                        let accent = Color32::from_rgb(100, 200, 180); // Teal for levels
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, GAME_CONTROLLER, accent, inactive)
                    }
                    TabKind::Terrain(idx) => {
                        let terrain = &scene_state.open_terrains[*idx];
                        let text = if terrain.is_modified { format!("{}*", terrain.name) } else { terrain.name.clone() };
                        let accent = Color32::from_rgb(140, 180, 100); // Olive for terrain
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, MOUNTAINS, accent, inactive)
                    }
                    TabKind::Shader(idx) => {
                        let script = &scene_state.open_scripts[*idx];
                        let name = script.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or("Shader".to_string());
                        let text = if script.is_modified { format!("{}*", name) } else { name };
                        let accent = Color32::from_rgb(180, 130, 255); // Purple for shaders
                        let [r, g, b, _] = accent.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, egui_phosphor::regular::MONITOR, accent, inactive)
                    }
                };

                // Calculate tab width based on text, with max width and ellipsis
                let font_id = egui::FontId::proportional(12.0);
                let base_width = TAB_PADDING * 2.0 + ICON_WIDTH + CLOSE_BTN_WIDTH; // padding + icon + close button
                let max_text_width = MAX_TAB_WIDTH - base_width;

                // Measure actual text width
                let text_width = ui.fonts_mut(|f| f.layout_no_wrap(tab_text.clone(), font_id.clone(), Color32::WHITE).size().x);

                // Truncate text if needed
                let (display_text, display_text_width) = if text_width > max_text_width {
                    // Need to truncate - binary search for best fit
                    let ellipsis = "...";
                    let ellipsis_width = ui.fonts_mut(|f| f.layout_no_wrap(ellipsis.to_string(), font_id.clone(), Color32::WHITE).size().x);
                    let available_for_text = max_text_width - ellipsis_width;

                    let mut truncated = String::new();
                    for ch in tab_text.chars() {
                        let test = format!("{}{}", truncated, ch);
                        let test_width = ui.fonts_mut(|f| f.layout_no_wrap(test.clone(), font_id.clone(), Color32::WHITE).size().x);
                        if test_width > available_for_text {
                            break;
                        }
                        truncated.push(ch);
                    }
                    let final_text = format!("{}...", truncated);
                    let final_width = ui.fonts_mut(|f| f.layout_no_wrap(final_text.clone(), font_id.clone(), Color32::WHITE).size().x);
                    (final_text, final_width)
                } else {
                    (tab_text.clone(), text_width)
                };

                // Calculate final tab width based on actual displayed text
                let tab_width = (display_text_width + base_width).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);

                let tab_rect = egui::Rect::from_min_size(
                    Pos2::new(x_offset, top_y + TOP_MARGIN + 2.0),
                    Vec2::new(tab_width, TAB_HEIGHT - 2.0),
                );

                tab_rects.push((order_idx, tab_rect));

                // Tab interaction - use drag sense
                let tab_response = ui.allocate_rect(tab_rect, egui::Sense::click_and_drag());
                let is_hovered = tab_response.hovered();
                let is_being_dragged = drag_state.dragging == Some(order_idx);

                // Show pointer cursor on hover (grabbing when dragging)
                if is_being_dragged {
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                } else if is_hovered {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }

                // Start drag
                if tab_response.drag_started() {
                    drag_state.dragging = Some(order_idx);
                }

                // Draw tab background (dimmed if being dragged)
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
                        Stroke::new(2.0, accent_color),
                    );
                }

                // Tab icon
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 8.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    icon,
                    egui::FontId::proportional(12.0),
                    if is_active { accent_color } else { icon_inactive_color },
                );

                // Tab text (with ellipsis if truncated)
                let txt_color = if is_active { text_active_color } else { text_color };
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 24.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &display_text,
                    egui::FontId::proportional(12.0),
                    txt_color,
                );

                // Close button (x)
                let close_rect = egui::Rect::from_min_size(
                    Pos2::new(tab_rect.max.x - 20.0, tab_rect.min.y + 6.0),
                    Vec2::new(14.0, 14.0),
                );

                let close_response = ui.allocate_rect(close_rect, egui::Sense::click());
                let close_hovered = close_response.hovered();

                // Show pointer cursor on close button hover
                if close_hovered {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }

                // Only show close button for scenes if there's more than one tab
                let can_close = match tab_kind {
                    TabKind::Scene(_) => scene_state.scene_tabs.len() > 1,
                    TabKind::Script(_) => true,
                    TabKind::Blueprint(_) => true,
                    TabKind::Image(_) => true,
                    TabKind::Video(_) => true,
                    TabKind::Audio(_) => true,
                    TabKind::Animation(_) => true,
                    TabKind::Texture(_) => true,
                    TabKind::ParticleFX(_) => true,
                    TabKind::Level(_) => true,
                    TabKind::Terrain(_) => true,
                    TabKind::Shader(_) => true,
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
                    tab_to_close = Some(tab_kind.clone());
                } else if tab_response.clicked() && !tab_response.dragged() {
                    tab_to_activate = Some(tab_kind.clone());
                }

                x_offset += tab_width + TAB_GAP;
            }

            // Add document button (+) with dropdown
            let add_btn_pos = Pos2::new(x_offset, top_y + TOP_MARGIN + 4.0);
            let add_btn_size = Vec2::new(24.0, 22.0);

            #[allow(deprecated)]
            let _menu_area = ui.allocate_ui_at_rect(
                egui::Rect::from_min_size(add_btn_pos, add_btn_size),
                |ui| {
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = tab_bg;
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = tab_hover_bg;
                    ui.style_mut().visuals.widgets.active.weak_bg_fill = tab_hover_bg;

                    #[allow(deprecated)]
                    let _menu_response = egui::menu::menu_button(ui, "+", |ui| {
                        ui.set_min_width(140.0);

                        let doc_types = [
                            NewDocumentType::Scene,
                            NewDocumentType::Blueprint,
                            NewDocumentType::Script,
                            NewDocumentType::Material,
                            NewDocumentType::Shader,
                        ];

                        for doc_type in doc_types {
                            let label = format!("{} {}", doc_type.icon(), doc_type.label());
                            if ui.button(label).clicked() {
                                match doc_type {
                                    NewDocumentType::Scene => {
                                        let new_tab_num = scene_state.scene_tabs.len() + 1;
                                        let new_idx = scene_state.scene_tabs.len();
                                        scene_state.scene_tabs.push(SceneTab {
                                            name: format!("Untitled {}", new_tab_num),
                                            ..Default::default()
                                        });
                                        scene_state.tab_order.push(TabKind::Scene(new_idx));
                                        blueprint_editor.active_blueprint = None;
                                        scene_state.set_active_document(TabKind::Scene(new_idx));
                                        layout_to_switch = Some("Scene");
                                    }
                                    NewDocumentType::Blueprint => {
                                        let new_tab_num = scene_state.scene_tabs.len() + 1;
                                        let new_idx = scene_state.scene_tabs.len();
                                        scene_state.scene_tabs.push(SceneTab {
                                            name: format!("Blueprint {}", new_tab_num),
                                            ..Default::default()
                                        });
                                        scene_state.tab_order.push(TabKind::Scene(new_idx));
                                        blueprint_editor.active_blueprint = None;
                                        scene_state.set_active_document(TabKind::Scene(new_idx));
                                        layout_to_switch = Some("Blueprints");
                                    }
                                    NewDocumentType::Script => {
                                        let script_num = scene_state.open_scripts.len() + 1;
                                        let new_idx = scene_state.open_scripts.len();
                                        scene_state.open_scripts.push(crate::core::OpenScript {
                                            name: format!("Script {}", script_num),
                                            path: std::path::PathBuf::new(),
                                            content: String::new(),
                                            is_modified: false,
                                            error: None,
                                            last_checked_content: String::new(),
                                        });
                                        scene_state.tab_order.push(TabKind::Script(new_idx));
                                        blueprint_editor.active_blueprint = None;
                                        scene_state.set_active_document(TabKind::Script(new_idx));
                                        layout_to_switch = Some("Scripting");
                                    }
                                    NewDocumentType::Material => {
                                        layout_to_switch = Some("Blueprints");
                                    }
                                    NewDocumentType::Shader => {
                                        layout_to_switch = Some("Scripting");
                                    }
                                }
                                #[allow(deprecated)]
                                ui.close_menu();
                            }
                        }
                    });
                },
            );

            // Handle drag-drop for reordering
            if drag_state.dragging.is_some() {
                let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                if let Some(pos) = pointer_pos {
                    // Find drop target
                    drag_state.drop_target = None;
                    for (order_idx, rect) in &tab_rects {
                        if pos.x < rect.center().x {
                            drag_state.drop_target = Some(*order_idx);
                            break;
                        }
                    }
                    // If past all tabs, drop at end
                    if drag_state.drop_target.is_none() && !tab_rects.is_empty() {
                        drag_state.drop_target = Some(tab_rects.len());
                    }

                    // Draw drop indicator
                    if let Some(drop_idx) = drag_state.drop_target {
                        let indicator_x = if drop_idx < tab_rects.len() {
                            tab_rects[drop_idx].1.min.x - 1.0
                        } else if let Some((_, last_rect)) = tab_rects.last() {
                            last_rect.max.x + 1.0
                        } else {
                            left_panel_width + 8.0
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

                // End drag on release
                if ui.input(|i| i.pointer.any_released()) {
                    if let (Some(from_idx), Some(to_idx)) = (drag_state.dragging, drag_state.drop_target) {
                        if from_idx != to_idx && from_idx + 1 != to_idx {
                            // Perform the reorder
                            let tab = scene_state.tab_order.remove(from_idx);
                            let insert_idx = if to_idx > from_idx { to_idx - 1 } else { to_idx };
                            scene_state.tab_order.insert(insert_idx, tab);
                        }
                    }
                    drag_state.dragging = None;
                    drag_state.drop_target = None;
                }
            }

            // Process tab actions
            if let Some(tab_kind) = tab_to_close {
                close_tab(scene_state, blueprint_editor, tab_kind, &mut layout_to_switch);
            }

            if let Some(tab_kind) = tab_to_activate {
                activate_tab(scene_state, blueprint_editor, tab_kind, &mut layout_to_switch);
            }
        });

    // Store drag state back to memory
    ctx.memory_mut(|mem| {
        mem.data.insert_temp(drag_id, drag_state);
    });

    // Switch layout if requested
    if let Some(layout_name) = layout_to_switch {
        docking_state.switch_layout(layout_name);
    }

    TAB_HEIGHT + TOP_MARGIN
}

/// Ensure tab_order is in sync with actual tabs (handles external additions/removals)
fn sync_tab_order(scene_state: &mut SceneManagerState, blueprint_editor: &BlueprintEditorState) {
    // Remove invalid entries
    scene_state.tab_order.retain(|kind| match kind {
        TabKind::Scene(idx) => *idx < scene_state.scene_tabs.len(),
        TabKind::Script(idx) => *idx < scene_state.open_scripts.len(),
        TabKind::Blueprint(path) => blueprint_editor.open_blueprints.contains_key(path),
        TabKind::Image(idx) => *idx < scene_state.open_images.len(),
        TabKind::Video(idx) => *idx < scene_state.open_videos.len(),
        TabKind::Audio(idx) => *idx < scene_state.open_audios.len(),
        TabKind::Animation(idx) => *idx < scene_state.open_animations.len(),
        TabKind::Texture(idx) => *idx < scene_state.open_textures.len(),
        TabKind::ParticleFX(idx) => *idx < scene_state.open_particles.len(),
        TabKind::Level(idx) => *idx < scene_state.open_levels.len(),
        TabKind::Terrain(idx) => *idx < scene_state.open_terrains.len(),
        TabKind::Shader(idx) => *idx < scene_state.open_scripts.len(),
    });

    // Add any missing scene tabs
    for idx in 0..scene_state.scene_tabs.len() {
        if !scene_state.tab_order.contains(&TabKind::Scene(idx)) {
            scene_state.tab_order.push(TabKind::Scene(idx));
        }
    }

    // Add any missing script tabs
    for idx in 0..scene_state.open_scripts.len() {
        if !scene_state.tab_order.contains(&TabKind::Script(idx)) {
            scene_state.tab_order.push(TabKind::Script(idx));
        }
    }

    // Add any missing blueprint tabs
    for path in blueprint_editor.open_blueprints.keys() {
        let tab_kind = TabKind::Blueprint(path.clone());
        if !scene_state.tab_order.contains(&tab_kind) {
            scene_state.tab_order.push(tab_kind);
        }
    }

    // Add any missing image tabs
    for idx in 0..scene_state.open_images.len() {
        if !scene_state.tab_order.contains(&TabKind::Image(idx)) {
            scene_state.tab_order.push(TabKind::Image(idx));
        }
    }

    // Add any missing video tabs
    for idx in 0..scene_state.open_videos.len() {
        if !scene_state.tab_order.contains(&TabKind::Video(idx)) {
            scene_state.tab_order.push(TabKind::Video(idx));
        }
    }

    // Add any missing audio tabs
    for idx in 0..scene_state.open_audios.len() {
        if !scene_state.tab_order.contains(&TabKind::Audio(idx)) {
            scene_state.tab_order.push(TabKind::Audio(idx));
        }
    }

    // Add any missing animation tabs
    for idx in 0..scene_state.open_animations.len() {
        if !scene_state.tab_order.contains(&TabKind::Animation(idx)) {
            scene_state.tab_order.push(TabKind::Animation(idx));
        }
    }

    // Add any missing texture tabs
    for idx in 0..scene_state.open_textures.len() {
        if !scene_state.tab_order.contains(&TabKind::Texture(idx)) {
            scene_state.tab_order.push(TabKind::Texture(idx));
        }
    }

    // Add any missing particle tabs
    for idx in 0..scene_state.open_particles.len() {
        if !scene_state.tab_order.contains(&TabKind::ParticleFX(idx)) {
            scene_state.tab_order.push(TabKind::ParticleFX(idx));
        }
    }

    // Add any missing level tabs
    for idx in 0..scene_state.open_levels.len() {
        if !scene_state.tab_order.contains(&TabKind::Level(idx)) {
            scene_state.tab_order.push(TabKind::Level(idx));
        }
    }

    // Add any missing terrain tabs
    for idx in 0..scene_state.open_terrains.len() {
        if !scene_state.tab_order.contains(&TabKind::Terrain(idx)) {
            scene_state.tab_order.push(TabKind::Terrain(idx));
        }
    }
}

fn close_tab(scene_state: &mut SceneManagerState, blueprint_editor: &mut BlueprintEditorState, tab_kind: TabKind, layout_to_switch: &mut Option<&'static str>) {
    // Find position in tab_order
    let order_pos = scene_state.tab_order.iter().position(|k| *k == tab_kind);

    // Check if the tab being closed is currently active and whether it's a scene tab
    let is_scene_tab = matches!(tab_kind, TabKind::Scene(_));
    let is_active_tab = scene_state.is_active_document(&tab_kind);

    match tab_kind {
        TabKind::Scene(idx) => {
            if scene_state.scene_tabs.len() > 1 {
                // Remove from tab_order first
                if let Some(pos) = order_pos {
                    scene_state.tab_order.remove(pos);
                }

                // Update tab_order indices for remaining scene tabs
                for kind in &mut scene_state.tab_order {
                    if let TabKind::Scene(scene_idx) = kind {
                        if *scene_idx > idx {
                            *scene_idx -= 1;
                        }
                    }
                }

                // Set pending close and switch
                if is_active_tab {
                    let new_active = if idx + 1 < scene_state.scene_tabs.len() {
                        idx
                    } else {
                        idx.saturating_sub(1)
                    };
                    scene_state.pending_tab_switch = Some(new_active);
                } else if idx < scene_state.active_scene_tab {
                    scene_state.pending_tab_switch = Some(scene_state.active_scene_tab - 1);
                }
                scene_state.pending_tab_close = Some(idx);
            }
        }
        TabKind::Script(idx) => {
            // Remove from tab_order first
            if let Some(pos) = order_pos {
                scene_state.tab_order.remove(pos);
            }

            // Update tab_order indices for remaining script tabs
            for kind in &mut scene_state.tab_order {
                if let TabKind::Script(script_idx) = kind {
                    if *script_idx > idx {
                        *script_idx -= 1;
                    }
                }
            }

            // Remove the script
            scene_state.open_scripts.remove(idx);

            // Update active script tab index if needed (but don't clear if active - activate_tab handles that)
            if let Some(active) = scene_state.active_script_tab {
                if active > idx {
                    scene_state.active_script_tab = Some(active - 1);
                }
            }
        }
        TabKind::Blueprint(path) => {
            // Remove from tab_order first
            if let Some(pos) = order_pos {
                scene_state.tab_order.remove(pos);
            }

            // Remove the blueprint
            blueprint_editor.open_blueprints.remove(&path);
        }
        TabKind::Image(idx) => {
            // Remove from tab_order first
            if let Some(pos) = order_pos {
                scene_state.tab_order.remove(pos);
            }

            // Update tab_order indices for remaining image tabs
            for kind in &mut scene_state.tab_order {
                if let TabKind::Image(image_idx) = kind {
                    if *image_idx > idx {
                        *image_idx -= 1;
                    }
                }
            }

            // Remove the image
            scene_state.open_images.remove(idx);

            // Update active image tab index if needed (but don't clear if active - activate_tab handles that)
            if let Some(active) = scene_state.active_image_tab {
                if active > idx {
                    scene_state.active_image_tab = Some(active - 1);
                }
            }
        }
        TabKind::Video(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::Video(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_videos.remove(idx);
            if let Some(active) = scene_state.active_video_tab {
                if active > idx { scene_state.active_video_tab = Some(active - 1); }
            }
        }
        TabKind::Audio(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::Audio(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_audios.remove(idx);
            if let Some(active) = scene_state.active_audio_tab {
                if active > idx { scene_state.active_audio_tab = Some(active - 1); }
            }
        }
        TabKind::Animation(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::Animation(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_animations.remove(idx);
            if let Some(active) = scene_state.active_animation_tab {
                if active > idx { scene_state.active_animation_tab = Some(active - 1); }
            }
        }
        TabKind::Texture(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::Texture(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_textures.remove(idx);
            if let Some(active) = scene_state.active_texture_tab {
                if active > idx { scene_state.active_texture_tab = Some(active - 1); }
            }
        }
        TabKind::ParticleFX(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::ParticleFX(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_particles.remove(idx);
            if let Some(active) = scene_state.active_particle_tab {
                if active > idx { scene_state.active_particle_tab = Some(active - 1); }
            }
        }
        TabKind::Level(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::Level(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_levels.remove(idx);
            if let Some(active) = scene_state.active_level_tab {
                if active > idx { scene_state.active_level_tab = Some(active - 1); }
            }
        }
        TabKind::Terrain(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::Terrain(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_terrains.remove(idx);
            if let Some(active) = scene_state.active_terrain_tab {
                if active > idx { scene_state.active_terrain_tab = Some(active - 1); }
            }
        }
        TabKind::Shader(idx) => {
            if let Some(pos) = order_pos { scene_state.tab_order.remove(pos); }
            for kind in &mut scene_state.tab_order {
                if let TabKind::Shader(i) = kind { if *i > idx { *i -= 1; } }
            }
            scene_state.open_scripts.remove(idx);
            if let Some(active) = scene_state.active_script_tab {
                if active > idx { scene_state.active_script_tab = Some(active - 1); }
            }
        }
    }

    // If the closed tab was active, switch to the next tab in tab_order
    if is_active_tab && !is_scene_tab {
        if let Some(order_pos) = order_pos {
            // Find the next tab to activate (prefer same position, then previous)
            let next_tab = if order_pos < scene_state.tab_order.len() {
                Some(scene_state.tab_order[order_pos].clone())
            } else if !scene_state.tab_order.is_empty() {
                Some(scene_state.tab_order[order_pos.saturating_sub(1)].clone())
            } else {
                None
            };

            if let Some(next_tab) = next_tab {
                activate_tab(scene_state, blueprint_editor, next_tab, layout_to_switch);
            } else {
                // No tabs left to switch to - clear all active states and go to default scene
                scene_state.active_script_tab = None;
                scene_state.active_image_tab = None;
                blueprint_editor.active_blueprint = None;
                *layout_to_switch = Some("Scene");
            }
        }
    }
}

fn activate_tab(scene_state: &mut SceneManagerState, blueprint_editor: &mut BlueprintEditorState, tab_kind: TabKind, layout_to_switch: &mut Option<&'static str>) {
    // Get the layout name before we move tab_kind
    let layout_name = tab_kind.layout_name();

    // Handle blueprint active state separately since it's in a different struct
    if let TabKind::Blueprint(ref path) = tab_kind {
        blueprint_editor.active_blueprint = Some(path.clone());
    } else {
        blueprint_editor.active_blueprint = None;
    }

    // Use the unified set_active_document method
    scene_state.set_active_document(tab_kind);

    // Set the layout to switch to
    *layout_to_switch = Some(layout_name);
}
