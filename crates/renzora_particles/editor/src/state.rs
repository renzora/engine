//! The open effect: one `.vfx` document, edited in memory and pushed live.
//!
//! [`VfxEditorState::doc`] is the working copy every panel reads and writes.
//! Nothing writes the asset directly. Instead [`observe_edits`] notices that the
//! document changed (it compares against a shadow, the way the material
//! editor's undo observer does) and does the four things an edit means, in one
//! place, whichever panel made it:
//!
//! - **records an undo snapshot** on the document's own stack, merged per
//!   gesture so a scrub is one step;
//! - **pushes the document into `Assets<VfxAsset>`**, which fires
//!   `AssetEvent::Modified`, which makes the runtime rebuild every
//!   `ParticleSystem` using the file (the preview and the scene alike) at the
//!   time it was at, so an edit shows up in place rather than as a restart;
//! - **recompiles** every emitter for the error list and the node highlights;
//! - marks the document dirty, for the toolbar's indicator and Save.
//!
//! One observer rather than a call at each edit site is what keeps a new
//! editor (a curve popover, a texture drop) from forgetting one of the four.
//!
//! ## Saving
//!
//! Save writes the file and claims the write in `SelfWrites`. The asset watcher
//! then reloads it, and the reloaded asset is equal to the one already pushed,
//! so nothing visibly happens. Unsaved edits are saved when another effect is
//! opened in their place, as the material editor does on a document switch.

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_editor_framework::{DocTabKind, EditorContext};
use renzora_particles::asset::{parse_vfx, write_vfx, NodeId, VfxAsset, VfxGraph};
use renzora_particles::compile::{compile_graph, CompileError};

use crate::model;

/// The editor's working copy of the open effect.
#[derive(Resource, Default)]
pub struct VfxEditorState {
    /// Project-relative path of the open `.vfx`. `None` until one is opened.
    pub path: Option<String>,
    pub doc: VfxAsset,
    /// The effect as the asset server knows it: the same handle every
    /// `ParticleSystem` with this path resolves to, which is what makes an
    /// edit reach all of them.
    pub handle: Option<Handle<VfxAsset>>,
    /// The emitter whose tab is showing.
    pub active_emitter: usize,
    pub selected_node: Option<NodeId>,
    pub dirty: bool,
    /// Each emitter's compile errors, index-aligned with `doc.emitters`.
    pub errors: Vec<Vec<CompileError>>,
    /// Why the file could not be opened, shown in place of the editor.
    pub load_error: Option<String>,
    /// Seconds (editor clock) of the last edit, for the preview's
    /// restart-on-edit, which waits for a gesture to settle.
    pub last_edit: Option<f64>,
}

impl VfxEditorState {
    pub fn is_open(&self) -> bool {
        self.path.is_some()
    }

    pub fn emitter_index(&self) -> Option<usize> {
        (!self.doc.emitters.is_empty()).then(|| self.active_emitter.min(self.doc.emitters.len() - 1))
    }

    pub fn graph(&self) -> Option<&VfxGraph> {
        self.emitter_index().map(|i| &self.doc.emitters[i].graph)
    }

    pub fn graph_mut(&mut self) -> Option<&mut VfxGraph> {
        let i = self.emitter_index()?;
        Some(&mut self.doc.emitters[i].graph)
    }

