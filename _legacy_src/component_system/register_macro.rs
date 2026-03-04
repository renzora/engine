//! Registration macro for unified component registration
//!
//! This macro provides a single point of registration for components,
//! automatically generating all required function pointers and handling
//! registration with multiple registries.

/// Register a component with all necessary registries using a single macro call
///
/// # Example
///
/// ```rust,ignore
/// register_component!(PhysicsBodyData {
///     type_id: "rigid_body",
///     display_name: "Rigid Body",
///     category: ComponentCategory::Physics,
///     icon: ATOM,
/// });
///
/// // With custom inspector:
/// register_component!(CameraNodeData {
///     type_id: "camera_3d",
///     display_name: "Camera3D",
///     category: ComponentCategory::Camera,
///     icon: VIDEO_CAMERA,
///     custom_inspector: my_custom_inspector_fn,
/// });
///
/// // With conflicts and requirements:
/// register_component!(PointLightData {
///     type_id: "point_light",
///     display_name: "Point Light",
///     category: ComponentCategory::Lighting,
///     icon: LIGHTBULB,
///     conflicts_with: ["directional_light", "spot_light"],
///     requires: [],
/// });
/// ```
#[macro_export]
macro_rules! register_component {
    // Main pattern with all options
    ($component:ty {
        type_id: $type_id:expr,
        display_name: $display_name:expr,
        category: $category:expr,
        icon: $icon:expr
        $(, priority: $priority:expr)?
        $(, conflicts_with: [$($conflict:expr),* $(,)?])?
        $(, requires: [$($req:expr),* $(,)?])?
        $(, custom_inspector: $inspector:expr)?
        $(, custom_add: $add_fn:expr)?
        $(, custom_remove: $remove_fn:expr)?
        $(, custom_serialize: $serialize_fn:expr)?
        $(, custom_deserialize: $deserialize_fn:expr)?
        $(, custom_script_properties: $script_props_fn:expr)?
        $(, custom_script_set: $script_set_fn:expr)?
        $(, custom_script_meta: $script_meta_fn:expr)?
        $(,)?
    }) => {
        $crate::component_system::register_macro::create_component_definition::<$component>(
            $type_id,
            $display_name,
            $category,
            $icon,
            register_component!(@priority $($priority)?),
            register_component!(@conflicts $($($conflict),*)?),
            register_component!(@requires $($($req),*)?),
            register_component!(@inspector $component $(, $inspector)?),
            register_component!(@add $component $(, $add_fn)?),
            register_component!(@remove $component $(, $remove_fn)?),
            register_component!(@serialize $component $(, $serialize_fn)?),
            register_component!(@deserialize $component $(, $deserialize_fn)?),
            register_component!(@script_properties $(, $script_props_fn)?),
            register_component!(@script_set $(, $script_set_fn)?),
            register_component!(@script_meta $(, $script_meta_fn)?),
        )
    };

    // Helper: Extract priority or default to 0
    (@priority) => { 0 };
    (@priority $priority:expr) => { $priority };

    // Helper: Build conflicts array
    (@conflicts) => { &[] };
    (@conflicts $($conflict:expr),*) => { &[$($conflict),*] };

    // Helper: Build requires array
    (@requires) => { &[] };
    (@requires $($req:expr),*) => { &[$($req),*] };

    // Helper: Use custom inspector or default auto-inspector
    (@inspector $component:ty) => {
        None::<$crate::component_system::InspectorFn>
    };
    (@inspector $component:ty, $inspector:expr) => {
        Some($inspector as $crate::component_system::InspectorFn)
    };

    // Helper: Use custom add or default
    (@add $component:ty) => {
        None::<$crate::component_system::AddComponentFn>
    };
    (@add $component:ty, $add_fn:expr) => {
        Some($add_fn as $crate::component_system::AddComponentFn)
    };

    // Helper: Use custom remove or default
    (@remove $component:ty) => {
        None::<$crate::component_system::RemoveComponentFn>
    };
    (@remove $component:ty, $remove_fn:expr) => {
        Some($remove_fn as $crate::component_system::RemoveComponentFn)
    };

    // Helper: Use custom serialize or default
    (@serialize $component:ty) => {
        None::<$crate::component_system::SerializeComponentFn>
    };
    (@serialize $component:ty, $serialize_fn:expr) => {
        Some($serialize_fn as $crate::component_system::SerializeComponentFn)
    };

    // Helper: Use custom deserialize or default
    (@deserialize $component:ty) => {
        None::<$crate::component_system::DeserializeComponentFn>
    };
    (@deserialize $component:ty, $deserialize_fn:expr) => {
        Some($deserialize_fn as $crate::component_system::DeserializeComponentFn)
    };

    // Helper: Use custom script properties or none
    (@script_properties) => {
        None::<$crate::component_system::GetScriptPropertiesFn>
    };
    (@script_properties, $script_props_fn:expr) => {
        Some($script_props_fn as $crate::component_system::GetScriptPropertiesFn)
    };

    // Helper: Use custom script set or none
    (@script_set) => {
        None::<$crate::component_system::SetScriptPropertyFn>
    };
    (@script_set, $script_set_fn:expr) => {
        Some($script_set_fn as $crate::component_system::SetScriptPropertyFn)
    };

    // Helper: Use custom script meta or none
    (@script_meta) => {
        None::<$crate::component_system::ScriptPropertyMetaFn>
    };
    (@script_meta, $script_meta_fn:expr) => {
        Some($script_meta_fn as $crate::component_system::ScriptPropertyMetaFn)
    };
}

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{de::DeserializeOwned, Serialize};

