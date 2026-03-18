//! Plugin Host Library
//!
//! Core infrastructure for loading and managing editor plugins via FFI.
//! Plugins are dynamic libraries (.dll/.so/.dylib) that implement the `editor_plugin_api`
//! interface. This crate provides the host-side loading, lifecycle management, and
//! event dispatching. Bevy integration systems live in `renzora_editor`.
//!
//! Disabled on WASM (no dynamic library loading).

pub mod abi;
pub mod api;
pub mod dependency;
#[cfg(not(target_arch = "wasm32"))]
pub mod host;
pub mod registry;

pub use abi::{EntityIdExt, PluginTransformExt};
pub use api::EditorApiImpl;
#[cfg(not(target_arch = "wasm32"))]
pub use host::{DisabledPlugin, PluginHost, PluginSource};
pub use registry::PluginRegistry;