    /// The active emitter's errors.
    pub fn active_errors(&self) -> &[CompileError] {
        self.emitter_index()
            .and_then(|i| self.errors.get(i))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The error messages naming `node` in the active emitter, joined.
    pub fn node_error(&self, node: NodeId) -> Option<String> {
        let messages: Vec<String> = self
            .active_errors()
            .iter()
            .filter(|e| e.node == Some(node))
            .map(|e| e.to_string())
            .collect();
        (!messages.is_empty()).then(|| messages.join("\n"))
    }

    /// One line for the panel's error strip: the first problem in the whole
    /// effect, with its emitter named when it is not the one on screen, and a
    /// count of the rest.
    pub fn error_summary(&self) -> Option<String> {
        let mut all = self
            .errors
            .iter()
            .enumerate()
            .flat_map(|(i, errs)| errs.iter().map(move |e| (i, e)));
        let (i, first) = all.next()?;
        let rest = all.count();
        let name = self.doc.emitters.get(i).map(|e| e.name.as_str()).unwrap_or("?");
        let mut line = if Some(i) == self.emitter_index() {
            first.to_string()
        } else {
            format!("{name}: {first}")
        };
        if rest > 0 {
            line.push_str(&format!(" (+{rest} more)"));
        }
        Some(line)
    }
}

/// Recompile every emitter. Cheap: a graph is a few dozen nodes.
pub fn compile_errors(doc: &VfxAsset) -> Vec<Vec<CompileError>> {
    doc.emitters
        .iter()
        .map(|e| compile_graph(&e.graph).err().unwrap_or_default())
        .collect()
}

/// What the observer last saw, so it can tell an edit from a document switch.
#[derive(Resource, Default)]
pub struct VfxShadow {
    path: Option<String>,
    doc: Option<VfxAsset>,
    /// The document has changed since it was last pushed into the asset.
    pending_push: bool,
    /// What the editor last wrote into the asset. An asset that differs from
    /// it was changed by someone else (a reload from disk); one that differs
    /// only from the *document* is just an edit not pushed yet.
    pushed: Option<VfxAsset>,
}

/// The `.vfx` the active document tab names, if it names one.
pub fn active_vfx_path(ctx: Option<&EditorContext>) -> Option<String> {
    match ctx? {
        EditorContext::Asset { path, kind: DocTabKind::Particle } if path.to_ascii_lowercase().ends_with(".vfx") => {
            Some(path.clone())
        }
        _ => None,
    }
}

/// Open the `.vfx` the active document tab names whenever that changes,
/// saving the outgoing effect first if it has unsaved edits.
pub fn load_active_document(world: &mut World) {
    let wanted = active_vfx_path(world.get_resource::<EditorContext>());
    let Some(path) = wanted else { return };
    if world.resource::<VfxEditorState>().path.as_deref() == Some(path.as_str()) {
        return;
    }
    if world.resource::<VfxEditorState>().dirty {
        save_document(world);
    }
    open_document(world, &path);
}

/// Load `path` (project-relative) into the editor.
pub fn open_document(world: &mut World, path: &str) {
    let fs_path = world.get_resource::<CurrentProject>().map(|p| p.resolve_path(path));
    let loaded = fs_path
        .as_ref()
        .ok_or_else(|| "no project is open".to_string())
        .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
        .and_then(|text| parse_vfx(&text).map_err(|e| e.to_string()));
    let handle: Handle<VfxAsset> = world.resource::<AssetServer>().load(path.to_string());

    let mut st = world.resource_mut::<VfxEditorState>();
    st.path = Some(path.to_string());
    st.handle = Some(handle);
    st.active_emitter = 0;
    st.selected_node = None;
    st.dirty = false;
    st.last_edit = None;
    match loaded {
        Ok(mut doc) => {
            // A file with no emitter or no output node still opens: the
            // repair is an edit like any other, so it marks the document dirty.
            let repaired = model::normalize(&mut doc);
            st.errors = compile_errors(&doc);
            st.doc = doc;
            st.load_error = None;
            st.dirty = repaired;
        }
        Err(e) => {
            warn!("[vfx_editor] could not open {path}: {e}");
            st.doc = VfxAsset::default();
            st.errors.clear();
            st.load_error = Some(e);
        }
    }
    let doc = st.doc.clone();
    let mut shadow = world.resource_mut::<VfxShadow>();
    shadow.path = Some(path.to_string());
    shadow.doc = Some(doc);
    // Push once even though the asset was just loaded from the same file: a
    // previous session's unsaved edits may still be sitting in it.
    shadow.pending_push = true;
    shadow.pushed = None;
}

/// Write the open effect to its file.
pub fn save_document(world: &mut World) {
    let (path, doc) = {
        let st = world.resource::<VfxEditorState>();
        let Some(path) = st.path.clone() else { return };
        if st.load_error.is_some() {
            // Never overwrite a file we failed to read with an empty effect.
            return;
        }
        (path, st.doc.clone())
    };
    let Some(fs_path) = world.get_resource::<CurrentProject>().map(|p| p.resolve_path(&path)) else {
        return;
    };
    let text = match write_vfx(&doc) {
        Ok(t) => t,
        Err(e) => {
            renzora::console_log::console_warn("Particles", format!("{path}: could not serialize: {e}"));
            return;
        }
    };
    if let Some(parent) = fs_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&fs_path, &text) {
        renzora::console_log::console_warn("Particles", format!("{path}: save failed: {e}"));
        return;
    }
    world
        .get_resource_or_insert_with(renzora::core::project_files::SelfWrites::default)
        .record(&fs_path, text.as_bytes());
    info!("[vfx_editor] saved {path}");
    world.resource_mut::<VfxEditorState>().dirty = false;
}

