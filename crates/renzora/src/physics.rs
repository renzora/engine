//! The contract between the engine's physics API and whatever actually
//! simulates.
//!
//! `renzora_physics` ships the physics *API* (the components scenes save, the
//! 2D/3D routing, script actions, read-back state, collide-and-slide) and no
//! solver. A physics engine implements [`PhysicsBackend`], adds its systems to
//! the [`PhysicsSet`]s, and hands itself to
//! [`AppPhysicsBackendExt::add_physics_backend`]. Avian is the bundled
//! `avian_physics` plugin, written like any installed one, and anyone can write
//! another.
//!
//! Here rather than in `renzora_physics` because a backend plugin must name
//! these types without linking the physics crate, and a boundary type has
//! exactly one definition, which is the rule this crate exists for.
//!
//! ## Who owns what
//!
//! **Renzora components say what exists.** [`PhysicsBodyData`] and
//! [`CollisionShapeData`] are what scenes save; the engine turns each collider
//! into a [`ResolvedCollider`] (geometry built, 2D mapped, mesh assets read) and
//! marks every wired-up entity [`RuntimePhysics`]. A backend mirrors those into
//! its own world and never decides on its own that a body exists.
//!
//! **The backend owns motion.** It writes poses, [`BodyVelocity`] and
//! [`PhysicsContacts`] back, and applies [`PhysicsCommands`] and
//! [`PhysicsSettings`]. Nothing else moves a simulated body.
//!
//! That split is what lets two very different engines sit behind one contract:
//! avian keeps its state as Bevy components and maps ours onto its own, while an
//! engine with its own world (Rapier, Jolt) keeps a map from `Entity` to its
//! handles. From above, they look the same.

use std::sync::Arc;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Scene data ───────────────────────────────────────────────────────────
//
// These moved here from `renzora_physics::data`. Scenes store a component by
// its reflected type path, so each keeps its old path through `type_path`:
// without it every saved scene with a collider would stop loading.

/// Type of physics body
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
#[type_path = "renzora_physics::data"]
pub enum PhysicsBodyType {
    #[default]
    RigidBody,
    StaticBody,
    KinematicBody,
}

/// Serializable physics body data, independent of any backend.
///
/// This component stores physics properties in a format that can be saved to
/// scenes. At runtime the selected backend mirrors it into its own bodies.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
#[type_path = "renzora_physics::data"]
pub struct PhysicsBodyData {
    pub body_type: PhysicsBodyType,
    pub mass: f32,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub lock_rotation_x: bool,
    pub lock_rotation_y: bool,
    pub lock_rotation_z: bool,
    pub lock_translation_x: bool,
    pub lock_translation_y: bool,
    pub lock_translation_z: bool,
    /// Continuous collision detection: sweep the body between steps so a fast
    /// one (a bullet, a thrown ball) cannot pass through a thin wall in a single
    /// step. Costs a sweep per step, so it is off unless asked for. A backend
    /// without [`Caps::CCD`] ignores it.
    #[serde(default)]
    #[reflect(default)]
    pub ccd: bool,
}

impl Default for PhysicsBodyData {
    fn default() -> Self {
        Self {
            body_type: PhysicsBodyType::RigidBody,
            mass: 1.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.05,
            lock_rotation_x: false,
            lock_rotation_y: false,
            lock_rotation_z: false,
            lock_translation_x: false,
            lock_translation_y: false,
            lock_translation_z: false,
            ccd: false,
        }
    }
}

impl PhysicsBodyData {
    pub fn static_body() -> Self {
        Self {
            body_type: PhysicsBodyType::StaticBody,
            ..Default::default()
        }
    }

    pub fn kinematic_body() -> Self {
        Self {
            body_type: PhysicsBodyType::KinematicBody,
            ..Default::default()
        }
    }
}

/// Type of collision shape
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
#[type_path = "renzora_physics::data"]
pub enum CollisionShapeType {
    #[default]
    Box,
    Sphere,
    Capsule,
    Cylinder,
    /// Trimesh generated from the entity's `Mesh3d`. No size fields apply.
    Mesh,
}

/// Serializable collision shape data, independent of any backend.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
#[type_path = "renzora_physics::data"]
pub struct CollisionShapeData {
    pub shape_type: CollisionShapeType,
    #[serde(default)]
    #[reflect(default)]
    pub offset: Vec3,
    pub half_extents: Vec3,
    pub radius: f32,
    pub half_height: f32,
    pub friction: f32,
    pub restitution: f32,
    pub is_sensor: bool,
    /// Turn of the shape about its `offset` point, relative to the entity. A
    /// capsule lying along a bone, or a box set at an angle on a prop, without
    /// a child entity just to hold the turn.
    #[serde(default)]
    #[reflect(default)]
    pub rotation: Quat,
    /// The collision layers this collider is on, one bit per layer (32 of
    /// them). Layer 0, the lowest bit, is where everything starts.
    #[serde(default = "default_collision_layers")]
    #[reflect(default = "default_collision_layers")]
    pub collision_layers: u32,
    /// The layers this collider collides with. Two colliders touch only when
    /// each is on a layer the other collides with, so either one can opt out.
    #[serde(default = "default_collides_with")]
    #[reflect(default = "default_collides_with")]
    pub collides_with: u32,
}

fn default_collision_layers() -> u32 {
    CollisionLayers::DEFAULT.memberships
}

fn default_collides_with() -> u32 {
    CollisionLayers::DEFAULT.filters
}

impl Default for CollisionShapeData {
    fn default() -> Self {
        Self {
            shape_type: CollisionShapeType::Box,
            offset: Vec3::ZERO,
            half_extents: Vec3::splat(0.5),
            radius: 0.5,
            half_height: 0.5,
            friction: 0.5,
            restitution: 0.0,
            is_sensor: false,
            rotation: Quat::IDENTITY,
            collision_layers: default_collision_layers(),
            collides_with: default_collides_with(),
        }
    }
}

