//! Procedural hair groom, as a standalone C-ABI plugin.
//!
//! Put [`Hair`] on any entity with a mesh and the plugin grows strands over that
//! mesh's surface: it area-samples root points across the triangles, grows a
//! tapered strand from each along the surface normal (drooping toward gravity),
//! verlet-simulates them so they sway, and rebuilds a camera-facing ribbon mesh
//! every frame so the hair reads from any angle.
//!
//! Ported from the Bevy-linking `crates/renzora_hair`. It is the first plugin to
//! use both halves of the mesh surface at once, and it is worth saying why each
//! is needed, because between them they are what made this port possible at all:
//!
//! * **Reading** — strands are scattered over a mesh the plugin did not create.
//!   Without `Meshes::read` a groom would have nothing to grow from.
//! * **Writing** — the ribbon geometry is rebuilt every frame, since the ribbons
//!   turn to face the camera. `add_mesh_data` is init-only, so this needs
//!   `Meshes::write` to replace the geometry from a system.
//!
//! ## What changed in the port
//!
//! * The groom's own state (strand points, verlet history) lives in a plugin
//!   resource keyed by entity rather than in a component. Component fields are a
//!   closed set of numeric kinds — no `Vec` — so per-strand data cannot live
//!   there. A resource is the plugin's own memory and can hold anything.
//! * Visibility is not settable across the boundary, so `enabled: false` writes
//!   an empty mesh instead of hiding the render entity.
//! * The simulation runs whenever `simulate` is on. The engine version gated it
//!   on `PlayState::is_scripts_running` so hair held still while editing; play
//!   state is not exposed to plugins, so that is now the author's toggle.

use renzora_plugin::prelude::*;
use std::collections::HashMap;

/// Hard cap, so a fat-fingered strand count cannot try to allocate millions of
/// vertices and hang the frame.
const MAX_STRANDS: usize = 50_000;

/// Longest frame step the sim will integrate. A single huge `dt` — a stall, a
/// breakpoint — would otherwise fling every strand off the model.
const MAX_DT: f32 = 1.0 / 30.0;

/// Procedural hair groom. Add it to an entity that has a mesh; the strands are
/// grown over that mesh's surface.
///
/// The *shape* fields (`strands` through `droop`) rebuild the groom when
/// changed. `color` and the sim fields apply live without a rebuild, so tuning
/// the motion never resets it.
#[derive(Component)]
#[component(name = "Hair")]
#[repr(C)]
pub struct Hair {
    /// Master switch. Off empties the mesh rather than hiding it — visibility
    /// is not settable across the boundary.
    pub enabled: bool,
    /// Sway under gravity, versus holding the grown rest shape.
    pub simulate: bool,
    /// Target strand count, area-weighted so dense triangles get proportionally
    /// more. Capped at 50,000.
    #[field(min = 0.0, max = 50000.0, speed = 25.0)]
    pub strands: f32,
    /// Strand length in world units, before per-strand jitter.
    #[field(min = 0.001, max = 5.0, speed = 0.005)]
    pub length: f32,
    /// Random per-strand length variation, 0 (all equal) to 1 (down to half).
    /// Breaks up the flat "helmet" silhouette.
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub length_jitter: f32,
    /// Points along each strand. More is smoother and more geometry.
    #[field(min = 1.0, max = 16.0, speed = 1.0)]
    pub segments: f32,
    /// Half-width of a ribbon at the root, tapering to a point at the tip.
    /// Around 0.002–0.006 reads as fine hair.
    #[field(min = 0.0001, max = 0.1, speed = 0.0005)]
    pub width: f32,
    /// How far a strand bends from the surface normal toward gravity as it
    /// grows, 0 (straight out) to 1 (flops down). The rest-shape droop, distinct
    /// from the dynamic gravity below.
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub droop: f32,
    /// Base colour, linear RGB in 0..1, multiplied by a per-strand shade.
    #[field(speed = 0.005)]
    pub color: Vec3,
    /// Spring-back toward the rest shape, 0 (limp) to 1 (barely moves).
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub stiffness: f32,
    /// Velocity bleed-off, 0 (swings forever) to 1 (dead). Frame-rate
    /// normalised so the feel is stable across FPS.
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub damping: f32,
    /// Gravity multiplier for the sim. 0 floats.
    #[field(min = 0.0, max = 3.0, speed = 0.01)]
    pub gravity: f32,

