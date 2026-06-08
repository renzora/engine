//! The confetti burst that celebrates each completed step.
//!
//! ember has no particle primitive (the `cinder` particle-UI is "migrating in
//! next"), so this is hand-built from animated `bevy_ui` nodes — the same
//! read-`Time`-write-`Node`/`BackgroundColor` pattern ember's own `spinner` /
//! `vu_meter` widgets use. The layer is click-through (`FocusPolicy::Pass`,
//! never an `OverlaySurface`) so it can never swallow a viewport click the user
//! is mid-task on.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use renzora::core::{EditorLocked, HideInHierarchy};
use renzora_ember::theme::{accent, play_green, rgb, warn_amber};

/// Full-window, pointer-transparent layer that hosts the live pieces.
#[derive(Component)]
pub struct ConfettiRoot;

/// One falling piece.
#[derive(Component)]
pub struct Confetti {
    vel: Vec2,
    pos: Vec2,
    age: f32,
    ttl: f32,
    color: Color,
}

/// Downward acceleration (logical px/s²) — tuned for a snappy ~1s arc.
const GRAVITY: f32 = 1500.0;

/// Spawn the persistent confetti layer (stacked above the tutorial card).
pub fn spawn_root(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            // Critically pointer-transparent: the user is still clicking the
            // viewport underneath while pieces rain down.
            FocusPolicy::Pass,
            GlobalZIndex(8200),
            HideInHierarchy,
            EditorLocked,
            ConfettiRoot,
            Name::new("tutorial-confetti"),
        ))
        .id()
}

/// Fire `count` pieces fanning upward from `origin` (logical px, top-left).
pub fn burst(commands: &mut Commands, root: Entity, origin: Vec2, count: usize, seed: u32) {
    let palette = [accent(), play_green(), warn_amber(), (255, 255, 255)];
    for i in 0..count {
        let mut rng = Xorshift::new(seed ^ (i as u32).wrapping_mul(2654435761));
        // Fan upward: -90° ± ~63°.
        let angle = -std::f32::consts::FRAC_PI_2 + rng.range(-1.1, 1.1);
        let speed = rng.range(280.0, 640.0);
        let vel = Vec2::new(angle.cos() * speed, angle.sin() * speed);
        let size = rng.range(6.0, 11.0);
        let color = rgb(palette[i % palette.len()]);
        let piece = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(origin.x),
                    top: Val::Px(origin.y),
                    width: Val::Px(size),
                    height: Val::Px(size),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(color),
                FocusPolicy::Pass,
                Confetti {
                    vel,
                    pos: origin,
                    age: 0.0,
                    ttl: rng.range(0.9, 1.5),
                    color,
                },
            ))
            .id();
        commands.entity(root).add_child(piece);
    }
}

/// Integrate + fade every live piece; despawn the expired ones.
pub fn tick(
    time: Res<Time>,
    mut commands: Commands,
    mut pieces: Query<(Entity, &mut Confetti, &mut Node, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();
    for (e, mut c, mut node, mut bg) in &mut pieces {
        c.age += dt;
        if c.age >= c.ttl {
            commands.entity(e).despawn();
            continue;
        }
        let vy = c.vel.y + GRAVITY * c.age; // gains downward speed over time
        let dx = c.vel.x * dt;
        let dy = vy * dt;
        c.pos.x += dx;
        c.pos.y += dy;
        node.left = Val::Px(c.pos.x);
        node.top = Val::Px(c.pos.y);
        let frac = 1.0 - (c.age / c.ttl);
        bg.0 = c.color.with_alpha(frac.clamp(0.0, 1.0));
    }
}

/// Tiny deterministic RNG so confetti gets per-piece jitter without pulling in a
/// `rand` dependency.
struct Xorshift(u32);
impl Xorshift {
    fn new(seed: u32) -> Self {
        Xorshift(seed | 1)
    }
    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}