impl CollisionShapeData {
    pub fn sphere(radius: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Sphere,
            radius,
            ..Default::default()
        }
    }

    pub fn capsule(radius: f32, half_height: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Capsule,
            radius,
            half_height,
            ..Default::default()
        }
    }

    pub fn cylinder(radius: f32, half_height: f32) -> Self {
        Self {
            shape_type: CollisionShapeType::Cylinder,
            radius,
            half_height,
            ..Default::default()
        }
    }

    pub fn mesh() -> Self {
        Self {
            shape_type: CollisionShapeType::Mesh,
            ..Default::default()
        }
    }

    pub fn cuboid(half_extents: Vec3) -> Self {
        Self {
            shape_type: CollisionShapeType::Box,
            half_extents,
            ..Default::default()
        }
    }

    pub fn world_center(&self, global_transform: &GlobalTransform) -> Vec3 {
        global_transform.transform_point(self.offset)
    }

    /// The layer pair a backend filters contacts with.
    pub fn layers(&self) -> CollisionLayers {
        CollisionLayers { memberships: self.collision_layers, filters: self.collides_with }
    }
}

/// Which colliders may touch. Each collider is on some layers (`memberships`)
/// and collides with others (`filters`); a pair touches only when each one's
/// memberships meet the other's filters. That two-sided test is what both
/// avian and Rapier do, so a scene filters the same on either.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CollisionLayers {
    pub memberships: u32,
    pub filters: u32,
}

impl CollisionLayers {
    /// On layer 0, colliding with everything.
    pub const DEFAULT: Self = Self { memberships: 1, filters: u32::MAX };
    /// Every layer, colliding with everything.
    pub const ALL: Self = Self { memberships: u32::MAX, filters: u32::MAX };

    pub const fn interacts_with(self, other: Self) -> bool {
        self.memberships & other.filters != 0 && other.memberships & self.filters != 0
    }
}

impl Default for CollisionLayers {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Serializable opt-in: force this entity onto the **2D** physics world.
///
/// The engine already routes sprites and anything under a `Node2d` to 2D
/// automatically; this marker covers 2D physics entities with no visual of
/// their own, such as the merged static colliders a tilemap layer generates,
/// or an invisible trigger area in a 2D scene.
#[derive(Component, Clone, Copy, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[type_path = "renzora_physics::data"]
pub struct Physics2d;

// ── Runtime state the engine writes ──────────────────────────────────────

/// Which simulation an entity, a backend or a query belongs to.
///
/// Two worlds, not one with a flag: a sprite and a 3D prop never collide, and
/// a project can hold scenes of both kinds at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhysicsDim {
    D3,
    D2,
}

/// The entity has been wired up for physics. Inserted by the engine once per
/// entity; a backend mirrors every entity that has it and tears its own state
/// down when it goes away (`RemovedComponents<RuntimePhysics>`).
#[derive(Component)]
pub struct RuntimePhysics;

/// The entity belongs to the **2D** world. Decided once, when the engine first
/// wires the entity up (by walking its ancestry), and remembered here so every
/// later system talks to the right simulation without walking again.
#[derive(Component)]
pub struct RuntimePhysics2d;

/// The physics data on this entity was generated at runtime, not authored, so
/// scenes never save its [`PhysicsBodyData`] or [`CollisionShapeData`].
///
/// A ragdoll builds a body and a collider on every bone of a skeleton. Saved,
/// those would land in the scene as instance overrides, and the next load would
/// find a "hand-placed" collider on every bone of a model that is meant to
/// generate its own. The marker itself is not reflected, so it is never saved
/// either and the generator simply runs again.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct RuntimeOnlyPhysics;

/// Collider geometry, already built, in the entity's local space.
///
/// The engine resolves this from [`CollisionShapeData`] so a backend never
/// reads a mesh asset or decides what a cylinder means in 2D: every backend
/// would otherwise repeat that work, and get its edge cases (index formats,
/// non-triangle topologies) wrong in its own way.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedShape {
    Cuboid { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
    Cylinder { radius: f32, half_height: f32 },
    /// Shared, because the same mesh collider can sit on many entities and a
    /// backend may keep a reference for as long as the body lives.
    TriMesh(Arc<TriMeshData>),
    /// A grid of heights. Far cheaper than the same ground as a [`Self::TriMesh`]:
    /// built in a fraction of the time (terrain streaming builds dozens of
    /// chunks at once, and their trimeshes cost a 60 ms hitch) and queried
    /// without a triangle tree. Spans `0..size` on X and Z in the entity's
    /// local space, from its origin.
    Heightfield(Arc<HeightfieldData>),
    Rectangle { half_extents: Vec2 },
    Circle { radius: f32 },
    Capsule2d { radius: f32, half_height: f32 },
}

/// Triangle soup for [`ResolvedShape::TriMesh`].
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TriMeshData {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<[u32; 3]>,
}

/// The heights of a [`ResolvedShape::Heightfield`]: `columns` samples along X
/// by `rows` along Z, row by row (`heights[z * columns + x]`), evenly spaced
/// across `size`, in local units.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct HeightfieldData {
    pub heights: Vec<f32>,
    pub columns: usize,
    pub rows: usize,
    pub size: Vec2,
}

impl HeightfieldData {
    /// The local position of sample `(x, z)`.
    pub fn point(&self, x: usize, z: usize) -> Vec3 {
        let step = Vec2::new(
            self.size.x / (self.columns.max(2) - 1) as f32,
            self.size.y / (self.rows.max(2) - 1) as f32,
        );
        Vec3::new(x as f32 * step.x, self.heights[z * self.columns + x], z as f32 * step.y)
    }

    /// The same surface as triangles, for consumers that only read meshes
    /// (navmesh obstacles), split along the same diagonal as the terrain's
    /// render mesh.
    pub fn triangles(&self) -> TriMeshData {
        let (c, r) = (self.columns, self.rows);
        let mut out = TriMeshData {
            vertices: Vec::with_capacity(c * r),
            indices: Vec::with_capacity(c.saturating_sub(1) * r.saturating_sub(1) * 2),
        };
        for z in 0..r {
            for x in 0..c {
                out.vertices.push(self.point(x, z));
            }
        }
        for z in 0..r.saturating_sub(1) {
            for x in 0..c.saturating_sub(1) {
                let tl = (z * c + x) as u32;
                let (tr, bl) = (tl + 1, tl + c as u32);
                let br = bl + 1;
                out.indices.push([tl, bl, tr]);
                out.indices.push([tr, bl, br]);
            }
        }
        out
    }
}

