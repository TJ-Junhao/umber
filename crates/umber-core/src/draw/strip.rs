//! Category samples in a shared orientation frame, with deterministic offsets.

use std::sync::Arc;

use super::MarkContext;
use super::mapping::{AxisMaps, axis_value};
use super::scatter::scatter_item;
use crate::resolve::swarm::{Placement, pack};
use crate::resolve::{AxisScale, Resolved, ResolvedAxis, ResolvedMark};
use crate::scene::Item;
use crate::{Error, Marker, Result, Rgba};

/// SplitMix64 seeded by the input category index and the strip's ordinal on its axes.
struct Jitter(u64);

impl Jitter {
    fn new(category: usize, strip: usize) -> Self {
        Self((category as u64).wrapping_add((strip as u64).wrapping_mul(0xd1b54a32d192ed03)))
    }

    /// The next uniform displacement in `[-1, 1)` using the high 53 random bits.
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        ((z >> 11) as f64 / ((1_u64 << 53) as f64)) * 2.0 - 1.0
    }
}

/// The first category that cannot fit, with its required and available widths in points.
struct Overflow {
    index: usize,
    needed: f64,
    room: f64,
}

/// Coordinates in the category-first frame, plus the first category that cannot fit.
struct Points {
    along: Vec<f64>,
    values: Vec<f64>,
    overflow: Option<Overflow>,
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
            Placement::Jitter { fraction, strip } => {
                let mut random = Jitter::new(i, strip);
                for &sample in samples.iter().filter(|v| v.is_finite()) {
                    let offset = random.next() * fraction * (end - start);
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
                let room = maps.x.points(end - start);
                let packing = pack(&physical, f64::from(size), room / 2.0);
                if !packing.complete && points.overflow.is_none() {
                    let full = pack(&physical, f64::from(size), f64::INFINITY);
                    let radius = full
                        .offsets
                        .iter()
                        .copied()
                        .map(f64::abs)
                        .fold(0.0, f64::max);
                    points.overflow = Some(Overflow {
                        index: i,
                        needed: 2.0 * radius + f64::from(size),
                        room,
                    });
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
    context: &'static str,
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
            if let Some(Overflow {
                index,
                needed,
                room,
            }) = points.overflow
            {
                let (needed, room) = crate::layout::widths(needed, room);
                return Err(Error::style(
                    context,
                    format!(
                        "the swarm of category '{}' needs {needed} pt across; \
                         its band is {room} pt.",
                        categories[index]
                    ),
                )
                .with_hint("use strip() or violin() for many points."));
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

#[cfg(test)]
mod tests {
    use super::Jitter;

    #[test]
    fn splitmix_stream_is_fixed_and_local_to_each_category_and_strip() {
        let mut a = Jitter::new(0, 0);
        let mut b = Jitter::new(0, 0);
        let mut category = Jitter::new(1, 0);
        let mut strip = Jitter::new(0, 1);
        assert_eq!(a.next(), 0.7666216164272852);
        let _ = b.next();
        for _ in 0..1000 {
            let x = a.next();
            assert_eq!(x, b.next());
            assert!((-1.0..1.0).contains(&x));
            assert_ne!(x, category.next());
            assert_ne!(x, strip.next());
        }
    }
}
