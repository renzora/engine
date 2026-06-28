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
use renzora_ember::markup::MarkupSource;
use renzora_editor_framework::EditorSelection;
use renzora_ember::game_ui::HtmlTemplatePath;
use renzora_scripting::ScriptComponent;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Make the code editor follow entity selection: selecting an entity *replaces*
/// the open tabs with its editable source files — every file-backed script on a
/// `ScriptComponent`, plus the `.html` UI templates reachable from it (its own
/// `HtmlTemplatePath` and those of every descendant, so selecting a `UiCanvas`
/// opens all of its children's templates as tabs). One tab each, the first
/// focused. Switching entities is not additive — the previous entity's sources
/// are closed.
///
/// Gated on a *signature* of `(selected entity, its resolved source paths)` kept
/// in a `Local`, so this only acts when the selection — or that entity's source
/// set — actually changes, never every frame (which would pin `active_tab` and
/// stop you switching/closing tabs). Selecting an entity with no editable source
/// leaves the editor untouched rather than blanking it on every stray click, so
/// it keeps showing the last source-bearing entity you looked at.
///
/// Unsaved (modified) tabs survive the replace so in-progress edits are never
/// silently dropped — the same rule `close_all`/`close_others` follow.
fn sync_selection_sources(
    selection: Res<EditorSelection>,
    mut state: ResMut<CodeEditorState>,
    project: Option<Res<CurrentProject>>,
    assets: Res<AssetServer>,
    script_query: Query<&ScriptComponent>,
    template_query: Query<&HtmlTemplatePath>,
    children_query: Query<&Children>,
    markup_query: Query<&MarkupSource>,
    mut last_sig: Local<u64>,
) {
    use std::hash::{Hash, Hasher};

    let Some(project) = project else { return };
    let selected = selection.get();

    // Project-relative -> absolute, existing path. Same convention as
    // ScriptEngine::resolve_path / the markup loader.
    let resolve = |rel: std::path::PathBuf| -> Option<std::path::PathBuf> {
        let abs = if rel.is_absolute() {
            rel
        } else {
            project.path.join(rel)
        };
        abs.exists().then_some(abs)
    };

    let mut resolved: Vec<std::path::PathBuf> = Vec::new();

    // Scripts: every file-backed entry, in attachment order. `script_id`-only
    // entries (registered scripts with no file) have no editable source.
    if let Some(sc) = selected.and_then(|e| script_query.get(e).ok()) {
        for entry in &sc.scripts {
            if let Some(abs) = entry.script_path.clone().and_then(resolve) {
                if !resolved.contains(&abs) {
                    resolved.push(abs);
                }
            }
        }
    }

    // UI templates. At edit time the path lives on an `HtmlTemplatePath`
    // component held by each template *instance* entity; a `UiCanvas` is a bare
    // parent whose children carry them. So walk the selected entity and all its
    // descendants collecting every `HtmlTemplatePath` — selecting one instance
    // opens its template, selecting the canvas opens all of its children's
    // templates as tabs.
    if let Some(root) = selected {
        let mut queue = std::collections::VecDeque::from([root]);
        while let Some(e) = queue.pop_front() {
            if let Ok(tp) = template_query.get(e) {
                if let Some(abs) = resolve(std::path::PathBuf::from(&tp.0)) {
                    if !resolved.contains(&abs) {
                        resolved.push(abs);
                    }
                }
            }
            if let Ok(children) = children_query.get(e) {
                for child in children.iter() {
                    queue.push_back(child);
                }
            }
        }
    }

    // At runtime the markup loader expands a template into a tree of nodes that
    // each carry `MarkupSource` (but not `HtmlTemplatePath`). Selecting a built
    // node deep inside the UI resolves back to its template via the asset handle,
    // covering the case the downward `HtmlTemplatePath` walk above can't reach.
    if let Some(ms) = selected.and_then(|e| markup_query.get(e).ok()) {
        if let Some(abs) = assets
            .get_path(&ms.template_handle)
            .map(|p| p.path().to_path_buf())
            .and_then(resolve)
        {
            if !resolved.contains(&abs) {
                resolved.push(abs);
            }
        }
    }

    let mut h = std::collections::hash_map::DefaultHasher::new();
    selected.map(|e| e.to_bits()).hash(&mut h);
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

    // Replace the tab set with this entity's sources: drop every tab that isn't
    // one of them, except unsaved ones (kept so edits aren't lost).
    state
        .open_files
        .retain(|f| f.is_modified || resolved.iter().any(|p| *p == f.path));
    for p in &resolved {
        // open_file is idempotent — already-open sources just keep their tab.
        state.open_file(p.clone());
    }
    // Focus the FIRST source so the tabs read left-to-right in attachment order
    // (open_file leaves the last-opened tab active).
    if let Some(idx) = state.open_files.iter().position(|f| f.path == resolved[0]) {
        state.active_tab = Some(idx);
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Builds the code-editor's panel toolbar: a font-size scrubber bound to the
/// editor's live zoom, and Minimap / Whitespace toggle switches bound to the
/// editor settings. Spawned once by the shell's toolbar host (and shown only
/// while the code editor is the active dock tab).
pub(crate) fn build_code_editor_toolbar(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
) -> Entity {
    use renzora_editor_framework::EditorSettings;
    use renzora_ember::font::ui_font;
    use renzora_ember::reactive::bind_2way;
    use renzora_ember::theme::{border, header_bg, rgb, text_muted, value_text};
    use renzora_ember::widgets::{drag_value, toggle_switch, DragRange};

    let label = |commands: &mut Commands, text: &str| {
        commands
            .spawn((
                Text::new(text),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_muted())),
            ))
            .id()
    };

    // A full-width bar inside the code editor panel (below its tab strip).
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("code-editor-toolbar"),
        ))
        .id();

    // Font size → the editor's live zoom (`CodeEditorState.font_size`).
    let size_label = label(commands, "Size");
    let size = drag_value(commands, &fonts.ui, "", value_text(), 14.0, 1.0);
    commands.entity(size).insert(DragRange { min: 8.0, max: 48.0 });
    bind_2way(
        commands,
        size,
        |w| {
            w.get_resource::<CodeEditorState>()
                .map(|s| s.font_size)
                .unwrap_or(14.0)
        },
        |w, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<CodeEditorState>() {
                s.font_size = *v;
            }
        },
    );

    // Minimap + Whitespace → editor settings (toggle switches).
    let mini_label = label(commands, "Minimap");
    let mini = toggle_switch(commands, false);
    bind_2way(
        commands,
        mini,
        |w| {
            w.get_resource::<EditorSettings>()
                .map(|s| s.code_show_minimap)
                .unwrap_or(false)
        },
        |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<EditorSettings>() {
                s.code_show_minimap = *v;
            }
        },
    );

    let ws_label = label(commands, "Whitespace");
    let ws = toggle_switch(commands, false);
    bind_2way(
        commands,
        ws,
        |w| {
            w.get_resource::<EditorSettings>()
                .map(|s| s.code_show_whitespace)
                .unwrap_or(false)
        },
        |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<EditorSettings>() {
                s.code_show_whitespace = *v;
            }
        },
    );

    commands
        .entity(row)
        .add_children(&[size_label, size, mini_label, mini, ws_label, ws]);
    row
}

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
                sync_selection_sources,
                consume_open_code_editor_file,
                reload_saved_ui_templates,
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

