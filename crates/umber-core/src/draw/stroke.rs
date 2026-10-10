//! Stroke widths as drawn, shared by the chrome and the lines.

/// The widest stroke drawn, whatever the theme says.
const MAX_STROKE_WIDTH: f32 = 1.0e4;

/// A stroke width that is finite and within `0..=MAX_STROKE_WIDTH`.
pub(super) fn stroke_width(width: f32) -> f32 {
    if width.is_finite() && width > 0.0 {
        width.min(MAX_STROKE_WIDTH)
    } else {
        0.0
    }
}
