//! A procedural maze by recursive-backtracker carving, built as walls. The
//! algorithm needs a grid of cell state the engine has no concept of, so the
//! grid lives in the script: passages are carved first, then the final wall
//! set is spawned once (spawning during carving would thrash the scene).
//!
//! Two changes from the Lua original: walls are scaled to span their cell
//! instead of being unit cubes with gaps between them, and the east and south
//! outer edges get walls too (the original left them open).
//!
//! Setup: attach to a manager entity; tune `W`, `H` and `CELL`. Walls are named
//! "maze_<n>". The layout is random per run; send a seed over an RPC if peers
//! must match.

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::{ScriptCtx, ScriptHook};

const W: usize = 8;
const H: usize = 8;
const CELL: f32 = 2.0;
const WALL_HEIGHT: f32 = 2.0;
const WALL_THICKNESS: f32 = 0.2;

const N: usize = 0;
const E: usize = 1;
const S: usize = 2;
const WEST: usize = 3;
/// Neighbour offset, the wall on this side, the matching wall on theirs.
const DIRS: [(i32, i32, usize, usize); 4] = [(0, -1, N, S), (1, 0, E, WEST), (0, 1, S, N), (-1, 0, WEST, E)];

struct Maze {
    /// Per cell: north, east, south, west walls present.
    walls: Vec<[bool; 4]>,
    visited: Vec<bool>,
    rng: u64,
}

impl Maze {
    fn carve(&mut self, x: usize, y: usize) {
        self.visited[y * W + x] = true;
        let mut dirs = DIRS;
        // Fisher-Yates, for an unbiased maze.
        for i in (1..dirs.len()).rev() {
            let j = (next_u64(&mut self.rng) % (i as u64 + 1)) as usize;
            dirs.swap(i, j);
        }
        for (dx, dy, mine, theirs) in dirs {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if self.visited[ny * W + nx] {
                continue;
            }
            self.walls[y * W + x][mine] = false;
            self.walls[ny * W + nx][theirs] = false;
            self.carve(nx, ny);
        }
    }
}

fn update(ctx: &mut ScriptCtx) {
    let _ = ctx;
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let rng = ctx.entity().to_bits() ^ u64::from(ctx.elapsed().to_bits());
    let mut maze = Maze { walls: vec![[true; 4]; W * H], visited: vec![false; W * H], rng };
    maze.carve(0, 0);

    let across = Vec3::new(CELL, WALL_HEIGHT, WALL_THICKNESS);
    let along = Vec3::new(WALL_THICKNESS, WALL_HEIGHT, CELL);
    let mut segments = Vec::new();
    for y in 0..H {
        for x in 0..W {
            let (cx, cz) = ((x + 1) as f32 * CELL, (y + 1) as f32 * CELL);
            let w = maze.walls[y * W + x];
            let half = CELL / 2.0;
            if w[N] {
                segments.push((Vec3::new(cx, 1.0, cz - half), across));
            }
            if w[WEST] {
                segments.push((Vec3::new(cx - half, 1.0, cz), along));
            }
            if x == W - 1 && w[E] {
                segments.push((Vec3::new(cx + half, 1.0, cz), along));
            }
            if y == H - 1 && w[S] {
                segments.push((Vec3::new(cx, 1.0, cz + half), across));
            }
        }
    }

    let world = ctx.world();
    for (i, (pos, scale)) in segments.iter().enumerate() {
        world.spawn((
            Name::new(format!("maze_{}", i + 1)),
            Transform::from_translation(*pos).with_scale(*scale),
            Visibility::default(),
            MeshPrimitive("cube".into()),
            MeshColor(Color::srgb(0.6, 0.6, 0.7)),
        ));
    }
    renzora::clog_info!("Script", "[maze] spawned {} wall segments", segments.len());
}

/// The next value of a 64-bit LCG, high bits first; there is no rand crate.
fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state >> 33
}

renzora::script!(update, hooks = hooks);
