//! Wheeled vehicles: a rigid body held up and driven by its wheels.
//!
//! A [`Vehicle`] on a model's root makes it a car. Each wheel casts down to
//! the ground and pushes on the body with its suspension (a spring and a
//! damper) and its tyre (drive, braking and sideways grip, limited by how hard
//! the wheel is pressed down). Nothing else is done by hand: the body is an
//! ordinary rigid body with its real mass, so pitching over a crest, tipping
//! off an edge, flying, landing and rolling over come from the physics
//! engine, whichever one simulates it. `renzora_physics` runs it.
//!
//! The wheels are parts of the model, found by name: each axle's wheels are
//! the parts named `<axle>_part1`, `<axle>_part2`, ..., which is what the
//! Hierarchy's Separate Parts makes of an axle whose two wheels are one mesh.
//! A part named exactly `<axle>` counts as one wheel. The names live here,
//! on the root, because a model's own parts are rebuilt from its file on
//! every load and anything put on them would not be saved.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Which wheels the engine turns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum VehicleDrive {
    Front,
    #[default]
    Rear,
    All,
}

/// A wheeled vehicle. See the module docs.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Vehicle {
    /// The front axle's name in the model: its wheels steer.
    pub front_axle: String,
    /// The rear axle's name in the model.
    pub rear_axle: String,
    pub drive: VehicleDrive,
    /// The engine's push at full throttle, in newtons, shared by the driven
    /// wheels.
    pub engine_force: f32,
    /// The brakes' push at full pedal, in newtons, shared by every wheel.
    pub brake_force: f32,
    /// Top speed forwards and in reverse, in metres per second.
    pub max_speed: f32,
    pub max_reverse_speed: f32,
    /// How far the front wheels turn at full lock, in degrees. Less at speed.
    pub max_steer_degrees: f32,
    /// How far each wheel moves up or down from where the model puts it, in
    /// metres.
    pub suspension_travel: f32,
    /// Spring stiffness in newtons per metre. Zero works it out from the mass,
    /// so the car sits with its wheels where the model put them.
    pub suspension_stiffness: f32,
    /// How firmly the suspension settles, from 0 (bouncy) to 1 (no bounce at
    /// all).
    pub suspension_damping: f32,
    /// How hard the tyres grip, as a friction coefficient: about 1 on tarmac.
    pub grip: f32,
    /// The wheels' radius in metres. Zero measures it from the wheels.
    pub wheel_radius: f32,
    /// Drive it from the input actions: `move` (W/S and the arrows to drive
    /// and brake, A/D or the left stick to steer), `accelerate` and `brake`
    /// (the gamepad triggers). Off, a script drives it through
    /// [`VehicleControls`].
    pub use_input: bool,
}

impl Default for Vehicle {
    fn default() -> Self {
        Self {
            front_axle: String::new(),
            rear_axle: String::new(),
            drive: VehicleDrive::Rear,
            engine_force: 12_000.0,
            brake_force: 15_000.0,
            max_speed: 50.0,
            max_reverse_speed: 12.0,
            max_steer_degrees: 32.0,
            suspension_travel: 0.2,
            suspension_stiffness: 0.0,
            suspension_damping: 0.5,
            grip: 1.2,
            wheel_radius: 0.0,
            use_input: true,
        }
    }
}

/// What the driver is asking for this frame, each 0..1 except `steer`, which
/// is -1 (left) to 1 (right). Written from the input actions when
/// [`Vehicle::use_input`] is on; a script writes it otherwise. Not saved.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct VehicleControls {
    pub throttle: f32,
    pub brake: f32,
    pub steer: f32,
}

/// How the vehicle is doing, for a script or a HUD to read. Not saved.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct VehicleState {
    /// Speed along the car's heading, in metres per second; negative in
    /// reverse.
    pub speed: f32,
    /// How many wheels are on the ground.
    pub wheels_on_ground: u32,
    /// How many wheels the vehicle found in its model.
    pub wheels: u32,
}