/// Hands the physics layer ready-made heights for a mesh collider, in place
/// of reading triangles out of the entity's `Mesh3d`. The terrain puts one on
/// each chunk: its heights are already a grid, and a grid builds and queries
/// far faster than a trimesh of the same ground.
#[derive(Component, Clone, Debug)]
pub struct HeightfieldSource(pub Arc<HeightfieldData>);

/// Everything a backend needs to build an entity's collider. Replaced (not
/// edited) whenever the shape changes, so `Changed<ResolvedCollider>` is the
/// one signal a backend watches.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ResolvedCollider {
    pub shape: ResolvedShape,
    /// Local offset of the shape from the entity's origin.
    pub offset: Vec3,
    /// Local turn of the shape about `offset`. A 2D backend reads only the turn
    /// about Z.
    pub rotation: Quat,
    pub friction: f32,
    pub restitution: f32,
    /// A trigger volume: reports contacts, never pushes back.
    pub sensor: bool,
    pub layers: CollisionLayers,
}

impl ResolvedCollider {
    /// A collider of `shape` with the defaults an authored one starts from.
    pub fn new(shape: ResolvedShape) -> Self {
        Self {
            shape,
            offset: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            friction: 0.5,
            restitution: 0.0,
            sensor: false,
            layers: CollisionLayers::DEFAULT,
        }
    }

    /// The collider's bounds in the entity's local space (unscaled), as
    /// `(min, max)`. Conservative for a turned shape: the box around the
    /// turned box. For the engine's own use, such as how tall a ladder is,
    /// where asking a backend would mean every backend answering it.
    pub fn local_aabb(&self) -> (Vec3, Vec3) {
        let half = match &self.shape {
            ResolvedShape::Cuboid { half_extents } => *half_extents,
            ResolvedShape::Sphere { radius } => Vec3::splat(*radius),
            ResolvedShape::Capsule { radius, half_height } => {
                Vec3::new(*radius, half_height + radius, *radius)
            }
            ResolvedShape::Cylinder { radius, half_height } => {
                Vec3::new(*radius, *half_height, *radius)
            }
            ResolvedShape::TriMesh(mesh) => {
                let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                for v in &mesh.vertices {
                    let p = self.rotation * *v;
                    min = min.min(p);
                    max = max.max(p);
                }
                if mesh.vertices.is_empty() {
                    return (self.offset, self.offset);
                }
                return (self.offset + min, self.offset + max);
            }
            ResolvedShape::Heightfield(field) => {
                let (lo, hi) = field
                    .heights
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
                if field.heights.is_empty() {
                    return (self.offset, self.offset);
                }
                let corners = [
                    Vec3::new(0.0, lo, 0.0),
                    Vec3::new(field.size.x, hi, field.size.y),
                    Vec3::new(field.size.x, lo, 0.0),
                    Vec3::new(0.0, hi, field.size.y),
                ];
                let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                for c in corners {
                    let p = self.rotation * c;
                    min = min.min(p);
                    max = max.max(p);
                }
                return (self.offset + min, self.offset + max);
            }
            ResolvedShape::Rectangle { half_extents } => half_extents.extend(0.0),
            ResolvedShape::Circle { radius } => Vec3::new(*radius, *radius, 0.0),
            ResolvedShape::Capsule2d { radius, half_height } => {
                Vec3::new(*radius, half_height + radius, 0.0)
            }
        };
        let turned = Mat3::from_quat(self.rotation).abs() * half;
        (self.offset - turned, self.offset + turned)
    }
}

// ── Runtime state the backend writes ─────────────────────────────────────

/// A body's velocity in world space, written by the backend after each step.
/// The engine inserts it on every body; a backend only fills it in.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyVelocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

/// One thing a body is touching.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The entity whose collider is touched.
    pub entity: Entity,
    /// The deepest contact point, in world space. For a sensor, which
    /// intersects rather than touches, the touching body's own position.
    pub point: Vec3,
    /// Unit normal at the contact, pointing from `entity` toward this body:
    /// the direction this body is being pushed. Zero for a sensor overlap.
    pub normal: Vec3,
    /// How hard the two met over the last step, as the normal impulse in
    /// newton-seconds. Zero for a sensor overlap, and for a resting contact
    /// that carries no load. A landing reads as a spike here, which is what a
    /// footstep or an impact sound wants.
    pub impulse: f32,
}

/// What this body is currently touching, rewritten by the backend every frame.
/// The engine inserts it on every body and turns it into enter/exit events
/// ([`CollisionStarted`], [`CollisionEnded`]) itself, once for every backend.
///
/// State rather than the backend's own enter/exit events, because a backend
/// that misses one event (or emits one twice) would leave a body "touching"
/// forever; a set rewritten every frame cannot drift.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct PhysicsContacts(pub Vec<Contact>);

impl PhysicsContacts {
    /// The entities touched, one per contact.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().map(|c| c.entity)
    }

    pub fn touches(&self, entity: Entity) -> bool {
        self.0.iter().any(|c| c.entity == entity)
    }
}

/// `entity` started touching `other` this frame. Triggered by the engine for
/// every body, whichever backend simulates it, so an observer on the entity
/// (or a global one) hears it.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct CollisionStarted {
    pub entity: Entity,
    pub other: Entity,
    /// The contact as it was on the first frame, for where to put a spark.
    pub contact: Contact,
}

/// `entity` stopped touching `other` this frame.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct CollisionEnded {
    pub entity: Entity,
    pub other: Entity,
}

// ── Joints ───────────────────────────────────────────────────────────────

