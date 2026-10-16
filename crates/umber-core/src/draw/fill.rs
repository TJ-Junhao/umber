//! The opacities of translucent fills, shared by the marks that draw them and by their legend
//! swatches, which show each fill as drawn.

use crate::Theme;

/// The opacity of area-like fills (areas, stacked areas, bands, spans, boxes and violins), as a
/// share of the series color's own: all over a solid fill, and at the colored end of an area's
/// gradient, which ramps down to 0 at the baseline.
pub(crate) const AREA_ALPHA: f32 = 0.32;

/// The opacity of a histogram gradient at the top of the tallest bin, as a share of the series
/// color's own.
pub(crate) const HISTOGRAM_TOP_ALPHA: f32 = 0.95;

/// The opacity of scatter, stem and bubble marker fills, as a share of the series color's own:
/// the theme's marker alpha, clamped to `0..=1`, or 1 when it is not finite.
pub(crate) fn marker_alpha(theme: &Theme) -> f32 {
    let alpha = theme.marker_alpha();
    if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    }
}
