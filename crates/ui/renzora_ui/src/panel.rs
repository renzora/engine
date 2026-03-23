//! EditorPanel trait and PanelRegistry resource
//!
//! External crates implement `EditorPanel` and register via `PanelRegistry`.

use bevy::prelude::*;
use bevy_egui::egui;

/// Where a panel prefers to appear by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelLocation {
    Left,
    Right,
    Bottom,
    Center,
}

/// Trait that all editor panels implement.
///
/// Panels receive `&self` — use interior mutability (e.g. `RefCell`) for local state.
pub trait EditorPanel: Send + Sync + 'static {
    /// Unique string identifier (e.g. `"hierarchy"`, `"inspector"`).
    fn id(&self) -> &str;

    /// Human-readable title shown in tab bars.
    fn title(&self) -> &str;

    /// Optional icon text (e.g. a single Unicode/Phosphor glyph).
    fn icon(&self) -> Option<&str> {
        None
    }

    /// Render the panel content into the given `egui::Ui`.
    fn ui(&self, ui: &mut egui::Ui, world: &World);

    /// Whether this panel can be closed by the user.
    fn closable(&self) -> bool {
        true
    }

    /// Minimum size `[width, height]` in logical pixels.
    fn min_size(&self) -> [f32; 2] {
        [100.0, 50.0]
    }

    /// Preferred default dock location.
    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}

/// Registry of all editor panels. External crates add panels here during plugin `build()`.
#[derive(Resource, Default)]
pub struct PanelRegistry {
    panels: Vec<Box<dyn EditorPanel>>,
}

impl PanelRegistry {
    /// Register a new panel. Duplicate IDs are silently ignored.
    pub fn register(&mut self, panel: impl EditorPanel) {
        let id = panel.id().to_string();
        if self.panels.iter().any(|p| p.id() == id) {
            return;
        }
        self.panels.push(Box::new(panel));
    }

    /// Look up a panel by ID.
    pub fn get(&self, id: &str) -> Option<&dyn EditorPanel> {
        self.panels.iter().find(|p| p.id() == id).map(|b| &**b)
    }

    /// Iterate over all registered panels.
    pub fn iter(&self) -> impl Iterator<Item = &dyn EditorPanel> {
        self.panels.iter().map(|b| &**b)
    }
}
