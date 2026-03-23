use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Pos2, RichText, Stroke, StrokeKind, Vec2};

use crate::core::{AppState, WindowState, EditorSettings};
use crate::project::{create_project, open_project, AppConfig, CurrentProject};
use renzora_theme::Theme;

use super::title_bar::{render_splash_title_bar, TITLE_BAR_HEIGHT};

/// Animated wireframe shape for splash background
#[derive(Clone)]
struct WireframeShape {
    pos: Vec2,
    vel: Vec2,
    rotation: f32,
    rot_speed: f32,
    size: f32,
    color: Color32,
    shape_type: ShapeType,
}

#[derive(Clone, Copy)]
enum ShapeType {
    Cube,
    Tetrahedron,
    Octahedron,
    Diamond,
}

impl WireframeShape {
    fn random(screen_size: Vec2, seed: u64) -> Self {
        let hash = |n: u64| -> f32 {
            let x = n.wrapping_mul(0x517cc1b727220a95);
            x as f32 / u64::MAX as f32
        };

        let colors = [
            Color32::from_rgb(100, 180, 255),
            Color32::from_rgb(255, 120, 180),
            Color32::from_rgb(120, 255, 180),
            Color32::from_rgb(255, 200, 100),
            Color32::from_rgb(180, 120, 255),
            Color32::from_rgb(255, 255, 120),
            Color32::from_rgb(120, 255, 255),
        ];

        let shapes = [ShapeType::Cube, ShapeType::Tetrahedron, ShapeType::Octahedron, ShapeType::Diamond];

        Self {
            pos: Vec2::new(
                hash(seed) * screen_size.x,
                hash(seed.wrapping_add(1)) * screen_size.y,
            ),
            vel: Vec2::new(
                (hash(seed.wrapping_add(2)) - 0.5) * 60.0,
                (hash(seed.wrapping_add(3)) - 0.5) * 60.0,
            ),
            rotation: hash(seed.wrapping_add(4)) * std::f32::consts::TAU,
            rot_speed: (hash(seed.wrapping_add(5)) - 0.5) * 2.0,
            size: 20.0 + hash(seed.wrapping_add(6)) * 40.0,
            color: colors[(seed as usize) % colors.len()].gamma_multiply(0.6),
            shape_type: shapes[(seed.wrapping_add(7) as usize) % shapes.len()],
        }
    }

    fn update(&mut self, dt: f32, screen_size: Vec2) {
        self.pos += self.vel * dt;
        self.rotation += self.rot_speed * dt;

        let margin = self.size;
        if self.pos.x < margin {
            self.pos.x = margin;
            self.vel.x = self.vel.x.abs();
        }
        if self.pos.x > screen_size.x - margin {
            self.pos.x = screen_size.x - margin;
            self.vel.x = -self.vel.x.abs();
        }
        if self.pos.y < margin {
            self.pos.y = margin;
            self.vel.y = self.vel.y.abs();
        }
        if self.pos.y > screen_size.y - margin {
            self.pos.y = screen_size.y - margin;
            self.vel.y = -self.vel.y.abs();
        }
    }

