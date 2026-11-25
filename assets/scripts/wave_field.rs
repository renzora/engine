//! An animated cube field: a grid of primitives driven by a travelling sine
//! wave. A single mesh cannot show per-cell motion, so the lattice is spawned
//! once and each frame only touches the cheap parts of every cell's Transform
//! (height, plus a Y stretch that makes crests read clearly). Phase is a
//! function of radial distance from the centre, which gives a ripple feel.
//!
//! The wave is a pure function of elapsed time, so every peer sees the same
//! field without any networking.
//!
//! Setup: attach to a manager entity. `GRID` sets the resolution (`GRID * GRID`
//! cells, named `wf_<i>_<j>`).

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::{ScriptCtx, ScriptHook};

/// Cells per side.
const GRID: i32 = 12;
const SPACING: f32 = 1.0;
/// Peak height of the wave.
const AMP: f32 = 1.5;
/// Cell size at rest; crests stretch it in Y up to 1.0.
const CELL_SCALE: f32 = 0.4;

#[derive(Component)]
struct WaveField {
    /// Each cell and its fixed XZ position.
    cells: Vec<(Entity, Vec2)>,
}

fn update(ctx: &mut ScriptCtx) {
    let t = ctx.elapsed() * 2.0;
    let Some(field) = ctx.get::<WaveField>() else { return };
    let cells = field.cells.clone();
    for (cell, xz) in cells {
        let h = (xz.length() * 0.8 - t).sin() * AMP;
        let crest = ((h + AMP) / (2.0 * AMP)).clamp(0.0, 1.0);
        if let Some(mut transform) = ctx.get_mut_on::<Transform>(cell) {
            transform.translation.y = h;
            transform.scale.y = CELL_SCALE + crest * 0.6;
        }
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let world = ctx.world();
    let mut cells = Vec::new();
    for i in 1..=GRID {
        for j in 1..=GRID {
            let x = (i - GRID / 2) as f32 * SPACING;
            let z = (j - GRID / 2) as f32 * SPACING;
            // The Lua version scaled the manager instead of the cells; the
            // cells are what was meant to be small.
            let cell = world
                .spawn((
                    Name::new(format!("wf_{i}_{j}")),
                    Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(CELL_SCALE)),
                    Visibility::default(),
                    MeshPrimitive("cube".into()),
                    MeshColor(Color::srgb(0.2, 0.4, 0.9)),
                ))
                .id();
            cells.push((cell, Vec2::new(x, z)));
        }
    }
    ctx.insert(WaveField { cells });
}

renzora::script!(update, hooks = hooks);
