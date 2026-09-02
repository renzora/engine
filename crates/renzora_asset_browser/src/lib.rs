//! The editor's Asset Browser: a toolbar and breadcrumb row over a wrapping
//! grid of folder/file tiles, with a folder tree beside it. Thumbnails,
//! drag-drop, rename/delete and the context menus all live here.
//!
//! The panel is split by region rather than by layer:
//!
//! - [`panel`] — `build`, the whole node tree, and the breadcrumbs
//! - [`grid`] — the cached listing, the tiles and the list rows
//! - [`tree`] — the folder tree (and, when narrow, the whole browser)
//! - [`state`] — [`state::NativeAssets`] and every marker component
//! - [`interact`] / [`menus`] / [`ops`] — clicks, menus, and the file operations
//! - [`selection`] / [`rename`] / [`drag_drop`] / [`layout`] — the gestures

mod drag_drop;
mod grid;
mod interact;
mod layout;
mod menus;
mod ops;
mod panel;
mod rename;
mod selection;
mod state;
mod tree;

pub mod model_thumbnails;
pub mod thumbnails;

use bevy::prelude::*;
use renzora_editor_framework::{EditorCommands, SplashState};
use renzora_ember::dock::panel_active;
use renzora_ember::panel::RegisterPanelContent;

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
        "lua" | "js" | "ts" | "py" => DocTabKind::Script,
        // Not `Script`. A `.html` is a UI template, and double-clicking one
        // dropped you in the *text* editor in the Scripting workspace — the
        // visual editor for it was reachable only by dragging the file onto the
        // viewport. It opens the UI workspace now; the code editor is still
        // tabbed beside the canvas there for anyone who wants the markup.
        "html" => DocTabKind::Ui,
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
            .init_resource::<thumbnails::FolderPreviews>()
            .add_systems(Update, thumbnails::update_thumbnail_cache)
            .add_plugins(model_thumbnails::ModelThumbnailPlugin);

        app.init_resource::<state::NativeAssets>();
        app.init_resource::<renzora::core::SelectAllClaimed>();
        app.init_resource::<renzora::core::AssetBrowserCwd>();
        app.init_resource::<renzora::core::AssetDropScrollRequest>();
        app.register_panel_content("assets", false, panel::build);
        // View systems — gated on the panel being the active (visible) tab. While
        // the Assets tab is a hidden background tab these stand down, so directory
        // scans, thumbnail loading and per-tile layout stop churning over entities
        // nobody can see (the bug where a backgrounded Assets tab held FPS at ~42).
        app.add_systems(
            Update,
            (
                interact::tile_click,
                interact::back_click,
                interact::search_sync,
                interact::request_thumbnails,
                thumbnails::scan_folder_previews,
                interact::tree_toggle_click,
                interact::tree_nav_click,
                interact::tree_tab_click,
                interact::tree_search_sync,
                menus::create_asset_click,
                menus::import_click,
                ops::crumb_click,
                state::load_persisted,
                ops::shortcut_click,
                menus::add_menu_open,
                menus::sort_menu_open,
                layout::view_toggle_click,
                layout::update_grid_layout,
                layout::responsive_layout,
            )
                .run_if(in_state(SplashState::Editor))
                .run_if(panel_active("assets")),
        );
        app.add_systems(
            Update,
            (menus::track_hover, menus::asset_context_menu)
                .chain()
                .run_if(in_state(SplashState::Editor))
                .run_if(panel_active("assets")),
        );
        app.add_systems(
            Update,
            layout::splitter_drag
                .run_if(in_state(SplashState::Editor))
                .run_if(panel_active("assets")),
        );
        // Drag lifecycle + ghost + spring are NOT gated on `panel_active("assets")`.
        // A drag begins from the Assets panel (so it's visible then), but the spring
        // below can switch the Assets leaf to a sibling tab mid-drag — which would
        // freeze a panel-gated `asset_drag`/`drag_ghost`, leaving the payload uncleared
        // and the ghost stranded on screen on release. Kept editor-gated only so the
        // whole drag (press → follow → release cleanup) always runs to completion.
        // `asset_drag_tab_spring` sets `FocusPanelRequest`, which the dock consumes
        // (at worst one frame later — imperceptible against the 0.35s dwell) through
        // the same in-place switch a tab click performs.
        // panel-systems-ungated: a drag STARTS here and continues over other panels; asset_drag_tab_spring springs other tabs open mid-drag
        app.add_systems(
            Update,
            (
                drag_drop::asset_drag,
                drag_drop::drag_ghost,
                drag_drop::asset_drag_tab_spring,
            )
                .run_if(in_state(SplashState::Editor)),
        );
        app.add_systems(
            Update,
            (
                (grid::refresh_listing, selection::track_visible_order).chain(),
                selection::marquee_select,
                selection::marquee_overlay,
                selection::marquee_autoscroll,
            )
                .run_if(in_state(SplashState::Editor))
                .run_if(panel_active("assets")),
        );
        app.add_systems(
            Update,
            (
                rename::rename_shortcut,
                rename::rename_arm_fire,
                rename::focus_asset_rename,
                rename::asset_rename_commit,
            )
                .run_if(in_state(SplashState::Editor))
                .run_if(panel_active("assets")),
        );
        // Kept ungated: it publishes `SelectAllClaimed` every frame from grid-hover,
        // and the hierarchy reads that flag to decide whether to handle Ctrl+A. If we
        // froze it while the panel was hidden, the flag could stay stuck "claimed"
        // and swallow the hierarchy's select-all. It's a single cheap query.
        // panel-systems-ungated: TODO review: select-all is panel-scoped and probably gateable, left alone to avoid churn without a test
        app.add_systems(
            Update,
            selection::select_all_shortcut.run_if(in_state(SplashState::Editor)),
        );
        // Publish the current folder so drag-and-drop imports (handled in
        // renzora_import_ui) target the folder on screen, not the importer default.
        // Ungated by panel visibility — it's a single cheap resource write, and the
        // drop handler needs a fresh value even if the Assets tab isn't the active
        // one when the file lands.
        // panel-systems-ungated: publish_cwd feeds renzora_import_ui's drop-target folder — a different crate reads it
        app.add_systems(
            Update,
            ops::publish_cwd.run_if(in_state(SplashState::Editor)),
        );
        // After a drop-import, pin the grid to the bottom for a short window so the
        // freshly-copied file scrolls into view once the rescan surfaces it.
        // panel-systems-ungated: scroll-on-drop must land after an import that may complete while focus moved
        app.add_systems(
            Update,
            drag_drop::scroll_grid_on_drop.run_if(in_state(SplashState::Editor)),
        );
        // PreUpdate (after input is collected) so it can consume Delete before the
        // gizmo's Update entity-delete sees the press. After `UiSystems::Focus` so
        // the grid's `cursor_over` is fresh this frame.
        // panel-systems-ungated: consumes Delete in PreUpdate before the gizmo's entity-delete sees it; the ordering is the point
        app.add_systems(
            PreUpdate,
            rename::asset_delete_shortcut
                .after(bevy::input::InputSystems)
                .after(bevy::ui::UiSystems::Focus)
                .run_if(in_state(SplashState::Editor)),
        );
        // Force the resize cursor while hovering/dragging the divider. In PostUpdate
        // so it wins over renzora_hui's Update cursor system (which would otherwise
        // reset to Default once the cursor leaves the thin splitter mid-drag).
        app.add_systems(
            PostUpdate,
            layout::divider_cursor
                .run_if(in_state(SplashState::Editor))
                .run_if(panel_active("assets")),
        );
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
