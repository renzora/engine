//! Viewport overlay registry — lets any plugin register a 2D painter callback
//! that draws on top of the 3D viewport image each frame.
//!
//! Used by the grid and gizmo crates to render CPU-projected geometry without
//! going through the Bevy render pipeline. The viewport panel is the only
//! caller of [`ViewportOverlayRegistry::draw_all`] — plugins stay decoupled
//! from the panel and from each other.

use bevy::prelude::{Resource, World};
use bevy_egui::egui;

/// A drawer function. Takes the panel's `Ui`, a read-only `World`, and the
/// viewport image rect (so drawers can map NDC into panel coordinates).
pub type ViewportOverlayDrawer = fn(&mut egui::Ui, &World, egui::Rect);

/// Registry of viewport overlay drawers. Populated at plugin-build time,
/// consumed by the viewport panel each frame.
#[derive(Resource, Default)]
pub struct ViewportOverlayRegistry {
    drawers: Vec<(i32, ViewportOverlayDrawer)>,
}

impl ViewportOverlayRegistry {
    /// Register a drawer. `order` controls back-to-front paint order
    /// (lowest first). Suggested bands: grid ~0, gizmos ~100, HUD ~200.
    pub fn register(&mut self, order: i32, drawer: ViewportOverlayDrawer) {
        self.drawers.push((order, drawer));
        self.drawers.sort_by_key(|(o, _)| *o);
    }

    /// Invoke every registered drawer in order.
    pub fn draw_all(&self, ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
        for (_, drawer) in &self.drawers {
            drawer(ui, world, rect);
        }
    }
}