/// A constraint between two bodies, on an entity of its own.
///
/// An entity rather than a field on either body because a body can have any
/// number of joints, and because despawning the joint entity is how a joint
/// breaks. Not reflected: joints are built at runtime by whatever wants them
/// (a ragdoll, a script, a door), so a saved scene never carries one.
///
/// The world (2D or 3D) is `body1`'s. A backend without [`Caps::JOINTS`]
/// ignores joints, and both bodies then move freely.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct PhysicsJoint {
    pub body1: Entity,
    pub body2: Entity,
    /// Where the joint attaches, in `body1`'s local space.
    pub anchor1: Vec3,
    /// Where the joint attaches, in `body2`'s local space.
    pub anchor2: Vec3,
    pub kind: JointKind,
    /// How soft the joint is (inverse stiffness, in m/N or rad/(N·m)): `0` is
    /// rigid. Applies to the joint's free directions and limits; the anchors
    /// themselves always stay pinned together, so a soft joint bends rather
    /// than coming apart.
    pub compliance: f32,
    /// Whether the two bodies still collide with each other. Off is usual for
    /// neighbouring ragdoll bones, whose capsules overlap at every joint.
    pub collide_connected: bool,
}

impl PhysicsJoint {
    /// A rigid joint of `kind` between two bodies, anchored at both origins.
    pub fn new(body1: Entity, body2: Entity, kind: JointKind) -> Self {
        Self {
            body1,
            body2,
            anchor1: Vec3::ZERO,
            anchor2: Vec3::ZERO,
            kind,
            compliance: 0.0,
            collide_connected: true,
        }
    }

    pub fn with_anchors(mut self, anchor1: Vec3, anchor2: Vec3) -> Self {
        self.anchor1 = anchor1;
        self.anchor2 = anchor2;
        self
    }

    pub fn with_compliance(mut self, compliance: f32) -> Self {
        self.compliance = compliance;
        self
    }

    pub fn with_collide_connected(mut self, collide: bool) -> Self {
        self.collide_connected = collide;
        self
    }
}

/// What a [`PhysicsJoint`] lets the two bodies do. Angles are radians, axes are
/// local to `body1` and need not be unit length. In 2D an axis reads only its
/// XY, and a hinge turns about Z whatever its axis says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JointKind {
    /// Welded: no relative motion at all.
    Fixed,
    /// A ball and socket: free to turn any way about the anchor, within an
    /// optional cone (`swing_limit`, the half-angle off `twist_axis`) and an
    /// optional twist about that axis. 3D only.
    Spherical {
        twist_axis: Vec3,
        swing_limit: Option<f32>,
        twist_limit: Option<f32>,
    },
    /// A hinge: turns about `axis` only, optionally between two angles.
    Revolute { axis: Vec3, limits: Option<(f32, f32)> },
    /// A slider: moves along `axis` only, optionally between two distances.
    Prismatic { axis: Vec3, limits: Option<(f32, f32)> },
    /// A rope or rod: the anchors stay between `min` and `max` apart, and the
    /// bodies are otherwise free.
    Distance { min: f32, max: f32 },
}

// ── What the engine asks of a backend ────────────────────────────────────

/// One request to change a body, applied by the backend in
/// [`PhysicsSet::Commands`].
///
/// A queue, not components a backend must recognise, so scripts and
/// engine crates push the same thing whichever engine is simulating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsCommand {
    /// A force, in world space, acting through every physics step until the
    /// next frame's commands are applied. Several in one frame add up. Pushed
    /// every frame, it is a steady force at any frame rate.
    AddForce { entity: Entity, force: Vec3 },
    /// A torque, in world space, with the same lifetime as [`Self::AddForce`].
    AddTorque { entity: Entity, torque: Vec3 },
    /// An instantaneous push through the centre of mass, in newton-seconds:
    /// the velocity changes by `impulse / mass`.
    AddImpulse { entity: Entity, impulse: Vec3 },
    /// An instantaneous spin, in newton-metre-seconds.
    AddAngularImpulse { entity: Entity, impulse: Vec3 },
    SetLinearVelocity { entity: Entity, velocity: Vec3 },
    /// In radians per second about each world axis; a 2D body reads only Z.
    SetAngularVelocity { entity: Entity, velocity: Vec3 },
    /// Throw away the backend's body and collider for this entity. The entity
    /// keeps its data and its [`RuntimePhysics`] marker, so it stays out of
    /// the simulation until its data changes. Terrain streams a far chunk's
    /// collider out this way and back in by re-inserting its shape.
    Strip { entity: Entity },
}

impl PhysicsCommand {
    pub fn entity(&self) -> Entity {
        match *self {
            Self::AddForce { entity, .. }
            | Self::AddTorque { entity, .. }
            | Self::AddImpulse { entity, .. }
            | Self::AddAngularImpulse { entity, .. }
            | Self::SetLinearVelocity { entity, .. }
            | Self::SetAngularVelocity { entity, .. }
            | Self::Strip { entity } => entity,
        }
    }
}

/// The frame's [`PhysicsCommand`]s. Every backend reads it in
/// [`PhysicsSet::Commands`], acting on the entities in its own world, and the
/// engine empties it straight after, so a command for an entity nobody
/// simulates is dropped rather than kept forever.
#[derive(Resource, Default, Debug)]
pub struct PhysicsCommands(pub Vec<PhysicsCommand>);

impl PhysicsCommands {
    pub fn push(&mut self, command: PhysicsCommand) {
        self.0.push(command);
    }
}

/// World-wide simulation settings. Owned by the engine (the editor's Physics
/// panel and the pause on leaving play mode write it) and applied by every
/// backend, so pausing means the same thing whichever one is running.
///
/// A backend applies it in `PreUpdate`, which runs before the fixed-step loop:
/// a pause written during one frame must stop the very next step, or the editor
/// would nudge the scene one tick every time it opens.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PhysicsSettings {
    pub gravity: Vec3,
    pub time_scale: f32,
    pub substeps: u32,
    pub paused: bool,
}

impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            time_scale: 1.0,
            substeps: 6,
            paused: false,
        }
    }
}

