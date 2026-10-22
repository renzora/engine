//! The effect editor's side of the animation timeline's Particles group.
//!
//! The Timeline panel (`renzora_animation_editor`) shows and edits an effect's
//! keyed parameters ([`renzora_particles::params`]). It does not write the
//! asset itself: every edit comes through [`edit_effect`], which applies it to
//! this crate's working copy ([`VfxEditorState::doc`]), opening the effect
//! there first when it is not the open one. That one route is what keeps the
//! two editors in step and gives a timeline edit everything an edit in the
//! graph gets, from the same observer: an undo step, a live push into
//! `Assets<VfxAsset>` (so the viewport and the preview follow), the dirty dot
//! and Save. A second, timeline-only route into the asset would have needed
//! its own undo, its own save and its own reconciliation with an open
//! document, and two copies of an effect being edited is exactly the state
//! that loses edits.
//!
//! Opening the effect does not open a document tab. The effect editor follows
//! the active tab only when that tab is a `.vfx`, and the timeline edits a
//! scene effect only when it is not (on a `.vfx` tab it edits that tab's effect
//! through the preview), so the two never pull the working copy back and forth.
//!
//! The other direction, the effect editor asking for the timeline, is
//! [`TimelineLink`]: the right-click "Animate" on a value field
//! ([`crate::animate`]) names the track it made, and the timeline selects it.

use bevy::prelude::*;

use renzora_particles::asset::VfxAsset;

use crate::state::{self, VfxEditorState};

pub use crate::preview::PreviewEffect;

/// What the timeline and the effect editor tell each other.
#[derive(Resource, Default)]
pub struct TimelineLink {
    /// The effect the timeline is showing (project-relative path) and its
    /// playhead, published by the timeline every frame. "Animate" keys at that
    /// time when the effect is the one shown, and at 0 otherwise.
    pub shown: Option<(String, f32)>,
    /// A track the timeline should select once it shows `path`: set by
    /// "Animate", taken by the timeline.
    pub focus: Option<(String, usize)>,
    /// The effect entity whose clock the timeline's playhead holds. The
    /// preview leaves its own speed and restarts alone while it is this one,
    /// or the two would take turns moving the effect.
    pub driving: Option<Entity>,
}

/// The id the Timeline panel registers under.
pub const TIMELINE_PANEL: &str = "timeline";

/// `a` and `b` name the same project file. Paths reach here from the asset
/// server, the document tabs and scene components, which do not all agree on
/// the separator.
pub fn same_path(a: &str, b: &str) -> bool {
    a.replace('\\', "/").eq_ignore_ascii_case(&b.replace('\\', "/"))
}

/// Whether `path` is a project `.vfx` the editor can write. A built-in effect
/// (`embedded://`) is not: the timeline shows its tracks but cannot change
/// them, the same rule as the inspector's Edit Effect.
pub fn is_editable_path(path: &str) -> bool {
    !path.is_empty() && !path.starts_with("embedded://") && path.to_ascii_lowercase().ends_with(".vfx")
}

/// The effect editor's working copy, when it holds `path` and loaded cleanly.
pub fn open_doc<'w>(world: &'w World, path: &str) -> Option<&'w VfxAsset> {
    let st = world.get_resource::<VfxEditorState>()?;
    let open = st.path.as_deref()?;
    (same_path(open, path) && st.load_error.is_none()).then_some(&st.doc)
}

/// Whether `path` has edits the effect editor has not saved.
pub fn is_dirty(world: &World, path: &str) -> bool {
    open_doc(world, path).is_some() && world.get_resource::<VfxEditorState>().is_some_and(|s| s.dirty)
}

/// Apply `edit` to the effect at `path` through the effect editor's document
/// (see the module docs), opening it there first if another effect is open,
/// which saves that one's unsaved edits as switching documents always does.
/// `false` when the effect cannot be edited: a built-in, or a file that would
/// not open.
pub fn edit_effect(world: &mut World, path: &str, edit: impl FnOnce(&mut VfxAsset)) -> bool {
    if !is_editable_path(path) || world.get_resource::<VfxEditorState>().is_none() {
        return false;
    }
    if open_doc(world, path).is_none() {
        if world.resource::<VfxEditorState>().dirty {
            state::save_document(world);
        }
        state::open_document(world, path);
        if open_doc(world, path).is_none() {
            return false;
        }
    }
    edit(&mut world.resource_mut::<VfxEditorState>().doc);
    true
}

/// Save the effect at `path` if it is the open document and has unsaved
/// edits: the timeline's Save, and its autosave of edits it made itself.
pub fn save_effect(world: &mut World, path: &str) {
    if is_dirty(world, path) {
        state::save_document(world);
    }
}

/// Whether `path` is the effect on the active document tab, so the user is
/// looking at the effect editor and saves it there themselves.
pub fn is_active_document(world: &World, path: &str) -> bool {
    state::active_vfx_path(world.get_resource::<renzora_editor_framework::EditorContext>())
        .is_some_and(|p| same_path(&p, path))
}

/// Bring the Timeline panel to the front, wherever it is docked: the global
/// bottom panel (where it ships), or a workspace leaf. A bottom panel that is
/// collapsed keeps the Timeline as its active tab for when it next opens; the
/// shell owns opening it.
pub fn focus_timeline(world: &mut World) {
    if let Some(mut fixed) = world.get_resource_mut::<renzora_ember::dock::FixedDock>() {
        if fixed.tree.contains_panel(TIMELINE_PANEL) {
            fixed.tree.set_active_tab(TIMELINE_PANEL);
            fixed.dirty = true;
        }
    }
    let in_fixed = world
        .get_resource::<renzora_ember::dock::FixedDock>()
        .is_some_and(|f| f.tree.contains_panel(TIMELINE_PANEL));
    if !in_fixed {
        let changed = world
            .get_resource_mut::<renzora_ember::dock::Dock>()
            .is_some_and(|mut d| d.tree.focus_or_add_panel(TIMELINE_PANEL));
        if changed {
            if let Some(mut dirty) = world.get_resource_mut::<renzora_ember::dock::DockDirty>() {
                dirty.0 = true;
            }
        }
    }
    if let Some(mut req) = world.get_resource_mut::<renzora_ember::dock::FocusPanelRequest>() {
        req.0 = Some(TIMELINE_PANEL.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_compare_across_separators_and_case() {
        assert!(same_path("effects\\Fire.vfx", "effects/fire.vfx"));
        assert!(!same_path("effects/fire.vfx", "effects/smoke.vfx"));
    }

    #[test]
    fn built_in_effects_are_read_only() {
        assert!(is_editable_path("effects/fire.vfx"));
        assert!(!is_editable_path("embedded://renzora_particles/presets/campfire.vfx"));
        assert!(!is_editable_path(""));
        assert!(!is_editable_path("effects/old.particle"));
    }
}
