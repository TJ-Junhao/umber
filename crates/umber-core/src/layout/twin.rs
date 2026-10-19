//! The two y axes of a Cartesian panel share one plot and one top label band.

use std::borrow::Cow;

use super::{Layout, LayoutInput, PanelBands};
use crate::error::quoted;
use crate::model::Layer;
use crate::{Error, Result};

#[cfg(test)]
#[path = "twin_tests.rs"]
mod tests;

/// Bands with the two y axes' reserves combined before the grid chooses its column widths.
pub(super) fn aligned<'a>(
    inputs: &[LayoutInput<'_>],
    bands: &'a [PanelBands],
) -> Cow<'a, [PanelBands]> {
    if !inputs.iter().any(|input| match input.axes {
        Layer::Primary(_) => false,
        Layer::Twin { .. } => true,
    }) {
        return Cow::Borrowed(bands);
    }
    let mut bands = bands.to_vec();
    align(inputs, &mut bands);
    Cow::Owned(bands)
}

/// The parent's bottom legend and colorbar precede the twin's bottom colorbar.
/// Before column placement the measured legend height is used; afterwards it is rewrapped.
pub(super) fn bottom_offset(
    layer: Layer<'_>,
    inputs: &[LayoutInput<'_>],
    bands: &[PanelBands],
    span: Option<(f64, f64)>,
) -> f64 {
    match layer {
        Layer::Primary(_) => 0.0,
        Layer::Twin { parent, .. } => {
            let legend = span.map_or(bands[parent].bottom_legend, |(left, right)| {
                inputs[parent].legend.bottom_reserve(left, right)
            });
            legend + bands[parent].bottom_colorbar
        }
    }
}

/// Uses the final wrapped legend height when stacking the two axes' bottom colorbars.
pub(crate) fn align_placed(
    inputs: &[LayoutInput<'_>],
    bands: &mut [PanelBands],
    solution: &super::GridSolution,
) {
    align(inputs, bands);
    for (i, input) in inputs.iter().enumerate() {
        let header = solution.panels[i].header;
        bands[i].bottom_offset =
            bottom_offset(input.axes, inputs, bands, Some((header.left, header.right)));
    }
}

/// Shares axis bands and orders right ticks, direct labels, colorbars, then the right legend.
/// Direct labels keep their own y scale and share one horizontal column.
pub(crate) fn align(inputs: &[LayoutInput<'_>], bands: &mut [PanelBands]) {
    for (i, input) in inputs.iter().enumerate() {
        let parent = match input.axes {
            Layer::Primary(_) => continue,
            Layer::Twin { parent, .. } => parent,
        };
        let bottom = bottom_offset(input.axes, inputs, bands, None);
        let (before, after) = bands.split_at_mut(i);
        let (left, right) = (&mut before[parent], &mut after[0]);
        let direct = if left.right_colorbar > 0.0 || right.right_colorbar > 0.0 {
            inputs[parent]
                .legend
                .direct
                .reserve(f64::INFINITY)
                .max(input.legend.direct.reserve(f64::INFINITY))
        } else {
            left.direct_reserve.max(right.direct_reserve)
        };
        left.direct_reserve = direct;
        right.direct_reserve = direct;
        left.direct_offset = right.right_axis;
        right.direct_offset = right.right_axis;
        left.right_offset = right.right_axis;
        left.right_legend_offset = if left.right_legend > 0.0 {
            right.right_colorbar
        } else {
            0.0
        };
        right.right_offset = right.right_axis + left.right_colorbar;
        right.bottom_offset = bottom;
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

/// Both y titles must fit on their shared band, including a gap between them.
pub(super) fn check_titles(
    left: &Layout,
    right: &Layout,
    sizes: [crate::SizeSpec; 2],
    context: &'static str,
) -> Result<()> {
    if let (Some(left), Some(right)) = (&left.ylabel, &right.ylabel) {
        let width = left.width() + super::AXIS_LABEL_GAP + right.width();
        let room = right.x + right.width() - left.x;
        if width > room + super::inset::CONTAINMENT_TOLERANCE {
            let (width, room) = super::widths(width, room);
            let size = sizes
                .into_iter()
                .find(|size| matches!(size, crate::SizeSpec::Points(_)))
                .unwrap_or(crate::SizeSpec::Auto);
            return Err(Error::style(context, format!(
                "the left y axis label {} and right y axis label {} are {width} pt wide with a gap; the panel has {room} pt.",
                quoted(&left.text), quoted(&right.text),
            )).with_hint(super::panel::text_size_hint(size)));
        }
    }
    Ok(())
}
