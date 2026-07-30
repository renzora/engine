//! The reload probe you can see rather than read.
//!
//! Add `Drift` to a few entities in the editor, press play or just watch the
//! viewport, then change [`SHAPE`] below and rebuild. The entities change course
//! mid-flight — no restart, no reload of the scene, and they keep their positions
//! and their `Drift` values.
//!
//! This is the loop the whole hot-swap effort is for: edit a constant, save, see
//! it. What makes it fast is that this crate links no Bevy, so a rebuild is about
//! a second rather than the half-minute a Bevy-linking plugin spends linking.
//!
//! ## Try breaking it
//!
//! - **Introduce a compile error.** Nothing happens — the reload is refused and
//!   this build keeps running. The generation counter only advances after a
//!   successful init.
//! - **Add a field to `Drift`.** The reload is refused with the reason, because
//!   entities already carrying it were allocated for the old layout and Bevy fixes
//!   a component's layout permanently. Restart to pick that up.

use renzora_plugin::prelude::*;

/// The motion to apply. **Change this and rebuild** — that is the experiment.
const SHAPE: Motion = Motion::Circle;

/// Metres per second, or radians per second for [`Motion::Spin`].
const RATE: f32 = 1.5;

// Only whichever variant `SHAPE` names is constructed, and swapping between them
// is the entire exercise — so "never constructed" is the expected state for the
// other three rather than a sign anything is wrong.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
enum Motion {
    /// Straight along +X.
    Line,
    /// Around the Y axis, keeping the starting radius.
    Circle,
    /// Up and down.
    Bob,
    /// Rotate in place. Nothing moves, which makes it obvious when a reload took
    /// effect and equally obvious if BOTH builds are somehow still running.
    Spin,
}

/// Per-entity drift state. One field on purpose: it is the thing to add a second
/// field to when you want to see the layout-change refusal.
#[derive(Component)]
#[repr(C)]
pub struct Drift {
    /// Scales `RATE`, so entities sharing a scene can move at different speeds.
    pub factor: f32,
}

impl Default for Drift {
    fn default() -> Self {
        Self { factor: 1.0 }
    }
}

fn drift(mut q: Query<(&mut Transform, &Drift)>, time: Res<Time>) {
    let dt = time.delta_secs();
    for (transform, d) in &mut q {
        let speed = RATE * d.factor;
        match SHAPE {
            Motion::Line => {
                transform.translation.x += speed * dt;
            }
            Motion::Circle => {
                // Rotate the position about the origin. Reading the current
                // translation rather than tracking an angle is what lets a reload
                // pick up mid-orbit: there is no phase to lose.
                let (x, z) = (transform.translation.x, transform.translation.z);
                let a = speed * dt;
                let (sin, cos) = (a.sin(), a.cos());
                transform.translation.x = x * cos - z * sin;
                transform.translation.z = x * sin + z * cos;
            }
            Motion::Bob => {
                transform.translation.y += (time.elapsed_secs() * speed).cos() * speed * dt;
            }
            Motion::Spin => {
                transform.rotate_y(speed * dt);
            }
        }
    }
}

pub struct DriftPlugin;

impl Plugin for DriftPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Drift>().add_systems(Update, drift);
    }
}

renzora_plugin::add!(DriftPlugin);
