//! A bone-based face rig, built from four points on the face (five on a head
//! whose eyes are painted on).
//!
//! Most heads arrive with no skeleton and no blend shapes (a scan, a sculpt, a
//! Sketchfab bust): just meshes for the skin, the eyes, the lashes, the mouth
//! interior and the hair. A face rig for talking and blinking needs a jaw,
//! lips, mouth corners, eyelids, eyes and brows, and the skin around each to
//! follow its bone. This builds exactly that.
//!
//! # What is asked of the user, and what is worked out
//!
//! Four points, clicked on the face: a mouth corner (the other is mirrored),
//! the middle of the upper lip, the middle of the lower lip, and the chin. The
//! rest is measured from the model:
//!
//! - **The eyes** from the eye meshes (any mesh named `eye`, not lashes or
//!   lids), split into left and right: their centres are the eye bones and the
//!   eyelids' pivots, and their size sets the scale of everything around them.
//!   A scan has none: its eyes are part of the skin, painted on. There a fifth
//!   point, the middle of one eye, places them instead (see
//!   [`FaceFrame::eyes_modelled`]).
//! - **Which way the face points**, from the head's centre to the eyes.
//! - **The jaw hinge**, back from the mouth and a little above it, in
//!   proportion to the eye spacing; **the brows** above the eyes.
//!
//! # Who follows which bone
//!
//! - **Skin**: smooth regional weights. The jaw takes everything below the
//!   line between the lips, sharply at the lips and more softly out across the
//!   cheeks, fading toward the jaw hinge and down the neck. Near the mouth,
//!   "below" is decided across the skin rather than by height: a vertex goes
//!   with whichever lip it is nearer along the surface (see `lip_sides`), so
//!   the inside of each lip, folded past the line, stays with its own lip. The lids take the
//!   skin hugging each eyeball, upper half and lower half; corners, lips and
//!   brows take a soft disc around their point. What is left is the head's.
//! - **Eyeballs** (and their highlight shells) are rigid on their eye bone.
//! - **Lashes** follow the upper or lower lid of their side.
//! - **The mouth interior** (teeth, tongue) splits at the lips: below goes
//!   with the jaw, above stays with the head.
//! - **Hair** and anything else rigid stays with the head.
//!
//! Every bone has the same orientation as the head bone, aligned to the face:
//! X toward the character's left, Y up, Z forward. The expressions in
//! [`crate::expressions`] are written in that frame.

use bevy::math::{Mat3, Quat, Vec3};

use crate::glb::{Glb, GlbError};
use crate::skin::{self, smoothstep, Applied, Bone, Surface, Vertex};

/// The points the user places, in the model's space.
#[derive(Clone, Copy, Debug)]
pub struct FaceMarkers {
    /// Either mouth corner; the other is mirrored across the face.
    pub mouth_corner: Vec3,
    pub upper_lip: Vec3,
    pub lower_lip: Vec3,
    pub chin: Vec3,
    /// The middle of either eye, on its surface; the other is mirrored. Only
    /// read for a head with no eye meshes, which has nothing else to find its
    /// eyes by.
    pub eye: Option<Vec3>,
}

/// What a mesh is to the face, from its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshRole {
    Skin,
    Eye,
    Lashes,
    MouthInterior,
    Rigid,
}

/// Classify a mesh by its own and its node's names. A wrong guess only costs
/// that mesh's weights, and the Face panel shows each mesh's role.
pub fn mesh_role(mesh_name: &str, node_name: &str) -> MeshRole {
    let n = format!("{} {}", mesh_name, node_name).to_lowercase();
    let has = |w: &str| n.contains(w);
    if has("lash") {
        MeshRole::Lashes
    } else if has("teeth") || has("tooth") || has("tongue") || has("mouth") || has("gum") {
        MeshRole::MouthInterior
    } else if has("hair") || has("beard") || has("hat") || has("helmet") {
        MeshRole::Rigid
    } else if (has("eye") || has("iris") || has("pupil") || has("cornea") || has("sclera")) && !has("brow") && !has("lid") {
        MeshRole::Eye
    } else {
        MeshRole::Skin
    }
}

/// The face's frame and measurements.
#[derive(Clone, Copy, Debug)]
pub struct FaceFrame {
    pub head_center: Vec3,
    pub eye_l: Vec3,
    pub eye_r: Vec3,
    pub eye_radius: f32,
    /// Toward the character's left.
    pub left: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    /// Distance between the eye centres: the face's unit of length.
    pub scale: f32,
    /// Whether the eyes are meshes of their own. When they are painted on
    /// the skin (a scan), the lids are left unweighted: the eye is the skin
    /// under them, so a blink would drag the painted eye down with the lid
    /// rather than cover it.
    pub eyes_modelled: bool,
}

impl FaceFrame {
    pub fn rotation(&self) -> Quat {
        Quat::from_mat3(&Mat3::from_cols(self.left, self.up, self.forward)).normalize()
    }
}

/// Measure the face: eyes, centre and orientation. Needs eye meshes; see
/// [`measure_with`] for a head whose eyes are painted on.
pub fn measure(glb: &Glb) -> Result<FaceFrame, GlbError> {
    measure_with(glb, None)
}

/// [`measure`], falling back to the markers' eye point when the head has no
/// eye meshes.
pub fn measure_with(glb: &Glb, markers: Option<&FaceMarkers>) -> Result<FaceFrame, GlbError> {
    let world = glb.world_matrices();
    let mut skin_pts: Vec<Vec3> = Vec::new();
    let mut eye_pts: Vec<Vec3> = Vec::new();
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(mesh) = n.get("mesh").and_then(|m| m.as_u64()).map(|m| m as usize) else { continue };
        let role = mesh_role(&glb.mesh_name(mesh), &glb.node_name(node));
        let prims = glb.meshes()[mesh].get("primitives").and_then(|p| p.as_array()).map_or(0, Vec::len);
        for prim in 0..prims {
            let pts = glb.positions(mesh, prim)?;
            let w = world[node];
            match role {
                MeshRole::Eye => eye_pts.extend(pts.iter().map(|p| w.transform_point3(*p))),
                MeshRole::Skin => skin_pts.extend(pts.iter().map(|p| w.transform_point3(*p))),
                _ => {}
            }
        }
    }
    if skin_pts.is_empty() {
        return Err(GlbError::Unsupported("no skin mesh found to rig".into()));
    }
    let (smin, smax) = bounds(&skin_pts);
    let head_center = (smin + smax) * 0.5;
    let up = Vec3::Y;
    if eye_pts.is_empty() {
        return match markers.and_then(|m| m.eye.map(|eye| (eye, m))) {
            Some((eye, m)) => painted_eyes(head_center, eye, m),
            None => Err(GlbError::Unsupported("no eye meshes found: place a point on one eye, or name the eye meshes `eye`".into())),
        };
    }

    // Split the eyes along the axis they spread over.
    let (emin, emax) = bounds(&eye_pts);
    let spread = emax - emin;
    let axis = if spread.x >= spread.z { Vec3::X } else { Vec3::Z };
    let mid = ((emin + emax) * 0.5).dot(axis);
    let (a, b): (Vec<Vec3>, Vec<Vec3>) = eye_pts.iter().partition(|p| p.dot(axis) < mid);
    if a.is_empty() || b.is_empty() {
        return Err(GlbError::Unsupported("could not tell the two eyes apart".into()));
    }
    let (amin, amax) = bounds(&a);
    let (bmin, bmax) = bounds(&b);
    let (ca, cb) = ((amin + amax) * 0.5, (bmin + bmax) * 0.5);
    let eye_radius = ((amax - amin).max_element().max((bmax - bmin).max_element())) * 0.5;

    // Forward: from the head's centre toward the eyes, level.
    let eyes_mid = (ca + cb) * 0.5;
    let mut forward = eyes_mid - head_center;
    forward.y = 0.0;
    let forward = forward.try_normalize().unwrap_or(Vec3::Z);
    let left = up.cross(forward).normalize();
    let forward = left.cross(up).normalize();
    let (eye_l, eye_r) = if (ca - cb).dot(left) > 0.0 { (ca, cb) } else { (cb, ca) };
    Ok(FaceFrame { head_center, eye_l, eye_r, eye_radius, left, up, forward, scale: (eye_l - eye_r).length().max(1e-4), eyes_modelled: true })
}