/// Where physics work happens each frame. The first four run in `Update`, in
/// this order; [`PhysicsSet::Commands`] runs in `PostUpdate`.
///
/// The simulation step itself is the backend's own business (avian steps in
/// `FixedPostUpdate`). Everything here happens between steps.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhysicsSet {
    /// Engine: route new entities to 2D or 3D, resolve colliders.
    Prepare,
    /// Backend: create and update bodies, colliders and joints from
    /// `RuntimePhysics`, `PhysicsBodyData`, `ResolvedCollider` and
    /// [`PhysicsJoint`].
    ///
    /// Removals mean: `RuntimePhysics` gone, drop everything of the entity's;
    /// `PhysicsBodyData` gone (the entity still wired), drop the body and keep
    /// its collider as a fixed one; `ResolvedCollider` gone, drop the collider;
    /// `PhysicsJoint` gone, drop the joint.
    Sync,
    /// Backend: write [`BodyVelocity`] and [`PhysicsContacts`].
    Writeback,
    /// Engine: turn what the backend wrote into script-readable state.
    Read,
    /// Backend, in `PostUpdate`: apply [`PhysicsCommands`].
    ///
    /// `PostUpdate` because scripts run in `Update` in no fixed order against
    /// these sets; applying any earlier would delay whatever a later script
    /// pushed by a frame. Everything pushed this frame lands before the next
    /// step.
    ///
    /// The set also exists in `FixedUpdate`, after the `on_fixed_update` script
    /// pass, so what a fixed hook pushes reaches the step that follows that
    /// tick. The engine empties the queue after it there too.
    ///
    /// Forces are applied differently in the fixed tick. In `PostUpdate` a
    /// backend first drops every force it applied last frame, so a force pushed
    /// every frame is a steady one. Doing that inside a fixed tick would drop
    /// the forces `on_update` scripts set on *other* bodies whenever any fixed
    /// hook pushed anything: a fan driven from `on_update` would stall because
    /// the player is driven from `on_fixed_update`. So in a fixed tick a backend
    /// resets only the bodies this tick gives a new force or torque, replacing
    /// what they had, and leaves the rest. The next `PostUpdate` clears them all
    /// as usual, so a fixed-hook force lasts the remaining ticks of its frame
    /// and is renewed by every tick that pushes it again.
    Commands,
}

// ── Queries ──────────────────────────────────────────────────────────────

/// What a [`Cast`] sweeps.
#[derive(Clone, Debug, PartialEq)]
pub enum CastShape {
    Ray,
    Shape(ResolvedShape),
    /// The collider this entity already has. For sweeping a body's own shape
    /// (collide-and-slide) without rebuilding it per cast, which for a trimesh
    /// would mean rebuilding its whole acceleration structure.
    Collider(Entity),
}

/// One sweep from `origin` along `direction` (unit length) for up to
/// `max_distance`, all in world space. A [`PhysicsDim::D2`] cast reads only the
/// XY of each vector.
#[derive(Clone, Debug, PartialEq)]
pub struct Cast {
    pub dim: PhysicsDim,
    pub shape: CastShape,
    pub origin: Vec3,
    pub rotation: Quat,
    pub direction: Vec3,
    pub max_distance: f32,
    /// Colliders the cast passes through, usually the caster itself.
    pub exclude: Vec<Entity>,
    /// Whether sensors (trigger volumes) count as hits.
    pub include_sensors: bool,
    /// Ignore a shape that already overlaps the cast at its origin, so a body
    /// resting on the floor can still sweep along it. A ray that starts inside
    /// a collider otherwise hits it at distance zero.
    pub ignore_origin_penetration: bool,
    /// The collision layers the cast can hit (see [`CollisionLayers`]); a
    /// collider on none of them is passed through.
    pub collides_with: u32,
}

impl Cast {
    /// A ray from `origin` along `direction`, hitting everything solid.
    pub fn ray(dim: PhysicsDim, origin: Vec3, direction: Vec3, max_distance: f32) -> Self {
        Self {
            dim,
            shape: CastShape::Ray,
            origin,
            rotation: Quat::IDENTITY,
            direction,
            max_distance,
            exclude: Vec::new(),
            include_sensors: false,
            ignore_origin_penetration: false,
            collides_with: u32::MAX,
        }
    }

    /// `shape`, turned by `rotation`, swept from `origin` along `direction`.
    pub fn shape(
        dim: PhysicsDim,
        shape: CastShape,
        origin: Vec3,
        rotation: Quat,
        direction: Vec3,
        max_distance: f32,
    ) -> Self {
        Self { shape, rotation, ..Self::ray(dim, origin, direction, max_distance) }
    }

    pub fn excluding(mut self, entities: impl IntoIterator<Item = Entity>) -> Self {
        self.exclude.extend(entities);
        self
    }

    pub fn with_sensors(mut self, include: bool) -> Self {
        self.include_sensors = include;
        self
    }

    pub fn ignoring_origin_penetration(mut self, ignore: bool) -> Self {
        self.ignore_origin_penetration = ignore;
        self
    }

    pub fn colliding_with(mut self, layers: u32) -> Self {
        self.collides_with = layers;
        self
    }
}

/// Where a [`Cast`] first hit a collider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CastHit {
    /// Along the cast's direction, from its origin.
    pub distance: f32,
    /// The contact point on the hit collider's surface.
    pub point: Vec3,
    /// The hit surface's outward normal at the contact.
    pub normal: Vec3,
    pub entity: Entity,
}

/// A batch of casts and their answers, index for index.
#[derive(Clone, Debug, Default)]
pub struct CastBatch {
    pub casts: Vec<Cast>,
    pub hits: Vec<Option<CastHit>>,
}

/// Which colliders a shape, placed and held still, overlaps: a trigger check
/// without a sensor entity, an explosion's reach, "is this spot free to spawn
/// in". Fields mean what they do on [`Cast`].
#[derive(Clone, Debug, PartialEq)]
pub struct Overlap {
    pub dim: PhysicsDim,
    pub shape: ResolvedShape,
    pub position: Vec3,
    pub rotation: Quat,
    pub exclude: Vec<Entity>,
    pub include_sensors: bool,
    pub collides_with: u32,
}

