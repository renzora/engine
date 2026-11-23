//! Running [`Vehicle`]s: wheels found in the model, suspension and tyre forces
//! each physics step, and the wheels posed to match.
//!
//! The model is a raycast vehicle, the way game engines generally build cars.
//! Each wheel casts a ray down from where its suspension is mounted. Where it
//! finds ground, the suspension pushes the body up with a spring and a damper,
//! and the tyre pushes along the ground: the engine's drive, the brakes, and
//! sideways grip that stops the car sliding, all limited by how hard that
//! wheel is pressed down (a wheel in the air does nothing). Everything is a
//! force on the body; nothing sets its velocity or turns it, so the physics
//! engine does the rest. It goes through [`PhysicsCommands`] and
//! [`PhysicsBackends`], so it works on any backend.
//!
//! Forces are worked out in `FixedUpdate`, every physics step, from rays cast
//! against the world as that step left it.

use bevy::prelude::*;
use renzora_scripting_api::ActionState;
use renzora_physics_api::physics::{
    BodyVelocity, Cast, PhysicsBackends, PhysicsCommand, PhysicsCommands, PhysicsDim, PhysicsSettings,
};
use renzora_physics_api::vehicle::{Vehicle, VehicleControls, VehicleDrive, VehicleState};

use crate::PhysicsBodyData;

/// How much of each tyre force acts at the height of the body's centre
/// rather than down at the contact patch: 1 removes the roll a tyre's grip
/// puts into the body, 0 leaves all of it. A box body has its centre of mass
/// halfway up, far higher than a real car's, so at full height it would roll
/// over on every corner.
const ROLL_STABILITY: f32 = 0.7;

/// How quickly a tyre's grip cancels the wheel's sideways sliding, per
/// second: the rate the grip force aims for, before it is limited by how
/// hard the wheel is pressed down. Not "cancel it in one step", which with a
/// velocity read once a frame overshoots and jitters.
const GRIP_RESPONSE: f32 = 12.0;

/// Rolling resistance, as a deceleration in metres per second squared at each
/// wheel: what stops a car you let roll.
const ROLLING_RESISTANCE: f32 = 0.4;

/// Below this speed, in metres per second, the brake pedal reverses instead.
const REVERSE_BELOW: f32 = 0.5;

/// How quickly the engine spins a wheel up when its tyre is not holding it
/// back (in the air, or spinning on the spot), per second.
const WHEELSPIN_RESPONSE: f32 = 3.0;

/// How quickly the brakes stop a wheel that is not held by the ground, per
/// second.
const LOCK_RESPONSE: f32 = 12.0;

/// How quickly a wheel in the air with nothing driving it slows, per second.
const FREE_WHEEL_SLOWING: f32 = 0.3;

/// Move `from` toward `to`, closing the gap at `rate` per second.
fn approach(from: f32, to: f32, rate: f32, dt: f32) -> f32 {
    from + (to - from) * (1.0 - (-rate * dt).exp())
}

/// How fast the engine spins a wheel nothing holds back, in radians per
/// second: as fast as it would turn at the top speed (or the reverse one),
/// scaled by the pedal.
fn revving(vehicle: &Vehicle, radius: f32, drive: f32) -> f32 {
    let top = if drive >= 0.0 { vehicle.max_speed } else { vehicle.max_reverse_speed };
    drive * top / radius.max(0.01)
}

/// One wheel found in the model.
#[derive(Clone, Debug)]
struct Wheel {
    entity: Entity,
    front: bool,
    driven: bool,
    /// The wheel's centre where the model puts it, in the vehicle's own space.
    hub: Vec3,
    /// That centre in the wheel's own space, scaled to the vehicle's: where
    /// the wheel turns about, which is not always its origin.
    pivot: Vec3,
    /// The same centre unscaled, in the wheel's own space.
    centre: Vec3,
    /// Pieces of the wheel that sit off its axle, such as a brake caliper,
    /// with where the model put them. They steer and ride the suspension with
    /// the wheel but do not spin: a caliper turning with the wheel looks like
    /// the wheel wobbling.
    still: Vec<(Entity, Transform)>,
    /// The wheel's parent's transform in the vehicle's space, and its inverse,
    /// to turn a movement in the vehicle's space into one in the parent's.
    parent_rotation: Quat,
    vehicle_to_parent: Mat4,
    /// The wheel's turn in the vehicle's space at rest.
    rotation: Quat,
    /// The axle the wheel rolls about, in its own space, pointing to the
    /// vehicle's left so a positive turn rolls it forwards. Measured from the
    /// mesh, not assumed to be the vehicle's sideways axis: a model's front
    /// wheels are often posed slightly steered or cambered, and turning one
    /// about any other axis makes it wobble.
    axle: Vec3,
    radius: f32,
    // Per step.
    compression: f32,
    on_ground: bool,
    /// How far the spring is extended this step, 0..2 x travel.
    spring_length: f32,
    /// How fast it turns, in radians per second, and how far it has turned.
    spin_rate: f32,
    spin: f32,
}

