pub mod highlight;
mod native_code_editor;
mod native_outline;
mod native_problems;
mod native_scripts;
pub mod outline;
pub mod scripts_on_entity;
pub mod state;

pub use state::*;

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_editor_framework::EditorSelection;
use renzora_scripting::ScriptComponent;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Make the code editor follow entity selection: selecting an entity *replaces*
/// the open tabs with exactly that entity's file-backed scripts (one tab each,
/// the first focused). Switching entities is not additive — the previous
/// entity's scripts are closed.
///
/// Gated on a *signature* of `(selected entity, its resolved script paths)` kept
/// in a `Local`, so this only acts when the selection — or the selected entity's
/// script set — actually changes, never every frame (which would pin `active_tab`
/// and stop you switching/closing tabs). Selecting an entity with *no* scripts
/// leaves the editor untouched rather than blanking it on every stray click, so
/// the editor keeps showing the last scripted entity you looked at.
///
/// Unsaved (modified) tabs survive the replace so in-progress edits are never
/// silently dropped — the same rule `close_all`/`close_others` follow.
fn sync_selection_scripts(
    selection: Res<EditorSelection>,
    mut state: ResMut<CodeEditorState>,
    project: Option<Res<CurrentProject>>,
    script_query: Query<&ScriptComponent>,
    mut last_sig: Local<u64>,
) {
    use std::hash::{Hash, Hasher};

    let Some(project) = project else { return };

    // Resolve the selected entity's file-backed scripts to absolute, existing
    // paths. `script_id`-only entries (registered scripts with no file) have no
    // editable source, so they're skipped.
    let resolved: Vec<std::path::PathBuf> = selection
        .get()
        .and_then(|e| script_query.get(e).ok())
        .map(|sc| {
            sc.scripts
                .iter()
                .filter_map(|entry| entry.script_path.as_ref())
                .map(|rel| {
                    // Project-relative path; same convention as
                    // ScriptEngine::resolve_path.
                    if rel.is_absolute() {
                        rel.clone()
                    } else {
                        project.path.join(rel)
                    }
                })
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default();

    let mut h = std::collections::hash_map::DefaultHasher::new();
    selection.get().map(|e| e.to_bits()).hash(&mut h);
    for p in &resolved {
        p.hash(&mut h);
    }
    let sig = h.finish();
    if sig == *last_sig {
        return;
    }
    *last_sig = sig;

    if resolved.is_empty() {
        return;
    }

    // Replace the tab set with this entity's scripts: drop every tab that isn't
    // one of them, except unsaved ones (kept so edits aren't lost).
    state
        .open_files
        .retain(|f| f.is_modified || resolved.iter().any(|p| *p == f.path));
    for p in &resolved {
        // open_file is idempotent — already-open scripts just keep their tab.
        state.open_file(p.clone());
    }
    // Focus the entity's FIRST script so the tabs read left-to-right in
    // attachment order (open_file leaves the last-opened tab active).
    if let Some(idx) = state.open_files.iter().position(|f| f.path == resolved[0]) {
        state.active_tab = Some(idx);
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct CodeEditorPlugin;

impl Plugin for CodeEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] CodeEditorPlugin");
        app.insert_resource(CodeEditorState::default());

        use renzora_editor_framework::SplashState;
        app.add_systems(
            Update,
            (
                sync_selection_scripts,
                consume_open_code_editor_file,
                sync_asset_filter_for_scripting,
                sync_code_editor_prefs_to_settings,
            )
                .run_if(in_state(SplashState::Editor)),
        );

        // Bevy-native (ember) code-editor panels for the bevy_ui shell. All
        // bind directly to the shared `CodeEditorState` resource.
        native_code_editor::register_native_code_editor(app);
        native_outline::register_native_outline(app);
        native_problems::register_native_problems(app);
        native_scripts::register_native_scripts_on_entity(app);
    }
}

/// Push the editor's view-preferences back into `EditorSettings` so the
/// settings overlay reflects toolbar toggles (and the values persist when
/// it's eventually serialised).
fn sync_code_editor_prefs_to_settings(
    editor_state: Res<CodeEditorState>,
    mut settings: ResMut<renzora_editor_framework::EditorSettings>,
) {
    if settings.code_show_minimap != editor_state.show_minimap {
        settings.code_show_minimap = editor_state.show_minimap;
    }
    if settings.code_show_whitespace != editor_state.show_whitespace {
        settings.code_show_whitespace = editor_state.show_whitespace;
    }
    if settings.code_auto_close_pairs != editor_state.auto_close_pairs {
        settings.code_auto_close_pairs = editor_state.auto_close_pairs;
    }
    if settings.code_trim_trailing_whitespace_on_save
        != editor_state.trim_trailing_whitespace_on_save
    {
        settings.code_trim_trailing_whitespace_on_save =
            editor_state.trim_trailing_whitespace_on_save;
    }
    if settings.mono_font != editor_state.mono_font {
        settings.mono_font = editor_state.mono_font.clone();
    }
}

/// Consume `OpenCodeEditorFile` resource inserted by other plugins (e.g. asset browser).
fn consume_open_code_editor_file(
    mut commands: Commands,
    request: Option<Res<renzora::core::OpenCodeEditorFile>>,
    mut state: ResMut<CodeEditorState>,
) {
    let Some(req) = request else { return };
    state.open_file(req.path.clone());
    commands.remove_resource::<renzora::core::OpenCodeEditorFile>();
}

/// While the Scripting workspace is active, restrict the asset browser to text
/// formats (scripts, shaders, configs). Reset when leaving so other workspaces
/// see the full asset list again.
fn sync_asset_filter_for_scripting(
    layout_mgr: Res<renzora_editor_framework::LayoutManager>,
    mut filter: ResMut<renzora_editor_framework::AssetBrowserExtensionFilter>,
) {
    let is_scripting = layout_mgr.active_name() == "Scripting";
    let desired: Option<Vec<String>> = if is_scripting {
        Some(
            [
                "lua", "rhai", "wgsl", "glsl", "json", "ron", "toml", "txt", "md",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        )
    } else {
        None
    };
    if filter.0 != desired {
        filter.0 = desired;
    }
}

renzora::add!(CodeEditorPlugin, Editor);
