use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, RichText, Stroke, StrokeKind, Vec2};

use crate::core::{AppState, EditorState};
use crate::project::{create_project, open_project, AppConfig, CurrentProject};

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
    initialized: bool,
}

pub fn render_splash(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<AppState>,
) {
    ctx.request_repaint();

    render_splash_title_bar(ctx, editor_state);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(Color32::from_rgb(18, 18, 22)))
        .show(ctx, |ui| {
            let screen_rect = ui.max_rect();
            let screen_size = Vec2::new(screen_rect.width(), screen_rect.height() + TITLE_BAR_HEIGHT);

            let anim_state = ui.memory_mut(|mem| {
                mem.data.get_temp_mut_or_default::<SplashAnimationState>(egui::Id::new("splash_anim")).clone()
            });

            let mut anim_state = anim_state;

            if !anim_state.initialized || anim_state.shapes.is_empty() {
                anim_state.shapes.clear();
                let num_shapes = 15;
                for i in 0..num_shapes {
                    anim_state.shapes.push(WireframeShape::random(screen_size, i as u64 * 12345));
                }
                anim_state.initialized = true;
                anim_state.last_time = ui.input(|i| i.time);
            }

            let current_time = ui.input(|i| i.time);
            let dt = (current_time - anim_state.last_time) as f32;
            anim_state.last_time = current_time;

            let painter = ui.painter();
            for shape in &mut anim_state.shapes {
                shape.update(dt.min(0.1), screen_size);
                shape.draw(painter);
            }

            ui.memory_mut(|mem| {
                *mem.data.get_temp_mut_or_default::<SplashAnimationState>(egui::Id::new("splash_anim")) = anim_state;
            });

            let splash_width = 420.0;
            let splash_height = 500.0;
            let window_pos = egui::pos2(
                (screen_rect.width() - splash_width) / 2.0,
                (screen_rect.height() - splash_height) / 2.0,
            );

            render_splash_window(ui.ctx(), editor_state, app_config, commands, next_state, window_pos, splash_width, splash_height);
        });
}