/// The wheels a [`Vehicle`] found, and what it remembers between steps.
#[derive(Component, Clone, Debug, Default)]
pub struct VehicleRig {
    wheels: Vec<Wheel>,
    steer: f32,
}

/// A name as it is compared: lower case, `.` read as `_`, since the Hierarchy
/// and the model file write a name like `Cylinder.001` differently.
fn normalised(name: &str) -> String {
    name.to_lowercase().replace('.', "_")
}

/// Whether `name` is one of `axle`'s wheels: `<axle>_part<n>`, or `<axle>`
/// itself.
fn is_wheel_of(name: &str, axle: &str) -> bool {
    if axle.is_empty() {
        return false;
    }
    let (name, axle) = (normalised(name), normalised(axle));
    if name == axle {
        return true;
    }
    name.strip_prefix(&axle)
        .and_then(|rest| rest.strip_prefix("_part"))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// Find each new or changed vehicle's wheels, once its model has spawned
/// them. Run every frame until found: a model's parts arrive a moment after
/// the scene.
///
/// Found again whenever a wheel it holds is gone: a model's parts are
/// spawned again when it finishes loading (and on a reload), and a rig
/// holding the old ones still drives the car, from the hubs it remembers,
/// while posing wheels that no longer exist.
pub fn rig_vehicles(world: &mut World) {
    let mut wanted: Vec<(Entity, Vehicle, bool)> = Vec::new();
    let mut q = world.query::<(Entity, Ref<Vehicle>, Option<&VehicleRig>)>();
    for (entity, vehicle, rig) in q.iter(world) {
        let changed = vehicle.is_changed();
        let stale = rig.is_some_and(|r| r.wheels.iter().any(|w| world.get_entity(w.entity).is_err()));
        if rig.is_none_or(|r| r.wheels.is_empty()) || changed || stale {
            wanted.push((entity, (*vehicle).clone(), changed || stale));
        }
    }
    for (entity, vehicle, changed) in wanted {
        let wheels = find_wheels(world, entity, &vehicle);
        // Still no wheels (the model has not spawned them, or no axle is
        // named): nothing to write, so nothing marked changed every frame.
        if wheels.is_empty() && !changed && world.get::<VehicleRig>(entity).is_some() {
            continue;
        }
        let count = wheels.len() as u32;
        let name = world.get::<Name>(entity).map_or_else(|| format!("{entity}"), |n| n.as_str().to_string());
        if wheels.is_empty() {
            // Usually just not loaded yet: the search keeps going quietly,
            // and logs again when it finds them.
            info!(
                "[vehicle] '{name}': no wheels yet (parts named after '{}' and '{}')",
                vehicle.front_axle, vehicle.rear_axle
            );
        } else {
            let list: Vec<String> = wheels
                .iter()
                .map(|w| {
                    let n = world.get::<Name>(w.entity).map_or("?", |n| n.as_str());
                    format!("{n} (r {:.2} m)", w.radius)
                })
                .collect();
            info!("[vehicle] '{name}': {} wheels: {}", wheels.len(), list.join(", "));
        }
        if let Ok(mut e) = world.get_entity_mut(entity) {
            e.insert(VehicleRig { wheels, steer: 0.0 });
            if !e.contains::<VehicleControls>() {
                e.insert(VehicleControls::default());
            }
            let state = VehicleState { wheels: count, ..e.get::<VehicleState>().copied().unwrap_or_default() };
            e.insert(state);
        }
    }
}

fn find_wheels(world: &World, vehicle: Entity, settings: &Vehicle) -> Vec<Wheel> {
    let mut out = Vec::new();
    let vehicle_scale = world.get::<Transform>(vehicle).map_or(1.0, |t| t.scale.y);
    // Each descendant with its transform in the vehicle's space, from the
    // chain of local transforms: global ones lag a frame behind parts that
    // have just spawned.
    let mut stack = vec![(vehicle, Mat4::IDENTITY)];
    while let Some((entity, to_vehicle)) = stack.pop() {
        let Some(children) = world.get::<Children>(entity) else { continue };
        for child in children.iter() {
            let local = world.get::<Transform>(child).copied().unwrap_or_default();
            let child_to_vehicle = to_vehicle * local.to_matrix();
            let name = world.get::<Name>(child).map(|n| n.as_str().to_string()).unwrap_or_default();
            // A part named `<axle>_part<n>` is a wheel. A node named just
            // `<axle>` is one only when it holds no such parts: once its
            // wheels are separated it is what holds them. (A part's own mesh
            // entities can share its name, so this asks only of the axle.)
            let has_parts = |axle: &str| {
                normalised(&name) == normalised(axle)
                    && world.get::<Children>(child).is_some_and(|kids| {
                        kids.iter().any(|k| {
                            world.get::<Name>(k).is_some_and(|n| is_wheel_of(n.as_str(), axle) && normalised(n.as_str()) != normalised(axle))
                        })
                    })
            };
            let front = is_wheel_of(&name, &settings.front_axle) && !has_parts(&settings.front_axle);
            let rear = !front && is_wheel_of(&name, &settings.rear_axle) && !has_parts(&settings.rear_axle);
            if front || rear {
                let (_, parent_rotation, _) = to_vehicle.to_scale_rotation_translation();
                let (wheel_scale, rotation, origin) = child_to_vehicle.to_scale_rotation_translation();
                let shape = measure_wheel(world, child);
                // Metres per unit of the wheel's own space.
                let to_metres = wheel_scale.y * vehicle_scale;
                let radius = if settings.wheel_radius > 0.0 {
                    Some(settings.wheel_radius)
                } else if let Some(shape) = &shape {
                    Some(shape.radius * to_metres)
                } else {
                    measure_radius(world, child)
                };
                // Not measurable yet (its meshes have not been bounded): try
                // again next frame rather than keep a wrong size.
                let Some(radius) = radius else { return Vec::new() };
                let axle = shape.as_ref().map_or(rotation.inverse() * Vec3::X, |s| s.axle);
                // Facing the vehicle's left (+X, as it faces +Z): a positive
                // turn about it then rolls the wheel forwards.
                let axle = if (rotation * axle).x < 0.0 { -axle } else { axle };
                let centre = shape.as_ref().map_or(Vec3::ZERO, |s| s.centre);
                let pivot = centre * wheel_scale;
                let hub = origin + rotation * pivot;
                // Straight ahead at rest. A model often poses its front wheels
                // turned (and a little toed or cambered), and keeping that as
                // the rest pose leaves them turned when the car goes straight
                // and steering further one way than the other.
                let rotation = Quat::from_rotation_arc((rotation * axle).normalize_or(Vec3::X), Vec3::X) * rotation;
                let still = shape.map_or_else(Vec::new, |s| s.still);
                out.push(Wheel {
                    entity: child,
                    front,
                    driven: match settings.drive {
                        VehicleDrive::Front => front,
                        VehicleDrive::Rear => !front,
                        VehicleDrive::All => true,
                    },
                    hub,
                    pivot,
                    centre,
                    still,
                    parent_rotation,
                    vehicle_to_parent: to_vehicle.inverse(),
                    rotation,
                    axle,
                    radius,
                    compression: 0.0,
                    on_ground: false,
                    spring_length: settings.suspension_travel,
                    spin_rate: 0.0,
                    spin: 0.0,
                });
                continue;
            }
            stack.push((child, child_to_vehicle));
        }
    }
    out
}

/// A wheel's radius in world metres, from the bounds of the meshes under it:
/// a wheel is as wide as it is wide, and its other two extents are its
/// diameter, so the middle of the three is the radius. `None` until the
/// meshes have bounds.
fn measure_radius(world: &World, wheel: Entity) -> Option<f32> {
    let mut best: Option<f32> = None;
    let mut stack = vec![wheel];
    while let Some(e) = stack.pop() {
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
        let (Some(aabb), Some(global)) = (world.get::<bevy::camera::primitives::Aabb>(e), world.get::<GlobalTransform>(e))
        else {
            continue;
        };
        let m = global.affine().matrix3;
        let abs = bevy::math::Mat3A::from_cols(m.x_axis.abs(), m.y_axis.abs(), m.z_axis.abs());
        let half = abs * aabb.half_extents;
        let mut sides = [half.x, half.y, half.z];
        sides.sort_by(f32::total_cmp);
        best = Some(best.map_or(sides[1], |b: f32| b.max(sides[1])));
    }
    best
}

/// A wheel's true axle, centre and radius, in its own space, and the pieces
/// of it that must not spin.
#[derive(Clone, Debug)]
struct WheelShape {
    axle: Vec3,
    centre: Vec3,
    radius: f32,
    /// Pieces directly under the wheel whose middle sits well off its axle
    /// (a brake caliper), with where the model put them.
    still: Vec<(Entity, Transform)>,
}

/// One mesh under a wheel: its triangles in the wheel's own space.
struct Piece {
    entity: Entity,
    rest: Transform,
    /// Directly under the wheel, so the wheel's own turn can be undone on it.
    direct: bool,
    triangles: Vec<[Vec3; 3]>,
}

/// Measure a wheel from its meshes. `None` while they are not loaded.
fn measure_wheel(world: &World, wheel: Entity) -> Option<WheelShape> {
    shape_of(&wheel_pieces(world, wheel)?)
}

/// A wheel's axle, centre and radius from its surface.
///
/// The axle is the direction the wheel's surface spreads least along, and the
/// centre the middle of that surface, both weighted by **area**, not by
/// vertex. A tyre, rim and disc are round about the axle, so their area is
/// spread evenly round it however the mesh is cut into triangles. Their
/// vertices are not: a model's simplified stand-in (what an `.rvg` model's
/// mesh is in memory) packs vertices where the detail was, and counting
/// vertices tilted a measured axle by 3 degrees, which turns into a wheel
/// wobbling as if it were loose.
///
/// Anything off the axle (a brake caliper) would pull the measurement over,
/// so it is found from a first measurement of everything, left out of the
/// second, and kept from spinning.
fn shape_of(pieces: &[Piece]) -> Option<WheelShape> {
    let all = pieces.iter().fold(Moments::default(), |m, p| m.with(&p.triangles));
    let (rough_axle, rough_centre) = all.axle_and_centre()?;
    let reach = radial_reach(pieces.iter(), rough_axle, rough_centre);
    let off_axle = |p: &Piece| {
        let Some((_, middle)) = Moments::default().with(&p.triangles).axle_and_centre() else { return false };
        let off = middle - rough_centre;
        p.direct && (off - rough_axle * off.dot(rough_axle)).length() > reach * 0.15
    };
    let (still, round): (Vec<&Piece>, Vec<&Piece>) = pieces.iter().partition(|p| off_axle(p));
    let round = if round.is_empty() { pieces.iter().collect() } else { round };
    let (axle, centre) = round.iter().fold(Moments::default(), |m, p| m.with(&p.triangles)).axle_and_centre()?;

    // The radius: a circle fitted to the tread, the outermost tenth.
    let (u, v) = axle.any_orthonormal_pair();
    let flat: Vec<Vec2> = round
        .iter()
        .flat_map(|p| p.triangles.iter().flatten())
        .map(|q| Vec2::new((*q - centre).dot(u), (*q - centre).dot(v)))
        .collect();
    let reach = flat.iter().map(|q| q.length()).fold(0.0f32, f32::max);
    let tread: Vec<Vec2> = flat.into_iter().filter(|q| q.length() > reach * 0.9).collect();
    let radius = fit_circle(&tread).map_or(reach, |(_, r)| r);
    (radius > 0.0).then(|| WheelShape {
        axle,
        centre,
        radius,
        still: still.iter().map(|p| (p.entity, p.rest)).collect(),
    })
}

/// How far the furthest vertex sits from the axle.
fn radial_reach<'a>(pieces: impl Iterator<Item = &'a Piece>, axle: Vec3, centre: Vec3) -> f32 {
    pieces
        .flat_map(|p| p.triangles.iter().flatten())
        .map(|q| {
            let d = *q - centre;
            (d - axle * d.dot(axle)).length()
        })
        .fold(0.0, f32::max)
}

