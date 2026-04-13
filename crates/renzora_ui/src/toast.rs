//! Toast notification system — ephemeral messages that appear in the bottom-right
//! corner of the editor and fade out after a configurable duration.

use bevy::prelude::*;
use bevy_egui::egui;

/// Severity level for a toast notification.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    pub fn color(&self) -> egui::Color32 {
        match self {
            ToastLevel::Info => egui::Color32::from_rgb(140, 180, 220),
            ToastLevel::Success => egui::Color32::from_rgb(100, 200, 120),
            ToastLevel::Warning => egui::Color32::from_rgb(230, 180, 80),
            ToastLevel::Error => egui::Color32::from_rgb(220, 80, 80),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ToastLevel::Info => egui_phosphor::regular::INFO,
            ToastLevel::Success => egui_phosphor::regular::CHECK_CIRCLE,
            ToastLevel::Warning => egui_phosphor::regular::WARNING,
            ToastLevel::Error => egui_phosphor::regular::X_CIRCLE,
        }
    }
}

/// A single toast notification.
#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created: f64,
    pub duration: f64,
}

const DEFAULT_DURATION: f64 = 3.0;
const FADE_DURATION: f64 = 0.5;

/// Resource that stores active toast notifications.
#[derive(Resource, Default)]
pub struct Toasts {
    entries: Vec<Toast>,
}

impl Toasts {
    pub fn info(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Info, message);
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Success, message);
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Warning, message);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Error, message);
    }

    pub fn add(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.entries.push(Toast {
            message: message.into(),
            level,
            created: 0.0, // filled in at render time if zero
            duration: DEFAULT_DURATION,
        });
    }

    /// Render toasts in the bottom-right corner. Call this from the main editor UI.
    pub fn show(&mut self, ctx: &egui::Context, current_time: f64) {
        // Stamp creation time on new toasts
        for toast in &mut self.entries {
            if toast.created == 0.0 {
                toast.created = current_time;
            }
        }

        // Remove expired
        self.entries.retain(|t| current_time - t.created < t.duration);

        if self.entries.is_empty() {
            return;
        }

        let screen = ctx.available_rect();
        let margin = 12.0;
        let toast_width = 300.0;
        let toast_height = 32.0;
        let spacing = 4.0;

        for (i, toast) in self.entries.iter().rev().enumerate() {
            let age = current_time - toast.created;
            let fade_in = (age / 0.15).min(1.0) as f32;
            let fade_out = if age > toast.duration - FADE_DURATION {
                ((toast.duration - age) / FADE_DURATION).max(0.0) as f32
            } else {
                1.0
            };
            let alpha = fade_in * fade_out;
            if alpha <= 0.0 {
                continue;
            }

            let y = screen.max.y - margin - (i as f32 + 1.0) * (toast_height + spacing);
            let x = screen.max.x - margin - toast_width;

            egui::Area::new(egui::Id::new("toast").with(i))
                .fixed_pos(egui::pos2(x, y))
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    let color = toast.level.color();
                    let bg = egui::Color32::from_rgba_unmultiplied(30, 30, 35, (alpha * 240.0) as u8);
                    let border = egui::Color32::from_rgba_unmultiplied(
                        color.r(), color.g(), color.b(), (alpha * 180.0) as u8,
                    );
                    let text_color = egui::Color32::from_rgba_unmultiplied(
                        230, 230, 230, (alpha * 255.0) as u8,
                    );
                    let icon_color = egui::Color32::from_rgba_unmultiplied(
                        color.r(), color.g(), color.b(), (alpha * 255.0) as u8,
                    );

                    egui::Frame::NONE
                        .fill(bg)
                        .stroke(egui::Stroke::new(1.0, border))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .corner_radius(egui::CornerRadius::same(6))
                        .show(ui, |ui| {
                            ui.set_width(toast_width - 22.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.label(
                                    egui::RichText::new(toast.level.icon())
                                        .size(14.0)
                                        .color(icon_color),
                                );
                                ui.label(
                                    egui::RichText::new(&toast.message)
                                        .size(12.0)
                                        .color(text_color),
                                );
                            });
                        });
                });
        }

        // Request repaint while toasts are visible
        ctx.request_repaint();
    }
}