fn render_splash_window(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<AppState>,
    window_pos: egui::Pos2,
    splash_width: f32,
    splash_height: f32,
) {
    let panel_bg = Color32::from_rgba_unmultiplied(28, 28, 35, 245);
    let accent_color = Color32::from_rgb(100, 160, 255);
    let text_muted = Color32::from_rgb(140, 140, 155);
    let border_color = Color32::from_rgb(50, 50, 60);
    let item_bg = Color32::from_rgb(35, 35, 45);
    let item_hover = Color32::from_rgb(45, 45, 58);

    egui::Area::new(egui::Id::new("splash_panel"))
        .fixed_pos(window_pos)
        .show(ctx, |ui| {
            let panel_rect = egui::Rect::from_min_size(window_pos, Vec2::new(splash_width, splash_height));

            ui.painter().rect(
                panel_rect,
                CornerRadius::same(12),
                panel_bg,
                Stroke::new(1.0, border_color),
                StrokeKind::Outside,
            );

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel_rect.shrink(24.0)), |ui| {
                ui.vertical(|ui| {
                    // Header
                    ui.add_space(8.0);
                    ui.label(RichText::new("Renzora Engine").size(24.0).color(Color32::WHITE).strong());
                    ui.add_space(4.0);
                    ui.label(RichText::new("Game Development Environment").size(12.0).color(text_muted));
                    ui.add_space(24.0);

                    // New Project Section
                    ui.label(RichText::new("NEW PROJECT").size(11.0).color(text_muted).strong());
                    ui.add_space(8.0);

                    // Project name input with custom styling
                    let input_width = splash_width - 48.0;
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            Vec2::new(input_width, 32.0),
                            egui::TextEdit::singleline(&mut editor_state.new_project_name)
                                .hint_text("Project name...")
                                .margin(egui::Margin::symmetric(12, 8)),
                        );
                    });

                    ui.add_space(12.0);

                    // Create button
                    let create_btn = egui::Button::new(
                        RichText::new("Create Project").color(Color32::WHITE)
                    )
                    .fill(accent_color)
                    .corner_radius(CornerRadius::same(6))
                    .min_size(Vec2::new(input_width, 36.0));

                    if ui.add(create_btn).clicked() {
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_title("Select Project Location")
                            .pick_folder()
                        {
                            let project_name = if editor_state.new_project_name.is_empty() {
                                "New Project"
                            } else {
                                &editor_state.new_project_name
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

                    ui.add_space(8.0);

                    // Open button
                    let open_btn = egui::Button::new(
                        RichText::new("Open Project...").color(Color32::from_rgb(200, 200, 210))
                    )
                    .fill(item_bg)
                    .stroke(Stroke::new(1.0, border_color))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(Vec2::new(input_width, 36.0));

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

                    ui.add_space(24.0);

                    // Recent Projects Section
                    ui.label(RichText::new("RECENT PROJECTS").size(11.0).color(text_muted).strong());
                    ui.add_space(8.0);

                    let mut project_to_open: Option<CurrentProject> = None;
                    let mut path_to_remove: Option<usize> = None;

                    if app_config.recent_projects.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("No recent projects").size(13.0).color(text_muted));
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                ui.set_width(input_width);

                                for (idx, project_path) in app_config.recent_projects.iter().enumerate() {
                                    let display_name = project_path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("Unknown");

                                    let path_display = project_path.to_string_lossy();
                                    let project_toml = project_path.join("project.toml");
                                    let exists = project_toml.exists();

                                    let item_rect = ui.available_rect_before_wrap();
                                    let item_rect = egui::Rect::from_min_size(
                                        item_rect.min,
                                        Vec2::new(input_width, 52.0),
                                    );

                                    let response = ui.allocate_rect(item_rect, egui::Sense::click());
                                    let is_hovered = response.hovered();

                                    // Draw item background
                                    let bg_color = if is_hovered && exists { item_hover } else { item_bg };
                                    ui.painter().rect(
                                        item_rect,
                                        CornerRadius::same(6),
                                        bg_color,
                                        Stroke::NONE,
                                        StrokeKind::Outside,
                                    );

                                    // Draw content
                                    let text_pos = item_rect.min + Vec2::new(12.0, 10.0);
                                    let path_pos = item_rect.min + Vec2::new(12.0, 30.0);

                                    if exists {
                                        ui.painter().text(
                                            text_pos,
                                            egui::Align2::LEFT_TOP,
                                            display_name,
                                            egui::FontId::proportional(14.0),
                                            Color32::WHITE,
                                        );

                                        // Truncate path if too long
                                        let max_path_len = 45;
                                        let path_str = if path_display.len() > max_path_len {
                                            format!("...{}", &path_display[path_display.len() - max_path_len..])
                                        } else {
                                            path_display.to_string()
                                        };

                                        ui.painter().text(
                                            path_pos,
                                            egui::Align2::LEFT_TOP,
                                            path_str,
                                            egui::FontId::proportional(11.0),
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
                                            format!("{} (missing)", display_name),
                                            egui::FontId::proportional(14.0),
                                            text_muted,
                                        );

                                        // Remove button for missing projects
                                        let remove_rect = egui::Rect::from_min_size(
                                            item_rect.right_top() + Vec2::new(-60.0, 14.0),
                                            Vec2::new(48.0, 24.0),
                                        );

                                        let remove_response = ui.allocate_rect(remove_rect, egui::Sense::click());
                                        let remove_color = if remove_response.hovered() {
                                            Color32::from_rgb(255, 100, 100)
                                        } else {
                                            text_muted
                                        };

                                        ui.painter().text(
                                            remove_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "Remove",
                                            egui::FontId::proportional(11.0),
                                            remove_color,
                                        );

                                        if remove_response.clicked() {
                                            path_to_remove = Some(idx);
                                        }
                                    }

                                    ui.add_space(4.0);
                                }
                            });
                    }

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
                });
            });
        });
}
