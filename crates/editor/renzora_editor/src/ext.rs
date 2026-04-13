//! Extension trait for `App` to simplify editor registrations.

use bevy::prelude::*;
use crate::inspector_registry::{InspectorEntry, InspectorRegistry};
use crate::spawn_registry::{EntityPreset, SpawnRegistry};
use crate::shortcut_registry::{ShortcutEntry, ShortcutRegistry};
use crate::toolbar_registry::{ToolEntry, ToolbarRegistry};
use renzora_ui::{PanelRegistry, StatusBarRegistry};

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

    /// Register a status bar item with the `StatusBarRegistry`.
    fn register_status_item(&mut self, item: impl renzora_ui::StatusBarItem + 'static) -> &mut Self;

    /// Register an `InspectorEntry` directly.
    fn register_inspector(&mut self, entry: InspectorEntry) -> &mut Self;

    /// Register a component that implements `InspectableComponent` (from derive macro).
    /// Also registers the type for Bevy reflection so scripts can access it.
    fn register_inspectable<T: InspectableComponent + bevy::reflect::GetTypeRegistration>(&mut self) -> &mut Self;

    /// Register an `EntityPreset` with the `SpawnRegistry`.
    fn register_entity_preset(&mut self, preset: EntityPreset) -> &mut Self;

    /// Register a component icon for the hierarchy tree.
    fn register_component_icon(&mut self, entry: crate::ComponentIconEntry) -> &mut Self;

    /// Register a button on the viewport toolbar. See [`ToolEntry`].
    fn register_tool(&mut self, entry: ToolEntry) -> &mut Self;

    /// Register a plugin keyboard shortcut. See [`ShortcutEntry`].
    fn register_shortcut(&mut self, entry: ShortcutEntry) -> &mut Self;

    /// Register a header drawer that takes over the horizontal viewport
    /// toolbar when the viewport is in the given mode.
    fn register_mode_options(
        &mut self,
        mode: renzora_core::viewport_types::ViewportMode,
        drawer: crate::ModeOptionsDrawer,
    ) -> &mut Self;

    /// Register a header drawer for a specific [`ActiveTool`] (Photoshop-style
    /// tool-options bar).
    fn register_tool_options(
        &mut self,
        tool: crate::ActiveTool,
        drawer: crate::ToolOptionsDrawer,
    ) -> &mut Self;

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

    fn register_status_item(&mut self, item: impl renzora_ui::StatusBarItem + 'static) -> &mut Self {
        self.init_resource::<StatusBarRegistry>();
        self.world_mut().resource_mut::<StatusBarRegistry>().register(item);
        self
    }

    fn register_inspector(&mut self, entry: InspectorEntry) -> &mut Self {
        self.init_resource::<InspectorRegistry>();
        self.world_mut().resource_mut::<InspectorRegistry>().register(entry);
        self
    }

    fn register_inspectable<T: InspectableComponent + bevy::reflect::GetTypeRegistration>(&mut self) -> &mut Self {
        self.register_type::<T>();
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

    fn register_tool(&mut self, entry: ToolEntry) -> &mut Self {
        self.init_resource::<ToolbarRegistry>();
        self.world_mut().resource_mut::<ToolbarRegistry>().register(entry);
        self
    }

    fn register_mode_options(
        &mut self,
        mode: renzora_core::viewport_types::ViewportMode,
        drawer: crate::ModeOptionsDrawer,
    ) -> &mut Self {
        self.init_resource::<crate::ViewportModeOptionsRegistry>();
        self.world_mut()
            .resource_mut::<crate::ViewportModeOptionsRegistry>()
            .register(mode, drawer);
        self
    }

    fn register_tool_options(
        &mut self,
        tool: crate::ActiveTool,
        drawer: crate::ToolOptionsDrawer,
    ) -> &mut Self {
        self.init_resource::<crate::ToolOptionsRegistry>();
        self.world_mut()
            .resource_mut::<crate::ToolOptionsRegistry>()
            .register(tool, drawer);
        self
    }

    fn register_shortcut(&mut self, entry: ShortcutEntry) -> &mut Self {
        self.init_resource::<ShortcutRegistry>();
        // Seed KeyBindings.plugin_bindings with the default (no-op if user
        // has already customised this id), then store the entry so the
        // dispatcher + Settings UI can find it.
        self.world_mut()
            .resource_mut::<renzora_core::keybindings::KeyBindings>()
            .set_plugin_default(entry.id, entry.default_binding);
        self.world_mut().resource_mut::<ShortcutRegistry>().register(entry);
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
