//! Spinner: rotates the border opening to simulate a loading spinner.
//!
//! Since bevy_ui doesn't support CSS-style rotation on nodes directly,
//! we animate the border color alpha to create a pulsing effect,
//! and rotate which border side is transparent.

use bevy::prelude::*;

use crate::components::SpinnerData;

pub fn spinner_system(
    time: Res<Time>,
    mut spinners: Query<(&SpinnerData, &mut BorderColor)>,
) {
    let t = time.elapsed_secs();

    for (data, mut border) in &mut spinners {
        // Cycle through which side is "open" based on time
        let phase = (t * data.speed) % 4.0;
        let srgba = data.color.to_srgba();
        let solid = Color::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha);
        let transparent = Color::srgba(srgba.red, srgba.green, srgba.blue, 0.15);

        // Rotate which side is transparent
        let (top, right, bottom, left) = match phase as u32 {
            0 => (solid, solid, solid, transparent),
            1 => (transparent, solid, solid, solid),
            2 => (solid, transparent, solid, solid),
            _ => (solid, solid, transparent, solid),
        };

        *border = BorderColor {
            top,
            right,
            bottom,
            left,
        };
    }
}