/// The area, first and second moments of a set of triangles, in `f64`: the
/// second moment of a surface is a difference of large, close numbers.
#[derive(Default)]
struct Moments {
    area: f64,
    first: bevy::math::DVec3,
    second: bevy::math::DMat3,
}

impl Moments {
    fn with(mut self, triangles: &[[Vec3; 3]]) -> Self {
        use bevy::math::{DMat3, DVec3};
        let outer = |v: DVec3| DMat3::from_cols(v * v.x, v * v.y, v * v.z);
        for t in triangles {
            let [a, b, c] = t.map(|p| p.as_dvec3());
            let area = 0.5 * (b - a).cross(c - a).length();
            if area <= 0.0 {
                continue;
            }
            let sum = a + b + c;
            self.area += area;
            self.first += sum * (area / 3.0);
            // A triangle's exact second moment about the origin.
            self.second += (outer(a) + outer(b) + outer(c) + outer(sum)) * (area / 12.0);
        }
        self
    }

    /// The direction the surface spreads least along (the smallest
    /// eigenvector of its covariance, found as the largest one of
    /// `trace - C` by power iteration) and its middle.
    fn axle_and_centre(&self) -> Option<(Vec3, Vec3)> {
        use bevy::math::{DMat3, DVec3};
        if self.area <= 0.0 {
            return None;
        }
        let mean = self.first / self.area;
        let c = self.second * (1.0 / self.area) - DMat3::from_cols(mean * mean.x, mean * mean.y, mean * mean.z);
        let shifted = DMat3::from_diagonal(DVec3::splat(c.x_axis.x + c.y_axis.y + c.z_axis.z)) - c;
        let mut v = DVec3::new(1.0, 0.31, 0.17).normalize();
        for _ in 0..200 {
            v = (shifted * v).normalize_or(DVec3::X);
        }
        Some((v.as_vec3(), mean.as_vec3()))
    }
}

