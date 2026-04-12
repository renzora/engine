//! Bottom status bar with plugin-contributed items.

use bevy::prelude::*;
use bevy_egui::egui;
use renzora_theme::Theme;

/// A single item displayed in the status bar.
///
/// Plugins implement this trait and register via `StatusBarRegistry`.
/// Items receive `&self` — use interior mutability for local state.
pub trait StatusBarItem: Send + Sync + 'static {
    /// Unique identifier (e.g. `"system_monitor"`).
    fn id(&self) -> &str;

    /// Where this item appears in the status bar.
    fn alignment(&self) -> StatusBarAlignment {
        StatusBarAlignment::Right
    }

    /// Render order within the alignment group. Lower values render first.
    fn order(&self) -> i32 {
        0
    }

    /// Render this item's content into the status bar `Ui`.
    fn ui(&self, ui: &mut egui::Ui, world: &World);
}

/// Which side of the status bar an item is placed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarAlignment {
    Left,
    Right,
}

/// Registry of all status bar items.
#[derive(Resource, Default)]
pub struct StatusBarRegistry {
    items: Vec<Box<dyn StatusBarItem>>,
}

impl StatusBarRegistry {
    /// Register a new status bar item. Duplicate IDs are silently ignored.
    pub fn register(&mut self, item: impl StatusBarItem) {
        let id = item.id().to_string();
        if self.items.iter().any(|i| i.id() == id) {
            return;
        }
        self.items.push(Box::new(item));
    }

    /// Iterate left-aligned items, sorted by order.
    pub fn left_items(&self) -> Vec<&dyn StatusBarItem> {
        let mut items: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.alignment() == StatusBarAlignment::Left)
            .map(|b| &**b)
            .collect();
        items.sort_by_key(|i| i.order());
        items
    }

    /// Iterate right-aligned items, sorted by order.
    pub fn right_items(&self) -> Vec<&dyn StatusBarItem> {
        let mut items: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.alignment() == StatusBarAlignment::Right)
            .map(|b| &**b)
            .collect();
        items.sort_by_key(|i| i.order());
        items
    }
}

/// Render the status bar at the bottom of the editor window.
pub fn render_status_bar(ctx: &egui::Context, theme: &Theme, world: &World) {
    let empty = StatusBarRegistry::default();
    let registry = world.get_resource::<StatusBarRegistry>().unwrap_or(&empty);
    let text_color = theme.text.secondary.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let panel_fill = theme.surfaces.panel.to_color32();

    egui::TopBottomPanel::bottom("renzora_status_bar")
        .exact_height(22.0)
        .frame(
            egui::Frame::NONE
                .fill(panel_fill)
                .stroke(egui::Stroke::new(1.0, border_color)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;

                // Left side: "Ready" label + left-aligned items
                ui.label(
                    egui::RichText::new("Ready").size(11.0).color(text_color),
                );

                for item in registry.left_items() {
                    ui.separator();
                    item.ui(ui, world);
                }

                // Right side: right-aligned items pushed to the end
                let right_items = registry.right_items();
                if !right_items.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        // Render in reverse so the lowest-order item ends up leftmost
                        for item in right_items.iter().rev() {
                            item.ui(ui, world);
                        }
                    });
                }
            });
        });
}
