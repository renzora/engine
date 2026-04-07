//! Extension trait for `App` to simplify editor registrations.

use bevy::prelude::*;
use crate::inspector_registry::{InspectorEntry, InspectorRegistry};
use crate::spawn_registry::{EntityPreset, SpawnRegistry};
use renzora_ui::PanelRegistry;

/// Trait implemented by components that can auto-generate their `InspectorEntry`.
///
/// Derived via `#[derive(Inspectable)]` or `#[post_process]`.
pub trait InspectableComponent: Component + Default + 'static {
    fn inspector_entry() -> InspectorEntry;
}

/// Extension methods on `App` for common editor registrations.
pub trait AppEditorExt {
    /// Register a panel with the `PanelRegistry`.
    fn register_panel(&mut self, panel: impl renzora_ui::EditorPanel + 'static) -> &mut Self;

    /// Register an `InspectorEntry` directly.
    fn register_inspector(&mut self, entry: InspectorEntry) -> &mut Self;

    /// Register a component that implements `InspectableComponent` (from derive macro).
    fn register_inspectable<T: InspectableComponent>(&mut self) -> &mut Self;

    /// Register an `EntityPreset` with the `SpawnRegistry`.
    fn register_entity_preset(&mut self, preset: EntityPreset) -> &mut Self;

    /// Register a component icon for the hierarchy tree.
    fn register_component_icon(&mut self, entry: crate::ComponentIconEntry) -> &mut Self;

    /// Convenience: register a post-process effect's type, plugin, and inspector entry.
    fn add_post_process<T>(&mut self) -> &mut Self
    where
        T: renzora_postprocess::PostProcessEffect + InspectableComponent + bevy::reflect::GetTypeRegistration;
}

impl AppEditorExt for App {
    fn register_panel(&mut self, panel: impl renzora_ui::EditorPanel + 'static) -> &mut Self {
        self.init_resource::<PanelRegistry>();
        self.world_mut().resource_mut::<PanelRegistry>().register(panel);
        self
    }

    fn register_inspector(&mut self, entry: InspectorEntry) -> &mut Self {
        self.init_resource::<InspectorRegistry>();
        self.world_mut().resource_mut::<InspectorRegistry>().register(entry);
        self
    }

    fn register_inspectable<T: InspectableComponent>(&mut self) -> &mut Self {
        self.register_inspector(T::inspector_entry())
    }

    fn register_entity_preset(&mut self, preset: EntityPreset) -> &mut Self {
        self.init_resource::<SpawnRegistry>();
        self.world_mut().resource_mut::<SpawnRegistry>().register(preset);
        self
    }

    fn register_component_icon(&mut self, entry: crate::ComponentIconEntry) -> &mut Self {
        self.init_resource::<crate::ComponentIconRegistry>();
        self.world_mut().resource_mut::<crate::ComponentIconRegistry>().register(entry);
        self
    }

    fn add_post_process<T>(&mut self) -> &mut Self
    where
        T: renzora_postprocess::PostProcessEffect + InspectableComponent + bevy::reflect::GetTypeRegistration,
    {
        self.register_type::<T>();
        self.add_plugins(renzora_postprocess::PostProcessPlugin::<T>::default());
        self.register_inspectable::<T>();
        self
    }
}
