//! Interaction transitions — make `hover:` / `pressed:` style overrides work,
//! with an optional smooth tween (`delay` / `duration`).
//!
//! The loader collects an element's base + `hover:` + `pressed:` background and
//! border colors into [`Interactive`]. Each frame, [`apply_interactive`] picks
//! the target color for the current `Interaction` state and either snaps to it
//! (instant) or eases toward it (when a duration is set).
//!
//! Today this covers `background` and `border_color` — the common state-styling
//! case (buttons, list rows, toggles). Other animatable props can be added the
//! same way.

use bevy::prelude::*;

/// State-dependent colors for an interactive element. Missing states fall back:
/// pressed → hover → base, hover → base.
#[derive(Component, Default)]
pub struct Interactive {
    pub base_bg: Option<Color>,
    pub hover_bg: Option<Color>,
    pub pressed_bg: Option<Color>,
    pub base_border: Option<Color>,
    pub hover_border: Option<Color>,
    pub pressed_border: Option<Color>,
    /// Tween time in seconds; 0 = snap instantly.
    pub duration: f32,
}

fn bg_target(i: &Interactive, s: Interaction) -> Option<Color> {
    match s {
        Interaction::Pressed => i.pressed_bg.or(i.hover_bg).or(i.base_bg),
        Interaction::Hovered => i.hover_bg.or(i.base_bg),
        Interaction::None => i.base_bg,
    }
}

fn border_target(i: &Interactive, s: Interaction) -> Option<Color> {
    match s {
        Interaction::Pressed => i.pressed_border.or(i.hover_border).or(i.base_border),
        Interaction::Hovered => i.hover_border.or(i.base_border),
        Interaction::None => i.base_border,
    }
}

/// Frame-rate-independent ease toward `b`. `t` is the per-frame fraction.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_linear();
    let b = b.to_linear();
    LinearRgba::new(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
    .into()
}

fn apply_interactive(
    time: Res<Time>,
    mut q: Query<(
        &Interaction,
        &Interactive,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
    )>,
) {
    let dt = time.delta_secs();
    for (interaction, it, bg, border) in &mut q {
        // Per-frame lerp factor: snap when no duration, else approach the
        // target (exponential ease-out — smooth and stable across frame rates).
        let factor = if it.duration <= 0.0 {
            1.0
        } else {
            (dt / it.duration).clamp(0.0, 1.0)
        };

        if let Some(mut bg) = bg {
            if let Some(target) = bg_target(it, *interaction) {
                bg.0 = lerp_color(bg.0, target, factor);
            }
        }
        if let Some(mut border) = border {
            if let Some(target) = border_target(it, *interaction) {
                let c = lerp_color(border.top, target, factor);
                *border = BorderColor::all(c);
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, apply_interactive);
}