/// A human eyeball's radius as a share of the distance between the eyes:
/// about 12 mm against 63.
const EYE_RADIUS_PER_SPACING: f32 = 0.19;

/// The frame of a head whose eyes are painted on, from the point clicked on
/// one eye. Forward is from the head's centre to the lips, level, so the
/// middle of the face is the plane through the upper lip; the other eye is
/// the clicked one mirrored across it, and each centre sits an eyeball's
/// radius behind the surface.
fn painted_eyes(head_center: Vec3, eye: Vec3, m: &FaceMarkers) -> Result<FaceFrame, GlbError> {
    let up = Vec3::Y;
    let mut forward = m.upper_lip - head_center;
    forward.y = 0.0;
    let forward = forward.try_normalize().unwrap_or(Vec3::Z);
    let left = up.cross(forward).normalize();
    let side = (eye - m.upper_lip).dot(left);
    let scale = 2.0 * side.abs();
    if scale < 1e-4 {
        return Err(GlbError::Unsupported("the eye point is on the middle of the face: place it on the middle of one eye".into()));
    }
    let radius = EYE_RADIUS_PER_SPACING * scale;
    let clicked = eye - forward * radius;
    let mirrored = clicked - left * side * 2.0;
    let (eye_l, eye_r) = if side > 0.0 { (clicked, mirrored) } else { (mirrored, clicked) };
    Ok(FaceFrame { head_center, eye_l, eye_r, eye_radius: radius, left, up, forward, scale, eyes_modelled: false })
}

fn bounds(pts: &[Vec3]) -> (Vec3, Vec3) {
    pts.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
}

/// The glTF extras key on a lid joint holding how many degrees it turns to
/// close: measured from the eye when the rig was built (see [`EyeOpening`]).
pub const BLINK_EXTRA: &str = "renzora_blink_degrees";

/// Beside [`BLINK_EXTRA`]: how far forward the lid moves as it shuts, in
/// face-scale units (see [`EyeOpening::push`]).
pub const BLINK_PUSH_EXTRA: &str = "renzora_blink_push";

/// Bone names, which the Face panel and the expressions find the rig by.
pub mod names {
    pub const HEAD: &str = "face_head";
    pub const JAW: &str = "face_jaw";
    pub const LIP_UPPER: &str = "face_lip_upper";
    pub const LIP_LOWER: &str = "face_lip_lower";
    pub const CORNER_L: &str = "face_mouth_corner_l";
    pub const CORNER_R: &str = "face_mouth_corner_r";
    pub const EYE_L: &str = "face_eye_l";
    pub const EYE_R: &str = "face_eye_r";
    pub const LID_UPPER_L: &str = "face_lid_upper_l";
    pub const LID_UPPER_R: &str = "face_lid_upper_r";
    pub const LID_LOWER_L: &str = "face_lid_lower_l";
    pub const LID_LOWER_R: &str = "face_lid_lower_r";
    pub const BROW_L: &str = "face_brow_l";
    pub const BROW_R: &str = "face_brow_r";
}

/// The rig's bones, placed. Indices are fixed (see the constants below), so
/// the weighting can name them.
pub fn bones(frame: &FaceFrame, m: &FaceMarkers) -> Vec<Bone> {
    let s = frame.scale;
    let rot = frame.rotation();
    let corner_l = mirror_side(frame, m.mouth_corner, true);
    let corner_r = mirror_side(frame, m.mouth_corner, false);
    let mouth = lip_line(frame, m);
    let jaw_pivot = mouth - frame.forward * s * 1.3 + frame.up * s * 0.45;
    let head_pivot = Vec3::new(frame.head_center.x, m.chin.y - s * 0.6, frame.head_center.z);
    let brow = |eye: Vec3| eye + frame.up * frame.eye_radius * 1.9 + frame.forward * frame.eye_radius * 0.6;
    let b = |name: &str, parent: Option<usize>, head: Vec3| Bone { name: name.to_string(), parent, head, rotation: rot };
    vec![
        b(names::HEAD, None, head_pivot),
        b(names::JAW, Some(HEAD), jaw_pivot),
        b(names::LIP_UPPER, Some(HEAD), m.upper_lip),
        b(names::LIP_LOWER, Some(JAW), m.lower_lip),
        b(names::CORNER_L, Some(HEAD), corner_l),
        b(names::CORNER_R, Some(HEAD), corner_r),
        b(names::EYE_L, Some(HEAD), frame.eye_l),
        b(names::EYE_R, Some(HEAD), frame.eye_r),
        b(names::LID_UPPER_L, Some(HEAD), frame.eye_l),
        b(names::LID_UPPER_R, Some(HEAD), frame.eye_r),
        b(names::LID_LOWER_L, Some(HEAD), frame.eye_l),
        b(names::LID_LOWER_R, Some(HEAD), frame.eye_r),
        b(names::BROW_L, Some(HEAD), brow(frame.eye_l)),
        b(names::BROW_R, Some(HEAD), brow(frame.eye_r)),
    ]
}

const HEAD: usize = 0;
const JAW: usize = 1;
const LIP_UPPER: usize = 2;
const LIP_LOWER: usize = 3;
const CORNER_L: usize = 4;
const CORNER_R: usize = 5;
const EYE_L: usize = 6;
const EYE_R: usize = 7;
const LID_UPPER_L: usize = 8;
const LID_UPPER_R: usize = 9;
const LID_LOWER_L: usize = 10;
const LID_LOWER_R: usize = 11;
const BROW_L: usize = 12;
const BROW_R: usize = 13;

/// The point on the requested side (left when `left`), mirroring `p` across
/// the face's centre plane if it is on the other side.
fn mirror_side(frame: &FaceFrame, p: Vec3, left: bool) -> Vec3 {
    let centre = (frame.eye_l + frame.eye_r) * 0.5;
    let side = (p - centre).dot(frame.left);
    if (side > 0.0) == left {
        p
    } else {
        p - frame.left * side * 2.0
    }
}

/// Where the lips meet: the line the jaw's skin splits from the head's.
///
/// At the mouth corners' height, not halfway between the lip points. The lip
/// points can be anywhere on their lip (the panel asks for "the middle of the
/// lip", and a click right on the line between the lips can fall through the
/// gap and miss), so their midpoint wanders by as much as a lip is thick. A
/// split below the real line left the top of the lower lip on the head, and
/// opening the jaw stretched it into a sheet instead of opening the mouth. The
/// corners are where the line ends, so they sit on it. Clamped between the lip
/// points, for a corner placed carelessly high or low.
fn lip_line(frame: &FaceFrame, m: &FaceMarkers) -> Vec3 {
    let mid = (m.upper_lip + m.lower_lip) * 0.5;
    let (a, b) = (m.upper_lip.dot(frame.up), m.lower_lip.dot(frame.up));
    let height = m.mouth_corner.dot(frame.up).clamp(a.min(b), a.max(b));
    mid + frame.up * (height - mid.dot(frame.up))
}

