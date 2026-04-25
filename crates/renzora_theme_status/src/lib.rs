//! Theme switcher in the status bar — click to open a dropup of available themes.

use std::sync::Mutex;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use egui_phosphor::regular;

use renzora_editor_framework::{
    AppEditorExt, DockingState, FloatingPanels, PanelRegistry, SplashState,
    StatusBarAlignment, StatusBarItem,
};
use renzora_theme::ThemeManager;

// ============================================================================
// Deferred-apply channels
// ============================================================================

/// Carries the user's theme selection from the immutable-world
/// `StatusBarItem::ui` call into a mutable-world system that applies it.
#[derive(Resource, Default)]
struct ThemeStatusPending {
    next: Mutex<Option<String>>,
    open_marketplace: Mutex<bool>,
}

fn apply_pending_theme(pending: Res<ThemeStatusPending>, mut tm: ResMut<ThemeManager>) {
    if let Ok(mut slot) = pending.next.lock() {
        if let Some(name) = slot.take() {
            if name != tm.active_theme_name {
                tm.load_theme(&name);
            }
        }
    }
}

fn apply_open_marketplace(
    pending: Res<ThemeStatusPending>,
    mut docking: ResMut<DockingState>,
    mut floating: ResMut<FloatingPanels>,
    registry: Res<PanelRegistry>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    let should_open = {
        let Ok(mut flag) = pending.open_marketplace.lock() else { return };
        std::mem::replace(&mut *flag, false)
    };
    if !should_open {
        return;
    }

    const TARGET: &str = "hub_store";

    if floating.contains(TARGET) {
        return;
    }

    docking.tree.remove_panel(TARGET);

    let size = registry
        .get(TARGET)
        .map(|p| {
            let min = p.min_size();
            egui::Vec2::new(min[0].max(720.0), min[1].max(520.0))
        })
        .unwrap_or(egui::Vec2::new(720.0, 520.0));

    let (screen_w, screen_h) = window
        .single()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((1280.0, 800.0));
    let pos = egui::Pos2::new(
        ((screen_w - size.x) * 0.5).max(0.0),
        ((screen_h - size.y) * 0.5).max(0.0),
    );

    floating.add(TARGET.to_string(), pos, size);
}

// ============================================================================
// Status bar item
// ============================================================================

#[derive(Default)]
struct ThemeStatusItem;

const POPUP_ID: &str = "renzora_theme_status_popup";
const ROW_HEIGHT: f32 = 20.0;
const POPUP_WIDTH: f32 = 200.0;
const POPUP_MAX_HEIGHT: f32 = 280.0;
const ROW_PAD_X: f32 = 6.0;
const CHECK_COL_W: f32 = 16.0;

impl StatusBarItem for ThemeStatusItem {
    fn id(&self) -> &str {
        "theme_switcher"
    }

    fn alignment(&self) -> StatusBarAlignment {
        StatusBarAlignment::Right
    }

    fn order(&self) -> i32 {
        -100
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let Some(tm) = world.get_resource::<ThemeManager>() else { return };
        let theme = tm.active_theme.clone();
        let active_name = tm.active_theme_name.clone();
        let available = tm.available_themes.clone();

        let text_color = theme.text.secondary.to_color32();
        let muted_color = theme.text.muted.to_color32();
        let accent_color = theme.semantic.accent.to_color32();
        let hover_bg = theme.widgets.hovered_bg.to_color32();
        let selection_bg = accent_color.linear_multiply(0.18);

        // Button
        let button_label = format!("{} {}  {}", regular::PALETTE, active_name, regular::CARET_UP);
        let btn = egui::Button::new(
            egui::RichText::new(button_label)
                .size(11.0)
                .color(text_color),
        )
        .frame(false);
        let resp = ui.add(btn);

        let popup_id = egui::Id::new(POPUP_ID);
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
        }

        egui::popup_above_or_below_widget(
            ui,
            popup_id,
            &resp,
            egui::AboveOrBelow::Above,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(POPUP_WIDTH);
                ui.set_max_width(POPUP_WIDTH);
                ui.spacing_mut().item_spacing.y = 2.0;

                ui.label(
                    egui::RichText::new("Themes")
                        .size(10.0)
                        .color(muted_color),
                );
                ui.add_space(2.0);

                egui::ScrollArea::vertical()
                    .max_height(POPUP_MAX_HEIGHT - 56.0)
                    .show(ui, |ui| {
                        for name in &available {
                            let is_active = name == &active_name;
                            let row_color = if is_active { accent_color } else { text_color };

                            if theme_row(ui, name, is_active, row_color, selection_bg, hover_bg) {
                                if let Some(pending) =
                                    world.get_resource::<ThemeStatusPending>()
                                {
                                    if let Ok(mut slot) = pending.next.lock() {
                                        *slot = Some(name.clone());
                                    }
                                }
                                ui.memory_mut(|m| m.close_popup(popup_id));
                            }
                        }
                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                if browse_row(ui, muted_color, hover_bg) {
                    if let Some(pending) = world.get_resource::<ThemeStatusPending>() {
                        if let Ok(mut flag) = pending.open_marketplace.lock() {
                            *flag = true;
                        }
                    }
                    ui.memory_mut(|m| m.close_popup(popup_id));
                }
            },
        );
    }
}

/// Left-aligned, ellipsis-truncated clickable theme row. Returns true on click.
fn theme_row(
    ui: &mut egui::Ui,
    name: &str,
    is_active: bool,
    text_color: egui::Color32,
    selection_bg: egui::Color32,
    hover_bg: egui::Color32,
) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(width, ROW_HEIGHT),
        egui::Sense::click(),
    );

    let bg = if is_active {
        selection_bg
    } else if resp.hovered() {
        hover_bg
    } else {
        egui::Color32::TRANSPARENT
    };
    if bg != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(3), bg);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Check mark column
    if is_active {
        ui.painter().text(
            egui::pos2(rect.min.x + ROW_PAD_X + CHECK_COL_W * 0.5, rect.center().y),
            egui::Align2::CENTER_CENTER,
            regular::CHECK,
            egui::FontId::proportional(11.0),
            text_color,
        );
    }

    // Truncated name
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + ROW_PAD_X + CHECK_COL_W, rect.min.y),
        egui::pos2(rect.max.x - ROW_PAD_X, rect.max.y),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add(
        egui::Label::new(
            egui::RichText::new(name)
                .size(11.0)
                .color(text_color),
        )
        .truncate()
        .selectable(false),
    );

    resp.clicked()
}

/// "Browse themes" footer row — opens the Marketplace panel.
fn browse_row(ui: &mut egui::Ui, muted: egui::Color32, hover_bg: egui::Color32) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(width, ROW_HEIGHT),
        egui::Sense::click(),
    );

    let bg = if resp.hovered() { hover_bg } else { egui::Color32::TRANSPARENT };
    if bg != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(3), bg);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    ui.painter().text(
        egui::pos2(rect.min.x + ROW_PAD_X, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{}  Browse themes…", regular::STOREFRONT),
        egui::FontId::proportional(11.0),
        muted,
    );

    resp.clicked()
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct ThemeStatusPlugin;

impl Plugin for ThemeStatusPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ThemeStatusPlugin");

        app.init_resource::<ThemeStatusPending>();
        app.add_systems(
            Update,
            (apply_pending_theme, apply_open_marketplace)
                .run_if(in_state(SplashState::Editor)),
        );
        app.register_status_item(ThemeStatusItem);
    }
}

renzora::add!(ThemeStatusPlugin, Editor);
