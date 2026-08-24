//! Material Editor — visual node graph for authoring PBR materials.
//!
//! Selection-driven: selecting a mesh entity in the viewport loads its material
//! into the graph editor. Edits auto-save to disk.

pub mod file_thumbnails;
mod material_inspector;
mod native_graph;
mod native_inspector;
mod native_material_ref;
mod native_preview;
mod pin_editors;
pub mod preview;

use bevy::prelude::*;
use renzora::content_problems::{ContentProblem, ContentProblems, ProblemSeverity};
use renzora::core::CurrentProject;
use renzora_editor_framework::{material_thumb_path, AppEditorExt, MaterialThumbnailRegistry};
use renzora_shader::material::graph::{MaterialDomain, MaterialGraph};
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::resolver::{MaterialCache, MaterialResolved};

/// What the material editor is currently doing.
#[derive(Clone, Debug)]
#[derive(Default)]
pub enum MaterialEditMode {
    /// No mesh entity selected (or selected entity has no mesh).
    #[default]
    Inactive,
    /// Entity has no MaterialRef yet — showing empty graph, will save on first edit.
    Pending { entity: Entity },
    /// Editing an existing .material file linked from a scene entity.
    Existing { path: String, entity: Entity },
    /// Asset-mode: editing a .material file standalone (opened via the asset
    /// browser, lives in a document tab). No entity context — saves write
    /// to `path` directly.
    EditingFile { path: String },
}


/// One material slot in the editor's tab bar. Selecting an entity populates the
/// tabs with every distinct material found in its subtree (deduped by file path),
/// so a multi-mesh model shows all its materials at once; switching tab loads
/// that material into `graph`.
#[derive(Clone, Debug)]
pub struct MaterialTab {
    /// The mesh entity this material is attached to. `None` for a standalone
    /// `.material` asset tab opened from the browser.
    pub entity: Option<Entity>,
    /// Project-relative `.material` path, or `None` for a mesh with no
    /// `MaterialRef` yet (a fresh graph that saves on first edit).
    pub path: Option<String>,
    /// Display label (material file stem, or the mesh's name).
    pub label: String,
}

/// Persistent editor state for the material editor.
#[derive(Resource)]
pub struct MaterialEditorState {
    /// The material graph currently being edited.
    pub graph: MaterialGraph,
    /// Which entity we're editing (follows EditorSelection).
    pub editing_entity: Option<Entity>,
    /// Current edit mode (Inactive / Pending / Existing).
    pub edit_mode: MaterialEditMode,
    /// Currently selected node (for the inspector).
    pub selected_node: Option<u64>,
    /// Last compiled WGSL (for preview / display).
    pub compiled_wgsl: Option<String>,
    /// Compilation errors (shown in UI).
    pub compile_errors: Vec<String>,
    /// True when graph has unsaved changes.
    pub is_dirty: bool,
    /// Material tabs for the current selection — one per distinct material in the
    /// selected entity's subtree. Drives the graph panel's tab bar.
    pub tabs: Vec<MaterialTab>,
    /// Index into `tabs` of the material currently loaded in `graph`.
    pub active_tab: Option<usize>,
}

impl Default for MaterialEditorState {
    fn default() -> Self {
        Self {
            graph: MaterialGraph::new("New Material", MaterialDomain::Surface),
            editing_entity: None,
            edit_mode: MaterialEditMode::Inactive,
            selected_node: None,
            compiled_wgsl: None,
            compile_errors: Vec::new(),
            is_dirty: false,
            tabs: Vec::new(),
            active_tab: None,
        }
    }
}

#[derive(Default)]
pub struct MaterialEditorPlugin;

impl Plugin for MaterialEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MaterialEditorPlugin");
        app.init_resource::<MaterialEditorState>();
        app.register_type::<Mesh3d>();
        app.add_plugins(preview::MaterialPreviewPlugin);
        app.add_plugins(file_thumbnails::MaterialFileThumbnailPlugin);
        app.add_plugins(pin_editors::MaterialPinEditors);
        app.add_plugins(native_inspector::NativeMaterialInspector);
        app.add_plugins(native_material_ref::NativeMaterialRef);
        app.add_plugins(native_preview::NativeMaterialPreview);
        app.add_plugins(native_graph::NativeMaterialGraph);

        // Register the material inspector entry
        app.register_inspector(material_inspector::material_entry());
    }
}

/// Save the current material graph to disk and invalidate the resolver cache.
/// Called from the Apply button in the graph panel toolbar.
pub fn apply_material(world: &mut World) {
    let path = {
        let state = world.resource::<MaterialEditorState>();
        match &state.edit_mode {
            MaterialEditMode::Existing { path, .. } => path.clone(),
            MaterialEditMode::EditingFile { path } => path.clone(),
            _ => return,
        }
    };

    let mut graph = world.resource::<MaterialEditorState>().graph.clone();
    if !save_material_graph(world, &path, &mut graph) {
        return;
    }
    // Mirror the freshly-saved graph back into editor state so UI sees the
    // embedded compile output (and a future diff doesn't think it's dirty).
    let mut state = world.resource_mut::<MaterialEditorState>();
    state.graph = graph;
    state.is_dirty = false;
}