/// For each vertex of a mouth-interior surface (teeth, gums, tongue), whether
/// the piece it is part of goes with the jaw: `Some(true)` jaw, `Some(false)`
/// head, `None` for a piece that straddles the lips (decide per vertex).
///
/// Split per vertex by height, a tooth sitting on the line between the lips
/// went half with the jaw and half with the head, and opening the mouth
/// stretched it. Teeth are rigid, so each piece (connected across its
/// triangles) moves whole: with the jaw when its middle is below the line. Only a piece lying
/// evenly across the line, an upper and lower set modelled as one, falls back
/// to the split, since sending it all one way would leave half a mouth behind.
fn mouth_pieces(frame: &FaceFrame, m: &FaceMarkers, s: &Surface) -> Vec<Option<bool>> {
    let n = s.positions.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let join = |parent: &mut Vec<usize>, a: usize, b: usize| {
        let (ra, rb) = (find(parent, a), find(parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    };
    // Joined by triangles only, never by position. Teeth sit against the
    // gums, touching them at the same points, so welding by position (as
    // the lip measure does, for UV seams) merged Angelica's upper teeth,
    // lower teeth, both gums and the tongue into one piece, and every tooth
    // fell back to the per-vertex split that stretches it.
    for t in &s.triangles {
        let [a, b, c] = t.map(|x| x as usize);
        if a < n && b < n && c < n {
            join(&mut parent, a, b);
            join(&mut parent, b, c);
        }
    }
    let line = lip_line(frame, m).dot(frame.up);
    let height_of = |p: Vec3| p.dot(frame.up) - line;
    // Per piece: lowest, highest, and summed height.
    let mut pieces: std::collections::HashMap<usize, (f32, f32, f32, usize)> = std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        let h = height_of(s.positions[i]);
        let e = pieces.entry(root).or_insert((f32::MAX, f32::MIN, 0.0, 0));
        *e = (e.0.min(h), e.1.max(h), e.2 + h, e.3 + 1);
    }
    let mut decision: std::collections::HashMap<usize, Option<bool>> = pieces
        .iter()
        .map(|(&root, &(lo, hi, sum, count))| {
            let mean = sum / count.max(1) as f32;
            let straddles = lo < 0.0 && hi > 0.0 && mean.abs() < 0.2 * (hi - lo);
            (root, (!straddles).then_some(mean < 0.0))
        })
        .collect();
    // The inside of a mouth need not divide where the lips meet: Angelica's
    // teeth sit higher than her lips, so her lower teeth and gum were
    // centred right on that line, straddling it. Where some pieces are
    // clearly up and some clearly down, the mouth divides halfway between
    // the two groups, and a straddling piece goes by which side of that its
    // middle is.
    let group_mean = |jaw: bool| {
        let (sum, count) = pieces
            .iter()
            .filter(|(r, _)| decision.get(r).copied().flatten() == Some(jaw))
            .fold((0.0f32, 0usize), |(s, c), (_, p)| (s + p.2, c + p.3));
        (count > 0).then(|| sum / count as f32)
    };
    if let (Some(down), Some(up)) = (group_mean(true), group_mean(false)) {
        let divide = (down + up) * 0.5;
        for (root, p) in &pieces {
            if decision.get(root).copied().flatten().is_none() {
                decision.insert(*root, Some((p.2 / p.3.max(1) as f32) < divide));
            }
        }
    }
    let roots: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();

    // A smaller piece follows the larger piece it is attached to, whatever
    // its own height says: teeth go with the gum they sit in, a tongue with
    // the floor of the mouth. Height judges the big pieces well (a gum, the
    // lining) and the small ones badly, because a mouth is rarely level:
    // stylized_face's lower molars sit higher than his upper front teeth, and
    // by height half his lower teeth and his tongue stayed with the head.
    // Larger pieces are settled first, so a chain (tooth, gum) resolves in one
    // pass; a piece follows only one at least half again its size, so a gum
    // does not follow the lining it touches at the back of the mouth.
    // "Attached" includes a small gap: stylized_face's tongue rests over his
    // lower gum without touching it, 3.5% of the face away.
    let near = 0.05 * frame.scale.max(1e-6);
    let cell = |p: Vec3| ((p.x / near).floor() as i64, (p.y / near).floor() as i64, (p.z / near).floor() as i64);
    {
        let mut by_cell: std::collections::HashMap<(i64, i64, i64), Vec<usize>> = std::collections::HashMap::new();
        for i in 0..n {
            by_cell.entry(cell(s.positions[i])).or_default().push(i);
        }
        let size = |r: usize| pieces.get(&r).map_or(0, |p| p.3);
        let mut order: Vec<usize> = pieces.keys().copied().collect();
        order.sort_by_key(|r| std::cmp::Reverse(size(*r)));
        for root in order {
            // How close this piece comes to each other piece nearby. The host
            // is the larger piece it comes closest to, not the one it is near
            // most of: an upper front tooth hangs down beside the lower teeth
            // and gum for most of its length, but it is rooted in its own gum.
            let mut closest: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
            for i in (0..n).filter(|&i| roots[i] == root) {
                let (p, c) = (s.positions[i], cell(s.positions[i]));
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            for &j in by_cell.get(&(c.0 + dx, c.1 + dy, c.2 + dz)).into_iter().flatten() {
                                if roots[j] == root || size(roots[j]) * 2 < size(root) * 3 {
                                    continue;
                                }
                                let d = s.positions[j].distance(p);
                                if d < near {
                                    let e = closest.entry(roots[j]).or_insert(f32::MAX);
                                    *e = e.min(d);
                                }
                            }
                        }
                    }
                }
            }
            let host = closest.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(|(other, _)| other);
            if let Some(side) = host.and_then(|h| decision.get(&h).copied().flatten()) {
                decision.insert(root, Some(side));
            }
        }
    }
    let mut out: Vec<Option<bool>> = roots.iter().map(|r| decision.get(r).copied().flatten()).collect();

    // With nothing on one side to measure the divide from, a piece across
    // the line follows what it is attached to: a row of teeth sits in its
    // gum, touching it. Touching is being within a little of a decided
    // piece's vertex; the side touched most wins.
    let mut grid: std::collections::HashMap<(i64, i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (i, side) in out.iter().enumerate().take(n) {
        if side.is_some() {
            grid.entry(cell(s.positions[i])).or_default().push(i);
        }
    }
    let undecided_roots: std::collections::HashSet<usize> = (0..n).filter(|&i| out[i].is_none()).map(|i| roots[i]).collect();
    for root in undecided_roots {
        // (jaw, head) touches.
        let mut votes = (0usize, 0usize);
        for i in (0..n).filter(|&i| roots[i] == root) {
            let p = s.positions[i];
            let c = cell(p);
            let mut touched: Option<bool> = None;
            'search: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for &j in grid.get(&(c.0 + dx, c.1 + dy, c.2 + dz)).into_iter().flatten() {
                            if s.positions[j].distance(p) < near {
                                touched = out[j];
                                break 'search;
                            }
                        }
                    }
                }
            }
            match touched {
                Some(true) => votes.0 += 1,
                Some(false) => votes.1 += 1,
                None => {}
            }
        }
        if votes.0 + votes.1 > 0 {
            let jaw = votes.0 > votes.1;
            for i in 0..n {
                if roots[i] == root {
                    out[i] = Some(jaw);
                }
            }
        }
    }

    // A mouth modelled as one surface (teeth, gums and the lining all
    // joined, as Angelica's is): each vertex goes with whichever is nearer
    // across that surface, the part well below the lips or the part well
    // above. An upper tooth hangs from the upper gum, so it is near the head
    // across the surface even with a lower tooth a hair below it, and the
    // lining between the two, which joins them, is what stretches.
    if out.iter().any(Option::is_none) {
        let scale = frame.scale;
        let band = 0.12 * scale;
        let (welded, points, edges) = welded_graph(s, scale);
        let height = |w: u32| height_of(points[w as usize]);
        let undecided: Vec<u32> = (0..n).filter(|&i| out[i].is_none()).map(|i| welded[i]).collect();
        let jaw: Vec<u32> = undecided.iter().copied().filter(|&w| height(w) < -band).collect();
        let head: Vec<u32> = undecided.iter().copied().filter(|&w| height(w) > band).collect();
        if !jaw.is_empty() && !head.is_empty() {
            let to_jaw = surface_distances(&edges, &jaw, f32::INFINITY);
            let to_head = surface_distances(&edges, &head, f32::INFINITY);
            for i in 0..n {
                if out[i].is_none() {
                    let w = welded[i] as usize;
                    out[i] = Some(to_jaw[w] < to_head[w]);
                }
            }
        }
    }
    out
}