/// Every mesh under `wheel`, as triangles in the wheel's own space.
fn wheel_pieces(world: &World, wheel: Entity) -> Option<Vec<Piece>> {
    let meshes = world.get_resource::<Assets<Mesh>>()?;
    let mut pieces = Vec::new();
    let mut stack = vec![(wheel, Mat4::IDENTITY, Transform::IDENTITY, false)];
    while let Some((e, to_wheel, rest, direct)) = stack.pop() {
        if let Some(children) = world.get::<Children>(e) {
            for child in children.iter() {
                let local = world.get::<Transform>(child).copied().unwrap_or_default();
                stack.push((child, to_wheel * local.to_matrix(), local, e == wheel));
            }
        }
        let Some(mesh) = world.get::<Mesh3d>(e).and_then(|m| meshes.get(&m.0)) else { continue };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            continue;
        };
        let at = |i: usize| to_wheel.transform_point3(Vec3::from_array(positions[i]));
        let indices: Vec<usize> = match mesh.indices() {
            Some(indices) => indices.iter().collect(),
            None => (0..positions.len()).collect(),
        };
        let triangles = indices
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|t| t.iter().all(|&i| i < positions.len()))
            .map(|&[a, b, c]| [at(a), at(b), at(c)])
            .collect();
        pieces.push(Piece { entity: e, rest, direct, triangles });
    }
    (!pieces.is_empty()).then_some(pieces)
}