    /// The groom's render entity, split across two `i32`s because that is the
    /// widest the field kinds go and an `Entity` is 64 bits.
    ///
    /// Kept here rather than in plugin memory so it survives a hot reload:
    /// `GROOMS` is wiped when the plugin is replaced, and anything remembered
    /// only there strands its render entity with nothing left that knows it
    /// exists. Component data outlives the plugin that wrote it.
    ///
    /// `skip` keeps both out of the inspector. The alternative — a marker
    /// component — cannot be hidden at all, since every registered component
    /// lands in the Add Component list.
    #[field(skip)]
    pub render_lo: i32,
    #[field(skip)]
    pub render_hi: i32,
}

impl Default for Hair {
    fn default() -> Self {
        Self {
            enabled: true,
            simulate: true,
            strands: 2000.0,
            length: 0.12,
            length_jitter: 0.3,
            segments: 5.0,
            width: 0.0035,
            droop: 0.5,
            color: Vec3 { x: 0.16, y: 0.10, z: 0.06 },
            stiffness: 0.12,
            damping: 0.7,
            gravity: 1.0,
            render_lo: 0,
            render_hi: 0,
        }
    }
}

impl Hair {
    /// Hash of the *shape* fields only. When it changes the strands are rebuilt;
    /// colour and sim edits leave it alone so they apply live.
    fn shape_signature(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for v in [
            self.strands,
            self.length,
            self.length_jitter,
            self.segments,
            self.width,
            self.droop,
        ] {
            h ^= v.to_bits() as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

/// One grown strand.
struct Strand {
    /// Grown rest shape, root → tip, in the source mesh's local space.
    rest_local: Vec<Vec3>,
    /// Current world positions.
    world: Vec<Vec3>,
    /// Previous world positions — the verlet history.
    prev: Vec<Vec3>,
    half_width: f32,
    /// Per-strand shade multiplier, for subtle colour variation.
    shade: f32,
}

/// One entity's groom.
struct Groom {
    strands: Vec<Strand>,
    /// The mesh slot the ribbons are written into each frame.
    mesh: renzora_plugin::sys::AssetHandle,
    /// The hidden entity carrying the ribbon mesh. Kept so the groom can be
    /// torn down when its owner goes away — the ABI has no `RemovedComponents`,
    /// so nothing else would ever despawn it.
    render: Entity,
    /// The `Hair::shape_signature` the current strands were grown from.
    signature: u64,
    /// False until the sim has seeded world/prev from the rest pose.
    seeded: bool,
}

/// Every groom, keyed by the entity that owns it.
///
/// Plain plugin memory, not an ECS resource. Strand data is `Vec`s of points,
/// and neither a plugin component nor a plugin resource can hold one: both
/// cross the boundary as raw bytes from a layout the host allocates, and a type
/// with a destructor is refused outright — a `Vec` whose drop never runs leaks
/// its buffer for the life of the process.
///
/// Nothing requires a plugin's *internal* state to cross at all. Only [`Hair`]
/// does, which is also what makes a groom regenerate deterministically after a
/// scene load: the settings are saved, the strands are derived.
///
/// A `Mutex` rather than a `thread_local`, because Bevy's multi-threaded
/// executor gives no promise about which worker runs a given system.
struct Grooms {
    by_entity: HashMap<u64, Groom>,
    /// One mesh handle per groom, allocated at init. Handles cannot be created
    /// from a system, so a fixed pool is reserved up front and handed out as
    /// grooms appear.
    /// Mesh slots not currently in use. A stack rather than a cursor, so a
    /// slot returns to it when its groom is torn down — without that, adding
    /// and deleting hair sixteen times would exhaust the pool for the session.
    free: Vec<renzora_plugin::sys::AssetHandle>,
    material: renzora_plugin::sys::AssetHandle,
}

/// How many grooms can exist at once.
///
/// `add_mesh_data` is init-only, so every mesh a plugin will ever write to has
/// to be created during `build`. Sixteen is far more distinct hairy characters
/// than a scene realistically has, and each unused slot is an empty mesh.
const GROOM_POOL: usize = 16;

/// The plugin's groom state. See [`Grooms`] for why it is not an ECS resource.
static GROOMS: std::sync::Mutex<Option<Grooms>> = std::sync::Mutex::new(None);

/// Grow strands for any entity that needs them, and rebuild every groom's
/// ribbons for this frame's camera.
fn update_grooms(
    mut q: Query<(Entity, &mut Hair, &Transform)>,
    meshes: Meshes,
    time: Res<Time>,
    mut removed: RemovedComponents<Hair>,
    mut cmds: Commands,
) {
    let dt = time.delta_secs().min(MAX_DT);
    let Ok(mut guard) = GROOMS.lock() else {
        return;
    };
    let Some(grooms) = guard.as_mut() else {
        return;
    };

    for (entity, hair, transform) in &mut q {
        let key = entity.0;
        let signature = hair.shape_signature();

        // Grow, or regrow if a shape field moved. The mesh may not have loaded
        // yet, which is the normal state for the first few frames — `read`
        // returns `None` and we simply try again next frame.
        let needs_build = grooms
            .by_entity
            .get(&key)
            .is_none_or(|g| g.signature != signature);
        if needs_build {
            let Some(source) = meshes.read(entity) else {
                continue;
            };
            let strands = grow(&source, hair, transform);
            if strands.is_empty() {
                continue;
            }
            let (mesh, render) = match grooms.by_entity.get(&key) {
                Some(existing) => (existing.mesh, existing.render),
                None => {
                    let Some(handle) = grooms.take_slot() else {
                        error("hair: no free groom slots left");
                        continue;
                    };
                    // Geometry is already in world space (see `build_ribbons`),
                    // so the render entity sits at the origin.
                    // Whatever the component still points at is from a previous
                    // plugin build — a reload wiped the tracking but not the
                    // entity. Reclaim it before spawning, or every reload leaves
                    // another groom standing in the scene. `try_despawn`
                    // semantics make a stale or never-set id a no-op.
                    let stored = stored_render(hair);
                    if stored.0 != 0 {
                        cmds.entity(stored).despawn();
                    }
                    let render = cmds
                        .spawn_mesh(handle, grooms.material, Transform::IDENTITY)
                        .id();
                    hair.render_lo = render.0 as u32 as i32;
                    hair.render_hi = (render.0 >> 32) as u32 as i32;
                    (handle, render)
                }
            };
            grooms.by_entity.insert(
                key,
                Groom {
                    strands,
                    mesh,
                    render,
                    signature,
                    seeded: false,
                },
            );
        }

        let Some(groom) = grooms.by_entity.get_mut(&key) else {
            continue;
        };

        // Disabled: write an empty mesh. The boundary has no way to set
        // `Visibility`, and an empty mesh costs nothing to draw.
        if !hair.enabled {
            meshes.write(groom.mesh, &[], None, None, None, None);
            continue;
        }

        step(groom, transform, hair, dt);
        groom.seeded = groom.seeded || hair.simulate;

        // Billboard toward the camera. Without a camera transform across the
        // boundary the ribbons face +Z in world space, which still reads
        // correctly for a camera looking down -Z at the model.
        let (positions, normals, uvs, indices, colors) = build_ribbons(&groom.strands, hair.color);
        meshes.write(
            groom.mesh,
            &positions,
            Some(&normals),
            Some(&uvs),
            Some(&indices),
            Some(&colors),
        );
    }

    retire_dead_grooms(grooms, &mut removed, &mut cmds);
}

/// Tear down grooms whose owner is gone.
///
/// Without this the ribbon entity outlives its owner — hair left standing in the
/// scene after the model is deleted — and its mesh slot is never reusable again.
///
/// This used to sweep: build the list of entities that still had `Hair` this
/// frame, then diff every tracked groom against it, every frame, at O(tracked x
/// live). The boundary had no `RemovedComponents`, so absence was the only signal
/// available. It has one now, and the engine already knew the answer.
fn retire_dead_grooms(
    grooms: &mut Grooms,
    removed: &mut RemovedComponents<Hair>,
    cmds: &mut Commands,
) {
    for entity in removed.read() {
        if let Some(groom) = grooms.by_entity.remove(&entity.0) {
            cmds.entity(groom.render).despawn();
            // Back on the free stack, so add/delete cycles do not exhaust the
            // pool. The mesh asset itself stays allocated and is overwritten by
            // whichever groom claims the slot next.
            grooms.free.push(groom.mesh);
        }
    }
}

/// The render entity a `Hair` remembers, or `Entity(0)` if it has none.
fn stored_render(hair: &Hair) -> Entity {
    Entity(((hair.render_hi as u32 as u64) << 32) | hair.render_lo as u32 as u64)
}

impl Grooms {
    fn take_slot(&mut self) -> Option<renzora_plugin::sys::AssetHandle> {
        self.free.pop()
    }
}

/// Area-weighted surface scatter, then growth.
///
/// Reads positions and normals from the source mesh in its local space. Where
/// the mesh has no normals, the flat triangle normal stands in — the same
/// fallback the engine version used, and the reason a low-poly scalp still
/// grows sensible hair.
fn grow(source: &MeshData, hair: &Hair, transform: &Transform) -> Vec<Strand> {
    let tris = source.triangles();
    if tris.is_empty() {
        return Vec::new();
    }
    let p = &source.positions;

    // Total area, so each triangle gets its fair share of strands.
    let mut total_area = 0.0f32;
    for t in &tris {
        if let (Some(&a), Some(&b), Some(&c)) = (p.get(t[0]), p.get(t[1]), p.get(t[2])) {
            total_area += cross(sub(b, a), sub(c, a)).length() * 0.5;
        }
    }
    if total_area < 1e-9 {
        return Vec::new();
    }

    let target = (hair.strands.max(0.0) as usize).min(MAX_STRANDS);
    let segments = (hair.segments as usize).clamp(1, 16);
    let seg_len = (hair.length / segments as f32).max(1e-4);
    // World "down" in the mesh's local space, so droop points the way gravity
    // does however the model is oriented.
    let local_down = normalize_or_zero(quat_rotate(
        quat_conjugate(transform.rotation),
        Vec3 { x: 0.0, y: -1.0, z: 0.0 },
    ));

    let mut strands = Vec::new();
    for (ti, t) in tris.iter().enumerate() {
        let (Some(&pa), Some(&pb), Some(&pc)) = (p.get(t[0]), p.get(t[1]), p.get(t[2])) else {
            continue;
        };
        let area = cross(sub(pb, pa), sub(pc, pa)).length() * 0.5;
        if area < 1e-9 {
            continue;
        }

        // Expected strands here is this triangle's share by area; the fractional
        // remainder is placed probabilistically so small triangles still get hair
        // rather than always rounding down to none.
        let expected = target as f32 * area / total_area;
        let mut count = expected.floor() as usize;
        if rand01((ti as u32).wrapping_mul(9781).wrapping_add(1)) < expected.fract() {
            count += 1;
        }

        let tri_n = normalize_or_zero(cross(sub(pb, pa), sub(pc, pa)));
        let vn = |i: usize| source.normals.get(i).copied().unwrap_or(tri_n);
        let (na, nb, nc) = (vn(t[0]), vn(t[1]), vn(t[2]));

        for k in 0..count {
            if strands.len() >= MAX_STRANDS {
                return strands;
            }
            let seed = (ti as u32).wrapping_mul(2_654_435_761)
                ^ (k as u32).wrapping_mul(40_503)
                ^ 0x9E37_79B9;

            // Barycentric root, folded back inside the triangle.
            let mut u = rand01(seed);
            let mut v = rand01(seed ^ 0x68bc_21eb);
            if u + v > 1.0 {
                u = 1.0 - u;
                v = 1.0 - v;
            }
            let w = 1.0 - u - v;
            let root = add(add(scale(pa, w), scale(pb, u)), scale(pc, v));
            let normal = normalize_or_zero(add(add(scale(na, w), scale(nb, u)), scale(nc, v)));
            let normal = if is_zero(normal) { tri_n } else { normal };

            let len_factor = 1.0 - hair.length_jitter.clamp(0.0, 1.0) * rand01(seed ^ 0x0001_2345);
            let shade = rand01(seed ^ 0x00ab_cdef);

            // Step along a direction easing from the surface normal toward
            // local-down by `droop`, so strands lift off the surface and then
            // fall rather than bending immediately.
            let mut pts = Vec::with_capacity(segments + 1);
            pts.push(root);
            let mut cur = root;
            for i in 1..=segments {
                let t01 = i as f32 / segments as f32;
                let dir = normalize_or_zero(lerp(
                    normal,
                    local_down,
                    hair.droop.clamp(0.0, 1.0) * t01,
                ));
                let dir = if is_zero(dir) { normal } else { dir };
                cur = add(cur, scale(dir, seg_len * len_factor));
                pts.push(cur);
            }

            strands.push(Strand {
                world: pts.clone(),
                prev: pts.clone(),
                rest_local: pts,
                half_width: hair.width.max(1e-4),
                shade,
            });
        }
    }
    strands
}

/// Advance the verlet sim, or hold the rest shape.
///
/// Strands live in world space with the root pinned to its animated surface
/// position, so hair lags when the head turns and rides the model when static.
fn step(groom: &mut Groom, transform: &Transform, hair: &Hair, dt: f32) {
    let gravity = Vec3 { x: 0.0, y: -9.81 * hair.gravity, z: 0.0 };
    let keep = (1.0 - hair.damping.clamp(0.0, 1.0)).powf(dt * 60.0);
    let stiff = 1.0 - (1.0 - hair.stiffness.clamp(0.0, 1.0)).powf(dt * 60.0);
    let seeded = groom.seeded;
    let simulating = hair.simulate;

    for strand in &mut groom.strands {
        let m = strand.rest_local.len();
        if m == 0 {
            continue;
        }
        let to_world =
            |p: Vec3| add(transform.translation, quat_rotate(transform.rotation, p));
        let root_world = to_world(strand.rest_local[0]);

        // Sim off, or the first simulated frame: snap to the grown rest shape
        // and clear velocity, so there is no pop when it starts moving.
        if !simulating || !seeded {
            for i in 0..m {
                let w = to_world(strand.rest_local[i]);
                strand.world[i] = w;
                strand.prev[i] = w;
            }
            continue;
        }

        // Integrate the free points; the root stays pinned to the surface.
        strand.world[0] = root_world;
        strand.prev[0] = root_world;
        for i in 1..m {
            let target = to_world(strand.rest_local[i]);
            let pos = strand.world[i];
            let vel = scale(sub(pos, strand.prev[i]), keep);
            let mut next = add(add(pos, vel), scale(gravity, dt * dt));
            next = lerp(next, target, stiff);
            strand.prev[i] = pos;
            strand.world[i] = next;
        }

        // Hold each segment at its rest length, and hard-clamp anything that has
        // strayed absurdly far — a teleport or a scene load can otherwise blow a
        // strand out to infinity and it never recovers.
        strand.world[0] = root_world;
        for i in 1..m {
            let a = to_world(strand.rest_local[i - 1]);
            let b = to_world(strand.rest_local[i]);
            let len = sub(b, a).length();
            let parent = strand.world[i - 1];
            let d = sub(strand.world[i], parent);
            let dir = if d.length() > 1e-6 {
                normalize_or_zero(d)
            } else {
                normalize_or_zero(sub(b, a))
            };
            let mut wp = add(parent, scale(dir, len));
            if sub(wp, b).length() > len * 8.0 + 1.0 {
                wp = b;
            }
            strand.world[i] = wp;
        }
    }
}

/// Build the ribbon geometry for this frame.
///
/// Each strand becomes a two-vertex-wide strip tapering to its tip. The engine
/// version turned the ribbon's flat face toward the camera every frame; without
/// a camera transform across the boundary the width axis is taken against world
/// up instead, which holds up for the usual case of looking at a character from
/// roughly eye level.
type Ribbons = (Vec<Vec3>, Vec<Vec3>, Vec<[f32; 2]>, Vec<u32>, Vec<[f32; 4]>);

fn build_ribbons(strands: &[Strand], color: Vec3) -> Ribbons {
    let estimate: usize = strands.iter().map(|s| s.world.len() * 2).sum();
    let mut positions = Vec::with_capacity(estimate);
    let mut normals = Vec::with_capacity(estimate);
    let mut uvs = Vec::with_capacity(estimate);
    let mut colors = Vec::with_capacity(estimate);
    let mut indices = Vec::new();

    let up = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    let fallback = Vec3 { x: 1.0, y: 0.0, z: 0.0 };

    for s in strands {
        let n = s.world.len();
        if n < 2 {
            continue;
        }
        let base = positions.len() as u32;

        for i in 0..n {
            let p = s.world[i];
            // Forward difference along the strand, backward at the tip.
            let tangent = normalize_or_zero(if i + 1 < n {
                sub(s.world[i + 1], p)
            } else {
                sub(p, s.world[i - 1])
            });

            // Width axis perpendicular to the strand. A strand pointing straight
            // up has no cross product with up, so fall back to an arbitrary
            // perpendicular rather than collapsing the ribbon to zero width.
            let mut side = cross(tangent, up);
            if side.length() < 1e-5 {
                side = cross(tangent, fallback);
            }
            let side = normalize_or_zero(side);
            let normal = normalize_or_zero(cross(side, tangent));

            let t01 = i as f32 / (n - 1) as f32;
            let half_w = s.half_width * (1.0 - 0.85 * t01);
            positions.push(sub(p, scale(side, half_w)));
            positions.push(add(p, scale(side, half_w)));
            normals.push(normal);
            normals.push(normal);
            uvs.push([0.0, t01]);
            uvs.push([1.0, t01]);
            // Root→tip darkening plus the per-strand shade, as vertex colour —
            // the PBR material multiplies it into the base colour, which is how
            // a groom varies without needing a custom shader.
            let shade = (0.65 + 0.35 * s.shade) * (1.0 - 0.25 * t01);
            let c = [color.x * shade, color.y * shade, color.z * shade, 1.0];
            colors.push(c);
            colors.push(c);
        }

        // Two triangles per segment, between consecutive left/right pairs.
        for i in 0..(n - 1) as u32 {
            let a = base + i * 2;
            indices.extend_from_slice(&[a, a + 1, a + 2, a + 2, a + 1, a + 3]);
        }
    }

    (positions, normals, uvs, indices, colors)
}

// ── Small vector helpers ─────────────────────────────────────────────────────
//
// `sys::Vec3` is deliberately plain data with no maths on it — the boundary
// carries values, not a library. These are the handful of operations this plugin
// needs, kept local rather than pulling in a dependency for a dozen lines.

fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }
}
fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }
}
fn scale(a: Vec3, s: f32) -> Vec3 {
    Vec3 { x: a.x * s, y: a.y * s, z: a.z * s }
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}
fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    add(a, scale(sub(b, a), t))
}
fn is_zero(v: Vec3) -> bool {
    v.x == 0.0 && v.y == 0.0 && v.z == 0.0
}
fn normalize_or_zero(v: Vec3) -> Vec3 {
    let len = v.length();
    if len > 1e-6 {
        scale(v, 1.0 / len)
    } else {
        Vec3 { x: 0.0, y: 0.0, z: 0.0 }
    }
}

