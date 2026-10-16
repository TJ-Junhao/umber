//! Stacked areas: flat fills between cumulative curves and solid upper outlines. Every layer
//! uses the union of the points that preserve each curve's envelope in visible device columns.

use super::MarkContext;
use super::band::band_fill_items;
use super::line::{downsample_columns, downsample_range, line_items, line_style};
use super::mapping::{axis_numbers, axis_value};
use crate::downsample::clipped_m4_indices;
use crate::model::{SeriesView, StackedAreaMark};
use crate::resolve::Resolved;
use crate::scene::Item;
use crate::stacking::AreaStacks;
use crate::{Dash, Rgba};

/// Draws each layer from its lower cumulative curve to its upper one, with a flat area fill
/// then a line along the upper curve. The first lower curve is the resolved baseline; on log
/// y axes it is the lower limit, not zero. All curves break at the same indices.
pub(super) fn stacked_area_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &StackedAreaMark,
    series: &[(SeriesView<'_>, Rgba)],
    stacks: &AreaStacks,
    width: f32,
    items: &mut Vec<Item>,
) {
    let mut xs: Vec<_> = mark
        .x()
        .iter()
        .map(|&v| axis_value(&resolved.x, v))
        .collect();
    let curves: Vec<_> = stacks
        .curves()
        .iter()
        .map(|curve| axis_numbers(&resolved.y, curve))
        .collect();
    if let Some(first) = curves.first() {
        for (x, y) in xs.iter_mut().zip(first) {
            if y.is_nan() {
                *x = f64::NAN;
            }
        }
    }
    let columns = downsample_columns(context.plot.width);
    let domain = (context.maps.x.lo, context.maps.x.hi);
    let (xs, curves) = kept_points(xs, curves, domain, columns, downsample_range(context));
    let baseline = context.maps.y.at(context.baseline.pos);
    let baseline: Vec<_> = xs
        .iter()
        .map(|x| if x.is_nan() { f64::NAN } else { baseline })
        .collect();
    let mut lower: &[f64] = &baseline;
    for (upper, &(_, color)) in curves.iter().zip(series) {
        band_fill_items(context, &xs, lower, upper, color, items);
        let style = line_style(width, color, Dash::Solid);
        line_items(context, &xs, upper, &style, items);
        lower = upper;
    }
}

/// Reduces all curves at the same indices: the union of their visible M4 downsampling samples.
/// The nearest outside neighbours are kept separately, so explicit-limit crossings remain exact and
/// out-of-view extrema cannot displace visible samples. Short inputs are unchanged.
fn kept_points(
    xs: Vec<f64>,
    curves: Vec<Vec<f64>>,
    domain: (f64, f64),
    columns: usize,
    clip_range: (f64, f64),
) -> (Vec<f64>, Vec<Vec<f64>>) {
    if xs.len() <= 4 * columns {
        return (xs, curves);
    }
    let mut kept: Vec<_> = curves
        .iter()
        .flat_map(|curve| clipped_m4_indices(&xs, curve, domain, columns, clip_range))
        .collect();
    kept.sort_unstable();
    kept.dedup();
    let pick = |v: &[f64]| kept.iter().map(|&i| v[i]).collect();
    (pick(&xs), curves.iter().map(|curve| pick(curve)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cumulative_curve_contributes_its_extrema_and_limit_neighbours() {
        let xs: Vec<_> = (0..10_000).map(|i| f64::from(i) / 100.0).collect();
        let mut a = vec![1.0; xs.len()];
        let mut b = vec![10.0; xs.len()];
        a[5_001] = 9.0;
        b[5_002] = 20.0;
        a[4_999] = 100.0;
        b[6_001] = 200.0;
        a[5_500] = f64::NAN;
        b[5_500] = f64::NAN;
        let (kept, curves) = kept_points(xs, vec![a, b], (50.0, 60.0), 10, (50.0, 60.0));
        for x in [49.99, 50.0, 50.01, 50.02, 55.0, 60.0, 60.01] {
            assert!(kept.contains(&x), "missing {x}");
        }
        assert!(kept.len() < 100);
        let spike = kept.iter().position(|x| *x == 50.01).unwrap();
        assert_eq!(curves[0][spike], 9.0);
        assert_eq!(curves[1][spike], 10.0);
        let gap = kept.iter().position(|x| *x == 55.0).unwrap();
        assert!(curves.iter().all(|curve| curve[gap].is_nan()));
    }
}