/// [`mouth_pieces`] over every mouth-interior primitive of the model at once,
/// by `(mesh, primitive)`.
///
/// Decided mesh by mesh, a mouth split into separate meshes had nothing to
/// measure against: stylized_face's tongue is a mesh of its own, so with no
/// clearly-up and clearly-down pieces beside it in the same mesh it fell back
/// to the line between the lips, which sits too low for that mouth, and stayed
/// with the head. Put together, the upper teeth in one mesh and the tongue in
/// another set the divide for all of them. Pieces still never join across
/// meshes: each primitive's triangles are only joined to themselves.
fn mouth_decisions(glb: &Glb, frame: &FaceFrame, m: &FaceMarkers) -> Result<std::collections::HashMap<(usize, usize), Vec<Option<bool>>>, GlbError> {
    let world = glb.world_matrices();
    let mut positions: Vec<Vec3> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut spans: Vec<((usize, usize), usize, usize)> = Vec::new();
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(mesh) = n.get("mesh").and_then(|v| v.as_u64()).map(|v| v as usize) else { continue };
        if mesh_role(&glb.mesh_name(mesh), &glb.node_name(node)) != MeshRole::MouthInterior {
            continue;
        }
        let prims = glb.meshes()[mesh].get("primitives").and_then(|p| p.as_array()).map_or(0, Vec::len);
        for prim in 0..prims {
            // A mesh used by two nodes is decided once, like skin::apply keys it.
            if spans.iter().any(|(k, ..)| *k == (mesh, prim)) {
                continue;
            }
            let pts = glb.positions(mesh, prim)?;
            let first = positions.len();
            let base = first as u32;
            positions.extend(pts.iter().map(|p| world[node].transform_point3(*p)));
            match glb.indices(mesh, prim)? {
                Some(idx) => triangles.extend(idx.chunks_exact(3).map(|t| [t[0] + base, t[1] + base, t[2] + base])),
                None => triangles.extend((0..pts.len() as u32 / 3).map(|t| [base + t * 3, base + t * 3 + 1, base + t * 3 + 2])),
            }
            spans.push(((mesh, prim), first, positions.len()));
        }
    }
    if positions.is_empty() {
        return Ok(Default::default());
    }
    let all = mouth_pieces(frame, m, &Surface { mesh: usize::MAX, prim: 0, positions, triangles });
    Ok(spans.into_iter().map(|(key, a, b)| (key, all[a..b].to_vec())).collect())
}

/// `(side, reach)` for one vertex; see [`lip_sides`].
type LipSide = (f32, f32);

/// For each vertex of a skin surface: which lip it belongs with, measured
/// across the skin, and how far to trust that over height.
///
/// `(side, reach)`: `side` is 1 with the lower lip and chin and 0 with the
/// upper lip, by which is nearer along the surface; `reach` is 1 near the
/// mouth and fades to 0 away from it, where height decides as before.
///
/// Height cannot tell the lips apart on its own. A closed mouth folds both
/// lips inward at the line where they meet, so the inside of the lower lip
/// rises above that line and the inside of the upper lip dips below it, in a
/// band thinner than a lip. Split by height, each lip's inside went with the
/// other bone and opening the jaw stretched it into a sheet. Across the skin
/// the two are far apart (they join only at the corners, or deep inside a
/// mouth cavity), so the nearer lip along the surface is the right one.
///
/// `None` when this surface does not hold both lip points, as on a head whose
/// lips are a separate mesh: height decides there.
fn lip_sides(frame: &FaceFrame, m: &FaceMarkers, surface: &Surface) -> Option<Vec<(f32, f32)>> {
    let s = frame.scale;
    let (welded, points, edges) = welded_graph(surface, s);

    // The points were clicked on the skin, so each sits on a vertex of the
    // surface it was clicked on, and nowhere near any other surface.
    let on_surface = |target: Vec3| {
        let (i, d) = points
            .iter()
            .enumerate()
            .map(|(i, p)| (i as u32, p.distance(target)))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        (d < 0.1 * s).then_some(i)
    };
    let upper = on_surface(m.upper_lip)?;
    let lower = on_surface(m.lower_lip)?;
    let lower_seeds: Vec<u32> = std::iter::once(lower).chain(on_surface(m.chin)).collect();

    // Far enough to cover the lips, the corners and the front of a mouth
    // cavity; anything farther is left to height.
    let limit = 1.2 * s;
    let to_lower = surface_distances(&edges, &lower_seeds, limit);
    let to_upper = surface_distances(&edges, &[upper], limit);

    Some(
        welded
            .iter()
            .map(|&w| {
                let (dl, du) = (to_lower[w as usize], to_upper[w as usize]);
                let reach = 1.0 - smoothstep(0.6 * s, limit, dl.min(du));
                let side = match (dl.is_finite(), du.is_finite()) {
                    (true, true) => 1.0 - smoothstep(0.4, 0.6, dl / (dl + du).max(1e-9)),
                    (true, false) => 1.0,
                    _ => 0.0,
                };
                (side, reach)
            })
            .collect(),
    )
}

/// A surface as a graph to measure across: each vertex's node, the nodes'
/// positions, and each node's neighbours with the edge lengths.
///
/// One node per point, not per vertex: a UV seam splits a point into several
/// vertices that share no edge, which would cut the surface.
fn welded_graph(surface: &Surface, scale: f32) -> (Vec<u32>, Vec<Vec3>, Vec<Vec<(u32, f32)>>) {
    let quantum = (scale * 1e-4).max(1e-9);
    let mut ids: std::collections::HashMap<(i64, i64, i64), u32> = std::collections::HashMap::new();
    let mut points: Vec<Vec3> = Vec::new();
    let welded: Vec<u32> = surface
        .positions
        .iter()
        .map(|p| {
            let key = ((p.x / quantum).round() as i64, (p.y / quantum).round() as i64, (p.z / quantum).round() as i64);
            *ids.entry(key).or_insert_with(|| {
                points.push(*p);
                (points.len() - 1) as u32
            })
        })
        .collect();
    let mut edges: Vec<Vec<(u32, f32)>> = vec![Vec::new(); points.len()];
    for t in &surface.triangles {
        let Some(w) = t.iter().map(|&i| welded.get(i as usize).copied()).collect::<Option<Vec<u32>>>() else { continue };
        for (a, b) in [(w[0], w[1]), (w[1], w[2]), (w[2], w[0])] {
            if a != b {
                let d = points[a as usize].distance(points[b as usize]);
                edges[a as usize].push((b, d));
                edges[b as usize].push((a, d));
            }
        }
    }
    (welded, points, edges)
}

/// Shortest distance along `edges` from any of `seeds` to every node, up to
/// `limit`; farther nodes are infinite.
fn surface_distances(edges: &[Vec<(u32, f32)>], seeds: &[u32], limit: f32) -> Vec<f32> {
    use std::cmp::Reverse;
    let mut dist = vec![f32::INFINITY; edges.len()];
    // Distances as integer micro-units, so the heap orders them without
    // a float wrapper; precision far below any mesh's edge length.
    let key = |d: f32| (d * 1e6) as u64;
    let mut heap = std::collections::BinaryHeap::new();
    for &seed in seeds {
        dist[seed as usize] = 0.0;
        heap.push(Reverse((0u64, seed)));
    }
    while let Some(Reverse((k, node))) = heap.pop() {
        let d = dist[node as usize];
        if k > key(d) {
            continue;
        }
        for &(next, len) in &edges[node as usize] {
            let nd = d + len;
            if nd < dist[next as usize] && nd <= limit {
                dist[next as usize] = nd;
                heap.push(Reverse((key(nd), next)));
            }
        }
    }
    dist
}

/// How far one eye is open: the angles of its upper and lower lid margins
/// straight in front of the pupil, in degrees about the eye's centre in the
/// face's up/forward plane, up positive.
///
/// A blink used to turn every head's lids by the same 36° and 8°, with the
/// skin at the lid margin weighted only partly to the lid. Angelica's lids
/// stopped a sliver short, and a cartoon eye that opens through 50° or more
/// could never close at all. Measured from the head itself, each lid turns
/// exactly as far as its own eye needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeOpening {
    pub upper: f32,
    pub lower: f32,
    /// How far forward (model units) the lids move as they close, so the
    /// shut lid clears the front of the eyeball. A lid turns about the eye's
    /// centre, so its margin stays at the distance it started at; on a head
    /// whose cornea bulges past the lid line (most stylised eyes) the closed
    /// lid passed behind the eyeball and the eye showed through it.
    pub push: f32,
}

/// How far past the meeting line each lid goes, degrees, so the margins press
/// together instead of stopping edge to edge with a hairline of eye between.
const LID_OVERLAP: f32 = 3.0;

impl EyeOpening {
    /// Where the lids meet when closed. A third of the way up from the lower
    /// lid: the upper lid does most of a blink, the lower one rises a little.
    pub fn meet(&self) -> f32 {
        self.lower + (self.upper - self.lower) / 3.0
    }