    fn draw(&self, painter: &egui::Painter) {
        let stroke = Stroke::new(1.5, self.color);
        let center = Pos2::new(self.pos.x, self.pos.y);

        let rotate = |p: Vec2| -> Pos2 {
            let cos = self.rotation.cos();
            let sin = self.rotation.sin();
            Pos2::new(
                center.x + p.x * cos - p.y * sin,
                center.y + p.x * sin + p.y * cos,
            )
        };

        let s = self.size;

        match self.shape_type {
            ShapeType::Cube => {
                let h = s * 0.5;
                let d = s * 0.3;

                let f1 = rotate(Vec2::new(-h, -h));
                let f2 = rotate(Vec2::new(h, -h));
                let f3 = rotate(Vec2::new(h, h));
                let f4 = rotate(Vec2::new(-h, h));

                let b1 = rotate(Vec2::new(-h + d, -h - d));
                let b2 = rotate(Vec2::new(h + d, -h - d));
                let b3 = rotate(Vec2::new(h + d, h - d));
                let b4 = rotate(Vec2::new(-h + d, h - d));

                painter.line_segment([f1, f2], stroke);
                painter.line_segment([f2, f3], stroke);
                painter.line_segment([f3, f4], stroke);
                painter.line_segment([f4, f1], stroke);

                painter.line_segment([b1, b2], stroke);
                painter.line_segment([b2, b3], stroke);
                painter.line_segment([b3, b4], stroke);
                painter.line_segment([b4, b1], stroke);

                painter.line_segment([f1, b1], stroke);
                painter.line_segment([f2, b2], stroke);
                painter.line_segment([f3, b3], stroke);
                painter.line_segment([f4, b4], stroke);
            }
            ShapeType::Tetrahedron => {
                let top = rotate(Vec2::new(0.0, -s * 0.7));
                let bl = rotate(Vec2::new(-s * 0.6, s * 0.5));
                let br = rotate(Vec2::new(s * 0.6, s * 0.5));
                let back = rotate(Vec2::new(0.0, s * 0.1));

                painter.line_segment([top, bl], stroke);
                painter.line_segment([top, br], stroke);
                painter.line_segment([top, back], stroke);
                painter.line_segment([bl, br], stroke);
                painter.line_segment([bl, back], stroke);
                painter.line_segment([br, back], stroke);
            }
            ShapeType::Octahedron => {
                let top = rotate(Vec2::new(0.0, -s * 0.8));
                let bottom = rotate(Vec2::new(0.0, s * 0.8));
                let left = rotate(Vec2::new(-s * 0.6, 0.0));
                let right = rotate(Vec2::new(s * 0.6, 0.0));
                let front = rotate(Vec2::new(0.0, -s * 0.2));
                let back = rotate(Vec2::new(0.0, s * 0.2));

                painter.line_segment([top, left], stroke);
                painter.line_segment([top, right], stroke);
                painter.line_segment([top, front], stroke);
                painter.line_segment([top, back], stroke);

                painter.line_segment([bottom, left], stroke);
                painter.line_segment([bottom, right], stroke);
                painter.line_segment([bottom, front], stroke);
                painter.line_segment([bottom, back], stroke);

                painter.line_segment([left, front], stroke);
                painter.line_segment([front, right], stroke);
                painter.line_segment([right, back], stroke);
                painter.line_segment([back, left], stroke);
            }
            ShapeType::Diamond => {
                let top = rotate(Vec2::new(0.0, -s));
                let bottom = rotate(Vec2::new(0.0, s));
                let left = rotate(Vec2::new(-s * 0.5, 0.0));
                let right = rotate(Vec2::new(s * 0.5, 0.0));

                painter.line_segment([top, left], stroke);
                painter.line_segment([top, right], stroke);
                painter.line_segment([bottom, left], stroke);
                painter.line_segment([bottom, right], stroke);

                painter.line_segment([left, right], stroke);
                painter.line_segment([top, bottom], stroke);
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct SplashAnimationState {
    shapes: Vec<WireframeShape>,
    last_time: f64,
    grid_timer: f32,
    initialized: bool,
}

fn draw_synthwave_background(painter: &egui::Painter, screen_size: Vec2, grid_timer: f32, time: f64) {
    let horizon_y = screen_size.y * 0.45;
    let center_x = screen_size.x * 0.5;

    // Smoothly cycle grid color over time
    let hue = (time * 0.05) % 1.0;
    let r = (120.0 + 80.0 * (hue * 6.28).cos()) as u8;
    let g = (120.0 + 80.0 * (hue * 6.28 + 2.09).cos()) as u8;
    let b = (120.0 + 80.0 * (hue * 6.28 + 4.18).cos()) as u8;
    let grid_base_color = Color32::from_rgb(r, g, b);
    
    let grid_color = grid_base_color.gamma_multiply(0.15);
    let glow_color = grid_base_color.gamma_multiply(0.08);

    // Vertical lines (perspective) - Wider spread to fill edges
    let num_v_lines = 24;
    let num_v_segments = 12; // Split vertical lines for gradient fade
    for i in 0..=num_v_lines {
        let t = i as f32 / num_v_lines as f32;
        let x_bottom_total = center_x + (t - 0.5) * screen_size.x * 3.5;
        let x_top_total = center_x + (t - 0.5) * screen_size.x * 1.2;
        
        for s in 0..num_v_segments {
            let s_start = s as f32 / num_v_segments as f32;
            let s_end = (s + 1) as f32 / num_v_segments as f32;
            
            // s=0 is horizon, s=1 is bottom
            let y_start = horizon_y + s_start * (screen_size.y - horizon_y);
            let y_end = horizon_y + s_end * (screen_size.y - horizon_y);
            
            let x_start = x_top_total + s_start * (x_bottom_total - x_top_total);
            let x_end = x_top_total + s_end * (x_bottom_total - x_top_total);
            
            // Smooth fade-out near horizon (smaller s)
            let alpha = (s_start * 2.5).min(1.0);
            let seg_grid_color = grid_color.gamma_multiply(alpha);
            let seg_glow_color = glow_color.gamma_multiply(alpha);

            painter.line_segment(
                [Pos2::new(x_start, y_start), Pos2::new(x_end, y_end)],
                Stroke::new(3.0, seg_glow_color),
            );
            painter.line_segment(
                [Pos2::new(x_start, y_start), Pos2::new(x_end, y_end)],
                Stroke::new(1.0, seg_grid_color),
            );
        }
    }

    // Horizontal lines (scrolling) with depth fade
    let num_h_lines = 12;
    for i in 0..num_h_lines {
        let t = ((i as f32 + grid_timer) / num_h_lines as f32) % 1.0;
        // Exponential spacing for perspective
        let p = t * t; 
        let y = horizon_y + p * (screen_size.y - horizon_y);
        
        // Fade out as it gets closer to the horizon (small p)
        let alpha = (p * 2.5).min(1.0);
        let fade_grid_color = grid_color.gamma_multiply(alpha);
        let fade_glow_color = glow_color.gamma_multiply(alpha);
        
        // Ensure horizontal lines always span the full width
        let x_start = 0.0;
        let x_end = screen_size.x;
        
        // Draw with glow
        painter.line_segment(
            [Pos2::new(x_start, y), Pos2::new(x_end, y)],
            Stroke::new(3.0, fade_glow_color),
        );
        painter.line_segment(
            [Pos2::new(x_start, y), Pos2::new(x_end, y)],
            Stroke::new(1.0, fade_grid_color),
        );
    }
}

pub fn render_splash(
    ctx: &egui::Context,
    window_state: &mut WindowState,
    settings: &mut EditorSettings,
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<AppState>,
    theme: &Theme,
    locale: &mut crate::locale::LocaleResource,
) {
    ctx.request_repaint();

    render_splash_title_bar(ctx, window_state, theme);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(Color32::from_rgb(5, 4, 10))) // Even darker background
        .show(ctx, |ui| {
            let screen_rect = ui.max_rect();
            let screen_size = Vec2::new(screen_rect.width(), screen_rect.height() + TITLE_BAR_HEIGHT);

            let anim_state = ui.memory_mut(|mem| {
                mem.data.get_temp_mut_or_default::<SplashAnimationState>(egui::Id::new("splash_anim")).clone()
            });

            let mut anim_state = anim_state;

            if !anim_state.initialized || anim_state.shapes.is_empty() {
                anim_state.shapes.clear();
                let num_shapes = 12;
                for i in 0..num_shapes {
                    let mut shape = WireframeShape::random(screen_size, i as u64 * 12345);
                    // Make shapes more neon
                    shape.color = match i % 3 {
                        0 => Color32::from_rgb(0, 180, 180),
                        1 => Color32::from_rgb(180, 0, 180),
                        _ => Color32::from_rgb(180, 180, 0),
                    }.gamma_multiply(0.35);
                    anim_state.shapes.push(shape);
                }
                anim_state.initialized = true;
                anim_state.last_time = ui.input(|i| i.time);
            }

            let current_time = ui.input(|i| i.time);
            let dt = (current_time - anim_state.last_time) as f32;
            anim_state.last_time = current_time;
            anim_state.grid_timer = (anim_state.grid_timer + dt * 0.35) % 1.0;

            let painter = ui.painter();
            
            // Cycle floating shape colors too
            let hue = (current_time * 0.05) % 1.0;
            let cycle_color = |base_h: f32| -> Color32 {
                let h = (hue + base_h as f64) % 1.0;
                let r = (150.0 + 100.0 * (h * 6.28).cos()) as u8;
                let g = (150.0 + 100.0 * (h * 6.28 + 2.09).cos()) as u8;
                let b = (150.0 + 100.0 * (h * 6.28 + 4.18).cos()) as u8;
                Color32::from_rgb(r, g, b).gamma_multiply(0.3)
            };

            // Draw synthwave background
            draw_synthwave_background(painter, screen_size, anim_state.grid_timer, current_time);

            // Draw floating shapes
            for (i, shape) in anim_state.shapes.iter_mut().enumerate() {
                shape.update(dt.min(0.1), screen_size);
                shape.color = cycle_color(i as f32 * 0.1);
                shape.draw(painter);
            }

            ui.memory_mut(|mem| {
                *mem.data.get_temp_mut_or_default::<SplashAnimationState>(egui::Id::new("splash_anim")) = anim_state;
            });

            render_two_column_layout(ui, settings, app_config, commands, next_state, locale);
        });
}

fn render_two_column_layout(
    ui: &mut egui::Ui,
    settings: &mut EditorSettings,
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<AppState>,
    locale: &mut crate::locale::LocaleResource,
) {
    let accent_color = Color32::from_rgb(80, 140, 220);
    let text_muted = Color32::from_rgb(120, 120, 135);
    let border_color = Color32::from_rgb(60, 65, 80);
    let item_bg = Color32::from_rgb(20, 20, 28); 
    let item_hover = Color32::from_rgb(30, 30, 45);

    let screen_rect = ui.max_rect();
    let column_gap = 100.0;
    // Ensure minimum sizes to prevent negative dimensions
    let content_width = (screen_rect.width() - 200.0 - column_gap).clamp(200.0, 1000.0);
    let left_width = (content_width * 0.45).max(100.0);
    let right_width = (content_width * 0.55).max(100.0);
    let content_height = 360.0; 

    let start_x = (screen_rect.width() - content_width - column_gap) / 2.0 + screen_rect.min.x;
    let start_y = screen_rect.min.y + (screen_rect.height() - content_height) / 2.0;

    // Left column area
    let left_rect = egui::Rect::from_min_size(
        egui::pos2(start_x, start_y),
        Vec2::new(left_width, content_height),
    );

    // Right column area
    let right_rect = egui::Rect::from_min_size(
        egui::pos2(start_x + left_width + column_gap, start_y),
        Vec2::new(right_width, content_height),
    );
    let mut project_to_open: Option<CurrentProject> = None;
    let mut path_to_remove: Option<usize> = None;

    // Left column - New Project
    #[allow(deprecated)]
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left_rect), |ui: &mut egui::Ui| {
        ui.vertical(|ui: &mut egui::Ui| {
            ui.add_space(content_height * 0.2); // Push down to center content
            
            // New Project Section
            ui.label(RichText::new(crate::locale::t("splash.new_project").to_uppercase()).size(10.0).color(text_muted).strong().extra_letter_spacing(1.0));
            ui.add_space(20.0);

            let input_width = (left_width - 10.0).max(50.0);
            
            egui::Frame::none()
                .fill(Color32::from_rgb(15, 15, 20))
                .stroke(Stroke::new(1.0, border_color))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(egui::Margin::symmetric(2, 2))
                .show(ui, |ui: &mut egui::Ui| {
                    ui.add_sized(
                        Vec2::new(input_width - 4.0, 40.0),
                        egui::TextEdit::singleline(&mut settings.new_project_name)
                            .hint_text("Enter project name...")
                            .margin(egui::Margin::symmetric(14, 12))
                            .frame(false), // Custom frame used instead
                    );
                });

            ui.add_space(20.0);

            // Buttons row
            ui.horizontal(|ui: &mut egui::Ui| {
                let btn_width = ((input_width - 16.0) / 2.0).max(40.0);

                let create_btn = egui::Button::new(
                    RichText::new(crate::locale::t("splash.create").to_uppercase()).color(Color32::WHITE).strong()
                )
                .fill(accent_color)
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(btn_width, 42.0));

                if ui.add(create_btn).clicked() {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_title("Select Project Location")
                        .pick_folder()
                    {
                        let project_name = if settings.new_project_name.is_empty() {
                            "New Project"
                        } else {
                            &settings.new_project_name
                        };
                        let project_path = folder.join(project_name.replace(' ', "_").to_lowercase());

                        match create_project(&project_path, project_name) {
                            Ok(project) => {
                                app_config.add_recent_project(project_path);
                                let _ = app_config.save();
                                commands.insert_resource(project);
                                next_state.set(AppState::Editor);
                            }
                            Err(e) => {
                                eprintln!("Failed to create project: {}", e);
                            }
                        }
                    }
                }

                let open_btn = egui::Button::new(
                    RichText::new(crate::locale::t("splash.open_project").to_uppercase()).color(Color32::from_rgb(220, 220, 230))
                )
                .fill(item_bg)
                .stroke(Stroke::new(1.0, border_color))
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(btn_width, 42.0));

                if ui.add(open_btn).clicked() {
                    if let Some(file) = rfd::FileDialog::new()
                        .set_title("Open Project")
                        .add_filter("Project File", &["toml"])
                        .pick_file()
                    {
                        match open_project(&file) {
                            Ok(project) => {
                                app_config.add_recent_project(project.path.clone());
                                let _ = app_config.save();
                                commands.insert_resource(project);
                                next_state.set(AppState::Editor);
                            }
                            Err(e) => {
                                eprintln!("Failed to open project: {}", e);
                            }
                        }
                    }
                }
            });

            ui.add_space(24.0);

            // Language selector
            let current_lang_name = locale.available.iter()
                .find(|l| l.code == locale.current)
                .map(|l| l.name.clone())
                .unwrap_or_else(|| "English".to_string());
            let mut changed_to: Option<String> = None;
            ui.horizontal(|ui: &mut egui::Ui| {
                ui.label(RichText::new(crate::locale::t("settings.language")).size(11.0).color(text_muted));
                egui::ComboBox::from_id_salt("splash_language_selector")
                    .selected_text(&current_lang_name)
                    .width((input_width * 0.6).max(80.0))
                    .show_ui(ui, |ui| {
                        let codes_and_names: Vec<(String, String)> = locale.available.iter()
                            .map(|l| (l.code.clone(), l.name.clone()))
                            .collect();
                        for (code, name) in &codes_and_names {
                            let is_selected = *code == locale.current;
                            if ui.selectable_label(is_selected, name).clicked() && !is_selected {
                                changed_to = Some(code.clone());
                            }
                        }
                    });
            });
            if let Some(new_code) = changed_to {
                locale.load_locale(&new_code);
                crate::locale::set_active_locale(&locale.strings);
                app_config.language = new_code;
                let _ = app_config.save();
            }
        });
    });

    // Right column - Recent Projects
    #[allow(deprecated)]
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |ui: &mut egui::Ui| {
        ui.vertical(|ui: &mut egui::Ui| {
            ui.add_space(20.0);
            ui.label(RichText::new(crate::locale::t("splash.recent").to_uppercase()).size(10.0).color(text_muted).strong().extra_letter_spacing(1.0));
            ui.add_space(16.0);

            if app_config.recent_projects.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui: &mut egui::Ui| {
                    ui.label(RichText::new(crate::locale::t("splash.no_recent")).size(13.0).color(text_muted).italics());
                });
            } else {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui: &mut egui::Ui| {
                        let item_width = (right_width - 12.0).max(50.0);

                        for (idx, project_path) in app_config.recent_projects.iter().enumerate() {
                            let display_name = project_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown Project");

                            let path_display = project_path.to_string_lossy();
                            let project_toml = project_path.join("project.toml");
                            let exists = project_toml.exists();

                            let item_rect = ui.available_rect_before_wrap();
                            let item_rect = egui::Rect::from_min_size(
                                item_rect.min,
                                Vec2::new(item_width, 60.0),
                            );

                            let response = ui.allocate_rect(item_rect, egui::Sense::click());
                            let is_hovered = response.hovered();
                            if is_hovered && exists {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }

                            let bg_color = if is_hovered && exists { item_hover } else { item_bg };
                            ui.painter().rect(
                                item_rect,
                                CornerRadius::same(4),
                                bg_color,
                                Stroke::new(1.0, border_color),
                                StrokeKind::Outside,
                            );

                            let text_pos = item_rect.min + Vec2::new(16.0, 14.0);
                            let path_pos = item_rect.min + Vec2::new(16.0, 36.0);

                            if exists {
                                ui.painter().text(
                                    text_pos,
                                    egui::Align2::LEFT_TOP,
                                    display_name.to_uppercase(),
                                    egui::FontId::proportional(14.0),
                                    if is_hovered { Color32::WHITE } else { Color32::from_rgb(220, 220, 230) },
                                );

                                let max_path_len = 50;
                                let path_str = if path_display.len() > max_path_len {
                                    format!("...{}", &path_display[path_display.len() - max_path_len..])
                                } else {
                                    path_display.to_string()
                                };

                                ui.painter().text(
                                    path_pos,
                                    egui::Align2::LEFT_TOP,
                                    path_str,
                                    egui::FontId::monospace(10.0),
                                    text_muted,
                                );

                                if response.clicked() {
                                    match open_project(&project_toml) {
                                        Ok(project) => {
                                            project_to_open = Some(project);
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to open project: {}", e);
                                        }
                                    }
                                }
                            } else {
                                ui.painter().text(
                                    text_pos,
                                    egui::Align2::LEFT_TOP,
                                    format!("{} (NOT FOUND)", display_name.to_uppercase()),
                                    egui::FontId::proportional(14.0),
                                    text_muted.gamma_multiply(0.6),
                                );

                                let remove_rect = egui::Rect::from_min_size(
                                    item_rect.right_top() + Vec2::new(-70.0, 18.0),
                                    Vec2::new(54.0, 24.0),
                                );

                                let remove_response = ui.allocate_rect(remove_rect, egui::Sense::click());
                                if remove_response.hovered() {
                                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                }
                                let remove_color = if remove_response.hovered() {
                                    Color32::from_rgb(255, 80, 80)
                                } else {
                                    text_muted.gamma_multiply(0.8)
                                };

                                ui.painter().text(
                                    remove_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "REMOVE",
                                    egui::FontId::proportional(10.0),
                                    remove_color,
                                );

                                if remove_response.clicked() {
                                    path_to_remove = Some(idx);
                                }
                            }

                            ui.add_space(8.0);
                        }
                    });
            }
        });
    });

    // Handle project opening/removal after UI
    if let Some(project) = project_to_open {
        app_config.add_recent_project(project.path.clone());
        let _ = app_config.save();
        commands.insert_resource(project);
        next_state.set(AppState::Editor);
    }

    if let Some(idx) = path_to_remove {
        app_config.recent_projects.remove(idx);
        let _ = app_config.save();
    }
}
