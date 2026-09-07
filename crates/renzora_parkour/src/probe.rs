//! Reading the geometry in front of the character.
//!
//! Everything the state machine decides is decided from one [`ParkourProbe`],
//! rebuilt each frame from a handful of ray casts. Casts, not colliders: a
//! traversal needs to know *where the top of the obstacle is* and *whether the
//! far side is open*, and no contact list answers that. Two rays do.
//!
//! The probe deliberately reports geometry, not intent — it says "there is a
//! 0.9 m ledge here, its top is clear, the far side drops away", and
//! [`crate::drive`] decides whether that is a vault, a mantle or a grab based
//! on what state the character is in. Keeping the classification out of here is
//! what lets the same ledge be mantled from the ground and grabbed from the
//! air without probing twice.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::{ParkourBlocker, ParkourController, ParkourLadder};

/// A ledge found ahead of the character: somewhere with a face to climb and a
/// top to stand on.
#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    /// World point on the top surface, just past the lip.
    pub top: Vec3,
    /// Height of `top` above the character's feet. The single number the
    /// vault/mantle/grab classification turns on.
    pub height: f32,
    /// Outward normal of the face — points back at the character.
    pub face_normal: Vec3,
    /// True when there is standing room above `top`. A ledge under a low
    /// soffit can still be grabbed, but mantling onto it would put the
    /// character's head inside the ceiling.
    pub clear: bool,
    /// True when the ground drops away within `vault_max_depth` past the lip,
    /// i.e. the obstacle is a *rail* rather than a *platform*. This is what
    /// separates a vault (over and down) from a mantle (up and onto).
    pub thin: bool,
    /// Where a vault would put the character down, on the far side.
    pub landing: Vec3,
}

/// A near-vertical surface found beside or ahead of the character.
#[derive(Clone, Copy, Debug)]
pub struct WallHit {
    /// Outward normal — points from the wall back at the character.
    pub normal: Vec3,
    /// Distance from the capsule's centre line to the surface. A wall run
    /// holds this at roughly the capsule radius; see `drive`.
    pub distance: f32,
}

/// Everything the state machine knows about the character's surroundings this
/// frame.
#[derive(Clone, Debug, Default)]
pub struct ParkourProbe {
    pub grounded: bool,
    pub ground_normal: Vec3,
    /// The ledge ahead, if any — see [`Ledge`].
    pub ledge: Option<Ledge>,
    /// A wall to the character's left, if within reach.
    pub wall_left: Option<WallHit>,
    pub wall_right: Option<WallHit>,
    /// A wall directly ahead. Used for wall jumps, which work off a wall you
    /// ran into as well as one you were running along.
    pub wall_front: Option<WallHit>,
    /// The [`ParkourLadder`] entity the character is facing, if any.
    pub ladder: Option<Entity>,
}

/// The queries [`probe`] needs to resolve a hit entity to the thing that was
/// actually hit — a collider is usually a child of the ladder/blocker that owns
/// it, so both lookups walk up the hierarchy.
#[derive(SystemParam)]
pub struct ProbeWorld<'w, 's> {
    pub parents: Query<'w, 's, &'static ChildOf>,
    pub children: Query<'w, 's, &'static Children>,
    pub ladders: Query<'w, 's, (), With<ParkourLadder>>,
    pub blockers: Query<'w, 's, (), With<ParkourBlocker>>,
}

impl ProbeWorld<'_, '_> {
    /// `root` and everything under it, written into `out`.
    ///
    /// This is what the sweeps exclude. Excluding only the character entity is
    /// not enough: an imported model usually carries its collider on a child
    /// mesh rather than on the entity the controller sits on, and a capsule
    /// that collides with its own body cannot move at all — the character
    /// simply stands still with no error anywhere.
    pub fn subtree_into(&self, root: Entity, out: &mut Vec<Entity>) {
        out.clear();
        out.push(root);
        // `out` doubles as the queue: everything appended is itself visited.
        let mut i = 0;
        while i < out.len() {
            if let Ok(kids) = self.children.get(out[i]) {
                out.extend(kids.iter());
            }
            i += 1;
        }
    }