impl Overlap {
    pub fn new(dim: PhysicsDim, shape: ResolvedShape, position: Vec3) -> Self {
        Self {
            dim,
            shape,
            position,
            rotation: Quat::IDENTITY,
            exclude: Vec::new(),
            include_sensors: false,
            collides_with: u32::MAX,
        }
    }
}

/// A batch of overlap queries and the entities each one found, index for index.
#[derive(Clone, Debug, Default)]
pub struct OverlapBatch {
    pub queries: Vec<Overlap>,
    pub hits: Vec<Vec<Entity>>,
}

// ── The backend ──────────────────────────────────────────────────────────

/// What a backend can actually do.
///
/// A bitfield rather than an assumption, for the same reason as audio's: the
/// engine must be able to tell a scene's author that their trimesh collider is
/// unsupported, instead of the collider silently not existing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps(pub u32);

impl Caps {
    /// Simulates the 3D world.
    pub const DIM_3D: Self = Self(1 << 0);
    /// Simulates the 2D world.
    pub const DIM_2D: Self = Self(1 << 1);
    /// Builds [`ResolvedShape::TriMesh`] colliders.
    pub const TRIMESH: Self = Self(1 << 2);
    /// Simulates joints.
    pub const JOINTS: Self = Self(1 << 3);
    /// Continuous collision detection for fast bodies.
    pub const CCD: Self = Self(1 << 4);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn dim(dim: PhysicsDim) -> Self {
        match dim {
            PhysicsDim::D3 => Self::DIM_3D,
            PhysicsDim::D2 => Self::DIM_2D,
        }
    }
}

/// A physics engine the engine can drive.
///
/// The simulation itself is not here: a backend adds ordinary systems to
/// [`PhysicsSet::Sync`] and [`PhysicsSet::Writeback`], gated with
/// [`physics_backend_selected`] so an unselected backend costs nothing per
/// frame. What is here is everything the engine asks *of* it from outside
/// those systems.
pub trait PhysicsBackend: Send + Sync + 'static {
    /// Unique, for logs and the backend picker. Two registrations under one
    /// name are one backend registered twice.
    fn name(&self) -> &str;

    /// What this backend does. Must include the dimension(s) it simulates.
    fn caps(&self) -> Caps;

    /// Answer every cast in `batch` whose `dim` this backend simulates, writing
    /// `batch.hits[i]` for cast `i`. The engine sizes `hits` to `casts` and
    /// fills it with `None` first, so a cast for another world is left alone.
    ///
    /// Read-only access to the world, deliberately: a caller can then hold
    /// other read-only queries open across its casts (a character controller
    /// asks what it hit between sweeps), which a `&mut World` would forbid.
    fn cast(&self, world: &World, batch: &mut CastBatch);

    /// Answer every overlap query in `batch` whose `dim` this backend
    /// simulates, filling `batch.hits[i]`. Same rules as [`Self::cast`]. The
    /// default finds nothing, so a backend written before overlaps existed
    /// still compiles; it just answers "nothing here".
    fn overlap(&self, world: &World, batch: &mut OverlapBatch) {
        let _ = (world, batch);
    }

    /// Prefixes of the type paths of every component this backend puts on
    /// entities (`"avian3d::"`). Scene saving drops them: they are rebuilt from
    /// the renzora data on load, and a scene that carried them would only load
    /// with this backend installed. A backend that keeps its state off the
    /// entities, as Rapier does, has none.
    fn runtime_type_prefixes(&self) -> &[&'static str] {
        &[]
    }

    /// This backend has just stopped simulating `dim`: throw away every body
    /// and collider it built for that world, and stop stepping it. The engine
    /// then re-wires every entity of that world, so the new backend sees each
    /// one arrive as if the scene had just loaded.
    ///
    /// Called with the world in hand rather than left to the backend's own
    /// systems, because those are gated on being selected and would never run
    /// again to clean up.
    fn detach(&self, world: &mut World, dim: PhysicsDim) {
        let _ = (world, dim);
    }
}

/// Every registered backend, and which one simulates each world.
///
/// Several may register, unlike audio or networking, where a second mixer
/// would fight the first over one device. Two physics engines conflict only if
/// both *run*, so they are kept and one is selected per dimension. The first
/// registration that claims a dimension holds it until something selects
/// another (the project's `[physics]` settings, through `renzora_physics`).
#[derive(Resource, Default, Clone)]
pub struct PhysicsBackends {
    registered: Vec<Arc<dyn PhysicsBackend>>,
    selected_3d: Option<usize>,
    selected_2d: Option<usize>,
    /// The first claim per dimension: what "default" means in project settings.
    default_3d: Option<usize>,
    default_2d: Option<usize>,
}

impl PhysicsBackends {
    /// The backend that simulates `dim` when the project names none: the first
    /// one registered for it, which is the bundled one on any build that has it.
    pub fn default_for(&self, dim: PhysicsDim) -> Option<&Arc<dyn PhysicsBackend>> {
        let index = match dim {
            PhysicsDim::D3 => self.default_3d,
            PhysicsDim::D2 => self.default_2d,
        }?;
        self.registered.get(index)
    }

    /// Every registered backend that can simulate `dim`, in registration order.
    pub fn for_dim(&self, dim: PhysicsDim) -> impl Iterator<Item = &Arc<dyn PhysicsBackend>> {
        self.registered.iter().filter(move |b| b.caps().contains(Caps::dim(dim)))
    }

    /// Make `name` the backend for `dim`. Only records the choice: switching a
    /// live world also needs the old backend detached and every entity re-wired,
    /// which `renzora_physics::switch_backend` does around this. Returns false
    /// when no registered backend by that name simulates `dim`.
    pub fn select(&mut self, name: &str, dim: PhysicsDim) -> bool {
        let Some(index) = self
            .registered
            .iter()
            .position(|b| b.name() == name && b.caps().contains(Caps::dim(dim)))
        else {
            return false;
        };
        match dim {
            PhysicsDim::D3 => self.selected_3d = Some(index),
            PhysicsDim::D2 => self.selected_2d = Some(index),
        }
        true
    }

