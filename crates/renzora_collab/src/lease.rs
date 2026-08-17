//! Leases — who is allowed to be editing what.
//!
//! State replication is last-writer-wins, and two people dragging the same
//! object is the one case where that is visibly bad: each sees the other's
//! position arrive and overwrite theirs, several times a second, and the object
//! jitters between two places until somebody lets go. No merge strategy fixes
//! that, because there is no correct merge of two different positions.
//!
//! So the fix is not to merge but to avoid: selecting an entity claims it, the
//! host arbitrates claims, and an entity claimed by someone else is shown as
//! theirs. It is a social lock rather than an enforced one — the editor does not
//! refuse to move a locked object, it just tells you it is not yours and lets
//! the owner's version win. Enforcement would mean auditing every mutation path
//! in the editor, which is exactly the cost that [`crate::sync`] exists to
//! avoid.

use bevy::prelude::*;

use crate::identity::CollabId;
use crate::protocol::CollabMsg;
use crate::session::CollabSession;

/// The last selection we announced, so a claim is sent on change rather than
/// every frame.
#[derive(Resource, Default)]
pub struct ClaimedSelection(Vec<u64>);

/// Claim whatever this editor has selected.
pub fn claim_selection(
    session: Res<CollabSession>,
    mut claimed: ResMut<ClaimedSelection>,
    selection: Option<Res<renzora::EditorSelection>>,
    ids: Query<&CollabId>,
) {
    if !session.is_active() {
        if !claimed.0.is_empty() {
            claimed.0.clear();
        }
        return;
    }
    let Some(selection) = selection else {
        return;
    };
    let mut current: Vec<u64> =
        selection.get_all().iter().filter_map(|&e| ids.get(e).ok().map(|id| id.0)).collect();
    current.sort_unstable();
    if current == claimed.0 {
        return;
    }

    // Release before claiming: the host's arbitration treats a request as the
    // peer's complete claim, so an entity dropped from the selection is released
    // by simply not being in the next request. The explicit release only matters
    // when the selection empties, where there is no request to carry it.
    if current.is_empty() {
        session.send_up(CollabMsg::LeaseRelease { ids: std::mem::take(&mut claimed.0) });
    } else {
        session.send_up(CollabMsg::LeaseRequest { ids: current.clone() });
        claimed.0 = current;
    }
}

/// Which peer, if any, holds this entity — for the hierarchy badge and the
/// inspector's "being edited by" notice.
pub fn owner_of(session: &CollabSession, id: u64) -> Option<&crate::session::PeerInfo> {
    session.peers.values().find(|p| p.leases.contains(&id))
}

/// Whether someone else is holding this entity.
pub fn is_locked_elsewhere(session: &CollabSession, world: &World, entity: Entity) -> bool {
    let Some(id) = world.get::<CollabId>(entity) else {
        return false;
    };
    owner_of(session, id.0).is_some()
}
