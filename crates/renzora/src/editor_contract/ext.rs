//! Extension trait for `App` to simplify editor registrations.

use super::inspector_registry::{InspectorEntry, InspectorRegistry};
use super::shortcut_registry::{ShortcutEntry, ShortcutRegistry};
use super::spawn_registry::{EntityPreset, SceneStarter, SceneStarterRegistry, SpawnRegistry};
use super::toolbar_registry::{ToolEntry, ToolbarRegistry};
use bevy::prelude::*;

/// Trait implemented by components that can auto-generate their `InspectorEntry`.
///
/// Derived via `#[derive(Inspectable)]` or `#[post_process]`.
pub trait InspectableComponent: Component + Default + 'static {
    fn inspector_entry() -> InspectorEntry;
}

/// Extension methods on `App` for common editor registrations.
pub trait AppEditorExt {
    /// Register an `InspectorEntry` directly.
    fn register_inspector(&mut self, entry: InspectorEntry) -> &mut Self;

    /// Register a component that implements `InspectableComponent` (from derive macro).
    /// Also registers the type for Bevy reflection so scripts can access it.
    fn register_inspectable<T: InspectableComponent + bevy::reflect::GetTypeRegistration>(
        &mut self,
    ) -> &mut Self;

    /// Register an `EntityPreset` with the `SpawnRegistry`.
    fn register_entity_preset(&mut self, preset: EntityPreset) -> &mut Self;

    /// Register a starter template shown on the hierarchy panel's empty-state
    /// picker. Each starter spawns a small pre-built scene (environment, UI,
    /// physics arena, etc.) with a single click.
    fn register_scene_starter(&mut self, starter: SceneStarter) -> &mut Self;

    /// Register a component icon for the hierarchy tree.
    fn register_component_icon(&mut self, entry: super::spawn_registry::ComponentIconEntry) -> &mut Self;

    /// Register a button on the viewport toolbar. See [`ToolEntry`].
    fn register_tool(&mut self, entry: ToolEntry) -> &mut Self;

    /// Register a native (bevy_ui) inspector drawer for a component `type_id`,
    /// for inspectors that need more than declarative `fields` (conditional rows,
    /// buttons, custom widgets). See [`NativeInspectorDrawer`].
    fn register_native_inspector_ui(
        &mut self,
        type_id: &'static str,
        drawer: super::inspector_registry::NativeInspectorDrawer,
    ) -> &mut Self;

    /// Register a plugin keyboard shortcut. See [`ShortcutEntry`].
    fn register_shortcut(&mut self, entry: ShortcutEntry) -> &mut Self;

    /// Attribute GPU render passes whose name starts with `pass_prefix` to the
    /// live entities carrying component `C` (shown, labelled `noun`, in the
    /// editor's GPU Pass Breakdown). Lets a plugin that adds its own GPU render
    /// pass surface *what* drives it without the debugger hardcoding the mapping.
    /// See [`GpuPassSourceRegistry`].
    fn register_gpu_pass_source<C: Component>(
        &mut self,
        pass_prefix: impl Into<std::borrow::Cow<'static, str>>,
        noun: impl Into<std::borrow::Cow<'static, str>>,
    ) -> &mut Self;

    /// Convenience: register a post-process effect's type, plugin, and inspector entry.
    fn add_post_process<T>(&mut self) -> &mut Self
    where
        T: crate::postprocess::PostProcessEffect
            + InspectableComponent
            + bevy::reflect::GetTypeRegistration;
}

impl AppEditorExt for App {
    fn register_inspector(&mut self, entry: InspectorEntry) -> &mut Self {
        self.init_resource::<InspectorRegistry>();
        self.world_mut()
            .resource_mut::<InspectorRegistry>()
            .register(entry);
        self
    }

    fn register_inspectable<T: InspectableComponent + bevy::reflect::GetTypeRegistration>(
        &mut self,
    ) -> &mut Self {
        self.register_type::<T>();
        self.register_inspector(T::inspector_entry())
    }

    fn register_entity_preset(&mut self, preset: EntityPreset) -> &mut Self {
        self.init_resource::<SpawnRegistry>();
        self.world_mut()
            .resource_mut::<SpawnRegistry>()
            .register(preset);
        self
    }

    fn register_scene_starter(&mut self, starter: SceneStarter) -> &mut Self {
        self.init_resource::<SceneStarterRegistry>();
        self.world_mut()
            .resource_mut::<SceneStarterRegistry>()
            .register(starter);
        self
    }

    fn register_component_icon(&mut self, entry: super::spawn_registry::ComponentIconEntry) -> &mut Self {
        self.init_resource::<super::spawn_registry::ComponentIconRegistry>();
        self.world_mut()
            .resource_mut::<super::spawn_registry::ComponentIconRegistry>()
            .register(entry);
        self
    }

    fn register_tool(&mut self, entry: ToolEntry) -> &mut Self {
        self.init_resource::<ToolbarRegistry>();
        self.world_mut()
            .resource_mut::<ToolbarRegistry>()
            .register(entry);
        self
    }

    fn register_native_inspector_ui(
        &mut self,
        type_id: &'static str,
        drawer: super::inspector_registry::NativeInspectorDrawer,
    ) -> &mut Self {
        self.init_resource::<super::inspector_registry::NativeInspectorRegistry>();
        self.world_mut()
            .resource_mut::<super::inspector_registry::NativeInspectorRegistry>()
            .register(type_id, drawer);
        self
    }

    fn register_shortcut(&mut self, entry: ShortcutEntry) -> &mut Self {
        self.init_resource::<ShortcutRegistry>();
        // Seed KeyBindings.plugin_bindings with the default (no-op if user
        // has already customised this id), then store the entry so the
        // dispatcher + Settings UI can find it.
        self.world_mut()
            .resource_mut::<crate::keybindings::KeyBindings>()
            .set_plugin_default(entry.id, entry.default_binding);
        self.world_mut()
            .resource_mut::<ShortcutRegistry>()
            .register(entry);
        self
    }

    fn register_gpu_pass_source<C: Component>(
        &mut self,
        pass_prefix: impl Into<std::borrow::Cow<'static, str>>,
        noun: impl Into<std::borrow::Cow<'static, str>>,
    ) -> &mut Self {
        // Resolve (and register, if new) the component's stable ComponentId now,
        // at plugin-build time, so the breakdown can count its entities later.
        let component = self.world_mut().register_component::<C>();
        self.init_resource::<super::gpu_pass_registry::GpuPassSourceRegistry>();
        self.world_mut()
            .resource_mut::<super::gpu_pass_registry::GpuPassSourceRegistry>()
            .register(super::gpu_pass_registry::GpuPassSource {
                prefix: pass_prefix.into(),
                component,
                noun: noun.into(),
            });
        self
    }

    fn add_post_process<T>(&mut self) -> &mut Self
    where
        T: crate::postprocess::PostProcessEffect
            + InspectableComponent
            + bevy::reflect::GetTypeRegistration,
    {
        self.register_type::<T>();
        self.add_plugins(crate::postprocess::PostProcessPlugin::<T>::default());
        self.register_inspectable::<T>();
        self
    }
}
