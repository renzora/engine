//! A quick confetti burst over the whole window — a little celebration for wins
//! like credits landing after a purchase. Fire it with [`Confetti::fire`]; the
//! pieces self-clean and never capture pointer input (`FocusPolicy::Pass`, and
//! unparented so they render at the UI root above every panel/overlay).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::theme::rgba;

/// Confetti trigger + a tiny per-burst PRNG seed.
#[derive(Resource, Default)]
pub(crate) struct Confetti {
    fire: bool,
    seed: u32,
}

impl Confetti {
    /// Request a burst on the next frame.
    pub(crate) fn fire(&mut self) {
        self.fire = true;
    }
}

#[derive(Component)]
pub(crate) struct ConfettiPiece {
    vx: f32,
    vy: f32,
    life: f32,
    color: [u8; 3],
}

const COUNT: usize = 80;
const COLORS: [[u8; 3]; 6] = [
    [240, 120, 100], // coral
    [91, 156, 245],  // blue
    [82, 196, 120],  // green
    [235, 180, 80],  // amber
    [167, 130, 245], // violet
    [70, 190, 190],  // teal
];

/// Spawn a shower of pieces from just above the top edge when a burst is armed.
pub(crate) fn spawn(mut commands: Commands, mut confetti: ResMut<Confetti>, windows: Query<&Window>) {
    if !confetti.fire {
        return;
    }
    confetti.fire = false;
    confetti.seed = confetti.seed.wrapping_add(0x9E37_79B9);
    // First window is fine — confetti placement is approximate, and single()
    // would panic once multi-monitor dock windows exist.
    let width = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0);

    // xorshift PRNG seeded per burst — no `rand` dependency needed.
    let mut state = confetti.seed | 1;
    let mut rand = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state as f32) / (u32::MAX as f32)
    };

    for i in 0..COUNT {
        let x = rand() * width;
        let size = 5.0 + rand() * 6.0;
        let color = COLORS[i % COLORS.len()];
        let vx = (rand() - 0.5) * 320.0;
        let vy = 40.0 + rand() * 120.0;
        let life = 1.6 + rand() * 1.4;
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(-20.0 - rand() * 120.0),
                width: Val::Px(size),
                height: Val::Px(size * 1.7),
                border_radius: BorderRadius::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(rgba([color[0], color[1], color[2], 255])),
            GlobalZIndex(10_000),
            FocusPolicy::Pass,
            ConfettiPiece { vx, vy, life, color },
            Name::new("confetti"),
        ));
    }
}

/// Fall under gravity with horizontal drift, fade out, and despawn off-screen.
pub(crate) fn animate(
    mut commands: Commands,
    time: Res<Time>,
    windows: Query<&Window>,
    mut pieces: Query<(Entity, &mut Node, &mut ConfettiPiece, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();
    let height = windows.iter().next().map(|w| w.height()).unwrap_or(720.0);
    for (e, mut node, mut p, mut bg) in &mut pieces {
        p.vy += 640.0 * dt; // gravity
        p.life -= dt;
        if let Val::Px(x) = node.left {
            node.left = Val::Px(x + p.vx * dt);
        }
        let mut off = false;
        if let Val::Px(y) = node.top {
            let ny = y + p.vy * dt;
            node.top = Val::Px(ny);
            off = ny > height + 60.0;
        }
        // Fade over the last half-second of life.
        let alpha = (p.life / 0.5).clamp(0.0, 1.0);
        bg.0 = rgba([p.color[0], p.color[1], p.color[2], (alpha * 255.0) as u8]);
        if p.life <= 0.0 || off {
            commands.entity(e).try_despawn();
        }
    }
}
