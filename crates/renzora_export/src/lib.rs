//! Export overlay for packaging Renzora projects into distributable builds.
//!
//! Provides a modal overlay with export settings (platform, packaging mode,
//! window config, icon, etc.) and handles packing assets into `.rpak` archives
//! using pre-built runtime templates.

#[cfg(not(target_arch = "wasm32"))]
mod apk_signer;
#[cfg(not(target_arch = "wasm32"))]
mod build;
/// Wrapping a finished export into an `.AppImage` or a `.app`.
mod bundle;
#[cfg(not(target_arch = "wasm32"))]
mod capabilities;
#[cfg(not(target_arch = "wasm32"))]
mod docker;
#[cfg(not(target_arch = "wasm32"))]
mod download;
#[cfg(not(target_arch = "wasm32"))]
mod icon;
#[cfg(not(target_arch = "wasm32"))]
mod dialog;
#[cfg(not(target_arch = "wasm32"))]
mod overlay;
#[cfg(not(target_arch = "wasm32"))]
mod presets;
#[cfg(not(target_arch = "wasm32"))]
mod templates;
#[cfg(not(target_arch = "wasm32"))]
mod toolchain;
#[cfg(not(target_arch = "wasm32"))]
mod upx;
/// The web lane's post-build chain (bindgen / `wasm-opt` / host page). Named for
/// the platform it targets, not the one it runs on — like every other module
/// here it is desktop-only, because exporting is a desktop operation.
#[cfg(not(target_arch = "wasm32"))]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use overlay::ExportOverlayState;
#[cfg(not(target_arch = "wasm32"))]
pub use presets::ExportPreset;
#[cfg(not(target_arch = "wasm32"))]
pub use templates::{ExportTemplate, Platform, TemplateManager};

use bevy::prelude::*;

#[derive(Default)]
pub struct ExportPlugin;

impl Plugin for ExportPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] ExportPlugin");
        #[cfg(not(target_arch = "wasm32"))]
        {
            _app.init_resource::<ExportOverlayState>()
                .init_resource::<TemplateManager>()
                .add_systems(Update, export_open_on_request);
            dialog::register(_app);
        }
    }
}

/// Backend-agnostic orchestration: open the export modal when the editor menu
/// fires [`ExportRequested`]. The modal in [`dialog`] renders the UI and
/// polls the worker while visible.
#[cfg(not(target_arch = "wasm32"))]
fn export_open_on_request(
    mut requested: Option<ResMut<ExportOverlayState>>,
    marker: Option<Res<renzora::core::ExportRequested>>,
    mut commands: Commands,
) {
    if marker.is_some() {
        if let Some(state) = requested.as_mut() {
            state.visible = true;
        }
        commands.remove_resource::<renzora::core::ExportRequested>();
    }
}

renzora::add!(ExportPlugin, Editor);
