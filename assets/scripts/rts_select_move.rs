//! An RTS unit director: spawn a squad, select it with a key, issue move
//! orders. A "selected set" plus a destination is not one component the engine
//! has, so the director owns the squad and sends `nav_set_destination` to each
//! unit with a formation offset, spreading them around the target instead of
//! stacking them on one point.
//!
//! There is no cursor-to-world query here, so a click regroups the squad around
//! the director's own position, as in the Lua version. Orders are local; they
//! could be broadcast over RPC for shared command.
//!
//! Setup: attach to a manager on a navmesh scene. The squad is spawned as
//! `unit_1` .. `unit_6`; a unit only walks once it has a nav agent (add one to
//! the spawned capsules, or swap the spawn for a prefab that carries it).
//! A: select the whole squad. Left click: move. Right click: hold position.

use bevy::ecs::system::SystemState;
use bevy::gizmos::config::GizmoConfigStore;
use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const SQUAD: usize = 6;

#[derive(Component)]
struct Squad {
    units: Vec<Entity>,
    selected: Vec<bool>,
}

/// A 3-wide grid, so units do not all path to one point.
fn formation_offset(i: usize) -> Vec2 {
    let col = (i % 3) as f32;
    let row = (i / 3) as f32;
    Vec2::new(col * 1.2 - 1.2, row * 1.2)
}

fn update(ctx: &mut ScriptCtx) {
    let select_all = ctx.get_resource::<ScriptInput>().is_some_and(|i| i.is_key_just_pressed(KeyCode::KeyA));
    let (click, held, clear) = match ctx.get_resource::<ButtonInput<MouseButton>>() {
        Some(mouse) => (
            mouse.just_pressed(MouseButton::Left),
            mouse.pressed(MouseButton::Left),
            mouse.just_pressed(MouseButton::Right),
        ),
        None => (false, false, false),
    };
    let Some(target) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let Some(mut squad) = ctx.get_mut::<Squad>() else { return };
    if select_all {
        squad.selected.iter_mut().for_each(|s| *s = true);
    }
    let orders: Vec<(usize, Entity)> = squad
        .units
        .iter()
        .enumerate()
        .filter(|(i, _)| squad.selected[*i])
        .map(|(i, e)| (i, *e))
        .collect();
    let units = squad.units.clone();
    drop(squad);

    // Ordered on the press rather than every held frame, so a long click does
    // not re-path the squad sixty times a second.
    if click {
        for (i, unit) in orders {
            let off = formation_offset(i);
            ScriptAction::new("nav_set_destination", unit)
                .arg("target", Vec3::new(target.x + off.x, 0.0, target.z + off.y))
                .send(ctx.world());
        }
    }
    if held {
        draw_line(ctx.world(), target + Vec3::new(-0.5, 0.1, 0.0), target + Vec3::new(0.5, 0.1, 0.0));
    }
    // Lua cleared its own destination once per unit; each unit is what holds.
    if clear {
        for unit in units {
            ScriptAction::new("nav_clear_destination", unit).send(ctx.world());
        }
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let world = ctx.world();
    let units = (1..=SQUAD)
        .map(|i| {
            world
                .spawn((
                    Name::new(format!("unit_{i}")),
                    Transform::from_xyz(i as f32 * 1.5 - 4.0, 1.0, 0.0),
                    Visibility::default(),
                    MeshPrimitive("capsule".into()),
                    MeshColor(Color::srgb(0.3, 0.7, 1.0)),
                ))
                .id()
        })
        .collect();
    ctx.insert(Squad { units, selected: vec![true; SQUAD] });
}

/// A one-frame debug line marking the order target.
fn draw_line(world: &mut World, a: Vec3, b: Vec3) {
    // Gizmos are absent in a build without the gizmo plugin; drawing is optional.
    if !world.contains_resource::<GizmoConfigStore>() {
        return;
    }
    let mut state = SystemState::<Gizmos>::new(world);
    if let Ok(mut gizmos) = state.get_mut(world) {
        gizmos.line(a, b, Color::WHITE);
    }
    state.apply(world);
}

renzora::script!(update, hooks = hooks);
