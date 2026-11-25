//! An L-system turtle: expand a grammar to a string, then walk it placing
//! cubes. Fractals are defined by string rewriting, so the production rules
//! are iterated here and a turtle stack (`[` and `]`) handles branching. Each
//! forward step drops a small primitive, building a plant-like structure.
//!
//! No randomness is involved, so the plant is identical on every peer.
//!
//! Setup: attach to a manager entity. Growth is exponential in `ITERS`; the
//! segments are named `frac_<n>`.

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::{ScriptCtx, ScriptHook};

const AXIOM: &str = "F";
const ITERS: usize = 3;
/// Turn per `+` / `-`, in degrees.
const ANGLE: f32 = 25.0;
/// Length of one forward step.
const STEP: f32 = 0.5;

/// The production for one symbol; symbols without a rule copy through.
fn rule(c: char) -> Option<&'static str> {
    match c {
        'F' => Some("FF+[+F-F-F]-[-F+F+F]"),
        _ => None,
    }
}

fn expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 8);
    for c in s.chars() {
        match rule(c) {
            Some(r) => out.push_str(r),
            None => out.push(c),
        }
    }
    out
}

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let mut s = AXIOM.to_string();
    for _ in 0..ITERS {
        s = expand(&s);
    }

    // The turtle walks the XY plane; heading is in degrees, 90 is straight up.
    let (mut x, mut y, mut heading) = (0.0f32, 0.0f32, 90.0f32);
    let mut stack: Vec<(f32, f32, f32)> = Vec::new();
    let mut n = 0;
    let world = ctx.world();
    for c in s.chars() {
        match c {
            'F' => {
                let rad = heading.to_radians();
                let nx = x + rad.cos() * STEP;
                let ny = y + rad.sin() * STEP;
                n += 1;
                world.spawn((
                    Name::new(format!("frac_{n}")),
                    Transform::from_xyz(nx, ny + 1.0, 0.0),
                    Visibility::default(),
                    MeshPrimitive("cube".into()),
                    MeshColor(Color::srgb(0.3, 0.8, 0.3)),
                ));
                x = nx;
                y = ny;
            }
            '+' => heading += ANGLE,
            '-' => heading -= ANGLE,
            '[' => stack.push((x, y, heading)),
            ']' => {
                if let Some(saved) = stack.pop() {
                    (x, y, heading) = saved;
                }
            }
            _ => {}
        }
    }
    renzora::clog_info!("Script", "[lsys] placed {} segments from {} symbols", n, s.len());
}

renzora::script!(update, hooks = hooks);