/// Compile `graph`, write it — shader and all — to the project-relative
/// `path`, and invalidate every cache that holds the old version: resolver,
/// thumbnails, and the `MaterialResolved` marker on each entity using the
/// material.
///
/// Split out of [`apply_material`] so callers that edit a material *without* it
/// being the one open in the graph editor — the component inspector's texture
/// slots — go through exactly the same save, rather than a second
/// implementation that would drift on the next change to the compile pipeline.
///
/// Returns `false` (having written nothing) when there is no project open or
/// the write failed. `graph` is left carrying the compiled artifact that
/// compilation produced, so the caller can keep the saved copy.
pub fn save_material_graph(world: &mut World, path: &str, graph: &mut MaterialGraph) -> bool {
    let project_root = match world.get_resource::<CurrentProject>() {
        Some(p) => p.path.clone(),
        None => {
            warn!("[material_editor] No project open; cannot save material");
            return false;
        }
    };
    let fs_path = project_root.join(path);

    if let Some(parent) = fs_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let (graph_json, report) =
        match renzora_shader::material::precompiled::save_compiled_and_serialize(graph, &fs_path) {
            Ok(out) => out,
            Err(e) => {
                warn!("[material_editor] Save compile failed: {}", e);
                return false;
            }
        };
    for err in &report.errors {
        warn!("[material_editor] codegen error in '{}': {}", path, err);
    }
    note_save_warnings(world, path, &report.warnings);

    if let Err(e) = std::fs::write(&fs_path, &graph_json) {
        warn!("[material_editor] Save failed: {}", e);
        return false;
    }
    info!("[material_editor] Saved {}", path);

    // Invalidate resolver cache so the mesh picks up the new material
    world.resource_mut::<MaterialCache>().invalidate(path);

    // Invalidate the cached PNG thumbnail + registry entry so the asset
    // browser re-captures next time this material is visible.
    let material_abs = std::path::PathBuf::from(&fs_path);
    if let Some(project) = world.get_resource::<CurrentProject>().cloned() {
        let thumb = material_thumb_path(&material_abs, &project);
        let _ = std::fs::remove_file(&thumb);
    }
    if let Some(mut reg) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
        reg.invalidate(&material_abs);
    }

    // Remove MaterialResolved from entities using this path so resolver re-processes them
    let entities: Vec<Entity> = world
        .query_filtered::<(Entity, &MaterialRef), With<MaterialResolved>>()
        .iter(world)
        .filter(|(_, mr)| mr.0 == path)
        .map(|(e, _)| e)
        .collect();
    for entity in entities {
        world.entity_mut(entity).remove::<MaterialResolved>();
    }
    true
}

/// Console + Problems panel for a save-compile's warnings.
///
/// Compile sites own the path's Warning rows — `set_severity` replaces them
/// without touching whatever the validator has to say. An empty `warnings`
/// still runs: a repaired graph must clear last save's rows.
pub(crate) fn note_save_warnings(world: &mut World, path: &str, warnings: &[String]) {
    for warning in warnings {
        warn!("[material_editor] codegen warning in '{}': {}", path, warning);
    }
    let Some(mut problems) = world.get_resource_mut::<ContentProblems>() else {
        return;
    };
    problems.set_severity(
        path,
        ProblemSeverity::Warning,
        warnings
            .iter()
            .map(|w| ContentProblem {
                severity: ProblemSeverity::Warning,
                message: w.clone(),
                line: None,
            })
            .collect(),
    );
}

/// Load the graph at `path`, hand it to `edit`, and save it if `edit` changed
/// anything. The entry point for editing a material that isn't necessarily the
/// one open in the graph editor.
///
/// When the graph panel *does* have this material open its in-memory copy is
/// the source — it may hold unsaved node edits — and the result is written back
/// into it. Reading from disk in that case would silently discard the user's
/// unapplied work; writing to disk without syncing would leave the panel's copy
/// stale, ready to clobber the edit on its next Apply.
pub fn edit_material_graph(
    world: &mut World,
    path: &str,
    edit: impl FnOnce(&mut MaterialGraph) -> bool,
) -> bool {
    let open_here = matches!(
        &world.resource::<MaterialEditorState>().edit_mode,
        MaterialEditMode::Existing { path: p, .. } | MaterialEditMode::EditingFile { path: p }
            if p == path
    );

    let mut graph = if open_here {
        world.resource::<MaterialEditorState>().graph.clone()
    } else {
        let fs_path = match world.get_resource::<CurrentProject>() {
            Some(p) => p.resolve_path(path),
            None => return false,
        };
        match std::fs::read_to_string(&fs_path)
            .ok()
            .and_then(|j| serde_json::from_str::<MaterialGraph>(&j).ok())
        {
            Some(g) => g,
            None => {
                warn!("[material_editor] '{}' isn't a material graph; not editing", path);
                return false;
            }
        }
    };

    if !edit(&mut graph) {
        return false;
    }
    if !save_material_graph(world, path, &mut graph) {
        return false;
    }
    if open_here {
        let result = renzora_shader::material::codegen::compile(&graph);
        let mut state = world.resource_mut::<MaterialEditorState>();
        state.compiled_wgsl = Some(result.fragment_shader);
        state.compile_errors = result.errors;
        state.graph = graph;
        state.is_dirty = false;
    }
    true
}

renzora::add!(MaterialEditorPlugin, Editor);
