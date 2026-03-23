//! Numeric utilities.

/// Sanitize a float to prevent egui `DragValue` panics on NaN/infinity.
///
/// If the value is not finite, it is replaced with `default`, then clamped to `[min, max]`.
pub fn sanitize_f32(value: &mut f32, min: f32, max: f32, default: f32) {
    if !value.is_finite() {
        *value = default;
    }
    *value = value.clamp(min, max);
}