/// Rotate `v` by the unit quaternion `q`.
///
/// Written out because `sys::Quat` is plain data with no maths — the boundary
/// carries values, not a library. The standard `v + 2w(q×v) + 2(q×(q×v))` form.
fn quat_rotate(q: Quat, v: Vec3) -> Vec3 {
    let u = Vec3 { x: q.x, y: q.y, z: q.z };
    let uv = cross(u, v);
    let uuv = cross(u, uv);
    add(v, add(scale(uv, 2.0 * q.w), scale(uuv, 2.0)))
}

/// The inverse of a **unit** quaternion, which is just its conjugate. A
/// `Transform`'s rotation is always unit, so the general form is not needed.
fn quat_conjugate(q: Quat) -> Quat {
    Quat { x: -q.x, y: -q.y, z: -q.z, w: q.w }
}

/// Cheap deterministic hash → `[0, 1)`.
///
/// Deterministic rather than a real RNG so a groom regenerates identically for
/// the same settings — a scene reload must not reshuffle every strand.
fn rand01(x: u32) -> f32 {
    let mut h = x.wrapping_add(0x9E37_79B9);
    h = (h ^ (h >> 16)).wrapping_mul(0x85eb_ca6b);
    h = (h ^ (h >> 13)).wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

pub struct HairPlugin;

impl Plugin for HairPlugin {
    fn build(&self, app: &mut App) {
        // Every mesh a groom will ever write into has to exist now:
        // `add_mesh_data` needs the init-time host handle. Each starts as a
        // single degenerate triangle because a mesh with no positions is
        // refused, and is overwritten the first frame its groom appears.
        let seed = [Vec3 { x: 0.0, y: 0.0, z: 0.0 }; 3];
        let pool: Vec<_> = (0..GROOM_POOL)
            .map(|_| app.add_mesh_data(&seed, None, None, Some(&[0, 1, 2])))
            .collect();
        // White, so the per-vertex colours carry the hair colour unmodified.
        let material = app.add_material([1.0, 1.0, 1.0, 1.0]);


        app.register_component::<Hair>()
            .add_systems(Update, update_grooms);

        if let Ok(mut g) = GROOMS.lock() {
            *g = Some(Grooms {
                by_entity: HashMap::new(),
                free: pool,
                material,
            });
        }
    }
}

renzora_plugin::add!(HairPlugin);