use super::{
    AddComponentFn, ComponentCategory, ComponentDefinition, DeserializeComponentFn,
    GetScriptPropertiesFn, HasComponentFn, InspectorFn, RemoveComponentFn,
    ScriptPropertyMetaFn, SerializeComponentFn, SetScriptPropertyFn,
};

/// Create a ComponentDefinition with automatic function generation
///
/// This is called by the register_component! macro
#[allow(clippy::too_many_arguments)]
pub fn create_component_definition<T>(
    type_id: &'static str,
    display_name: &'static str,
    category: ComponentCategory,
    icon: &'static str,
    priority: i32,
    conflicts_with: &'static [&'static str],
    requires: &'static [&'static str],
    custom_inspector: Option<InspectorFn>,
    custom_add: Option<AddComponentFn>,
    custom_remove: Option<RemoveComponentFn>,
    custom_serialize: Option<SerializeComponentFn>,
    custom_deserialize: Option<DeserializeComponentFn>,
    custom_script_properties: Option<GetScriptPropertiesFn>,
    custom_script_set: Option<SetScriptPropertyFn>,
    custom_script_meta: Option<ScriptPropertyMetaFn>,
) -> ComponentDefinition
where
    T: Component<Mutability = bevy::ecs::component::Mutable> + Default + Serialize + DeserializeOwned + 'static,
{
    ComponentDefinition {
        type_id,
        display_name,
        category,
        icon,
        priority,
        add_fn: custom_add.unwrap_or(default_add::<T>),
        remove_fn: custom_remove.unwrap_or(default_remove::<T>),
        has_fn: default_has::<T>,
        serialize_fn: custom_serialize.unwrap_or(default_serialize::<T>),
        deserialize_fn: custom_deserialize.unwrap_or(default_deserialize::<T>),
        inspector_fn: custom_inspector.unwrap_or(default_inspector::<T>),
        conflicts_with,
        requires,
        get_script_properties_fn: custom_script_properties,
        set_script_property_fn: custom_script_set,
        script_property_meta_fn: custom_script_meta,
    }
}

/// Default add function - inserts T::default()
fn default_add<T: Component + Default>(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(T::default());
}

/// Default remove function - removes T
fn default_remove<T: Component>(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<T>();
}

/// Default has function - checks if entity has T
fn default_has<T: Component>(world: &World, entity: Entity) -> bool {
    world.get::<T>(entity).is_some()
}

/// Default serialize function - uses serde_json
fn default_serialize<T: Component + Serialize>(
    world: &World,
    entity: Entity,
) -> Option<serde_json::Value> {
    let component = world.get::<T>(entity)?;
    serde_json::to_value(component).ok()
}

