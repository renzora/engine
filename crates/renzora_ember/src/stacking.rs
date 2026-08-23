//! Stacking depths that survive being docked anywhere.
//!
//! `GlobalZIndex` is *global*: it lifts a node out of every ancestor's stacking
//! context into the root order. That's the only way to express some layouts —
//! layering a part against a sibling *subtree's* children, or floating something
//! out of a clipping parent — but it makes a widget a bad neighbour, because the
//! depth it picks is only right while nothing above it claims a higher one.
//!
//! The editor has a container that does: the global bottom panel sits at
//! `GlobalZIndex(100)` so graph panels docked in the main area can't paint over
//! it. Any panel content using a depth below that turns invisible the moment the
//! user docks *it* into the bottom panel — its parts render under the panel's own
//! background. The node graph (parts at 0–10) came up as an empty canvas; the
//! Mixer's rename field and the Timeline's playhead had the same latent bug.
//!
//! [`ZTier`] fixes the class rather than each case: give the part a depth
//! *relative to its own widget* and [`z_tier_rebase`] resolves it against
//! whatever context the widget is mounted in. Nothing that hosts the widget has
//! to know its band, and the widget never escapes its host.
//!
//! ```ignore
//! // Was: GlobalZIndex(10) — invisible inside the bottom panel.
//! commands.spawn((Node { .. }, ZTier(10), GlobalZIndex(10)));
//! ```
//!
//! Spawn both: the `GlobalZIndex` is the value used until the first rebase (and
//! the component the rebase writes into), the `ZTier` is what it means.
//!
//! This is for depth *within* panel content. A genuinely floating surface — a
//! menu, a dropdown, a modal, a drag ghost — wants to be above everything
//! including the bottom panel, so it keeps its absolute band (500/700/1000/…);
//! `dropdown`'s `floating_z` does the same ancestor walk for those, upward.

use bevy::prelude::*;
use bevy::ui::GlobalZIndex;

/// A stacking depth relative to the widget it belongs to, resolved into a real
/// `GlobalZIndex` by [`z_tier_rebase`]. See the module docs.
///
/// Write to this rather than to `GlobalZIndex` when something changes a part's
/// depth at runtime (e.g. raising a selected node above its peers) — the rebase
/// runs afterwards and would otherwise overwrite the change every frame.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTier(pub i32);

/// Resolve every [`ZTier`] into a `GlobalZIndex`, offset by the highest one any
/// ancestor claims. In the main dock nothing claims one, so tiers land on their
/// face values; inside the global bottom panel (100) they land at 100 + tier,
/// above its background instead of beneath it.
///
/// `Without<ZTier>` on the ancestor query is what keeps this from compounding:
/// it reads only depths the tier system doesn't own, so re-running on an
/// already-rebased tree computes the same answer.
pub fn z_tier_rebase(
    mut parts: Query<(Entity, &ZTier, &mut GlobalZIndex)>,
    hosts: Query<&GlobalZIndex, Without<ZTier>>,
    parents: Query<&ChildOf>,
) {
    for (e, tier, mut gz) in &mut parts {
        let mut base = 0;
        let mut cur = e;
        while let Ok(c) = parents.get(cur) {
            cur = c.parent();
            if let Ok(host) = hosts.get(cur) {
                base = base.max(host.0);
            }
        }
        let want = base.saturating_add(tier.0);
        if gz.0 != want {
            gz.0 = want;
        }
    }
}

/// Runs [`z_tier_rebase`]; ordered so anything that writes a [`ZTier`] during
/// `Update` can schedule itself `.before(ZTierSet)`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZTierSet;

pub(crate) struct StackingPlugin;

impl Plugin for StackingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, z_tier_rebase.in_set(ZTierSet));
    }
}