    /// Degrees the upper lid turns down to close.
    pub fn close_upper(&self) -> f32 {
        self.upper - self.meet() + LID_OVERLAP
    }

    /// Degrees the lower lid turns up to close.
    pub fn close_lower(&self) -> f32 {
        self.meet() - self.lower + LID_OVERLAP
    }
}

/// Every triangle of the head's skin meshes, in model space.
fn skin_triangles(glb: &Glb) -> Result<Vec<[Vec3; 3]>, GlbError> {
    let world = glb.world_matrices();
    let mut out = Vec::new();
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(mesh) = n.get("mesh").and_then(|m| m.as_u64()).map(|m| m as usize) else { continue };
        if mesh_role(&glb.mesh_name(mesh), &glb.node_name(node)) != MeshRole::Skin {
            continue;
        }
        let prims = glb.meshes()[mesh].get("primitives").and_then(|p| p.as_array()).map_or(0, Vec::len);
        for prim in 0..prims {
            let pts: Vec<Vec3> = glb.positions(mesh, prim)?.iter().map(|p| world[node].transform_point3(*p)).collect();
            let idx: Vec<u32> = match glb.indices(mesh, prim)? {
                Some(idx) => idx,
                None => (0..pts.len() as u32).collect(),
            };
            out.extend(idx.chunks_exact(3).filter_map(|t| Some([*pts.get(t[0] as usize)?, *pts.get(t[1] as usize)?, *pts.get(t[2] as usize)?])));
        }
    }
    Ok(out)
}

/// Distance along the ray to the triangle, either side facing.
fn ray_hit(origin: Vec3, dir: Vec3, t: &[Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - t[0];
    let u = s.dot(p) * inv;
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    let d = e2.dot(q) * inv;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && d > 0.0).then_some(d)
}

/// Measure one eye's opening: rays from the eye's centre, swept up and down
/// from straight ahead, until each meets the skin of a lid.
///
/// Within a few eye radii only, so the brow or the cheek far beyond is not
/// taken for a lid. `None` when straight ahead is covered (a head modelled
/// with its eyes shut, or skin without a hole for the eye) or when either
/// sweep finds no lid: the blink then keeps its stock angles.
///
/// `front` is how far the eyeball reaches in front of its centre; the lids
/// are pushed forward by whatever that exceeds the lid margins' distance.
fn measure_eye(frame: &FaceFrame, eye: Vec3, skin: &[[Vec3; 3]], front: f32) -> Option<EyeOpening> {
    let reach = 3.0 * frame.eye_radius;
    let hit = |deg: f32| {
        let a = deg.to_radians();
        let dir = (frame.forward * a.cos() + frame.up * a.sin()).normalize();
        skin.iter().filter_map(|t| ray_hit(eye, dir, t)).filter(|d| *d < reach).reduce(f32::min)
    };
    if hit(0.0).is_some() {
        return None;
    }
    let edge = |step: f32| (1..=85).map(|i| i as f32 * step).find_map(|deg| hit(deg).map(|d| (deg - step * 0.5, d)));
    let ((upper, d_upper), (lower, d_lower)) = (edge(1.0)?, edge(-1.0)?);
    // A little past the eyeball, so the lid does not graze it.
    let push = (front * 1.05 - d_upper.min(d_lower)).max(0.0);
    Some(EyeOpening { upper, lower, push })
}

/// How far each eye's meshes reach in front of its centre: the point a shut
/// lid has to clear.
fn eye_fronts(glb: &Glb, frame: &FaceFrame) -> Result<[f32; 2], GlbError> {
    let world = glb.world_matrices();
    let mut front = [0.0f32; 2];
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(mesh) = n.get("mesh").and_then(|m| m.as_u64()).map(|m| m as usize) else { continue };
        if mesh_role(&glb.mesh_name(mesh), &glb.node_name(node)) != MeshRole::Eye {
            continue;
        }
        let prims = glb.meshes()[mesh].get("primitives").and_then(|p| p.as_array()).map_or(0, Vec::len);
        for prim in 0..prims {
            for p in glb.positions(mesh, prim)? {
                let p = world[node].transform_point3(p);
                let side = usize::from(p.distance(frame.eye_r) < p.distance(frame.eye_l));
                let centre = if side == 0 { frame.eye_l } else { frame.eye_r };
                let off = p - centre;
                // In front only, within an eye's size: not the other eye.
                if off.dot(frame.forward) > 0.0 && off.length() < 2.0 * frame.eye_radius {
                    front[side] = front[side].max(off.length());
                }
            }
        }
    }
    Ok(front)
}

/// Weights for one vertex. See the module doc for the rules. `lips` is the
/// vertex's [`lip_sides`] entry, when its surface has one; `eyes` the
/// measured opening of the left and right eye.
fn weigh(frame: &FaceFrame, m: &FaceMarkers, v: &Vertex, lips: Option<(f32, f32)>, eyes: [Option<EyeOpening>; 2], piece_on_jaw: Option<bool>) -> Vec<(usize, f32)> {
    let p = v.position;
    let s = frame.scale;
    let r = frame.eye_radius;
    let centre = (frame.eye_l + frame.eye_r) * 0.5;
    let is_left = (p - centre).dot(frame.left) > 0.0;
    let opening = eyes[usize::from(!is_left)];
    // Which side of the meeting line a point near the eye is on, 1 for the
    // upper lid: by angle about the eye when the opening was measured, so the
    // whole margin of each lid goes with it and the lids close fully. Sharp
    // only at the margin; farther from the eyeball the change is spread over
    // more degrees, because there the two lids turning opposite ways would
    // otherwise tear the skin along the line they meet at.
    let upper_side = |off: Vec3| match opening {
        Some(o) => {
            let deg = off.dot(frame.up).atan2(off.dot(frame.forward)).to_degrees();
            let width = 2.0 + 20.0 * smoothstep(0.0, 0.6 * r, off.length() - r);
            smoothstep(o.meet() - width, o.meet() + width, deg)
        }
        None => smoothstep(-0.15, 0.25, off.dot(frame.up) / r.max(1e-6)),
    };
    let mouth = lip_line(frame, m);
    let below_mouth = (mouth - p).dot(frame.up);
    let lateral = ((p - mouth) - frame.up * (p - mouth).dot(frame.up)).length();

    let jaw_share = || {
        // Sharp at the lips, softer out across the cheeks.
        let soft = 0.02 * s + 0.5 * lateral;
        let by_height = smoothstep(-0.25 * soft, soft, below_mouth);
        // Near the mouth, which lip the vertex is nearer across the skin.
        let v_ = match lips {
            Some((side, reach)) => by_height + (side - by_height) * reach,
            None => by_height,
        };
        let jaw_pivot = mouth - frame.forward * s * 1.3 + frame.up * s * 0.45;
        let back = smoothstep(-0.15 * s, 0.35 * s, (p - jaw_pivot).dot(frame.forward));
        let neck = 1.0 - smoothstep(0.1 * s, 0.55 * s, (m.chin - p).dot(frame.up));
        v_ * back * neck
    };

    match mesh_role(v.mesh_name, v.node_name) {
        MeshRole::Eye => vec![(if is_left { EYE_L } else { EYE_R }, 1.0)],
        MeshRole::Lashes => {
            let eye = if is_left { frame.eye_l } else { frame.eye_r };
            let upper = match opening {
                Some(_) => upper_side(p - eye) > 0.5,
                None => (p - eye).dot(frame.up) > -0.1 * r,
            };
            let bone = match (is_left, upper) {
                (true, true) => LID_UPPER_L,
                (true, false) => LID_LOWER_L,
                (false, true) => LID_UPPER_R,
                (false, false) => LID_LOWER_R,
            };
            vec![(bone, 1.0)]
        }
        MeshRole::MouthInterior => vec![(if piece_on_jaw.unwrap_or(below_mouth > 0.0) { JAW } else { HEAD }, 1.0)],
        MeshRole::Rigid => vec![(HEAD, 1.0)],
        MeshRole::Skin => {
            let mut out: Vec<(usize, f32)> = Vec::with_capacity(6);
            let jaw = jaw_share();
            // Lips: a disc round each lip's middle, on its own side of the mouth.
            let disc = |c: Vec3, inner: f32, outer: f32| 1.0 - smoothstep(inner * s, outer * s, (p - c).length());
            let lower = jaw * disc(m.lower_lip, 0.08, 0.35);
            let upper = (1.0 - jaw) * disc(m.upper_lip, 0.08, 0.35);
            out.push((JAW, (jaw - lower).max(0.0)));
            out.push((LIP_LOWER, lower));
            out.push((LIP_UPPER, upper));
            let corner_l = mirror_side(frame, m.mouth_corner, true);
            let corner_r = mirror_side(frame, m.mouth_corner, false);
            out.push((CORNER_L, 0.85 * disc(corner_l, 0.06, 0.45)));
            out.push((CORNER_R, 0.85 * disc(corner_r, 0.06, 0.45)));

            // Lids: the skin hugging the eyeball, in front, upper and lower half.
            let (eye, lid_u, lid_l, brow_bone) =
                if is_left { (frame.eye_l, LID_UPPER_L, LID_LOWER_L, BROW_L) } else { (frame.eye_r, LID_UPPER_R, LID_LOWER_R, BROW_R) };
            let off = p - eye;
            let near = 1.0 - smoothstep(0.0, 1.2 * r, off.length() - r);
            let front = smoothstep(-0.2 * r, 0.4 * r, off.dot(frame.forward));
            match opening {
                // Painted eyes: the skin under a lid is the eye itself.
                _ if !frame.eyes_modelled => {}
                // Measured: each lid takes its whole side of the meeting
                // line, margins at full weight, so they travel all the way.
                // Only the skin hugging the eyeball, though: a measured blink
                // turns farther than the stock one, and with the stock reach
                // it dragged the bridge of the nose and the socket under the
                // brow along with the lids.
                // And only across the opening: past the corners of the eye
                // there is nothing for the lids to close over, and skin
                // there split at the meeting line tore along it, out
                // sideways from the closed eye towards the nose and temple.
                Some(_) => {
                    let shell = 1.0 - smoothstep(0.1 * r, 0.6 * r, off.length() - r);
                    let across = 1.0 - smoothstep(0.8 * r, 1.4 * r, off.dot(frame.left).abs());
                    let up = upper_side(off);
                    out.push((lid_u, shell * front * across * up));
                    out.push((lid_l, shell * front * across * (1.0 - up)));
                }
                None => {
                    let vert = off.dot(frame.up) / r.max(1e-6);
                    out.push((lid_u, near * front * smoothstep(-0.15, 0.25, vert)));
                    out.push((lid_l, 0.8 * near * front * (1.0 - smoothstep(-0.35, -0.05, vert))));
                }
            }
            let brow = eye + frame.up * r * 1.9 + frame.forward * r * 0.6;
            out.push((brow_bone, 0.9 * (1.0 - smoothstep(0.3 * r, 1.6 * r, (p - brow).length()))));

            // Whatever the face bones did not take is the head's.
            let taken: f32 = out.iter().map(|(_, w)| w).sum();
            if taken > 1.0 {
                for (_, w) in &mut out {
                    *w /= taken;
                }
            }
            out.push((HEAD, (1.0 - taken).max(0.0)));
            out
        }
    }
}

