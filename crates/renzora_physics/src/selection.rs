//! Which backend simulates each world, and moving a live world from one to
//! another.
//!
//! The choice lives in `project.toml` (`[physics] backend_3d / backend_2d`) and
//! [`follow_project_backends`] keeps the running world in line with it, so the
//! editor's Project Settings, a hand-edited file and the play-mode game process
//! (which reads the same file at boot) all agree. A backend named there that is
//! not installed is reported once and the world stays on the default, which is
//! better than a game with no physics at all.

use bevy::prelude::*;
use renzora::physics::{
    PhysicsBackends, PhysicsDim, PhysicsJoint, PhysicsSettings, ResolvedCollider, RuntimePhysics,
    RuntimePhysics2d,
};
use renzora::CurrentProject;

use crate::resolve::PendingMeshCollider;

/// Move `dim` onto the backend called `name`.
///
/// The old backend is detached first (its bodies and colliders thrown away),
/// then every entity of that world loses its wiring markers, so the next
/// frame's routing hands each one to the new backend exactly as a freshly
/// loaded scene would. The settings are marked changed so the new backend
/// picks up gravity, time scale and, above all, whether the editor has the
/// simulation paused.
///
/// Returns false, changing nothing, when no registered backend by that name
/// simulates `dim`. Selecting the backend already in charge is a no-op.
pub fn switch_backend(world: &mut World, dim: PhysicsDim, name: &str) -> bool {
    let Some(backends) = world.get_resource::<PhysicsBackends>() else {
        return false;
    };
    if backends.is_selected(name, dim) {
        return true;
    }
    if !backends.for_dim(dim).any(|b| b.name() == name) {
        return false;
    }
    let old = backends.selected(dim).cloned();

    if let Some(old) = &old {
        old.detach(world, dim);
    }
    world.resource_mut::<PhysicsBackends>().select(name, dim);

    let mut rewire = world.query_filtered::<(Entity, Has<RuntimePhysics2d>), With<RuntimePhysics>>();
    let entities: Vec<Entity> = rewire
        .iter(world)
        .filter(|(_, is_2d)| *is_2d == (dim == PhysicsDim::D2))
        .map(|(e, _)| e)
        .collect();
    for entity in &entities {
        world.entity_mut(*entity).remove::<(
            RuntimePhysics,
            RuntimePhysics2d,
            ResolvedCollider,
            PendingMeshCollider,
        )>();
    }
    // Joints are not re-wired by routing; the new backend builds one when it
    // sees it change. Every joint is touched, not only this world's: a joint
    // rebuilt by a backend that already had it is the same joint.
    let mut joints = world.query::<&mut PhysicsJoint>();
    for mut joint in joints.iter_mut(world) {
        joint.set_changed();
    }
    world.get_resource_or_insert_with(PhysicsSettings::default).set_changed();

    let from = old.map(|b| b.name().to_string()).unwrap_or_else(|| "none".into());
    let label = match dim {
        PhysicsDim::D3 => "3D",
        PhysicsDim::D2 => "2D",
    };
    info!("[Physics] {label} world moved from `{from}` to `{name}` ({} entities)", entities.len());
    renzora::console_log::console_info(
        "Physics",
        format!("{label} physics now runs on {name} (was {from})"),
    );
    true
}

/// What the project asked for last time we looked, per world, so a missing
/// backend is reported once rather than every frame.
#[derive(Default)]
pub(crate) struct LastRequest {
    d3: Option<Option<String>>,
    d2: Option<Option<String>>,
    /// The backends last announced as running, so the one-line summary is
    /// logged when they change and not every frame.
    announced: Option<(String, String)>,
}

/// Keep each world on the backend `project.toml` names.
///
/// Exclusive because a switch needs the whole world. Cheap when nothing
/// changed: two string comparisons a frame.
pub(crate) fn follow_project_backends(world: &mut World, mut last: Local<LastRequest>) {
    let Some(project) = world.get_resource::<CurrentProject>() else {
        return;
    };
    let wanted = project.config.physics.clone();
    let LastRequest { d3, d2, announced } = &mut *last;
    for (dim, wanted, seen) in [
        (PhysicsDim::D3, wanted.backend_3d, d3),
        (PhysicsDim::D2, wanted.backend_2d, d2),
    ] {
        let first_look = seen.as_ref() != Some(&wanted);
        *seen = Some(wanted.clone());

        // No backend registered at all: nothing to select, but still worth the
        // warning below.
        let Some(backends) = world.get_resource::<PhysicsBackends>() else {
            break;
        };
        let target = match &wanted {
            Some(name) if backends.for_dim(dim).any(|b| b.name() == name) => name.clone(),
            Some(name) => {
                if first_look {
                    let msg = format!(
                        "project.toml asks for physics backend `{name}`, which is not installed; using the default"
                    );
                    warn!("[Physics] {msg}");
                    renzora::console_log::console_warn("Physics", msg);
                }
                match backends.default_for(dim) {
                    Some(b) => b.name().to_string(),
                    None => continue,
                }
            }
            None => match backends.default_for(dim) {
                Some(b) => b.name().to_string(),
                None => continue,
            },
        };
        if !backends.is_selected(&target, dim) {
            switch_backend(world, dim, &target);
        }
    }
    announce_running(world, announced);
}

/// One line saying which engine runs each world, logged once the project's
/// choice has been applied and again whenever it changes. The registration
/// lines at boot cannot say this: the project is read a few frames later.
///
/// The engine ships no physics engine of its own: avian and Rapier are both
/// plugins. So "none" is a real state, and an easy one to miss, because every
/// body and collider still loads and simply never moves. It is said as a warning.
fn announce_running(world: &World, announced: &mut Option<(String, String)>) {
    let backends = world.get_resource::<PhysicsBackends>();
    let name = |dim| {
        backends.and_then(|b| b.selected(dim)).map(|b| b.name().to_string()).unwrap_or_else(|| "none".into())
    };
    let now = (name(PhysicsDim::D3), name(PhysicsDim::D2));
    if announced.as_ref() == Some(&now) {
        return;
    }
    if backends.is_none_or(|b| b.iter().next().is_none()) {
        warn!("[Physics] no physics engine is installed; bodies and colliders will not simulate");
        renzora::console_log::console_warn(
            "Physics",
            "No physics engine is installed, so bodies and colliders will not move or collide. \
             Install a physics plugin (Avian Physics or Rapier Physics) from the marketplace.",
        );
    } else {
        info!("[Physics] running 3D on `{}`, 2D on `{}`", now.0, now.1);
        renzora::console_log::console_info("Physics", format!("3D physics: {}, 2D physics: {}", now.0, now.1));
    }
    *announced = Some(now);
}
