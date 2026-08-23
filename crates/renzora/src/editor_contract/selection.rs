//! Global editor selection — shared between hierarchy, inspector, and viewport.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use bevy::prelude::*;

/// Global editor selection resource.
///
/// Uses `RwLock` so panels (which receive `&World`) can read selection,
/// while the hierarchy (also `&World`) can write via interior mutability.
#[derive(Resource)]
pub struct EditorSelection {
    selected: RwLock<Vec<Entity>>,
    /// Bumped by every write. Because the writes go through `&self`, Bevy's
    /// change detection never sees them — the resource's change tick would sit
    /// at whatever it was when the resource was inserted, forever.
    ///
    /// That is not cosmetic. Ember's reactive bindings subscribe to the change
    /// ticks of whatever a closure reads, and skip the closure when none of
    /// them moved. A row that binds its background to `is_selected(entity)`
    /// therefore recomputed exactly once (its first run, when the dep set is
    /// still empty and everything counts as dirty) and then went permanently
    /// clean: selecting a different entity left the old row painted with the
    /// selection accent until something *else* it read — its own `Interaction`,
    /// i.e. the mouse happening to pass over it — dirtied the binding.
    ///
    /// So writers bump this counter and one system per frame turns a change in
    /// it into a real `set_changed()`. See `sync_selection_change_tick` in
    /// `renzora_editor_framework`.
    version: AtomicU64,
}

impl Default for EditorSelection {
    fn default() -> Self {
        Self {
            selected: RwLock::new(Vec::new()),
            version: AtomicU64::new(0),
        }
    }
}

impl EditorSelection {
    /// Monotonic write counter — see the [`version`](Self::version) field. Read
    /// by the system that mirrors it onto the resource's Bevy change tick.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    fn bump(&self) {
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the primary selected entity (first in list), for backward compat.
    pub fn get(&self) -> Option<Entity> {
        self.selected.read().unwrap().first().copied()
    }

    /// Get all selected entities.
    pub fn get_all(&self) -> Vec<Entity> {
        self.selected.read().unwrap().clone()
    }

    /// Set a single selected entity (clears previous selection).
    pub fn set(&self, entity: Option<Entity>) {
        let mut sel = self.selected.write().unwrap();
        self.bump();
        sel.clear();
        if let Some(e) = entity {
            sel.push(e);
            crate::console_log::console_info("Selection", format!("Selected {:?}", e));
        } else {
            crate::console_log::console_info("Selection", "Selection cleared");
        }
    }

    /// Set multiple selected entities.
    pub fn set_multiple(&self, entities: Vec<Entity>) {
        crate::console_log::console_info(
            "Selection",
            format!("Multi-select: {} entities {:?}", entities.len(), entities),
        );
        *self.selected.write().unwrap() = entities;
        self.bump();
    }

    /// Toggle an entity in the selection (add if absent, remove if present).
    pub fn toggle(&self, entity: Entity) {
        let mut sel = self.selected.write().unwrap();
        self.bump();
        if let Some(pos) = sel.iter().position(|&e| e == entity) {
            sel.remove(pos);
            crate::console_log::console_info("Selection", format!("Deselected {:?}", entity));
        } else {
            sel.push(entity);
            crate::console_log::console_info(
                "Selection",
                format!("Added {:?} to selection", entity),
            );
        }
    }

    /// Check if an entity is currently selected.
    pub fn is_selected(&self, entity: Entity) -> bool {
        self.selected.read().unwrap().contains(&entity)
    }

    /// Select a range of entities from the visible order list.
    /// Selects all entities between `anchor` and `target` (inclusive) in the given order.
    pub fn select_range(&self, visible_order: &[Entity], anchor: Entity, target: Entity) {
        let anchor_idx = visible_order.iter().position(|&e| e == anchor);
        let target_idx = visible_order.iter().position(|&e| e == target);
        if let (Some(a), Some(b)) = (anchor_idx, target_idx) {
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            let range: Vec<Entity> = visible_order[start..=end].to_vec();
            *self.selected.write().unwrap() = range;
            self.bump();
        }
    }

