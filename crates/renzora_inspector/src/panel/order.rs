//! The user's own order for component sections, and how it survives the fact
//! that no two entities show the same set of components.
//!
//! A drag (see [`reorder`](super::reorder)) rearranges what is on screen, but
//! what gets saved is a **ranking of component types**, not the arrangement of
//! one entity's inspector. It has to be: the next entity shows a different set,
//! and a per-entity record would have nothing to say about a pair of components
//! it had never seen side by side.
//!
//! Two operations keep that ranking and the screen in agreement, and they are
//! deliberate mirror images of each other:
//!
//! * [`arrangement`] reads the ranking onto the sections actually present. A
//!   type nobody has ever dragged has no rank, so it *follows the ranked section
//!   it already sat behind* rather than being swept to the end — a component the
//!   user has never touched stays where the built-in order put it.
//! * [`merge`] folds a new on-screen order back into the ranking, putting each
//!   type the current entity does not show back behind the same neighbour it was
//!   behind before. So arranging components on one entity leaves the order of
//!   components on every other entity alone.

use bevy::prelude::*;

use super::spec::SectionSpec;

/// The saved component-type ranking, read from `~/.renzora/editor.toml` at
/// startup and written back on every drop.
#[derive(Resource, Default)]
pub(crate) struct InspectorSectionOrder {
    /// Component `type_id`s, most-preferred first. Empty = never rearranged, and
    /// the built-in order (`spec::section_priority` + registry order) stands.
    order: Vec<String>,
    /// Bumped whenever `order` changes. Hashed into the rebuild signature, which
    /// is what turns a drop into a rebuilt panel — the sections are not moved in
    /// place, so this is the only thing that makes a drop visible.
    pub(crate) revision: u64,
}

impl InspectorSectionOrder {
    pub(crate) fn from_disk() -> Self {
        Self {
            order: renzora::core::load_inspector_component_order(),
            revision: 0,
        }
    }

    /// Rearrange `sections` (already in built-in order) to match the ranking.
    pub(crate) fn sort(&self, sections: &mut Vec<SectionSpec>) {
        if self.order.is_empty() || sections.len() < 2 {
            return;
        }
        let type_ids: Vec<&str> = sections.iter().map(|s| s.type_id).collect();
        let idx = arrangement(&self.order, &type_ids);
        // `SectionSpec` is not `Clone` (it carries the boxed accessors), so the
        // permutation is applied by moving each spec out of an `Option` slot
        // rather than by copying.
        let mut taken: Vec<Option<SectionSpec>> = sections.drain(..).map(Some).collect();
        sections.extend(idx.into_iter().filter_map(|i| taken[i].take()));
    }

    /// Record a new on-screen order (component `type_id`s, top to bottom) and
    /// persist it. A no-op when it changes nothing, so a drag that lands a
    /// section back where it started costs neither a disk write nor a rebuild.
    pub(crate) fn record(&mut self, displayed: &[String]) {
        let next = merge(&self.order, displayed);
        if next == self.order {
            return;
        }
        self.order = next;
        self.revision = self.revision.wrapping_add(1);
        if let Err(e) = renzora::core::save_inspector_component_order(&self.order) {
            warn!("could not save the inspector component order: {e}");
        }
    }
}

/// The order to show `type_ids` in, as indices into it, given the saved ranking.
///
/// Ranked types sort by their rank. An unranked one takes the rank of the
/// nearest ranked type *ahead of it in the built-in order* and sorts just behind
/// it (the `1` in the key), so it keeps the neighbour it already had. One that
/// has nothing ranked ahead of it keeps the front of the list.
fn arrangement(order: &[String], type_ids: &[&str]) -> Vec<usize> {
    let mut keys: Vec<(i32, u8, usize)> = Vec::with_capacity(type_ids.len());
    let mut anchor: i32 = -1;
    for (i, t) in type_ids.iter().enumerate() {
        match order.iter().position(|s| s == t) {
            Some(rank) => {
                anchor = rank as i32;
                keys.push((anchor, 0, i));
            }
            None => keys.push((anchor, 1, i)),
        }
    }
    let mut idx: Vec<usize> = (0..type_ids.len()).collect();
    idx.sort_by_key(|&i| keys[i]);
    idx
}

/// Fold the on-screen order `displayed` back into the ranking `master`.
///
/// Every type the current entity does not show is put back behind the same
/// displayed type it was behind before (or at the very front, if it was ahead of
/// all of them). That is what stops arranging one entity's components from
/// silently reshuffling another entity's.
fn merge(master: &[String], displayed: &[String]) -> Vec<String> {
    // (the displayed type it sits behind, the type itself) for each ranked type
    // the current entity does not show. `None` = ahead of every displayed one.
    let mut trailing: Vec<(Option<&str>, &str)> = Vec::new();
    let mut anchor: Option<&str> = None;
    for m in master {
        if displayed.iter().any(|d| d == m) {
            anchor = Some(m.as_str());
        } else {
            trailing.push((anchor, m.as_str()));
        }
    }
    let mut out: Vec<String> = Vec::with_capacity(master.len() + displayed.len());
    out.extend(
        trailing
            .iter()
            .filter(|(a, _)| a.is_none())
            .map(|(_, m)| (*m).to_string()),
    );
    for d in displayed {
        out.push(d.clone());
        out.extend(
            trailing
                .iter()
                .filter(|(a, _)| *a == Some(d.as_str()))
                .map(|(_, m)| (*m).to_string()),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn arranged<'a>(order: &[&str], ids: &[&'a str]) -> Vec<&'a str> {
        arrangement(&owned(order), ids)
            .into_iter()
            .map(|i| ids[i])
            .collect()
    }

    #[test]
    fn ranked_sections_follow_the_saved_order() {
        assert_eq!(
            arranged(&["material", "transform"], &["transform", "material"]),
            vec!["material", "transform"]
        );
    }

    #[test]
    fn an_unranked_section_keeps_the_neighbour_it_had() {
        // `mesh` was never dragged: it stays behind `transform`, which it
        // followed in the built-in order, rather than falling to the end.
        assert_eq!(
            arranged(
                &["material", "transform"],
                &["transform", "mesh", "material"]
            ),
            vec!["material", "transform", "mesh"]
        );
    }

    #[test]
    fn unranked_sections_ahead_of_everything_keep_the_front() {
        assert_eq!(
            arranged(&["transform"], &["scripts", "transform"]),
            vec!["scripts", "transform"]
        );
    }

    #[test]
    fn merging_leaves_types_this_entity_does_not_show_alone() {
        // The user put `light` ahead of `transform` on some other entity. This
        // entity shows transform + mesh, and moving mesh to the top must not
        // cost `light` its place ahead of `transform`.
        assert_eq!(
            merge(&owned(&["light", "transform"]), &owned(&["mesh", "transform"])),
            owned(&["light", "mesh", "transform"])
        );
    }

    #[test]
    fn merging_an_empty_ranking_is_the_screen_order() {
        assert_eq!(merge(&[], &owned(&["a", "b"])), owned(&["a", "b"]));
    }

    #[test]
    fn merging_keeps_trailing_types_behind_their_anchor() {
        assert_eq!(
            merge(
                &owned(&["a", "hidden", "b"]),
                &owned(&["b", "a"])
            ),
            owned(&["b", "a", "hidden"])
        );
    }
}
