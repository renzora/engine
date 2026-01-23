//! Plugin traits that plugins must implement.

use crate::abi::{PluginError, PluginManifest};
use crate::api::EditorApi;
use crate::events::EditorEvent;

// Re-export bevy types for plugins
pub use bevy::app::App;
pub use bevy::ecs::world::World;
pub use bevy::ecs::system::Commands;

/// Main plugin trait - all plugins must implement this.
///
/// This trait defines the lifecycle and interface for editor plugins.
/// Plugins can extend the editor with new functionality like script engines,
/// custom gizmos, inspectors, and panels.
///
/// # Bevy Integration
///
/// Plugins have direct access to Bevy's World and can register systems:
///
/// ```rust,ignore
/// fn build(&self, app: &mut App) {
///     app.add_systems(Update, my_custom_system);
///     app.insert_resource(MyResource::default());
/// }
///
/// fn on_world_update(&mut self, world: &mut World) {
///     // Direct world manipulation
///     let mut query = world.query::<(&Transform, &MyComponent)>();
///     for (transform, comp) in query.iter(world) {
///         // ...
///     }
/// }
/// ```
pub trait EditorPlugin: Send + Sync {
    /// Return the plugin manifest with metadata
    fn manifest(&self) -> PluginManifest;

    /// Called during App building to register Bevy systems and resources.
    ///
    /// This is where you add systems, resources, events, and any other
    /// Bevy configuration your plugin needs.
    ///
    /// ```rust,ignore
    /// fn build(&self, app: &mut App) {
    ///     app.add_systems(Update, my_system);
    ///     app.insert_resource(MyResource::default());
    ///     app.add_event::<MyEvent>();
    /// }
    /// ```
    fn build(&self, _app: &mut App) {
        // Default: no systems to register
    }

    /// Called when the plugin is loaded.
    /// Use this to register UI elements like panels, menus, status bar items.
    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError>;

    /// Called when the plugin is about to be unloaded.
    /// Clean up any resources here.
    fn on_unload(&mut self, api: &mut dyn EditorApi);

    /// Called every frame for UI updates.
    /// Use this for updating panels, status bar, etc.
    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32);

    /// Called every frame with direct World access.
    ///
    /// Use this for operations that need direct ECS access:
    /// - Querying components
    /// - Spawning/despawning entities
    /// - Modifying resources
    /// - Drawing gizmos
    ///
    /// ```rust,ignore
    /// fn on_world_update(&mut self, world: &mut World) {
    ///     // Query entities
    ///     let mut query = world.query::<(&Transform, &Velocity)>();
    ///     for (transform, vel) in query.iter(world) {
    ///         // Draw debug visualization
    ///     }
    ///
    ///     // Access resources
    ///     if let Some(mut gizmos) = world.get_resource_mut::<GizmoBuffers>() {
    ///         gizmos.line(start, end, Color::RED);
    ///     }
    /// }
    /// ```
    fn on_world_update(&mut self, _world: &mut World) {
        // Default: no world operations
    }

    /// Called when an editor event occurs.
    /// Only receives events the plugin subscribed to.
    fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent);
}

/// Type alias for the plugin creation function.
/// This is the extern "C" function that plugins export.
pub type CreatePluginFn = unsafe extern "C" fn() -> *mut dyn EditorPlugin;