    /// Whether more than one entity is selected.
    pub fn has_multi_selection(&self) -> bool {
        self.selected.read().unwrap().len() > 1
    }

    /// Clear the selection.
    pub fn clear(&self) {
        self.selected.write().unwrap().clear();
        self.bump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entities(n: u32) -> Vec<Entity> {
        (0..n).map(Entity::from_raw_u32).map(Option::unwrap).collect()
    }

    #[test]
    fn a_fresh_selection_is_empty() {
        let sel = EditorSelection::default();
        assert_eq!(sel.get(), None);
        assert!(sel.get_all().is_empty());
        assert!(!sel.has_multi_selection());
    }

    /// `get` is the single-selection view the inspector reads. It must be the
    /// FIRST entity, not an arbitrary one, or the inspector shows a different
    /// entity from the one the hierarchy highlights.
    #[test]
    fn the_primary_selection_is_the_first_entity() {
        let e = entities(3);
        let sel = EditorSelection::default();
        sel.set_multiple(e.clone());
        assert_eq!(sel.get(), Some(e[0]));
    }

    /// Single-select replaces rather than appends — clicking a second entity
    /// without a modifier must not leave the first one selected.
    #[test]
    fn setting_one_entity_replaces_the_whole_selection() {
        let e = entities(3);
        let sel = EditorSelection::default();
        sel.set_multiple(e.clone());
        sel.set(Some(e[2]));
        assert_eq!(sel.get_all(), vec![e[2]]);
        assert!(!sel.has_multi_selection());
    }

    #[test]
    fn setting_none_clears_the_selection() {
        let e = entities(2);
        let sel = EditorSelection::default();
        sel.set_multiple(e);
        sel.set(None);
        assert!(sel.get_all().is_empty());
        assert_eq!(sel.get(), None);
    }

    #[test]
    fn clearing_empties_the_selection() {
        let sel = EditorSelection::default();
        sel.set_multiple(entities(4));
        sel.clear();
        assert!(sel.get_all().is_empty());
    }

    // ── ctrl-click toggling ──────────────────────────────────────────────────

    #[test]
    fn toggling_adds_then_removes_the_same_entity() {
        let e = entities(2);
        let sel = EditorSelection::default();

        sel.toggle(e[0]);
        assert!(sel.is_selected(e[0]));

        sel.toggle(e[0]);
        assert!(!sel.is_selected(e[0]), "a second toggle must deselect");
        assert!(sel.get_all().is_empty());
    }

    /// Toggling one entity out of a multi-selection must leave the others where
    /// they were — including their order, since `get()` reads the first.
    #[test]
    fn toggling_one_out_leaves_the_rest_in_order() {
        let e = entities(4);
        let sel = EditorSelection::default();
        sel.set_multiple(e.clone());

        sel.toggle(e[1]);

        assert_eq!(sel.get_all(), vec![e[0], e[2], e[3]]);
        assert_eq!(sel.get(), Some(e[0]));
    }

    #[test]
    fn toggling_appends_to_the_end() {
        let e = entities(3);
        let sel = EditorSelection::default();
        sel.set(Some(e[0]));
        sel.toggle(e[2]);
        assert_eq!(sel.get_all(), vec![e[0], e[2]]);
        assert!(sel.has_multi_selection());
    }

    // ── shift-click range selection ──────────────────────────────────────────

    #[test]
    fn a_range_selects_everything_between_inclusive() {
        let order = entities(6);
        let sel = EditorSelection::default();
        sel.select_range(&order, order[1], order[4]);
        assert_eq!(sel.get_all(), order[1..=4].to_vec());
    }

    /// Shift-clicking *upward* is as common as downward, and the anchor is
    /// whichever end the user started from. Without the swap this selects
    /// nothing (an inverted slice range would panic outright).
    #[test]
    fn a_range_works_when_the_target_is_above_the_anchor() {
        let order = entities(6);
        let sel = EditorSelection::default();
        sel.select_range(&order, order[4], order[1]);
        assert_eq!(sel.get_all(), order[1..=4].to_vec());
    }

    #[test]
    fn a_range_of_one_selects_just_that_entity() {
        let order = entities(4);
        let sel = EditorSelection::default();
        sel.select_range(&order, order[2], order[2]);
        assert_eq!(sel.get_all(), vec![order[2]]);
    }

    /// A range replaces the previous selection rather than adding to it.
    #[test]
    fn a_range_replaces_what_was_selected_before() {
        let order = entities(6);
        let sel = EditorSelection::default();
        sel.set_multiple(vec![order[5]]);
        sel.select_range(&order, order[0], order[1]);
        assert_eq!(sel.get_all(), vec![order[0], order[1]]);
    }

    /// An entity not in the visible order is reachable: shift-click after
    /// collapsing the tree branch the anchor was in. Leaving the selection
    /// untouched is right — the alternative is a slice index that panics.
    #[test]
    fn a_range_against_an_entity_that_is_not_visible_changes_nothing() {
        let order = entities(4);
        let hidden = Entity::from_raw_u32(99).unwrap();
        let sel = EditorSelection::default();
        sel.set_multiple(vec![order[0]]);

        sel.select_range(&order, hidden, order[2]);
        assert_eq!(sel.get_all(), vec![order[0]], "anchor missing");

        sel.select_range(&order, order[0], hidden);
        assert_eq!(sel.get_all(), vec![order[0]], "target missing");
    }

    #[test]
    fn a_range_over_an_empty_order_changes_nothing() {
        let sel = EditorSelection::default();
        let e = entities(1);
        sel.set_multiple(e.clone());
        sel.select_range(&[], e[0], e[0]);
        assert_eq!(sel.get_all(), e);
    }

    // ── the multi-selection flag ─────────────────────────────────────────────

    /// The inspector switches to its multi-edit mode on this, so "exactly one"
    /// must not count as multi.
    #[test]
    fn multi_selection_means_strictly_more_than_one() {
        let e = entities(2);
        let sel = EditorSelection::default();
        assert!(!sel.has_multi_selection());

        sel.set(Some(e[0]));
        assert!(!sel.has_multi_selection(), "one entity is not a multi-selection");

        sel.toggle(e[1]);
        assert!(sel.has_multi_selection());
    }

    // ── the write counter ────────────────────────────────────────────────────

    /// Every write must move `version`, because that counter is the *only*
    /// signal the reactive UI has that the selection changed — the `RwLock`
    /// hides the writes from Bevy's change detection. A write that forgets to
    /// bump leaves selection highlights painted on the wrong rows.
    #[test]
    fn every_write_moves_the_version() {
        let e = entities(4);
        let sel = EditorSelection::default();
        let mut v = sel.version();
        let mut moved = |sel: &EditorSelection, what: &str| {
            let now = sel.version();
            assert!(now != v, "{what} did not bump the version");
            v = now;
        };

        sel.set(Some(e[0]));
        moved(&sel, "set");
        sel.set(None);
        moved(&sel, "set(None)");
        sel.set_multiple(e.clone());
        moved(&sel, "set_multiple");
        sel.toggle(e[1]);
        moved(&sel, "toggle off");
        sel.toggle(e[1]);
        moved(&sel, "toggle on");
        sel.select_range(&e, e[0], e[2]);
        moved(&sel, "select_range");
        sel.clear();
        moved(&sel, "clear");
    }

    /// A range that resolves to nothing changes no state, so it must not bump —
    /// otherwise every failed shift-click wakes every selection binding.
    #[test]
    fn a_range_that_selects_nothing_leaves_the_version_alone() {
        let order = entities(3);
        let hidden = Entity::from_raw_u32(99).unwrap();
        let sel = EditorSelection::default();
        let before = sel.version();
        sel.select_range(&order, hidden, order[1]);
        assert_eq!(sel.version(), before);
    }

    #[test]
    fn is_selected_only_reports_entities_actually_in_the_set() {
        let e = entities(3);
        let sel = EditorSelection::default();
        sel.set_multiple(vec![e[0], e[2]]);
        assert!(sel.is_selected(e[0]));
        assert!(!sel.is_selected(e[1]));
        assert!(sel.is_selected(e[2]));
    }
}