/// Turn document changes into undo steps, a live asset update and fresh
/// errors. See the module docs.
///
/// The comparison runs every frame rather than behind change detection: a
/// document is a few dozen nodes, and a missed change would be an edit that
/// neither reached the preview nor the undo stack.
pub fn observe_edits(world: &mut World) {
    let (path, differs) = {
        let st = world.resource::<VfxEditorState>();
        let shadow = world.resource::<VfxShadow>();
        if shadow.path != st.path {
            // A different document (or none): the open path reseeds it.
            (None, false)
        } else {
            (st.path.clone(), shadow.doc.as_ref() != Some(&st.doc))
        }
    };
    if path.is_none() {
        return;
    }
    if differs {
        let (before, after) = {
            let st = world.resource::<VfxEditorState>();
            let shadow = world.resource::<VfxShadow>();
            (shadow.doc.clone().unwrap_or_default(), st.doc.clone())
        };
        // Each step names its file. The Timeline edits whichever effect the
        // selection has, so one undo stack can hold steps for several files,
        // and a step must land on its own file whichever one is open by then
        // (and never merge with another file's step).
        let path = path.unwrap_or_default();
        let ctx = renzora_undo::active_context(world);
        renzora_undo::record(
            world,
            ctx,
            Box::new(renzora_undo::SnapshotCmd {
                label: "Particle Effect".to_string(),
                before: (path.clone(), before),
                after: (path.clone(), after.clone()),
                restore: restore_snapshot,
                merge_key: Some(format!("vfx-effect:{path}")),
            }),
        );
        let now = world.resource::<Time<Real>>().elapsed_secs_f64();
        let errors = compile_errors(&after);
        let mut st = world.resource_mut::<VfxEditorState>();
        st.errors = errors;
        st.dirty = true;
        st.last_edit = Some(now);
        let mut shadow = world.resource_mut::<VfxShadow>();
        shadow.doc = Some(after);
        shadow.pending_push = true;
    }
    push_to_asset(world);
}

/// Copy the document into `Assets<VfxAsset>` if it changed since the last
/// push. Waits, without dropping the push, while the asset is still loading.
fn push_to_asset(world: &mut World) {
    if !world.resource::<VfxShadow>().pending_push {
        return;
    }
    let (handle, doc) = {
        let st = world.resource::<VfxEditorState>();
        let Some(handle) = st.handle.clone() else { return };
        if st.load_error.is_some() {
            return;
        }
        (handle, st.doc.clone())
    };
    {
        let mut assets = world.resource_mut::<Assets<VfxAsset>>();
        let Some(mut asset) = assets.get_mut(&handle) else { return };
        if *asset != doc {
            *asset = doc.clone();
        }
    }
    let mut shadow = world.resource_mut::<VfxShadow>();
    shadow.pending_push = false;
    shadow.pushed = Some(doc);
}

/// The `restore` of the effect's undo snapshots: put the document back and
/// keep the shadow in step, so the restore is pushed live but not recorded as
/// a new edit. A step for a file that is no longer the open one opens it
/// first (saving the open one, as any switch does) rather than writing one
/// effect's content into another.
fn restore_snapshot(world: &mut World, snapshot: &(String, VfxAsset)) {
    let (path, doc) = snapshot;
    let open = world.resource::<VfxEditorState>().path.clone();
    if !path.is_empty() && open.as_deref() != Some(path.as_str()) {
        if world.resource::<VfxEditorState>().dirty {
            save_document(world);
        }
        open_document(world, path);
        if world.resource::<VfxEditorState>().load_error.is_some() {
            return;
        }
    }
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    {
        let mut st = world.resource_mut::<VfxEditorState>();
        st.doc = doc.clone();
        st.errors = compile_errors(doc);
        st.dirty = true;
        st.last_edit = Some(now);
        if st.active_emitter >= st.doc.emitters.len() {
            st.active_emitter = st.doc.emitters.len().saturating_sub(1);
        }
    }
    let mut shadow = world.resource_mut::<VfxShadow>();
    shadow.doc = Some(doc.clone());
    shadow.pending_push = true;
}

