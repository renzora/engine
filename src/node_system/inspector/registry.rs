use bevy::prelude::*;
use bevy_egui::egui;
use std::any::TypeId;

use super::widget::InspectorWidget;

// Type-erased function signature (kept for future use)
#[allow(dead_code)]
pub type InspectorCheckAndRenderFn =
    Box<dyn Fn(Entity, &World, &mut egui::Ui, &mut dyn FnMut(&mut egui::Ui, &mut dyn std::any::Any) -> bool) -> bool + Send + Sync>;

/// Registry of inspector widgets
/// Components register themselves here to appear in the inspector panel
/// (Infrastructure kept for future dynamic inspector widget registration)
#[derive(Resource, Default)]
pub struct InspectorRegistry {
    /// Registered widget types with their names and type IDs
    #[allow(dead_code)]
    widgets: Vec<RegisteredWidget>,
}

#[allow(dead_code)]
struct RegisteredWidget {
    name: &'static str,
    type_id: TypeId,
}

impl InspectorRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
        }
    }

    /// Register a component type for inspector rendering
    #[allow(dead_code)]
    pub fn register<T: InspectorWidget + 'static>(&mut self) {
        self.widgets.push(RegisteredWidget {
            name: T::NAME,
            type_id: TypeId::of::<T>(),
        });
    }

    /// Get all registered widget names (for debugging)
    #[allow(dead_code)]
    pub fn widget_names(&self) -> Vec<&'static str> {
        self.widgets.iter().map(|w| w.name).collect()
    }
}
