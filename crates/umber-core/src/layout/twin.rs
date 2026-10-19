//! The two y axes of a Cartesian panel share one plot and one top label band.

use std::borrow::Cow;

use super::{LayoutInput, PanelBands};
use crate::Axes;

#[cfg(test)]
#[path = "twin_tests.rs"]
mod tests;

/// Bands with the two y axes' reserves combined before the grid chooses its column widths.
pub(super) fn aligned<'a>(
    inputs: &[LayoutInput<'_>],
    bands: &'a [PanelBands],
) -> Cow<'a, [PanelBands]> {
    if !inputs
        .iter()
        .any(|input| input.axes.cartesian().is_some_and(Axes::is_twin))
    {
        return Cow::Borrowed(bands);
    }
    let mut bands = bands.to_vec();
    align(inputs, &mut bands);
    Cow::Owned(bands)
}

/// Uses the final wrapped legend height when stacking the two axes' bottom colorbars.
pub(crate) fn align_placed(
    inputs: &[LayoutInput<'_>],
    bands: &mut [PanelBands],
    solution: &super::GridSolution,
) {
    align(inputs, bands);
    for i in 1..inputs.len() {
        if inputs[i].axes.cartesian().is_some_and(Axes::is_twin) {
            let header = solution.panels[i - 1].header;
            bands[i].bottom_offset = inputs[i - 1]
                .legend
                .bottom_reserve(header.left, header.right)
                + bands[i - 1].bottom_colorbar;
        }
    }
}

/// Shares top and bottom axis bands and places right-hand decorations outside the right ticks.
/// Direct labels keep their own y scale; each axes gets a separate horizontal column.
pub(crate) fn align(inputs: &[LayoutInput<'_>], bands: &mut [PanelBands]) {
    for i in 1..inputs.len() {
        if !inputs[i].axes.cartesian().is_some_and(Axes::is_twin) {
            continue;
        }
        let (before, after) = bands.split_at_mut(i);
        let (left, right) = (&mut before[i - 1], &mut after[0]);
        left.right_offset = right.right_axis;
        right.right_offset = right.right_axis + left.beside_axis();
        right.bottom_offset = left.bottom_legend + left.bottom_colorbar;
        right.x_band = left.x_band;
        right.x_title_band = left.x_title_band;
        let rise = left.top_label_rise().max(right.top_label_rise());
        let gap = left.y_label_gap.max(right.y_label_gap);
        let band = (left.top_label_band() + gap - left.y_label_gap)
            .max(right.top_label_band() + gap - right.y_label_gap);
        let drop = left.label_drop.max(right.label_drop);
        for bands in [left, right] {
            bands.label_rise = rise;
            bands.y_label_band = band;
            bands.y_label_gap = gap;
            bands.label_drop = drop;
        }
    }
}
