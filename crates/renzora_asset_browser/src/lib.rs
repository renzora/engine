mod native;
pub mod model_thumbnails;
pub mod thumbnails;

use bevy::prelude::*;
use renzora_editor_framework::EditorCommands;

/// Route a double-clicked asset to the right editor: scripts/shaders go to
/// the code editor, .material / .particle / .blueprint get their dedicated
/// layout. All recognized kinds also spawn a document tab. Unknown file
/// types fall through to the legacy code-editor "plain text" flow.
pub(crate) fn open_double_clicked(world: &bevy::prelude::World, path: std::path::PathBuf) {
    use renzora_editor_framework::DocTabKind;

    if let Some(kind) = asset_doc_kind(&path) {
        if let Some(cmds) = world.get_resource::<EditorCommands>() {
            let p = path.clone();
            cmds.push(move |world: &mut bevy::prelude::World| {
                // Scenes own a 3D world, so they can't just open an (empty) doc
                // tab — they must be loaded from disk into a new scene tab. Route
                // them to the scene system; every other kind opens as an asset tab.
                if matches!(kind, DocTabKind::Scene) {
                    world.insert_resource(renzora::core::OpenScenePathRequested(p));
                } else {
                    renzora_editor_framework::open_asset_tab(world, &p, kind);
                }
            });
        }
        return;
    }

    // Unrecognized kind — fall back to opening in code editor if it's a text-ish file.
    // `.ron` is intentionally absent: it's the engine's scene format and is
    // routed to a Scene doc tab via `asset_doc_kind` so the scene system
    // can load it instead of dumping the raw text into the code editor.
    let is_editable = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "rs" | "json" | "toml" | "yaml" | "yml" | "txt" | "md"
            )
        })
        .unwrap_or(false);
    if is_editable {
        if let Some(cmds) = world.get_resource::<EditorCommands>() {
            cmds.push(move |world: &mut bevy::prelude::World| {
                renzora_editor_framework::open_asset_tab(world, &path, DocTabKind::Script);
            });
        }
    }
}

/// Map a file path to the document tab kind it represents, or `None` if the
/// file doesn't correspond to a known editor-opening asset type.
fn asset_doc_kind(path: &std::path::Path) -> Option<renzora_editor_framework::DocTabKind> {
    use renzora_editor_framework::DocTabKind;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())?;
    if name.ends_with(".material_bp") || name.ends_with(".material") {
        return Some(DocTabKind::Material);
    }
    if name.ends_with(".particle") {
        return Some(DocTabKind::Particle);
    }
    if name.ends_with(".blueprint") || name.ends_with(".bp") {
        return Some(DocTabKind::Blueprint);
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "bsn" | "ron" => DocTabKind::Scene,
        "rhai" | "lua" | "js" | "ts" | "py" | "html" => DocTabKind::Script,
        "wgsl" | "glsl" | "vert" | "frag" => DocTabKind::Shader,
        _ => return None,
    })
}

/// Plugin that registers the asset browser with the editor.
#[derive(Default)]
pub struct AssetBrowserPlugin;

impl Plugin for AssetBrowserPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AssetBrowserPlugin");
        app.init_resource::<thumbnails::ThumbnailCache>()
            .add_systems(Update, thumbnails::update_thumbnail_cache)
            .add_plugins(model_thumbnails::ModelThumbnailPlugin);
        native::register_native_asset_browser(app);
    }
}

/// Fire an `AssetPathChanged` event via `EditorCommands` so scene entities
/// that reference the moved asset patch their stored paths. Paths are
/// computed asset-relative (to the current project) before the event fires.
pub(crate) fn emit_asset_path_change(
    world: &World,
    old_abs: &std::path::Path,
    new_abs: &std::path::Path,
    is_dir: bool,
) {
    let Some(project) = world.get_resource::<renzora::core::CurrentProject>() else {
        return;
    };
    let old_rel = project.make_asset_relative(old_abs);
    let new_rel = project.make_asset_relative(new_abs);
    if old_rel.is_empty() || new_rel.is_empty() || old_rel == new_rel {
        return;
    }

    let Some(cmds) = world.get_resource::<EditorCommands>() else {
        return;
    };
    cmds.push(move |world: &mut bevy::prelude::World| {
        world.trigger(renzora::core::AssetPathChanged {
            old: old_rel,
            new: new_rel,
            is_dir,
        });
    });
}

renzora::add!(AssetBrowserPlugin, Editor);
