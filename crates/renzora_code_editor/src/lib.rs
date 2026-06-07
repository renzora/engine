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

/// When the selected entity has scripts attached, auto-open them in the code editor.
fn sync_selection_scripts(
    selection: Res<EditorSelection>,
    mut state: ResMut<CodeEditorState>,
    project: Option<Res<CurrentProject>>,
    script_query: Query<&ScriptComponent>,
) {
    let Some(entity) = selection.get() else {
        return;
    };
    let Ok(sc) = script_query.get(entity) else {
        return;
    };
    let Some(project) = project else { return };

    for entry in &sc.scripts {
        if let Some(ref rel_path) = entry.script_path {
            // Project-relative path; same convention as
            // ScriptEngine::resolve_path.
            let resolved = if rel_path.is_absolute() {
                rel_path.clone()
            } else {
                project.path.join(rel_path)
            };
            if resolved.exists() {
                // open_file is idempotent — if already open it just switches tab
                state.open_file(resolved);
            }
        }
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
