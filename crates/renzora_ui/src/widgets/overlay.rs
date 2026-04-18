//! Overlay primitives — modal, popover, themed tooltip.
//!
//! Thin wrappers over egui with renzora theming applied. These underpin
//! confirm dialogs, asset pickers, richer tooltips, etc.

use bevy_egui::egui::{self, Align2};
use renzora_theme::Theme;

/// Modal dialog with dimmed backdrop. The backdrop consumes clicks so the
/// underlying UI isn't reachable. Returns the closure's result.
pub fn modal<R>(
    ctx: &egui::Context,
    id: impl std::hash::Hash,
    title: &str,
    theme: &Theme,
    open: &mut bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    if !*open {
        return None;
    }
    // Backdrop
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new(&id).with("backdrop"))
        .order(egui::Order::Background)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(
                resp.rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140),
            );
            if resp.clicked() {
                *open = false;
            }
        });

    let mut result = None;
    let mut keep_open = *open;
    egui::Window::new(title)
        .id(egui::Id::new(id))
        .open(&mut keep_open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            egui::Frame::new()
                .fill(theme.surfaces.panel.to_color32())
                .stroke(egui::Stroke::new(
                    1.0,
                    theme.widgets.border.to_color32(),
                ))
                .inner_margin(egui::Margin::same(12))
                .corner_radius(6),
        )
        .show(ctx, |ui| {
            result = Some(add_contents(ui));
        });
    *open = keep_open;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }
    result
}

/// Popover anchored to a trigger response. Shows `add_contents` in a small
/// floating frame when `open` is true. Closes on outside click.
pub fn popover<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    anchor: &egui::Response,
    theme: &Theme,
    open: &mut bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    if !*open {
        return None;
    }
    let popup_id = egui::Id::new(id);
    let mut result = None;
    let area = egui::Area::new(popup_id)
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.rect.left_bottom() + egui::vec2(0.0, 4.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(theme.surfaces.panel.to_color32())
                .stroke(egui::Stroke::new(
                    1.0,
                    theme.widgets.border.to_color32(),
                ))
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4)
                .show(ui, |ui| {
                    result = Some(add_contents(ui));
                })
                .response
        })
        .response;

    // Close on outside click
    if ui.input(|i| i.pointer.any_click()) {
        let pointer = ui.ctx().pointer_interact_pos();
        let inside_popup = pointer.map_or(false, |p| area.rect.contains(p));
        let inside_anchor = pointer.map_or(false, |p| anchor.rect.contains(p));
        if !inside_popup && !inside_anchor {
            *open = false;
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }
    result
}

/// Themed tooltip — richer than egui's default. Pass `RichText` or simple
/// strings; frame + stroke follow the theme.
pub fn tooltip(response: &egui::Response, theme: &Theme, body: impl FnOnce(&mut egui::Ui)) {
    response.clone().on_hover_ui(|ui| {
        egui::Frame::new()
            .fill(theme.surfaces.panel.to_color32())
            .stroke(egui::Stroke::new(
                1.0,
                theme.widgets.border.to_color32(),
            ))
            .inner_margin(egui::Margin::symmetric(8, 4))
            .corner_radius(4)
            .show(ui, |ui| body(ui));
    });
}

/// Convenience: yes/no confirm modal. Returns `Some(true)` on confirm,
/// `Some(false)` on cancel, `None` while open / not shown.
pub fn confirm_modal(
    ctx: &egui::Context,
    id: impl std::hash::Hash,
    title: &str,
    message: &str,
    confirm_label: &str,
    cancel_label: &str,
    theme: &Theme,
    open: &mut bool,
) -> Option<bool> {
    modal(ctx, id, title, theme, open, |ui| {
        ui.label(message);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let confirm = ui.button(confirm_label).clicked();
            let cancel = ui.button(cancel_label).clicked();
            if confirm { Some(true) } else if cancel { Some(false) } else { None }
        })
        .inner
    })
    .flatten()
    .inspect(|_| *open = false)
}