/// Rig the head in `glb` from `markers` (model space). Returns what was built
/// and the measured frame.
pub fn build(glb: &mut Glb, markers: &FaceMarkers) -> Result<(Applied, FaceFrame), GlbError> {
    let frame = measure_with(glb, Some(markers))?;
    let bones = bones(&frame, markers);
    let eyes = if frame.eyes_modelled {
        let skin = skin_triangles(glb)?;
        let front = eye_fronts(glb, &frame)?;
        [measure_eye(&frame, frame.eye_l, &skin, front[0]), measure_eye(&frame, frame.eye_r, &skin, front[1])]
    } else {
        [None; 2]
    };
    // Worked out once per skin surface, then read per vertex.
    let mut sides: std::collections::HashMap<(usize, usize), Option<Vec<LipSide>>> = std::collections::HashMap::new();
    let pieces = mouth_decisions(glb, &frame, markers)?;
    let applied = skin::apply(glb, &bones, HEAD, |v| {
        let role = mesh_role(v.mesh_name, v.node_name);
        let lips = v
            .surface
            .filter(|_| role == MeshRole::Skin)
            .and_then(|s| sides.entry((s.mesh, s.prim)).or_insert_with(|| lip_sides(&frame, markers, s)).as_ref())
            .and_then(|all| all.get(v.index).copied());
        let piece_on_jaw = v
            .surface
            .filter(|_| role == MeshRole::MouthInterior)
            .and_then(|s| pieces.get(&(s.mesh, s.prim)).and_then(|all| all.get(v.index)).copied())
            .flatten();
        Some(weigh(&frame, markers, v, lips, eyes, piece_on_jaw))
    })?;
    // The closing angles travel with the model, on the lid joints, where the
    // editor finds them as the bones' glTF extras.
    for (opening, upper, lower) in [(eyes[0], names::LID_UPPER_L, names::LID_LOWER_L), (eyes[1], names::LID_UPPER_R, names::LID_LOWER_R)] {
        let Some(o) = opening else { continue };
        // The push in face-scale units, the expressions' unit of length.
        let push = o.push / frame.scale;
        for (bone, degrees) in [(upper, o.close_upper()), (lower, o.close_lower())] {
            if let Some(node) = glb.json["nodes"].as_array_mut().and_then(|ns| ns.iter_mut().find(|n| n["name"] == bone)) {
                node["extras"] = serde_json::json!({ BLINK_EXTRA: degrees, BLINK_PUSH_EXTRA: push });
            }
        }
    }
    Ok((applied, frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meshes_are_classified_by_name() {
        assert_eq!(mesh_role("Eye_Eye_0", ""), MeshRole::Eye);
        assert_eq!(mesh_role("EyeScleraReflect_EyeScleraReflect_0", ""), MeshRole::Eye);
        assert_eq!(mesh_role("Eyelashes_Eyelashes_0", ""), MeshRole::Lashes);
        assert_eq!(mesh_role("Mouth_Mouth_0", ""), MeshRole::MouthInterior);
        assert_eq!(mesh_role("Hair_Hair_0", ""), MeshRole::Rigid);
        assert_eq!(mesh_role("Head_Head_0", ""), MeshRole::Skin);
        assert_eq!(mesh_role("Eyebrows", ""), MeshRole::Skin);
    }

    fn frame() -> FaceFrame {
        FaceFrame {
            head_center: Vec3::new(0.0, 0.0, 0.0),
            eye_l: Vec3::new(0.03, 0.03, 0.08),
            eye_r: Vec3::new(-0.03, 0.03, 0.08),
            eye_radius: 0.012,
            left: Vec3::X,
            up: Vec3::Y,
            forward: Vec3::Z,
            scale: 0.06,
            eyes_modelled: true,
        }
    }

    fn markers() -> FaceMarkers {
        FaceMarkers {
            mouth_corner: Vec3::new(-0.022, -0.04, 0.085),
            upper_lip: Vec3::new(0.0, -0.035, 0.095),
            lower_lip: Vec3::new(0.0, -0.045, 0.094),
            chin: Vec3::new(0.0, -0.08, 0.085),
            eye: None,
        }
    }

    fn vertex(p: Vec3) -> Vertex<'static> {
        Vertex { mesh_name: "Head", node_name: "", position: p, index: 0, surface: None }
    }

    fn weight_of(bone: usize, p: Vec3) -> f32 {
        let w = weigh(&frame(), &markers(), &vertex(p), None, [None; 2], None);
        w.iter().filter(|(b, _)| *b == bone).map(|(_, w)| w).sum()
    }

    /// A closed mouth: two lips, each a strip folding inward, sharing only
    /// the corner columns. The inside of the lower lip rises above the line
    /// the lips meet at (the corners' height, -0.04) and the inside of the
    /// upper lip dips below it, as real closed mouths do.
    fn closed_mouth() -> Surface {
        let upper = [(-0.025, 0.092), (-0.035, 0.095), (-0.039, 0.092), (-0.041, 0.087), (-0.040, 0.080)];
        let lower = [(-0.060, 0.090), (-0.045, 0.094), (-0.041, 0.092), (-0.038, 0.087), (-0.039, 0.080)];
        let cols = 11;
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        for profile in [upper, lower] {
            let base = positions.len() as u32;
            for (y, z) in profile {
                for c in 0..cols {
                    // The end columns collapse onto the corners, which is
                    // the only place the two strips meet.
                    let x = -0.022 + c as f32 * 0.0044;
                    positions.push(if c == 0 || c == cols - 1 { Vec3::new(x, -0.04, 0.085) } else { Vec3::new(x, y, z) });
                }
            }
            for r in 0..profile.len() as u32 - 1 {
                for c in 0..cols as u32 - 1 {
                    let i = base + r * cols as u32 + c;
                    let below = i + cols as u32;
                    triangles.push([i, below, i + 1]);
                    triangles.push([i + 1, below, below + 1]);
                }
            }
        }
        Surface { mesh: 0, prim: 0, positions, triangles }
    }

    fn jaw_weight(surface: &Surface, p: Vec3, by_surface: bool) -> f32 {
        let index = surface.positions.iter().position(|q| q.distance(p) < 1e-6).expect("a vertex of the test mouth");
        let lips = by_surface.then(|| lip_sides(&frame(), &markers(), surface).expect("both lip points are on it")[index]);
        let v = Vertex { mesh_name: "Head", node_name: "", position: p, index, surface: Some(surface) };
        weigh(&frame(), &markers(), &v, lips, [None; 2], None).iter().filter(|(b, _)| *b == JAW || *b == LIP_LOWER).map(|(_, w)| w).sum()
    }

    /// The bug this exists for: split by height, the inside of each lip went
    /// with the other bone, and opening the jaw stretched instead of opened.
    #[test]
    fn the_inside_of_each_lip_goes_with_its_own_lip() {
        let mouth = closed_mouth();
        let inner_lower = Vec3::new(0.0, -0.038, 0.087);
        let inner_upper = Vec3::new(0.0, -0.041, 0.087);
        assert!(jaw_weight(&mouth, inner_lower, false) < 0.4, "height alone gives the inner lower lip to the head");
        assert!(jaw_weight(&mouth, inner_lower, true) > 0.6, "inner lower lip {}", jaw_weight(&mouth, inner_lower, true));
        assert!(jaw_weight(&mouth, inner_upper, true) < 0.4, "inner upper lip {}", jaw_weight(&mouth, inner_upper, true));
    }

    /// A surface without the lip points (lips modelled as their own mesh, an
    /// eyebrow card) has no say, and height decides.
    #[test]
    fn a_surface_without_the_lips_leaves_it_to_height() {
        let far = Surface { mesh: 0, prim: 0, positions: vec![Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.01, 0.5, 0.0), Vec3::new(0.0, 0.51, 0.0)], triangles: vec![[0, 1, 2]] };
        assert!(lip_sides(&frame(), &markers(), &far).is_none());
    }

    #[test]
    fn the_chin_follows_the_jaw_and_the_forehead_does_not() {
        let chin = weight_of(JAW, Vec3::new(0.0, -0.07, 0.085)) + weight_of(LIP_LOWER, Vec3::new(0.0, -0.07, 0.085));
        assert!(chin > 0.7, "chin jaw weight {chin}");
        assert!(weight_of(JAW, Vec3::new(0.0, 0.08, 0.08)) < 0.01);
    }

    /// A box of teeth from `lo` to `hi` (height), as its own piece of a
    /// surface, its vertices from `first`.
    fn tooth(lo: f32, hi: f32, first: u32, positions: &mut Vec<Vec3>, triangles: &mut Vec<[u32; 3]>) {
        for y in [lo, hi] {
            positions.extend([Vec3::new(-0.01, y, 0.09), Vec3::new(0.01, y, 0.09)]);
        }
        triangles.extend([[first, first + 1, first + 2], [first + 1, first + 3, first + 2]]);
    }

    /// Teeth move whole: an upper tooth dipping below the line between the
    /// lips stays with the head, a lower one rising above it goes with the
    /// jaw.
    #[test]
    fn teeth_move_as_pieces() {
        let (mut positions, mut triangles) = (Vec::new(), Vec::new());
        // The lips meet at -0.04 (the corners' height).
        tooth(-0.045, -0.02, 0, &mut positions, &mut triangles);
        tooth(-0.065, -0.035, 4, &mut positions, &mut triangles);
        let s = Surface { mesh: 0, prim: 0, positions, triangles };
        let pieces = mouth_pieces(&frame(), &markers(), &s);
        assert!(pieces[0..4].iter().all(|p| *p == Some(false)), "upper tooth with the head: {:?}", &pieces[0..4]);
        assert!(pieces[4..8].iter().all(|p| *p == Some(true)), "lower tooth with the jaw: {:?}", &pieces[4..8]);
    }

    /// A row of teeth reaching across the lips' line goes with the gum it
    /// sits in, not half each way.
    #[test]
    fn straddling_teeth_follow_their_gum() {
        let (mut positions, mut triangles) = (Vec::new(), Vec::new());
        // The lower gum, clearly below the line (-0.04).
        tooth(-0.08, -0.06, 0, &mut positions, &mut triangles);
        // Lower teeth rising from the gum's top to well above the line.
        tooth(-0.06, -0.01, 4, &mut positions, &mut triangles);
        let s = Surface { mesh: 0, prim: 0, positions, triangles };
        let pieces = mouth_pieces(&frame(), &markers(), &s);
        assert!(pieces.iter().all(|p| *p == Some(true)), "{pieces:?}");
    }

    /// A tilted mouth: a lower back tooth sits higher than the lips' line,
    /// but it sits in the lower gum, so it goes with the jaw.
    #[test]
    fn a_tooth_follows_its_gum_whatever_its_height() {
        let (mut positions, mut triangles) = (Vec::new(), Vec::new());
        // Upper gum, clearly above (large), lower gum, clearly below (large).
        let gum = |lo: f32, hi: f32, first: u32, positions: &mut Vec<Vec3>, triangles: &mut Vec<[u32; 3]>| {
            for k in 0..4 {
                tooth(lo, hi, first + k * 4, positions, triangles);
                let last = positions.len();
                for p in &mut positions[last - 4..] {
                    p.x += k as f32 * 0.02;
                }
                // Join the strips into one piece.
                if k > 0 {
                    let a = first + (k - 1) * 4;
                    let b = first + k * 4;
                    triangles.push([a + 1, b, a + 3]);
                }
            }
        };
        gum(-0.02, 0.01, 0, &mut positions, &mut triangles);
        gum(-0.09, -0.06, 16, &mut positions, &mut triangles);
        // A lower back tooth standing on the lower gum's top edge at -0.06
        // and rising to -0.02, centred above the line at -0.04... and higher.
        tooth(-0.06, 0.0, 32, &mut positions, &mut triangles);
        let s = Surface { mesh: 0, prim: 0, positions, triangles };
        let p = mouth_pieces(&frame(), &markers(), &s);
        assert!(p[32..36].iter().all(|x| *x == Some(true)), "{:?}", &p[32..36]);
    }

    /// A mouth whose inside sits higher than the lips: the lower gum and
    /// teeth straddle the lips' line, the upper teeth are clearly above it
    /// and the tongue clearly below. The lower set goes with the jaw.
    #[test]
    fn a_high_mouth_divides_between_its_clear_pieces() {
        let (mut positions, mut triangles) = (Vec::new(), Vec::new());
        // Line at -0.04. Upper teeth -0.03..0.0 (above), tongue -0.07..-0.045.
        tooth(-0.03, 0.0, 0, &mut positions, &mut triangles);
        tooth(-0.07, -0.045, 4, &mut positions, &mut triangles);
        // Lower gum and lower teeth, both centred near the line.
        tooth(-0.05, -0.029, 8, &mut positions, &mut triangles);
        tooth(-0.055, -0.022, 12, &mut positions, &mut triangles);
        let s = Surface { mesh: 0, prim: 0, positions, triangles };
        let p = mouth_pieces(&frame(), &markers(), &s);
        assert!(p[0..4].iter().all(|x| *x == Some(false)), "upper teeth {p:?}");
        assert!(p[4..16].iter().all(|x| *x == Some(true)), "tongue and lower set {p:?}");
    }

    /// A mouth modelled as one surface: the upper tooth's tip dips just
    /// below the lips' line and the lower tooth's tip rises just above it,
    /// joined only through the lining at the back. Each tip follows its own
    /// gum, across the surface, not its height.
    #[test]
    fn a_joined_mouth_splits_across_its_surface() {
        // A strip through the side profile (y, z), extruded across x.
        let profile = [(-0.042, 0.09), (-0.02, 0.09), (-0.02, 0.05), (-0.06, 0.05), (-0.06, 0.09), (-0.038, 0.09)];
        let mut positions = Vec::new();
        for (y, z) in profile {
            positions.extend([Vec3::new(-0.01, y, z), Vec3::new(0.01, y, z)]);
        }
        let mut triangles = Vec::new();
        for k in 0..profile.len() as u32 - 1 {
            let (a, b) = (k * 2, k * 2 + 2);
            triangles.extend([[a, a + 1, b], [a + 1, b + 1, b]]);
        }
        let s = Surface { mesh: 0, prim: 0, positions, triangles };
        let pieces = mouth_pieces(&frame(), &markers(), &s);
        assert_eq!(pieces[0], Some(false), "the upper tooth's tip stays with the head: {pieces:?}");
        assert_eq!(pieces[10], Some(true), "the lower tooth's tip goes with the jaw: {pieces:?}");
    }

    /// A lower-lip point clicked low on the lip must not leave the top of the
    /// lower lip on the head: the split follows the corners, not the midpoint.
    #[test]
    fn the_whole_lower_lip_opens_with_the_jaw_when_its_point_is_low() {
        let low = FaceMarkers { lower_lip: Vec3::new(0.0, -0.052, 0.094), ..markers() };
        let jaw = |p: Vec3| {
            let w = weigh(&frame(), &low, &vertex(p), None, [None; 2], None);
            w.iter().filter(|(b, _)| *b == JAW || *b == LIP_LOWER).map(|(_, w)| w).sum::<f32>()
        };
        // Just under where the lips meet (the corners' height, -0.04).
        assert!(jaw(Vec3::new(0.0, -0.042, 0.095)) > 0.7, "top of the lower lip {}", jaw(Vec3::new(0.0, -0.042, 0.095)));
        // Just over it stays.
        assert!(jaw(Vec3::new(0.0, -0.037, 0.095)) < 0.3);
    }

    /// Skin in front of the left eye with a slit from 10° above to 25° below
    /// straight ahead: a wall at 1.5 eye radii, with the slit cut out.
    fn lids_around_slit() -> Vec<[Vec3; 3]> {
        let f = frame();
        let d = 1.5 * f.eye_radius;
        let wall = |from: f32, to: f32| {
            let y = |deg: f32| f.eye_l.y + d * deg.to_radians().tan();
            let (a, b) = (y(from), y(to));
            let (x0, x1, z) = (f.eye_l.x - 0.01, f.eye_l.x + 0.01, f.eye_l.z + d);
            [[Vec3::new(x0, a, z), Vec3::new(x1, a, z), Vec3::new(x0, b, z)], [Vec3::new(x1, a, z), Vec3::new(x1, b, z), Vec3::new(x0, b, z)]]
        };
        wall(10.0, 70.0).into_iter().chain(wall(-70.0, -25.0)).collect()
    }

    #[test]
    fn an_eye_is_measured_from_its_lids() {
        let r = frame().eye_radius;
        let o = measure_eye(&frame(), frame().eye_l, &lids_around_slit(), r).expect("both lids found");
        assert!((o.upper - 10.0).abs() <= 1.0, "upper lid at {}", o.upper);
        assert!((o.lower + 25.0).abs() <= 1.0, "lower lid at {}", o.lower);
        // Closed, the lids meet and overlap a little.
        assert!(o.upper - o.close_upper() < o.lower + o.close_lower());
        // An eyeball inside the lids needs no push.
        assert_eq!(o.push, 0.0);
    }

    /// An eyeball bulging past the lids (the wall is 1.5 radii out, so the
    /// nearer lid margin is about 1.52 radii away) pushes the lids forward
    /// far enough to clear it.
    #[test]
    fn a_bulging_eye_pushes_the_lids_forward() {
        let r = frame().eye_radius;
        let o = measure_eye(&frame(), frame().eye_l, &lids_around_slit(), 2.0 * r).expect("both lids found");
        let margin = 1.5 * r / 10f32.to_radians().cos();
        assert!((o.push - (2.1 * r - margin)).abs() < 0.02 * r, "push {} radii", o.push / r);
    }

    #[test]
    fn measured_lids_leave_the_nose_and_do_not_tear() {
        let f = frame();
        let o = EyeOpening { upper: 25.0, lower: -15.0, push: 0.0 };
        let lid = |bone: usize, p: Vec3| -> f32 {
            let w = weigh(&f, &markers(), &vertex(p), None, [Some(o); 2], None);
            w.iter().filter(|(b, _)| *b == bone).map(|(_, w)| w).sum()
        };
        // The bridge of the nose, inward of the left eye and in front.
        let bridge = f.eye_l + Vec3::new(-0.018, 0.0, 0.01);
        assert!(lid(LID_UPPER_L, bridge) + lid(LID_LOWER_L, bridge) < 0.01);
        // Away from the margin, a point just above the meeting line and one
        // just below are not handed to opposite lids.
        let at = |deg: f32| {
            let a = (o.meet() + deg).to_radians();
            f.eye_l + Vec3::new(0.0, a.sin(), a.cos()) * 1.3 * f.eye_radius
        };
        assert!((lid(LID_UPPER_L, at(1.0)) - lid(LID_UPPER_L, at(-1.0))).abs() < 0.2);
        // At the margin the split stays sharp, so each lid closes fully.
        let margin = |deg: f32| {
            let a = (o.meet() + deg).to_radians();
            f.eye_l + Vec3::new(0.0, a.sin(), a.cos()) * 1.02 * f.eye_radius
        };
        assert!(lid(LID_UPPER_L, margin(4.0)) > 0.95);
        assert!(lid(LID_LOWER_L, margin(-4.0)) > 0.95);
    }

    /// A scan's eyes come from the one clicked point: mirrored across the
    /// middle of the face, set back into the head, and with no lid weights,
    /// since the lid skin there is the painted eye.
    #[test]
    fn painted_eyes_are_placed_from_one_point() {
        let m = FaceMarkers { eye: Some(Vec3::new(-0.03, 0.03, 0.092)), ..markers() };
        let f = painted_eyes(Vec3::ZERO, m.eye.unwrap(), &m).expect("frame");
        assert!(!f.eyes_modelled);
        assert!((f.scale - 0.06).abs() < 1e-5);
        assert!(f.eye_l.x > 0.0 && f.eye_r.x < 0.0, "the clicked right eye is mirrored to the left");
        assert!((f.eye_l.x + f.eye_r.x).abs() < 1e-5);
        assert!(f.eye_r.z < 0.092, "the centre is behind the surface");
        let over_eye = f.eye_l + Vec3::new(0.0, 0.3, 1.0).normalize() * f.eye_radius * 1.05;
        let w = weigh(&f, &m, &vertex(over_eye), None, [None; 2], None);
        assert!(w.iter().all(|(b, w)| ![LID_UPPER_L, LID_LOWER_L].contains(b) || *w == 0.0));
    }

    #[test]
    fn an_eye_with_no_opening_keeps_the_stock_blink() {
        let f = frame();
        let z = f.eye_l.z + 1.5 * f.eye_radius;
        let sheet = vec![[Vec3::new(-1.0, -1.0, z), Vec3::new(1.0, -1.0, z), Vec3::new(0.0, 1.0, z)]];
        assert!(measure_eye(&f, f.eye_l, &sheet, f.eye_radius).is_none());
    }

    #[test]
    fn the_skin_over_the_eye_follows_the_upper_lid() {
        let over = weight_of(LID_UPPER_L, Vec3::new(0.03, 0.04, 0.093));
        assert!(over > 0.4, "upper lid weight {over}");
        assert!(weight_of(LID_UPPER_R, Vec3::new(0.03, 0.04, 0.093)) < 0.01, "the other side stays");
    }

    #[test]
    fn the_mirrored_corner_lands_on_the_left() {
        let f = frame();
        let l = mirror_side(&f, markers().mouth_corner, true);
        assert!(l.x > 0.0 && (l.x - 0.022).abs() < 1e-6);
    }
}
