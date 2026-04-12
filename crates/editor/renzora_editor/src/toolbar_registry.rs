//! Viewport toolbar registry — lets plugins add buttons to the vertical tool
//! overlay without editing the viewport crate.
//!
//! Built-in tools (Select/Translate/Rotate/Scale/TerrainSculpt/TerrainPaint/
//! FoliagePaint) register through this same registry at editor plugin build
//! time. Community plugins register their own tools the same way via
//! `App::register_tool()`.
//!
//! The toolbar renderer (in `renzora_viewport`) iterates entries grouped by
//! [`ToolSection`] and calls each entry's predicates + activator closures.
//! No coupling to specific tool enums.

use bevy::prelude::*;
use std::sync::Arc;

/// Logical grouping on the toolbar. Entries within a section share a divider.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToolSection {
    /// Gizmo tools: select, translate, rotate, scale.
    Transform,
    /// Context-sensitive terrain/foliage tools. Visible when a terrain is selected.
    Terrain,
    /// Plugin-defined section. The str is a stable identifier used for sort grouping.
    Custom(&'static str),
}

pub type ToolPredicate = Arc<dyn Fn(&World) -> bool + Send + Sync>;
pub type ToolActivator = Arc<dyn Fn(&mut World) + Send + Sync>;

/// One button on the viewport toolbar.
#[derive(Clone)]
pub struct ToolEntry {
    /// Stable id (e.g. `"builtin.select"`, `"mesh_draw.rect"`). Debug + keybind lookup.
    pub id: &'static str,
    /// Phosphor icon glyph string.
    pub icon: &'static str,
    /// Hover tooltip (include shortcut hint if any).
    pub tooltip: &'static str,
    pub section: ToolSection,
    /// Sort order within the section. Lower = earlier.
    pub order: i32,
    /// Whether this button is currently shown at all.
    pub visible: ToolPredicate,
    /// Whether this tool is the active one (renders highlighted).
    pub is_active: ToolPredicate,
    /// Called when the user clicks the button. Runs as a deferred EditorCommand.
    pub activate: ToolActivator,
}

impl ToolEntry {
    pub fn new(
        id: &'static str,
        icon: &'static str,
        tooltip: &'static str,
        section: ToolSection,
    ) -> Self {
        Self {
            id,
            icon,
            tooltip,
            section,
            order: 0,
            visible: Arc::new(|_| true),
            is_active: Arc::new(|_| false),
            activate: Arc::new(|_| {}),
        }
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn visible_if(mut self, f: impl Fn(&World) -> bool + Send + Sync + 'static) -> Self {
        self.visible = Arc::new(f);
        self
    }

    pub fn active_if(mut self, f: impl Fn(&World) -> bool + Send + Sync + 'static) -> Self {
        self.is_active = Arc::new(f);
        self
    }

    pub fn on_activate(mut self, f: impl Fn(&mut World) + Send + Sync + 'static) -> Self {
        self.activate = Arc::new(f);
        self
    }
}

/// Resource holding every registered tool. Built-in tools are populated by
/// [`crate::RenzoraEditorPlugin`]; plugins add more via `App::register_tool()`.
#[derive(Resource, Default, Clone)]
pub struct ToolbarRegistry {
    entries: Vec<ToolEntry>,
}

impl ToolbarRegistry {
    pub fn register(&mut self, entry: ToolEntry) {
        self.entries.push(entry);
    }

    /// All entries (caller filters by section + visibility).
    pub fn entries(&self) -> &[ToolEntry] {
        &self.entries
    }

    /// Entries in a specific section, sorted by `order`, filtered by visibility.
    pub fn visible_in_section(&self, world: &World, section: &ToolSection) -> Vec<ToolEntry> {
        let mut out: Vec<ToolEntry> = self
            .entries
            .iter()
            .filter(|e| &e.section == section && (e.visible)(world))
            .cloned()
            .collect();
        out.sort_by_key(|e| e.order);
        out
    }

    /// Distinct custom section ids present in the registry (for rendering
    /// plugin-defined sections after the built-in ones).
    pub fn custom_sections(&self) -> Vec<&'static str> {
        let mut ids: Vec<&'static str> = self
            .entries
            .iter()
            .filter_map(|e| match e.section {
                ToolSection::Custom(id) => Some(id),
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}