    /// The backend simulating `dim`, if any.
    pub fn selected(&self, dim: PhysicsDim) -> Option<&Arc<dyn PhysicsBackend>> {
        let index = match dim {
            PhysicsDim::D3 => self.selected_3d,
            PhysicsDim::D2 => self.selected_2d,
        }?;
        self.registered.get(index)
    }

    /// True when the backend called `name` is the one simulating `dim`.
    pub fn is_selected(&self, name: &str, dim: PhysicsDim) -> bool {
        self.selected(dim).is_some_and(|b| b.name() == name)
    }

    /// Every registered backend, in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn PhysicsBackend>> {
        self.registered.iter()
    }

    /// The backends simulating the worlds `dims` names, each once.
    fn selected_for(&self, dims: impl Iterator<Item = PhysicsDim> + Clone) -> Vec<&Arc<dyn PhysicsBackend>> {
        let mut run: Vec<&Arc<dyn PhysicsBackend>> = Vec::with_capacity(2);
        for dim in [PhysicsDim::D3, PhysicsDim::D2] {
            if !dims.clone().any(|d| d == dim) {
                continue;
            }
            if let Some(b) = self.selected(dim) {
                if !run.iter().any(|r| Arc::ptr_eq(r, b)) {
                    run.push(b);
                }
            }
        }
        run
    }

    /// Run `batch` against the selected backends. Takes the world rather than
    /// `&self` because a backend's query reads its own state out of it.
    pub fn cast(world: &World, batch: &mut CastBatch) {
        batch.hits.clear();
        batch.hits.resize(batch.casts.len(), None);
        if batch.casts.is_empty() {
            return;
        }
        let Some(backends) = world.get_resource::<PhysicsBackends>() else {
            return;
        };
        for backend in backends.selected_for(batch.casts.iter().map(|c| c.dim)) {
            backend.cast(world, batch);
        }
        // Whatever a backend did to the vector, a caller indexes it by cast.
        batch.hits.resize(batch.casts.len(), None);
    }

    /// One cast, for a caller with one question. A batch is cheaper for many.
    pub fn cast_one(world: &World, cast: Cast) -> Option<CastHit> {
        let mut batch = CastBatch { casts: vec![cast], hits: Vec::new() };
        Self::cast(world, &mut batch);
        batch.hits[0]
    }

    /// Run `batch`'s overlap queries against the selected backends.
    pub fn overlap(world: &World, batch: &mut OverlapBatch) {
        batch.hits.clear();
        batch.hits.resize(batch.queries.len(), Vec::new());
        if batch.queries.is_empty() {
            return;
        }
        let Some(backends) = world.get_resource::<PhysicsBackends>() else {
            return;
        };
        for backend in backends.selected_for(batch.queries.iter().map(|q| q.dim)) {
            backend.overlap(world, batch);
        }
        batch.hits.resize(batch.queries.len(), Vec::new());
    }

    /// True when `type_path` names a component some registered backend keeps on
    /// entities, so a scene save should leave it out. Every registered backend,
    /// not only the selected ones: a world that just switched may still hold
    /// the old one's components for a frame.
    pub fn is_runtime_type(&self, type_path: &str) -> bool {
        self.registered
            .iter()
            .any(|b| b.runtime_type_prefixes().iter().any(|p| type_path.starts_with(p)))
    }
}

/// Run condition: the backend called `name` simulates `dim`.
pub fn physics_backend_selected(
    name: &'static str,
    dim: PhysicsDim,
) -> impl Fn(Option<Res<PhysicsBackends>>) -> bool + Clone {
    move |backends: Option<Res<PhysicsBackends>>| {
        backends.is_some_and(|b| b.is_selected(name, dim))
    }
}

/// Install a physics backend.
pub trait AppPhysicsBackendExt {
    /// Register `backend`. It simulates each dimension it claims that no
    /// earlier registration already holds.
    fn add_physics_backend(&mut self, backend: impl PhysicsBackend) -> &mut Self;
}