/// Default deserialize function - uses serde_json
fn default_deserialize<T: Component + DeserializeOwned>(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(component) = serde_json::from_value::<T>(data.clone()) {
        entity_commands.insert(component);
    }
}

/// Default inspector function - stub (all components should provide custom inspectors)
fn default_inspector<T: Component>(
    _ui: &mut egui::Ui,
    _world: &mut World,
    _entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    false
}

/// Builder for creating ComponentDefinition with customization
pub struct ComponentDefBuilder<T> {
    type_id: &'static str,
    display_name: &'static str,
    category: ComponentCategory,
    icon: &'static str,
    priority: i32,
    conflicts_with: &'static [&'static str],
    requires: &'static [&'static str],
    inspector_fn: Option<InspectorFn>,
    add_fn: Option<AddComponentFn>,
    remove_fn: Option<RemoveComponentFn>,
    serialize_fn: Option<SerializeComponentFn>,
    deserialize_fn: Option<DeserializeComponentFn>,
    script_properties_fn: Option<GetScriptPropertiesFn>,
    script_set_fn: Option<SetScriptPropertyFn>,
    script_meta_fn: Option<ScriptPropertyMetaFn>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> ComponentDefBuilder<T>
where
    T: Component<Mutability = bevy::ecs::component::Mutable> + Default + Serialize + DeserializeOwned + 'static,
{
    /// Create a new builder
    pub fn new(
        type_id: &'static str,
        display_name: &'static str,
        category: ComponentCategory,
        icon: &'static str,
    ) -> Self {
        Self {
            type_id,
            display_name,
            category,
            icon,
            priority: 0,
            conflicts_with: &[],
            requires: &[],
            inspector_fn: None,
            add_fn: None,
            remove_fn: None,
            serialize_fn: None,
            deserialize_fn: None,
            script_properties_fn: None,
            script_set_fn: None,
            script_meta_fn: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Set priority
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set conflicts
    pub fn conflicts_with(mut self, conflicts: &'static [&'static str]) -> Self {
        self.conflicts_with = conflicts;
        self
    }

    /// Set requirements
    pub fn requires(mut self, requires: &'static [&'static str]) -> Self {
        self.requires = requires;
        self
    }

    /// Set custom inspector function
    pub fn inspector(mut self, inspector: InspectorFn) -> Self {
        self.inspector_fn = Some(inspector);
        self
    }

    /// Set custom add function
    pub fn add_fn(mut self, add_fn: AddComponentFn) -> Self {
        self.add_fn = Some(add_fn);
        self
    }

    /// Set custom remove function
    pub fn remove_fn(mut self, remove_fn: RemoveComponentFn) -> Self {
        self.remove_fn = Some(remove_fn);
        self
    }

    /// Set custom serialize function
    pub fn serialize_fn(mut self, serialize_fn: SerializeComponentFn) -> Self {
        self.serialize_fn = Some(serialize_fn);
        self
    }

    /// Set custom deserialize function
    pub fn deserialize_fn(mut self, deserialize_fn: DeserializeComponentFn) -> Self {
        self.deserialize_fn = Some(deserialize_fn);
        self
    }

    /// Set custom script properties function
    pub fn script_properties(mut self, f: GetScriptPropertiesFn) -> Self {
        self.script_properties_fn = Some(f);
        self
    }

    /// Set custom script set function
    pub fn script_set(mut self, f: SetScriptPropertyFn) -> Self {
        self.script_set_fn = Some(f);
        self
    }

    /// Set script property meta function
    pub fn script_meta(mut self, f: ScriptPropertyMetaFn) -> Self {
        self.script_meta_fn = Some(f);
        self
    }

    /// Build the ComponentDefinition
    pub fn build(self) -> ComponentDefinition {
        create_component_definition::<T>(
            self.type_id,
            self.display_name,
            self.category,
            self.icon,
            self.priority,
            self.conflicts_with,
            self.requires,
            self.inspector_fn,
            self.add_fn,
            self.remove_fn,
            self.serialize_fn,
            self.deserialize_fn,
            self.script_properties_fn,
            self.script_set_fn,
            self.script_meta_fn,
        )
    }
}
