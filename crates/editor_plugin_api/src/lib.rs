//! Editor Plugin API
//!
//! This crate provides the API for creating plugins for the Bevy Editor.
//! Plugin authors should depend on this crate to build their plugins.
//!
//! # Example
//!
//! ```rust,ignore
//! use editor_plugin_api::prelude::*;
//!
//! pub struct MyPlugin {
//!     counter: i32,
//! }
//!
//! impl EditorPlugin for MyPlugin {
//!     fn manifest(&self) -> PluginManifest {
//!         PluginManifest::new("com.example.my-plugin", "My Plugin", "1.0.0")
//!             .author("Your Name")
//!             .description("A custom editor plugin")
//!             .capability(PluginCapability::Panel)
//!     }
//!
//!     fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
//!         api.log_info("My plugin loaded!");
//!
//!         // Register a panel
//!         api.register_panel(PanelDefinition::new("my_panel", "My Panel")
//!             .location(PanelLocation::Right)
//!             .min_size(200.0, 100.0));
//!
//!         Ok(())
//!     }
//!
//!     fn on_unload(&mut self, api: &mut dyn EditorApi) {
//!         api.log_info("My plugin unloaded!");
//!     }
//!
//!     fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
//!         // Update panel content
//!         api.set_panel_content("my_panel", vec![
//!             Widget::label("Hello from plugin!"),
//!             Widget::button("Click me", UiId(1)),
//!         ]);
//!     }
//!
//!     fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent) {
//!         if let EditorEvent::UiEvent(UiEvent::ButtonClicked(id)) = event {
//!             if id.0 == 1 {
//!                 self.counter += 1;
//!                 api.log_info(&format!("Button clicked {} times!", self.counter));
//!             }
//!         }
//!     }
//! }
//!
//! // Export the plugin entry point
//! declare_plugin!(MyPlugin, MyPlugin { counter: 0 });
//! ```

pub mod abi;
pub mod api;
pub mod events;
pub mod traits;
pub mod ui;

pub mod prelude;

// Re-export core types at crate root
pub use abi::*;
pub use api::*;
pub use events::*;
pub use traits::*;
pub use ui::*;

/// Macro for plugin authors to declare their plugin entry point.
///
/// This creates the `create_plugin` function that the editor calls to instantiate the plugin.
///
/// # Example
///
/// ```rust,ignore
/// declare_plugin!(MyPlugin, MyPlugin::new());
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:expr) => {
        #[no_mangle]
        pub extern "C" fn create_plugin() -> *mut dyn $crate::EditorPlugin {
            let plugin: Box<dyn $crate::EditorPlugin> = Box::new($constructor);
            Box::into_raw(plugin)
        }
    };
}