/// Pick up a change to the open file made outside the editor (a text editor,
/// a `git pull`): the asset server reloads it, and when the reloaded asset is
/// not what the editor last pushed, the editor adopts it. Unsaved edits win; a
/// warning says the file changed under them.
pub fn adopt_external_changes(
    mut events: MessageReader<AssetEvent<VfxAsset>>,
    assets: Res<Assets<VfxAsset>>,
    mut state: ResMut<VfxEditorState>,
    mut shadow: ResMut<VfxShadow>,
) {
    let Some(id) = state.handle.as_ref().map(|h| h.id()) else {
        events.clear();
        return;
    };
    let mut modified = false;
    for event in events.read() {
        if let AssetEvent::Modified { id: m } = event {
            modified |= *m == id;
        }
    }
    if !modified || shadow.pending_push {
        return;
    }
    let Some(asset) = assets.get(id) else { return };
    // Our own push (or the reload of our own save) leaves the asset equal to
    // what was pushed; before the first push there is nothing to compare.
    if shadow.pushed.as_ref().is_none_or(|p| p == asset) || *asset == state.doc {
        return;
    }
    if state.dirty {
        renzora::console_log::console_warn(
            "Particles",
            format!(
                "{} changed on disk while it had unsaved edits; keeping the edits (Save overwrites the file)",
                state.path.as_deref().unwrap_or("effect")
            ),
        );
        shadow.pending_push = true;
        return;
    }
    let mut doc = asset.clone();
    model::normalize(&mut doc);
    state.errors = compile_errors(&doc);
    state.doc = doc.clone();
    if state.active_emitter >= state.doc.emitters.len() {
        state.active_emitter = 0;
    }
    shadow.pushed = Some(asset.clone());
    shadow.doc = Some(doc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_particles::presets::PRESETS;

    fn state_with(doc: VfxAsset) -> VfxEditorState {
        VfxEditorState {
            path: Some("fx/test.vfx".into()),
            errors: compile_errors(&doc),
            doc,
            ..Default::default()
        }
    }

    #[test]
    fn presets_compile_cleanly() {
        for preset in PRESETS {
            let doc = parse_vfx(preset.source).unwrap();
            assert!(compile_errors(&doc).iter().all(Vec::is_empty), "{} has errors", preset.name);
        }
    }

    #[test]
    fn a_broken_node_is_named_by_the_error_lookup() {
        let mut doc = parse_vfx(PRESETS[1].source).unwrap();
        let g = &mut doc.emitters[0].graph;
        let out = g.nodes.iter().find(|n| n.node_type == "output/particle").unwrap().id;
        // A colour into a float pin is a type error the compiler pins on the
        // receiving node.
        let c = model::add_node(g, "math/color", [0.0, 0.0]).unwrap();
        model::connect(g, c, "value", out, "lifetime").unwrap();
        let st = state_with(doc);
        assert!(st.node_error(out).is_some(), "the output node should carry the error");
        assert!(st.error_summary().is_some());
    }

    #[test]
    fn the_summary_names_another_emitters_problem() {
        let mut doc = parse_vfx(PRESETS[0].source).unwrap();
        assert!(doc.emitters.len() > 1);
        doc.emitters[1].graph.nodes.push(renzora_particles::VfxNode::new(900, "math/teleport"));
        let st = state_with(doc);
        let line = st.error_summary().unwrap();
        assert!(line.starts_with(&st.doc.emitters[1].name), "{line}");
    }

    #[test]
    fn only_vfx_asset_tabs_are_opened() {
        let vfx = EditorContext::Asset { path: "a/b.vfx".into(), kind: DocTabKind::Particle };
        // An old `.particle` is converted and reopened as its `.vfx`
        // (`crate::legacy`), never loaded into the editor as itself.
        let old = EditorContext::Asset { path: "a/b.particle".into(), kind: DocTabKind::Particle };
        assert_eq!(active_vfx_path(Some(&vfx)).as_deref(), Some("a/b.vfx"));
        assert_eq!(active_vfx_path(Some(&old)), None);
        assert_eq!(active_vfx_path(Some(&EditorContext::Scene)), None);
    }
}