/// Hot-reload the game UI when a `.html` template is saved in the editor.
///
/// Saving only writes the file to disk; nothing else changes by itself. To make
/// the running UI pick up the edit immediately, we force the asset server to
/// re-read the template from disk. That replaces the parsed `HtmlTemplate` in
/// `Assets<HtmlTemplate>` (same id) and emits `AssetEvent::Modified`, which the
/// markup plugin watches to despawn and rebuild every canvas using it. The
/// reload is explicit rather than relying on Bevy's file watcher so this works
/// regardless of whether watching is enabled.
///
/// `HtmlTemplatePath`/the loader address templates by their project-relative
/// path (e.g. `ui/health_bar.html`), so we strip the project root off the saved
/// absolute path and normalise to forward slashes — the same asset path the
/// template was loaded under, otherwise `reload` would target a different id.
fn reload_saved_ui_templates(
    mut state: ResMut<CodeEditorState>,
    project: Option<Res<CurrentProject>>,
    server: Res<AssetServer>,
    mut reloads: ResMut<renzora_ember::markup::TemplateReloadRequests>,
) {
    if state.recently_saved.is_empty() {
        return;
    }
    let saved = std::mem::take(&mut state.recently_saved);
    let Some(project) = project else { return };
    for path in saved {
        let is_html = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
            .unwrap_or(false);
        if !is_html {
            continue;
        }
        // Only templates under the project asset root have a loadable path.
        let Ok(rel) = path.strip_prefix(&project.path) else {
            continue;
        };
        let asset_path = rel.to_string_lossy().replace('\\', "/");
        info!("code_editor: hot-reloading saved UI template `{asset_path}`");
        // Register the asset id BEFORE the reload so the markup plugin treats the
        // resulting `Modified` as an intentional save (and rebuilds), not an
        // inspector writeback (which it ignores). The reload forces an immediate
        // disk re-read so we don't wait on the file watcher.
        reloads.request(&server, &asset_path);
        server.reload(asset_path);
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
