use bevy::prelude::*;
use bevy_egui::egui;

/// Trait for components that can render their own inspector UI
/// Implement this trait for any component you want to be editable in the inspector
/// (Kept for future dynamic inspector widget registration)
#[allow(dead_code)]
pub trait InspectorWidget: Component + Sized {
    /// Display name shown as the collapsing header title
    const NAME: &'static str;

    /// Render the inspector UI for this component
    /// Returns true if any value was changed
    fn render_inspector(&mut self, ui: &mut egui::Ui) -> bool;
}

/// Type-erased inspector widget renderer
/// This allows us to store different widget types in a registry
/// (Kept for future dynamic inspector widget registration)
#[allow(dead_code)]
pub type InspectorRenderFn = Box<dyn Fn(&mut World, Entity, &mut egui::Ui) -> bool + Send + Sync>;

/// Create an inspector render function for a component type that implements InspectorWidget
/// (Kept for future dynamic inspector widget registration)
#[allow(dead_code)]
pub fn create_inspector_render_fn<T: InspectorWidget>() -> InspectorRenderFn {
    Box::new(|_world: &mut World, _entity: Entity, _ui: &mut egui::Ui| {
        // We need to check if the entity has this component, then render it
        // This requires temporarily removing the component, rendering, then re-adding
        // For now, we'll use a different approach in the actual inspector
        false
    })
}
