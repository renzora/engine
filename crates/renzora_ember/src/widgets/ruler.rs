//! Ruler tick spacing and label formatting.
//!
//! Pure arithmetic, shared by the 2D viewport's world-space rulers and the UI
//! editor's design-space ones. The two draw very differently — one into window
//! coordinates from a camera projection, the other into a panel from a pan/zoom
//! — but "where do the ticks go and what do they say" is the same question, and
//! it is the part that is fiddly to get right.

/// Round a raw spacing to the nearest 1-2-5×10ⁿ value, so ticks land on
/// human-readable coordinates at any zoom.
pub fn nice_step(raw: f32) -> f32 {
    if raw <= 0.0 || raw.is_nan() {
        return 1.0;
    }
    let pow = 10f32.powf(raw.log10().floor());
    let frac = raw / pow;
    let mult = if frac < 1.5 {
        1.0
    } else if frac < 3.0 {
        2.0
    } else if frac < 7.0 {
        5.0
    } else {
        10.0
    };
    mult * pow
}

/// Tick step: the 1-2-5 spacing normally, but a power-of-two multiple (or
/// subdivision) of `grid_size` when a grid is showing — so every labelled tick
/// lands exactly on a grid line and the ruler never reads as misaligned with it.
///
/// Subdivisions still align: grid lines sit on multiples of the grid size, which
/// are multiples of any `size / 2ᵏ` step.
pub fn ruler_step(raw: f32, grid_size: Option<f32>) -> f32 {
    let Some(grid) = grid_size.filter(|g| *g > 0.0) else {
        return nice_step(raw);
    };
    if raw <= 0.0 || raw.is_nan() {
        return grid;
    }
    let level = (raw / grid).log2().ceil().clamp(-6.0, 32.0);
    grid * 2f32.powf(level)
}

/// Format a coordinate for a ruler label: whole numbers when the step is ≥ 1,
/// otherwise just enough decimals to tell adjacent ticks apart.
pub fn fmt_coord(v: f32, step: f32) -> String {
    if step >= 1.0 {
        format!("{}", v.round() as i64)
    } else if step >= 0.1 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_step_lands_on_1_2_5() {
        assert_eq!(nice_step(1.0), 1.0);
        assert_eq!(nice_step(1.4), 1.0);
        assert_eq!(nice_step(2.9), 2.0);
        assert_eq!(nice_step(6.0), 5.0);
        assert_eq!(nice_step(9.0), 10.0);
        assert_eq!(nice_step(140.0), 100.0);
        assert_eq!(nice_step(0.4), 0.5);
    }

    /// Degenerate inputs come from a zoom of zero or a NaN pan; a ruler that
    /// panics or emits an infinite tick loop is worse than one that is coarse.
    #[test]
    fn nice_step_survives_nonsense() {
        assert_eq!(nice_step(0.0), 1.0);
        assert_eq!(nice_step(-5.0), 1.0);
        assert_eq!(nice_step(f32::NAN), 1.0);
    }

    /// With a grid, every step must divide or multiply the grid by a power of
    /// two — that is what keeps labelled ticks on grid lines.
    #[test]
    fn ruler_step_stays_on_the_grid() {
        for raw in [3.0, 9.0, 40.0, 700.0] {
            let step = ruler_step(raw, Some(10.0));
            let ratio = step / 10.0;
            assert!(
                (ratio.log2().fract()).abs() < 1e-4,
                "step {step} is not a power-of-two multiple of the grid"
            );
            assert!(step >= raw, "step {step} is finer than the requested {raw}");
        }
    }

    #[test]
    fn ruler_step_without_a_grid_is_just_nice_step() {
        assert_eq!(ruler_step(6.0, None), 5.0);
        assert_eq!(ruler_step(6.0, Some(0.0)), 5.0);
    }

    #[test]
    fn coords_get_decimals_only_when_the_step_needs_them() {
        assert_eq!(fmt_coord(12.4, 1.0), "12");
        assert_eq!(fmt_coord(12.44, 0.5), "12.4");
        assert_eq!(fmt_coord(12.444, 0.05), "12.44");
        assert_eq!(fmt_coord(-0.4, 1.0), "0");
    }
}