/// The circle through `points` with the least error, as its centre and
/// radius: the algebraic fit, solving `x² + y² = 2ax + 2by + c` by least
/// squares. Points spread all the way round (a tyre's tread) fit exactly.
fn fit_circle(points: &[Vec2]) -> Option<(Vec2, f32)> {
    let n = points.len() as f64;
    let (mut sx, mut sy, mut sxx, mut syy, mut sxy, mut sxz, mut syz, mut sz) = (0.0f64, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in points {
        let (x, y) = (p.x as f64, p.y as f64);
        let z = x * x + y * y;
        sx += x;
        sy += y;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
        sxz += x * z;
        syz += y * z;
        sz += z;
    }
    let m = bevy::math::DMat3::from_cols_array(&[sxx, sxy, sx, sxy, syy, sy, sx, sy, n]);
    if m.determinant().abs() < 1e-12 {
        return None;
    }
    let s = m.inverse() * bevy::math::DVec3::new(sxz, syz, sz);
    let (a, b) = (s.x / 2.0, s.y / 2.0);
    let r2 = s.z + a * a + b * b;
    (r2 > 0.0).then(|| (Vec2::new(a as f32, b as f32), r2.sqrt() as f32))
}

/// Write each vehicle's controls from the input actions, where it asks for it.
pub fn read_vehicle_input(actions: Option<Res<ActionState>>, mut vehicles: Query<(&Vehicle, &mut VehicleControls)>) {
    let Some(actions) = actions else { return };
    for (vehicle, mut controls) in &mut vehicles {
        if !vehicle.use_input {
            continue;
        }
        let stick = actions.axis_2d("move");
        // How far each trigger is squeezed, not just whether it is.
        let accelerate = actions.axis_1d("accelerate");
        let brake = actions.axis_1d("brake");
        let next = VehicleControls {
            throttle: stick.y.max(0.0).max(accelerate),
            brake: (-stick.y).max(0.0).max(brake),
            steer: stick.x.clamp(-1.0, 1.0),
        };
        if *controls != next {
            *controls = next;
        }
    }
}

/// Every physics step: each wheel's suspension and tyre forces, summed into
/// one force and one torque on the body.
pub fn drive_vehicles(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    if dt <= 0.0 {
        return;
    }
    let gravity = world.get_resource::<PhysicsSettings>().map_or(9.81, |s| s.gravity.length());
    let mut q = world.query::<(Entity, &Vehicle, &VehicleRig, &VehicleControls, &Transform, &PhysicsBodyData, Option<&BodyVelocity>)>();
    let jobs: Vec<_> = q
        .iter(world)
        .filter(|(.., rig, _, _, _, _)| !rig.wheels.is_empty())
        .map(|(e, v, rig, c, t, body, vel)| (e, v.clone(), rig.clone(), *c, *t, body.clone(), vel.copied().unwrap_or_default()))
        .collect();

    for (entity, vehicle, mut rig, controls, transform, body, velocity) in jobs {
        let wheel_count = rig.wheels.len() as f32;
        let driven_count = rig.wheels.iter().filter(|w| w.driven).count().max(1) as f32;
        let mass = body.mass.max(0.001);
        let weight = mass * gravity * body.gravity_scale;
        let mass_per_wheel = mass / wheel_count;
        let travel = vehicle.suspension_travel.max(0.01);
        // The car sits with each spring half compressed (each wheel where the
        // model puts it) when it is stiff enough to hold its share there.
        let stiffness = if vehicle.suspension_stiffness > 0.0 {
            vehicle.suspension_stiffness
        } else {
            weight / wheel_count / travel
        };
        let damping = 2.0 * vehicle.suspension_damping.clamp(0.0, 1.0) * (stiffness * mass_per_wheel).sqrt();

        let up = transform.rotation * Vec3::Y;
        let forward = transform.rotation * Vec3::Z;
        let centre = transform.translation;
        let speed = velocity.linear.dot(forward);

        // Steering eases to the stick and turns less at speed. The car faces
        // +Z with +Y up, so its right is -X and right is a negative turn.
        let pace = (speed.abs() / vehicle.max_speed.max(1.0)).clamp(0.0, 1.0);
        let lock = vehicle.max_steer_degrees.to_radians() * (1.0 - 0.6 * pace);
        let ease = 1.0 - (-8.0 * dt).exp();
        rig.steer += (controls.steer.clamp(-1.0, 1.0) - rig.steer) * ease;
        let steer_angle = -rig.steer * lock;

        // Throttle and brake: the brake pedal brakes while rolling forwards
        // and reverses once nearly stopped.
        let reversing = controls.brake > 0.0 && controls.throttle <= 0.0 && speed < REVERSE_BELOW;
        let drive = if reversing {
            if -speed < vehicle.max_reverse_speed { -controls.brake } else { 0.0 }
        } else if speed < vehicle.max_speed {
            controls.throttle
        } else {
            0.0
        };
        let braking = if reversing { 0.0 } else { controls.brake };

        let mut force = Vec3::ZERO;
        let mut torque = Vec3::ZERO;
        let mut on_ground = 0;
        for wheel in &mut rig.wheels {
            let hub = transform.transform_point(wheel.hub);
            let origin = hub + up * travel;
            let reach = 2.0 * travel + wheel.radius;
            let hit = PhysicsBackends::cast_one(world, Cast::ray(PhysicsDim::D3, origin, -up, reach).excluding([entity]));
            let Some(hit) = hit else {
                wheel.on_ground = false;
                wheel.compression = 0.0;
                wheel.spring_length = 2.0 * travel;
                continue;
            };
            on_ground += 1;
            let spring = (hit.distance - wheel.radius).clamp(0.0, 2.0 * travel);
            let compression = 2.0 * travel - spring;
            let rate = if wheel.on_ground { (compression - wheel.compression) / dt } else { 0.0 };
            wheel.on_ground = true;
            wheel.compression = compression;
            wheel.spring_length = spring;

            // Suspension: pushes, never pulls the car down onto the ground.
            let load = (stiffness * compression + damping * rate).max(0.0);
            let contact = hit.point;
            force += up * load;
            torque += (contact - centre).cross(up * load);

            // Tyre: along the ground in the wheel's heading.
            let normal = hit.normal;
            let heading = transform.rotation * Quat::from_rotation_y(if wheel.front { steer_angle } else { 0.0 }) * Vec3::Z;
            let along = (heading - normal * heading.dot(normal)).normalize_or_zero();
            let across = normal.cross(along).normalize_or_zero();
            let point_velocity = velocity.linear + velocity.angular.cross(contact - centre);
            let rolling = point_velocity.dot(along);
            let sliding = point_velocity.dot(across);

            let mut tyre = Vec3::ZERO;
            if wheel.driven {
                tyre += along * (vehicle.engine_force * drive / driven_count);
            }
            if braking > 0.0 {
                let stop = (rolling.abs() * mass_per_wheel / dt).min(vehicle.brake_force * braking / wheel_count);
                tyre -= along * rolling.signum() * stop;
            }
            tyre -= along * rolling.signum() * (ROLLING_RESISTANCE * mass_per_wheel).min(rolling.abs() * mass_per_wheel / dt);
            tyre -= across * sliding * mass_per_wheel * GRIP_RESPONSE;
            // Friction circle: no more than the grip allows for the load.
            let limit = vehicle.grip * load;
            let saturated = tyre.length() > limit;
            if saturated {
                tyre = tyre.normalize_or_zero() * limit;
            }
            let height = (centre - contact).dot(up) * ROLL_STABILITY;
            let at = contact + up * height;
            force += tyre;
            torque += (at - centre).cross(tyre);

            // The wheel turns with the ground, unless the tyre has lost its
            // grip: then the engine spins it up (stuck, or spinning off the
            // line) or the brakes lock it.
            let rolling_rate = rolling / wheel.radius.max(0.01);
            wheel.spin_rate = if saturated && wheel.driven && drive != 0.0 {
                // Spinning up from no slower than the ground is passing.
                let behind = (wheel.spin_rate - rolling_rate) * drive.signum() < 0.0;
                let from = if behind { rolling_rate } else { wheel.spin_rate };
                approach(from, revving(&vehicle, wheel.radius, drive), WHEELSPIN_RESPONSE, dt)
            } else if saturated && braking > 0.0 {
                approach(wheel.spin_rate, 0.0, LOCK_RESPONSE, dt)
            } else {
                rolling_rate
            };
        }
        // A wheel in the air runs free: the engine revs a driven one up, the
        // brakes stop it, and otherwise it slowly winds down.
        for wheel in rig.wheels.iter_mut().filter(|w| !w.on_ground) {
            wheel.spin_rate = if braking > 0.0 {
                approach(wheel.spin_rate, 0.0, LOCK_RESPONSE, dt)
            } else if wheel.driven && drive != 0.0 {
                approach(wheel.spin_rate, revving(&vehicle, wheel.radius, drive), WHEELSPIN_RESPONSE, dt)
            } else {
                approach(wheel.spin_rate, 0.0, FREE_WHEEL_SLOWING, dt)
            };
        }
        for wheel in &mut rig.wheels {
            wheel.spin = (wheel.spin + wheel.spin_rate * dt).rem_euclid(std::f32::consts::TAU);
        }

        if let Some(mut queue) = world.get_resource_mut::<PhysicsCommands>() {
            if on_ground > 0 {
                queue.push(PhysicsCommand::AddForce { entity, force });
                queue.push(PhysicsCommand::AddTorque { entity, torque });
            }
        }
        if let Ok(mut e) = world.get_entity_mut(entity) {
            e.insert(VehicleState { speed, wheels_on_ground: on_ground, wheels: rig.wheels.len() as u32 });
            if let Some(mut r) = e.get_mut::<VehicleRig>() {
                r.steer = rig.steer;
                for (stored, wheel) in r.wheels.iter_mut().zip(rig.wheels) {
                    *stored = wheel;
                }
            }
        }
    }
}

/// Place and turn each wheel: up and down with its suspension, steered, and
/// rolled by how far it has gone.
pub fn pose_vehicle_wheels(vehicles: Query<(&Vehicle, &VehicleRig, &Transform)>, mut wheels: Query<&mut Transform, Without<VehicleRig>>) {
    for (vehicle, rig, transform) in &vehicles {
        let travel = vehicle.suspension_travel.max(0.01);
        let lock = vehicle.max_steer_degrees.to_radians();
        let steer_angle = -rig.steer * lock;
        for wheel in &rig.wheels {
            let Ok(mut t) = wheels.get_mut(wheel.entity) else { continue };
            // In the vehicle's own space: the spring's change from rest, made
            // a length in that space (its scale stretches world metres).
            let lift = (travel - wheel.spring_length) / transform.scale.y.max(1e-4);
            let steer = if wheel.front { Quat::from_rotation_y(steer_angle) } else { Quat::IDENTITY };
            let in_vehicle = steer * wheel.rotation * Quat::from_axis_angle(wheel.axle, wheel.spin);
            // Turned about its centre, which stays on the hub (raised by the
            // spring) wherever the wheel's own origin happens to be.
            let origin = wheel.hub + Vec3::Y * lift - in_vehicle * wheel.pivot;
            let rotation = wheel.parent_rotation.inverse() * in_vehicle;
            let translation = wheel.vehicle_to_parent.transform_point3(origin);
            if t.translation != translation || t.rotation != rotation {
                t.translation = translation;
                t.rotation = rotation;
            }
            // Turned back by the spin about the wheel's centre, so they stay
            // where they are on the hub.
            let back = Quat::from_axis_angle(wheel.axle, -wheel.spin);
            for (piece, rest) in &wheel.still {
                let Ok(mut p) = wheels.get_mut(*piece) else { continue };
                let rotation = back * rest.rotation;
                let translation = wheel.centre + back * (rest.translation - wheel.centre);
                if p.translation != translation || p.rotation != rotation {
                    p.translation = translation;
                    p.rotation = rotation;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tyre 0.35 m in radius and 0.25 m wide, its axle tilted 10 degrees
    /// and its centre off the origin, cut into triangles far more finely on
    /// one side than the other (as a simplified stand-in mesh is), with a
    /// brake caliper on one side: the measured axle, centre and radius are
    /// the tyre's, and the caliper is the piece kept from spinning.
    #[test]
    fn a_wheel_is_measured_from_its_surface_not_its_vertices() {
        let tilt = Quat::from_rotation_z(10f32.to_radians());
        let centre = Vec3::new(0.02, 0.05, -0.03);
        let at = |w: f32, r: f32, a: f32| centre + tilt * Vec3::new(w, a.cos() * r, a.sin() * r);
        // Angles bunched towards zero: most of the vertices on one side.
        let steps = 48;
        let angle = |i: usize| (i as f32 / steps as f32).powi(2) * std::f32::consts::TAU;
        let mut tyre = Vec::new();
        for i in 0..steps {
            let (a0, a1) = (angle(i), angle(i + 1));
            // Tread, then both sidewalls from the tread in to the rim.
            for (p, q) in [((-0.125, 0.35), (0.125, 0.35)), ((-0.125, 0.35), (-0.125, 0.22)), ((0.125, 0.35), (0.125, 0.22))] {
                let (p0, p1, q0, q1) = (at(p.0, p.1, a0), at(p.0, p.1, a1), at(q.0, q.1, a0), at(q.0, q.1, a1));
                tyre.push([p0, q0, q1]);
                tyre.push([p0, q1, p1]);
            }
        }
        // The caliper: a slab inside the rim, all on one side.
        let lump = |x: f32, y: f32, z: f32| centre + tilt * Vec3::new(x, y, z);
        let caliper = vec![
            [lump(-0.04, 0.15, -0.06), lump(0.04, 0.15, -0.06), lump(0.04, 0.21, 0.06)],
            [lump(-0.04, 0.15, -0.06), lump(0.04, 0.21, 0.06), lump(-0.04, 0.21, 0.06)],
        ];
        let mut world = World::new();
        let (tyre_entity, caliper_entity) = (world.spawn_empty().id(), world.spawn_empty().id());
        let pieces = [
            Piece { entity: tyre_entity, rest: Transform::IDENTITY, direct: true, triangles: tyre },
            Piece { entity: caliper_entity, rest: Transform::IDENTITY, direct: true, triangles: caliper },
        ];
        let shape = shape_of(&pieces).unwrap();
        let axle = tilt * Vec3::X;
        // Within half a degree, though the coarse side's flat facets make the
        // tyre not quite round.
        assert!(shape.axle.dot(axle).abs() > 0.999_96, "axle {:?}", shape.axle);
        let off = shape.centre - centre;
        assert!((off - axle * off.dot(axle)).length() < 0.002, "centre {:?}", shape.centre);
        assert!((shape.radius - 0.35).abs() < 0.01, "radius {}", shape.radius);
        assert_eq!(shape.still.iter().map(|(e, _)| *e).collect::<Vec<_>>(), vec![caliper_entity]);
    }

    #[test]
    fn wheels_are_found_by_axle_name() {
        assert!(is_wheel_of("cylinder_001_part1", "cylinder_001"));
        assert!(is_wheel_of("Cylinder.001_part2", "cylinder_001"));
        assert!(is_wheel_of("front_wheel", "front_wheel"));
        assert!(!is_wheel_of("cylinder_001_part1_rubber", "cylinder_001"));
        assert!(!is_wheel_of("cylinder_0011", "cylinder_001"));
        assert!(!is_wheel_of("anything", ""));
    }
}
