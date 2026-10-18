//! Category samples in a shared orientation frame, with deterministic offsets.

use std::sync::Arc;

use super::MarkContext;
use super::mapping::{AxisMaps, axis_value};
use super::scatter::scatter_item;
use crate::resolve::swarm::{Jitter, Placement, pack};
use crate::resolve::{AxisScale, Resolved, ResolvedAxis, ResolvedMark};
use crate::scene::Item;
use crate::{Error, Marker, Result, Rgba};

/// Coordinates in the category-first frame, plus the first category that cannot fit.
struct Points {
    along: Vec<f64>,
    values: Vec<f64>,
    overflow: Option<(usize, usize)>,
}

/// Maps and spreads groups in their category-first frame. Only points whose circles can
/// reach the value-axis window enter a swarm. The jitter stream skips missing samples.
fn coordinates(
    maps: AxisMaps,
    (along, values): (&ResolvedAxis, &ResolvedAxis),
    names: &[String],
    groups: &[Arc<[f64]>],
    size: f32,
    placement: Placement,
) -> Points {
    let mut points = Points {
        along: Vec::new(),
        values: Vec::new(),
        overflow: None,
    };
    let AxisScale::Category(categories) = &along.scale else {
        return points;
    };
    let reach = f64::from(size) / 2.0 + f64::from(super::scatter::MARKER_OUTLINE) + 1.0;
    let (low, high) = maps.y.expanded(reach);
    for (i, (name, samples)) in names.iter().zip(groups).enumerate() {
        let Some(index) = categories.index_of(name) else {
            continue;
        };
        let (start, end) = categories.band(index);
        let center = (start + end) / 2.0;
        match placement {
            Placement::Jitter(jitter) => {
                let mut random = Jitter::new(index);
                for &sample in samples.iter().filter(|v| v.is_finite()) {
                    let offset = random.next() * jitter * (end - start);
                    let value = axis_value(values, sample);
                    if (low..=high).contains(&value) {
                        points.along.push(center + offset);
                        points.values.push(value);
                    }
                }
            }
            Placement::Swarm => {
                let mut mapped: Vec<_> = samples
                    .iter()
                    .filter(|v| v.is_finite())
                    .map(|&sample| axis_value(values, sample))
                    .filter(|v| (low..=high).contains(v))
                    .collect();
                mapped.sort_by(f64::total_cmp);
                let physical: Vec<_> = mapped.iter().map(|&v| maps.y.plot_at(v)).collect();
                let packing = pack(&physical, f64::from(size), maps.x.points(end - start) / 2.0);
                if !packing.complete && points.overflow.is_none() {
                    points.overflow = Some((i, samples.iter().filter(|v| v.is_finite()).count()));
                }
                for (value, offset) in mapped.into_iter().zip(packing.offsets) {
                    points.along.push(center + maps.x.length(offset));
                    points.values.push(value);
                }
            }
        }
    }
    points
}

/// Checks every swarm against the final plot dimensions, before any scene is returned.
/// Tentative legend placement can draw an incomplete prefix; it cannot bypass this check.
pub(crate) fn check(
    resolved: &Resolved,
    layout: &crate::layout::Layout,
    marks: &[ResolvedMark<'_>],
) -> Result<()> {
    let maps = AxisMaps::new(&resolved.x, &resolved.y, layout.plot);
    for mark in marks {
        if let ResolvedMark::Points {
            categories,
            groups,
            size,
            orientation,
            placement: Placement::Swarm,
            ..
        } = mark
        {
            let points = coordinates(
                maps.oriented(*orientation),
                resolved.along_and_values(*orientation),
                categories,
                groups,
                *size,
                Placement::Swarm,
            );
            if let Some((index, count)) = points.overflow {
                return Err(Error::style("Axes.swarm()", format!("category '{}' has {count} points, which do not fit in its width; use fewer or smaller points, or a wider category band.", categories[index]))
                    .with_parameter("groups").with_hint("use strip() or violin() for many points."));
            }
        }
    }
    Ok(())
}

/// Draws category points as one circular-marker batch, using the scatter alpha and outline.
pub(super) fn items(
    context: &MarkContext<'_>,
    axes: (&ResolvedAxis, &ResolvedAxis),
    names: &[String],
    groups: &[Arc<[f64]>],
    (color, size, placement): (Rgba, f32, Placement),
    items: &mut Vec<Item>,
) {
    let points = coordinates(context.maps, axes, names, groups, size, placement);
    if let Some(item) = scatter_item(
        context,
        &points.along,
        &points.values,
        color,
        Marker::Circle,
        size,
    ) {
        items.push(item);
    }
}