impl AppPhysicsBackendExt for App {
    fn add_physics_backend(&mut self, backend: impl PhysicsBackend) -> &mut Self {
        let name = backend.name().to_string();
        let caps = backend.caps();
        let mut slot = self
            .world_mut()
            .get_resource_or_insert_with(PhysicsBackends::default);
        if slot.registered.iter().any(|b| b.name() == name) {
            error!("physics backend `{name}` is ignored: a backend by that name is already registered");
            return self;
        }
        let index = slot.registered.len();
        slot.registered.push(Arc::new(backend));
        let mut claimed = Vec::new();
        if caps.contains(Caps::DIM_3D) && slot.selected_3d.is_none() {
            slot.selected_3d = Some(index);
            slot.default_3d = Some(index);
            claimed.push("3D");
        }
        if caps.contains(Caps::DIM_2D) && slot.selected_2d.is_none() {
            slot.selected_2d = Some(index);
            slot.default_2d = Some(index);
            claimed.push("2D");
        }
        if claimed.is_empty() {
            // Not "not selected": the project's choice is applied a few frames
            // later, and this line read as though that choice had been refused.
            info!("[physics] backend `{name}` registered; available to select in Project Settings > Physics");
        } else {
            info!("[physics] backend `{name}` registered, simulating {}", claimed.join(" + "));
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::reflect::TypePath;

    /// Scenes on disk name these by their old `renzora_physics::data` path. A
    /// change here is every saved collider failing to load.
    #[test]
    fn moved_types_keep_their_scene_paths() {
        assert_eq!(PhysicsBodyData::type_path(), "renzora_physics::data::PhysicsBodyData");
        assert_eq!(PhysicsBodyType::type_path(), "renzora_physics::data::PhysicsBodyType");
        assert_eq!(CollisionShapeData::type_path(), "renzora_physics::data::CollisionShapeData");
        assert_eq!(CollisionShapeType::type_path(), "renzora_physics::data::CollisionShapeType");
        assert_eq!(Physics2d::type_path(), "renzora_physics::data::Physics2d");
    }

    struct Fake(&'static str, Caps);

    impl PhysicsBackend for Fake {
        fn name(&self) -> &str {
            self.0
        }
        fn caps(&self) -> Caps {
            self.1
        }
        fn cast(&self, _world: &World, batch: &mut CastBatch) {
            let dim = if self.1.contains(Caps::DIM_3D) { PhysicsDim::D3 } else { PhysicsDim::D2 };
            for (cast, hit) in batch.casts.iter().zip(batch.hits.iter_mut()) {
                if cast.dim == dim {
                    *hit = Some(CastHit { distance: 1.0, point: Vec3::ZERO, normal: Vec3::Y, entity: Entity::PLACEHOLDER });
                }
            }
        }
    }

    #[test]
    fn first_claim_per_dimension_wins() {
        let mut app = App::new();
        app.add_physics_backend(Fake("a3", Caps::DIM_3D));
        app.add_physics_backend(Fake("b2", Caps::DIM_2D));
        app.add_physics_backend(Fake("c3", Caps::DIM_3D));
        let backends = app.world().resource::<PhysicsBackends>();
        assert!(backends.is_selected("a3", PhysicsDim::D3));
        assert!(backends.is_selected("b2", PhysicsDim::D2));
        assert!(!backends.is_selected("c3", PhysicsDim::D3));
        assert_eq!(backends.iter().count(), 3);
    }

    #[test]
    fn selecting_moves_one_world_and_keeps_the_default() {
        let mut app = App::new();
        app.add_physics_backend(Fake("a3", Caps::DIM_3D));
        app.add_physics_backend(Fake("c3", Caps::DIM_3D));
        let mut backends = app.world_mut().resource_mut::<PhysicsBackends>();
        assert!(backends.select("c3", PhysicsDim::D3));
        assert!(backends.is_selected("c3", PhysicsDim::D3));
        assert_eq!(backends.default_for(PhysicsDim::D3).unwrap().name(), "a3");
        assert!(!backends.select("c3", PhysicsDim::D2), "c3 cannot simulate 2D");
        assert!(!backends.select("nope", PhysicsDim::D3));
        assert_eq!(backends.for_dim(PhysicsDim::D3).count(), 2);
    }

    #[test]
    fn a_cast_reaches_only_its_own_world() {
        let mut app = App::new();
        app.add_physics_backend(Fake("a3", Caps::DIM_3D));
        let cast = |dim| Cast::ray(dim, Vec3::ZERO, Vec3::NEG_Y, 10.0);
        let mut batch = CastBatch { casts: vec![cast(PhysicsDim::D3), cast(PhysicsDim::D2)], hits: Vec::new() };
        PhysicsBackends::cast(app.world(), &mut batch);
        assert!(batch.hits[0].is_some());
        assert!(batch.hits[1].is_none(), "no 2D backend, so the 2D cast is a miss");
    }

    #[test]
    fn layers_filter_from_both_sides() {
        let a = CollisionLayers { memberships: 0b01, filters: 0b10 };
        let b = CollisionLayers { memberships: 0b10, filters: 0b01 };
        let c = CollisionLayers { memberships: 0b10, filters: 0b10 };
        assert!(a.interacts_with(b) && b.interacts_with(a));
        assert!(!a.interacts_with(c), "c does not collide with a's layer");
        assert!(CollisionLayers::DEFAULT.interacts_with(CollisionLayers::DEFAULT));
    }

    #[test]
    fn a_turned_box_has_a_wider_aabb() {
        let mut c = ResolvedCollider::new(ResolvedShape::Cuboid { half_extents: Vec3::new(1.0, 0.1, 0.1) });
        assert_eq!(c.local_aabb(), (Vec3::new(-1.0, -0.1, -0.1), Vec3::new(1.0, 0.1, 0.1)));
        c.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        c.offset = Vec3::Y;
        let (min, max) = c.local_aabb();
        assert!((max.y - 2.0).abs() < 1e-5 && (min.y + 0.0).abs() < 1e-5, "{min} {max}");
    }

    /// Scenes saved before the new fields existed must load with today's
    /// defaults, not zeros: a zero layer mask would collide with nothing.
    #[test]
    fn old_scene_shapes_keep_colliding() {
        let old = r#"(shape_type: Box, half_extents: (0.5, 0.5, 0.5), radius: 0.5, half_height: 0.5, friction: 0.5, restitution: 0.0, is_sensor: false)"#;
        let shape: CollisionShapeData = ron::from_str(old).unwrap();
        assert_eq!(shape.layers(), CollisionLayers::DEFAULT);
        assert_eq!(shape.rotation, Quat::IDENTITY);
    }

    /// The same through reflection, which is the path scenes actually load by.
    #[test]
    fn old_reflected_shapes_and_bodies_fill_in_defaults() {
        use bevy::reflect::structs::DynamicStruct;
        use bevy::reflect::FromReflect;
        let mut shape = DynamicStruct::default();
        shape.insert("shape_type", CollisionShapeType::Sphere);
        shape.insert("half_extents", Vec3::splat(0.5));
        shape.insert("radius", 2.0f32);
        shape.insert("half_height", 0.5f32);
        shape.insert("friction", 0.5f32);
        shape.insert("restitution", 0.0f32);
        shape.insert("is_sensor", false);
        let shape = CollisionShapeData::from_reflect(&shape).expect("old shape loads");
        assert_eq!(shape.radius, 2.0);
        assert_eq!(shape.layers(), CollisionLayers::DEFAULT);
        assert_eq!(shape.rotation, Quat::IDENTITY);

        let mut body = DynamicStruct::default();
        body.insert("body_type", PhysicsBodyType::RigidBody);
        for f in ["mass", "gravity_scale", "linear_damping", "angular_damping"] {
            body.insert(f, 1.0f32);
        }
        for f in ["x", "y", "z"] {
            body.insert(format!("lock_rotation_{f}").as_str(), false);
            body.insert(format!("lock_translation_{f}").as_str(), false);
        }
        let body = PhysicsBodyData::from_reflect(&body).expect("old body loads");
        assert!(!body.ccd);
    }
}
