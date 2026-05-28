//! Component template registry — looks up `<stat_bar>` / `<menu_button>` etc.
//! by file stem to the loaded `HtmlTemplate` asset.
//!
//! Folder-loads `assets/ui/components/*.html` at startup, mirrors how
//! bevy_hui's `HuiAutoLoadPlugin` populates its `HtmlComponents` registry, but
//! ours stores the `HtmlTemplate` handle directly so the loader can fetch the
//! parsed AST without going through bevy_hui's runtime.

use bevy::asset::LoadedFolder;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_hui::prelude::HtmlTemplate;

/// Folders (relative to the asset root) scanned for component templates.
/// Any `.html` file in these folders is registered by its file stem and can
/// then be invoked as `<file_stem .../>` from other templates. We scan
/// several common locations so projects with a flat `templates/` layout work
/// alongside the engine's own `assets/ui/components/` convention.
const COMPONENT_DIRS: &[&str] = &[
    "ui/components",
    "templates/components",
    "templates",
];

#[derive(Resource, Default)]
pub struct ComponentRegistry {
    folders: Vec<Handle<LoadedFolder>>,
    by_name: HashMap<String, Handle<HtmlTemplate>>,
}

impl ComponentRegistry {
    pub fn handle_for(&self, name: &str) -> Option<&Handle<HtmlTemplate>> {
        self.by_name.get(name)
    }
}

fn start_loading_components(mut registry: ResMut<ComponentRegistry>, server: Res<AssetServer>) {
    for dir in COMPONENT_DIRS {
        registry.folders.push(server.load_folder(*dir));
    }
}

/// As each component folder finishes loading, index its `.html` assets in the
/// registry by file stem (`assets/ui/components/menu_button.html` →
/// `menu_button`).
fn index_loaded_components(
    mut registry: ResMut<ComponentRegistry>,
    mut events: MessageReader<AssetEvent<LoadedFolder>>,
    folders: Res<Assets<LoadedFolder>>,
) {
    let mut should_index = false;
    for ev in events.read() {
        if matches!(ev, AssetEvent::LoadedWithDependencies { .. } | AssetEvent::Modified { .. }) {
            should_index = true;
        }
    }
    if !should_index {
        return;
    }

    for folder_handle in registry.folders.clone() {
        let Some(folder) = folders.get(&folder_handle) else {
            continue;
        };
        for asset_handle in folder.handles.iter() {
            if let Ok(template_handle) = asset_handle.clone().try_typed::<HtmlTemplate>() {
                if let Some(path) = template_handle.path() {
                    let stem = path
                        .path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string());
                    if let Some(name) = stem {
                        registry.by_name.insert(name, template_handle);
                    }
                }
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ComponentRegistry>()
        .add_systems(Startup, start_loading_components)
        .add_systems(Update, index_loaded_components);
}