    /// Nearest ancestor of `entity` (including itself) carrying `M`.
    fn ancestor_with<M: Component>(
        &self,
        entity: Entity,
        q: &Query<(), With<M>>,
    ) -> Option<Entity> {
        let mut e = entity;
        loop {
            if q.contains(e) {
                return Some(e);
            }
            match self.parents.get(e) {
                Ok(child_of) => e = child_of.parent(),
                Err(_) => return None,
            }
        }
    }

    fn is_blocked(&self, entity: Entity) -> bool {
        self.ancestor_with(entity, &self.blockers).is_some()
    }
}

/// Build this frame's probe.
///
/// `foot` is the world point at the bottom of the character's capsule and
/// `forward` is the horizontal direction being probed — the movement intent
/// when there is one, otherwise where the character is facing.
pub fn probe(
    spatial: &SpatialQuery,
    world: &ProbeWorld,
    controller: &ParkourController,
    shape: &Collider,
    foot: Vec3,
    forward: Vec3,
    filter: &SpatialQueryFilter,
) -> ParkourProbe {
    let mut out = ParkourProbe {
        ground_normal: Vec3::Y,
        ..Default::default()
    };

    let cast = |origin: Vec3, dir: Vec3, dist: f32| -> Option<(Vec3, Vec3, Entity)> {
        let dir3 = Dir3::new(dir).ok()?;
        // `solid = true`: a probe that starts inside a collider should report
        // that immediately rather than punching out through the far side and
        // reporting the geometry behind it.
        let hit = spatial.cast_ray(origin, dir3, dist, true, filter)?;
        if world.is_blocked(hit.entity) {
            return None;
        }
        Some((origin + dir * hit.distance, hit.normal, hit.entity))
    };

    // ── Ground ───────────────────────────────────────────────────────────
    // The capsule, not a ray down the centre line — and deliberately the same
    // test the collide-and-slide primitive uses to decide it has landed. The
    // two have to agree: on the edge of a ledge, a centre ray misses while the
    // capsule is still resting on the corner, and the character then stands
    // there physically supported while the state machine believes they are
    // falling and never lets them land.
    //
    // Also not routed through `cast` below, which drops hits on
    // `ParkourBlocker` geometry: a blocker opts out of *traversal*, not out of
    // being a floor. Standing on one must still work.
    let max_slope = controller.max_slope.to_radians();
    if let Some(hit) = spatial.cast_shape(
        shape,
        foot + Vec3::Y * (controller.height * 0.5),
        Quat::IDENTITY,
        Dir3::NEG_Y,
        &ShapeCastConfig {
            max_distance: 0.2,
            ignore_origin_penetration: true,
            ..Default::default()
        },
        filter,
    ) {
        if hit.normal1.angle_between(Vec3::Y) <= max_slope {
            out.grounded = true;
            out.ground_normal = hit.normal1;
        }
    }

    let Ok(fwd) = Dir3::new(forward.with_y(0.0)) else {
        // No horizontal direction to probe along (straight up/down input on a
        // character that has never faced anywhere). Ground is still valid.
        return out;
    };
    let fwd = *fwd;
    let right = fwd.cross(Vec3::Y).normalize_or_zero();
    let reach = controller.radius + controller.forward_reach;
    let chest = controller.height * 0.6;

    // ── Walls ────────────────────────────────────────────────────────────
    // Near-vertical surfaces beside and ahead of the character, for wall runs
    // and wall jumps. `normal.y` rejects floors and steep ramps, which a ray
    // at chest height can still find on sloped terrain.
    let is_wall = |n: Vec3| n.y.abs() < 0.35;
    let side_reach = controller.radius + 0.35;
    let chest_at = foot + Vec3::Y * chest;
    let wall_along = |dir: Vec3, reach: f32| -> Option<WallHit> {
        let (point, normal, _) = cast(chest_at, dir, reach)?;
        is_wall(normal).then_some(WallHit {
            normal,
            distance: (point - chest_at).length(),
        })
    };
    out.wall_left = wall_along(-right, side_reach);
    out.wall_right = wall_along(right, side_reach);
    out.wall_front = wall_along(fwd, reach);

    // ── The face ahead ───────────────────────────────────────────────────
    // Knee height first: a low rail is invisible to a chest ray, and the low
    // hit is also the one that tells us the obstacle starts near the floor.
    // Falling back to chest height catches walls whose bottom is set back
    // (an overhanging balcony, a shelf).
    let knee = foot + Vec3::Y * (controller.step_height + 0.05);
    let face = cast(knee, fwd, reach)
        .filter(|(_, n, _)| is_wall(*n))
        .or_else(|| cast(foot + Vec3::Y * chest, fwd, reach).filter(|(_, n, _)| is_wall(*n)));

    let Some((face_point, face_normal, face_entity)) = face else {
        return out;
    };

    // A ladder is identified by what was hit, not by its shape — climbable
    // geometry is a level-design decision, and rungs are usually a trimesh no
    // probe could tell from a fence.
    out.ladder = world.ancestor_with(face_entity, &world.ladders);

    // ── The top of it ────────────────────────────────────────────────────
    // Scan straight down from above, a little past the face so the ray lands
    // on the top surface rather than skimming the lip. Anything higher than
    // `mantle_max_height` above the feet is out of reach from here and simply
    // isn't reported — the character has to jump first, and then the same
    // probe, run from a higher `foot`, finds it.
    let scan = face_point + fwd * (0.05 + controller.radius * 0.25);
    let ceiling = foot.y + controller.mantle_max_height + 0.35;
    let scan_origin = Vec3::new(scan.x, ceiling, scan.z);
    let Some((top, top_normal, _)) = cast(scan_origin, Vec3::NEG_Y, controller.mantle_max_height + 0.45)
        // A hit at zero distance means the scan started *inside* the obstacle,
        // i.e. it is taller than the scan ceiling and has no reachable top.
        // Without this a plain 4 m wall reports a phantom ledge at exactly the
        // ceiling height, which every classification below then rejects — but
        // `can_mantle` and `ledge_height` would have shown it to the game.
        .filter(|(top, _, _)| scan_origin.y - top.y > 0.01)
    else {
        return out;
    };
    // A top you would slide off is not a top.
    if top_normal.angle_between(Vec3::Y) > max_slope {
        return out;
    }
    let height = top.y - foot.y;
    if height <= controller.step_height {
        // Low enough that collide-and-slide walks straight over it. Reporting
        // it would make the character vault every kerb.
        return out;
    }
    if height > controller.mantle_max_height {
        // Out of reach from where the character is standing. Not an error —
        // jumping raises `foot`, and the same probe then finds it.
        return out;
    }

    // Headroom above the top, for standing on it.
    let clear = cast(top + Vec3::Y * 0.05, Vec3::Y, controller.height * 0.9).is_none();

    // ── The far side ─────────────────────────────────────────────────────
    // A vault needs the ground past the lip to drop away *and* the space above
    // it to be open. A wall right behind the rail makes it a mantle instead,
    // however thin the rail is.
    let past = top + fwd * controller.vault_max_depth;
    let far_blocked = cast(top + Vec3::Y * 0.3, fwd, controller.vault_max_depth).is_some();
    let far_ground = cast(past + Vec3::Y * 0.1, Vec3::NEG_Y, height + 1.2);
    let (thin, landing) = match far_ground {
        // Ground on the far side, well below the lip: a rail to go over.
        Some((p, _, _)) if p.y < top.y - 0.35 => (true, p),
        // Ground level with the top: it is a platform, mantle onto it.
        Some(_) => (false, top),
        // Nothing within reach on the far side — a drop. Still a vault; the
        // landing is nominal and the character falls from it.
        None => (true, past - Vec3::Y * 0.5),
    };

    out.ledge = Some(Ledge {
        top,
        height,
        face_normal,
        clear,
        thin: thin && !far_blocked,
        landing,
    });
    out
}
