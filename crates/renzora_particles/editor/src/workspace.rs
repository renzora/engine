//! Showing the effect editor's panels when a `.vfx` document opens.
//!
//! Effects open in the Particles workspace (`DocTabKind::Particle`). A saved
//! layout is the user's, so it is never replaced wholesale: whatever is in it
//! stays, and the effect panels are slotted in where they are missing and
//! brought to the front. The swap runs once per document activation, not every
//! frame, so closing a panel on purpose is respected until the next effect
//! opens.
//!
//! A `.particle` (the retired Hanabi format) never reaches this: opening one
//! opens its converted `.vfx` instead ([`crate::legacy`]). The Hanabi editor's
//! own panels left with Hanabi, and `renzora_shell` strips their ids from a
//! saved layout.

use bevy::prelude::*;

use renzora_editor_framework::{DocTabKind, EditorContext};
use renzora_ember::dock::{Dock, DockDirty, DockTree};

pub const PREVIEW: &str = crate::preview::PANEL;
pub const GRAPH: &str = crate::graph::PANEL;
pub const SETTINGS: &str = crate::emitter_settings::PANEL;

/// The effect editor's own layout: the preview over the emitter settings, the
/// graph taking the rest. Mirrored by `renzora_shell::dock::layout_particles`,
/// which cannot depend on this crate.
pub fn layout() -> DockTree {
    DockTree::horizontal(DockTree::vertical(DockTree::leaf(PREVIEW), DockTree::leaf(SETTINGS), 0.6), DockTree::leaf(GRAPH), 0.42)
}

/// Put the effect editor's panels into `tree` in front (see the module docs).
/// Returns whether anything changed.
pub fn adopt_panels(tree: &mut DockTree) -> bool {
    if tree.contains_panel(GRAPH) && tree.contains_panel(PREVIEW) {
        let before = tree.clone();
        tree.set_active_tab(SETTINGS);
        tree.set_active_tab(GRAPH);
        tree.set_active_tab(PREVIEW);
        return *tree != before;
    }
    if tree.is_empty() {
        *tree = layout();
        return true;
    }
    let before = tree.clone();
    if !tree.contains_panel(PREVIEW) {
        tree.adopt_panel(PREVIEW);
    }
    if !tree.contains_panel(GRAPH) {
        // Its own column beside the preview rather than a tab hidden behind it.
        let split = tree.find_leaf_mut(PREVIEW).map(|leaf| {
            let old = std::mem::replace(leaf, DockTree::Empty);
            *leaf = DockTree::horizontal(old, DockTree::leaf(GRAPH), 0.42);
        });
        if split.is_none() {
            tree.adopt_panel(GRAPH);
        }
    }
    if !tree.contains_panel(SETTINGS) {
        if let Some(leaf) = tree.find_leaf_mut(PREVIEW) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            *leaf = DockTree::vertical(old, DockTree::leaf(SETTINGS), 0.6);
        }
    }
    tree.set_active_tab(SETTINGS);
    tree.set_active_tab(PREVIEW);
    tree.set_active_tab(GRAPH);
    *tree != before
}

/// The active `.vfx` document, if there is one.
pub fn active_vfx_doc(ctx: Option<&EditorContext>) -> Option<String> {
    match ctx? {
        EditorContext::Asset { path, kind: DocTabKind::Particle } if path.to_ascii_lowercase().ends_with(".vfx") => {
            Some(path.clone())
        }
        _ => None,
    }
}

pub fn ensure_panels(
    context: Option<Res<EditorContext>>,
    dock: Option<ResMut<Dock>>,
    dirty: Option<ResMut<DockDirty>>,
    mut last: Local<Option<String>>,
) {
    let doc = active_vfx_doc(context.as_deref());
    if *last == doc {
        return;
    }
    *last = doc.clone();
    let (Some(_), Some(mut dock), Some(mut dirty)) = (doc, dock, dirty) else { return };
    let mut tree = dock.tree.clone();
    if adopt_panels(&mut tree) {
        dock.tree = tree;
        dirty.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_layout_keeps_its_panels_and_gains_ours() {
        let mut tree = DockTree::horizontal(DockTree::leaf("inspector"), DockTree::leaf("console"), 0.8);
        assert!(adopt_panels(&mut tree));
        for id in [PREVIEW, GRAPH, SETTINGS, "inspector", "console"] {
            assert!(tree.contains_panel(id), "{id} missing");
        }
        assert!(tree.is_active_tab(PREVIEW));
        assert!(tree.is_active_tab(GRAPH));
    }

    #[test]
    fn adopting_twice_changes_nothing_the_second_time() {
        let mut tree = DockTree::leaf("inspector");
        adopt_panels(&mut tree);
        let once = tree.clone();
        assert!(!adopt_panels(&mut tree));
        assert_eq!(tree, once);
    }

    #[test]
    fn an_empty_dock_gets_the_editor_layout() {
        let mut tree = DockTree::Empty;
        assert!(adopt_panels(&mut tree));
        assert_eq!(tree, layout());
    }

    #[test]
    fn only_a_vfx_is_an_effect_document() {
        let ctx = |p: &str| EditorContext::Asset { path: p.into(), kind: DocTabKind::Particle };
        assert_eq!(active_vfx_doc(Some(&ctx("a/b.VFX"))).as_deref(), Some("a/b.VFX"));
        assert_eq!(active_vfx_doc(Some(&ctx("a/b.particle"))), None);
        assert_eq!(active_vfx_doc(Some(&EditorContext::Scene)), None);
    }
}
