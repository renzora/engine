//! Utility functions for inspector widgets

/// Sanitize a float value to prevent egui smart_aim panics.
/// Returns a finite value clamped to the given range, with a fallback default.
///
/// This should be called before passing values to `DragValue` widgets to ensure
/// values loaded from files or modified during editing don't cause panics.
pub fn sanitize_f32(value: &mut f32, min: f32, max: f32, default: f32) {
    if !value.is_finite() {
        *value = default;
    }
    *value = value.clamp(min, max);
}
