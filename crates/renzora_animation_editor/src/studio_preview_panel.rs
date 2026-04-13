//! Studio Preview Panel — displays the isolated animation preview viewport
//! with a vertical toolbar for toggle controls.

use bevy::prelude::*;
use renzora::bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use renzora::bevy_egui::EguiUserTextures;
use renzora::egui_phosphor::regular;

use renzora::editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora::theme::ThemeManager;

use crate::studio_preview::{StudioPreviewImage, StudioPreviewOrbit, StudioPreviewSettings};

pub struct StudioPreviewPanel;

impl EditorPanel for StudioPreviewPanel {
    fn id(&self) -> &str {
        "studio_preview"
    }

    fn title(&self) -> &str {
        "Studio Preview"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::VIDEO_CAMERA)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }

    fn min_size(&self) -> [f32; 2] {
        [280.0, 200.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let preview = world.get_resource::<StudioPreviewImage>();
        let user_textures = world.get_resource::<EguiUserTextures>();
        let settings = world.get_resource::<StudioPreviewSettings>();

        let texture_id = preview.and_then(|p| {
            p.texture_id.or_else(|| {
                user_textures.and_then(|ut| ut.image_id(p.handle.id()))
            })
        });

        let (text_primary, text_muted, surface, accent) = world
            .get_resource::<ThemeManager>()
            .map(|tm| {
                let t = &tm.active_theme;
                (
                    t.text.primary.to_color32(),
                    t.text.muted.to_color32(),
                    t.surfaces.panel.to_color32(),
                    t.semantic.accent.to_color32(),
                )
            })
            .unwrap_or((
                Color32::WHITE,
                Color32::from_white_alpha(80),
                Color32::from_rgb(35, 35, 40),
                Color32::from_rgb(100, 149, 237),
            ));

        if let Some(texture_id) = texture_id {
            let full_available = ui.available_rect_before_wrap();
            const TOOLBAR_WIDTH: f32 = 36.0;

            // ── Vertical toolbar ──
            let toolbar_rect = Rect::from_min_size(
                full_available.min,
                Vec2::new(TOOLBAR_WIDTH, full_available.height()),
            );
            let tb_response = ui.allocate_rect(toolbar_rect, egui::Sense::hover());
            let tb_painter = ui.painter_at(toolbar_rect);

            // Toolbar background
            tb_painter.rect_filled(toolbar_rect, 0.0, Color32::from_rgb(30, 30, 35));
            // Right edge separator
            tb_painter.line_segment(
                [
                    Pos2::new(toolbar_rect.max.x, toolbar_rect.min.y),
                    Pos2::new(toolbar_rect.max.x, toolbar_rect.max.y),
                ],
                Stroke::new(1.0, Color32::from_rgb(50, 50, 55)),
            );

            // Read current settings
            let (show_skeleton, show_floor, show_wireframe) = settings
                .map(|s| (s.show_skeleton, s.show_floor, s.show_wireframe))
                .unwrap_or((true, true, false));

            // Tool buttons
            struct ToolBtn {
                icon: &'static str,
                tooltip: &'static str,
                active: bool,
                action: u8, // 0=skeleton, 1=floor, 2=wireframe, 3=reset_camera
            }

            let tools = [
                ToolBtn { icon: regular::BONE, tooltip: "Skeleton", active: show_skeleton, action: 0 },
                ToolBtn { icon: regular::GRID_FOUR, tooltip: "Floor", active: show_floor, action: 1 },
                ToolBtn { icon: regular::CUBE, tooltip: "Wireframe", active: show_wireframe, action: 2 },
                ToolBtn { icon: regular::CROSSHAIR, tooltip: "Reset Camera", active: false, action: 3 },
            ];

            const ICON_SIZE: f32 = 16.0;
            const BTN_SIZE: f32 = 28.0;
            const BTN_PAD: f32 = 4.0;
            let btn_x = toolbar_rect.min.x + (TOOLBAR_WIDTH - BTN_SIZE) / 2.0;
            let mut y_offset = toolbar_rect.min.y + BTN_PAD;

            for tool in &tools {
                let btn_rect = Rect::from_min_size(
                    Pos2::new(btn_x, y_offset),
                    Vec2::new(BTN_SIZE, BTN_SIZE),
                );

                let hovered = tb_response.hovered()
                    && ui.ctx().pointer_latest_pos().map_or(false, |p| btn_rect.contains(p));

                // Background
                let bg = if tool.active {
                    Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 40)
                } else if hovered {
                    surface
                } else {
                    Color32::TRANSPARENT
                };
                tb_painter.rect_filled(btn_rect, 3.0, bg);

                // Icon
                let icon_color = if tool.active { accent } else if hovered { text_primary } else { text_muted };
                tb_painter.text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    tool.icon,
                    egui::FontId::proportional(ICON_SIZE),
                    icon_color,
                );

                // Tooltip
                if hovered {
                    let tip_pos = Pos2::new(toolbar_rect.max.x + 6.0, btn_rect.center().y);
                    let tip_galley = tb_painter.layout_no_wrap(
                        tool.tooltip.to_string(),
                        egui::FontId::proportional(11.0),
                        text_primary,
                    );
                    let tip_rect = Rect::from_min_size(
                        Pos2::new(tip_pos.x - 4.0, tip_pos.y - tip_galley.size().y / 2.0 - 3.0),
                        Vec2::new(tip_galley.size().x + 8.0, tip_galley.size().y + 6.0),
                    );
                    let fg = ui.ctx().layer_painter(egui::LayerId::new(
                        egui::Order::Tooltip,
                        ui.id().with("preview_toolbar_tip"),
                    ));
                    fg.rect_filled(tip_rect, 4.0, Color32::from_rgb(50, 50, 55));
                    fg.text(
                        tip_pos,
                        egui::Align2::LEFT_CENTER,
                        tool.tooltip,
                        egui::FontId::proportional(11.0),
                        text_primary,
                    );
                }

                // Click
                if hovered && ui.ctx().input(|i| i.pointer.any_click()) {
                    let action = tool.action;
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_resource_mut::<StudioPreviewSettings>() {
                                match action {
                                    0 => s.show_skeleton = !s.show_skeleton,
                                    1 => s.show_floor = !s.show_floor,
                                    2 => s.show_wireframe = !s.show_wireframe,
                                    3 => {
                                        // Reset camera
                                        if let Some(mut orbit) = world.get_resource_mut::<StudioPreviewOrbit>() {
                                            orbit.yaw = 0.5;
                                            orbit.pitch = 0.2;
                                        }
                                        if let Some(mut tracker) = world.get_resource_mut::<crate::studio_preview::StudioPreviewTracker>() {
                                            tracker.auto_fitted = false;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        });
                    }
                }

                y_offset += BTN_SIZE + 2.0;
            }

            // ── Preview image (right of toolbar) ──
            let image_rect = Rect::from_min_max(
                Pos2::new(full_available.min.x + TOOLBAR_WIDTH, full_available.min.y),
                full_available.max,
            );
            let image_size = image_rect.size();

            // Request render target resize
            let rw = (image_size.x as u32).max(64);
            let rh = (image_size.y as u32).max(64);
            if let Some(preview) = world.get_resource::<StudioPreviewImage>() {
                if preview.requested_size != (rw, rh) {
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            if let Some(mut p) = world.get_resource_mut::<StudioPreviewImage>() {
                                p.requested_size = (rw, rh);
                            }
                        });
                    }
                }
            }

            // Allocate the image area
            let image_ui_rect = ui.allocate_rect(image_rect, egui::Sense::click_and_drag());
            ui.painter().image(
                texture_id,
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            // ── Orbit controls ──
            if let Some(orbit) = world.get_resource::<StudioPreviewOrbit>() {
                let orbit_sensitivity = 0.005;
                let mut new_yaw = orbit.yaw;
                let mut new_pitch = orbit.pitch;
                let mut orbit_changed = false;

                // Right-click drag: orbit
                if image_ui_rect.dragged_by(egui::PointerButton::Secondary) {
                    let delta = image_ui_rect.drag_delta();
                    new_yaw -= delta.x * orbit_sensitivity;
                    new_pitch = (new_pitch + delta.y * orbit_sensitivity).clamp(-1.5, 1.5);
                    orbit_changed = true;
                }

                if orbit_changed {
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            let mut orbit = world.resource_mut::<StudioPreviewOrbit>();
                            orbit.yaw = new_yaw;
                            orbit.pitch = new_pitch;
                        });
                    }
                }

                // Middle-click drag: pan
                if image_ui_rect.dragged_by(egui::PointerButton::Middle) {
                    let delta = image_ui_rect.drag_delta();
                    let right = bevy::math::Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin());
                    let up = bevy::math::Vec3::Y;
                    let pan_speed = orbit.distance * 0.003;
                    let new_target = orbit.target - right * delta.x * pan_speed + up * delta.y * pan_speed;
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            world.resource_mut::<StudioPreviewOrbit>().target = new_target;
                        });
                    }
                }

                // Scroll: smooth zoom (change orbit distance)
                if image_ui_rect.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll.abs() > 0.0 {
                        // Multiplicative zoom: scroll up = closer, scroll down = further
                        // Using a small factor for smooth feel
                        let zoom_factor = 1.0 - scroll * 0.003;
                        let new_distance = (orbit.distance * zoom_factor).clamp(0.2, 50.0);
                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                            cmds.push(move |world: &mut World| {
                                world.resource_mut::<StudioPreviewOrbit>().distance = new_distance;
                            });
                        }
                    }
                }
            }
        } else {
            // No preview texture — empty state
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);
                ui.label(
                    egui::RichText::new(regular::VIDEO_CAMERA)
                        .size(32.0)
                        .color(text_muted),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Select an animated entity")
                        .size(13.0)
                        .color(text_muted),
                );
                ui.label(
                    egui::RichText::new("to preview animations here")
                        .size(11.0)
                        .color(text_muted),
                );
            });
        }
    }
}
