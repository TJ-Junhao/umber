//! Colorbars: one gradient rectangle, or the bands of a filled contour, its outline, ticks and
//! measured texts.

use super::chrome::{segment, text};
use super::mapping::point;
use crate::color_scale::{ColorLevels, ResolvedColorScale};
use crate::layout::{ColorbarSide, PlacedColorbar};
use crate::resolve::NormalizedPos;
use crate::{FillRule, Item, LineCap, LineJoin, Paint, Path, Rgba, StrokeStyle, Theme};

/// Draws the placed colorbar as one unclipped group: a smooth gradient, or for the levels of a
/// filled contour ([`ColorLevels::banded`](crate::color_scale::ColorLevels)) its bands.
pub(crate) fn draw(theme: &Theme, color: &ResolvedColorScale, placed: &PlacedColorbar) -> Item {
    let bar = placed.bar;
    let mut path = Path::new();
    path.move_to(point(bar.x, bar.y));
    path.line_to(point(bar.right(), bar.y));
    path.line_to(point(bar.right(), bar.bottom()));
    path.line_to(point(bar.x, bar.bottom()));
    path.close();
    let (from, to) = match placed.position {
        ColorbarSide::Right => (point(bar.x, bar.bottom()), point(bar.x, bar.y)),
        ColorbarSide::Bottom => (point(bar.x, bar.y), point(bar.right(), bar.y)),
    };
    let style = |color: Rgba, width| StrokeStyle {
        color,
        width,
        dash: Vec::new(),
        cap: LineCap::Butt,
        join: LineJoin::Miter,
    };
    let mut items = match &color.levels {
        Some(levels) if levels.banded => bands(color, levels, placed),
        _ => vec![Item::Fill {
            path: path.clone(),
            paint: Paint::Sampled {
                cmap: color.cmap.clone(),
                from,
                to,
            },
            rule: FillRule::NonZero,
        }],
    };
    items.push(Item::Stroke {
        path,
        style: style(theme.grid_color(), 0.5),
    });
    let length = f64::from(theme.colorbar_tick_length());
    for (pos, label) in &placed.ticks {
        let (from, to) = match placed.position {
            ColorbarSide::Right => (
                point(bar.right(), bar.y_at(*pos)),
                point(bar.right() + length, bar.y_at(*pos)),
            ),
            ColorbarSide::Bottom => (
                point(bar.x_at(*pos), bar.bottom()),
                point(bar.x_at(*pos), bar.bottom() + length),
            ),
        };
        items.push(Item::Stroke {
            path: segment(from, to),
            style: style(theme.muted_color(), 0.75),
        });
        items.push(text(label, theme.muted_color()));
    }
    if let Some(label) = &placed.label {
        items.push(text(label, theme.text_color()));
    }
    Item::Group { clip: None, items }
}

/// The bands of a filled contour along the placed bar, in the colors its bands take
/// ([`ResolvedColorScale::band_color`]), between the positions of its `levels` on the color
/// range: the band below the first level from the low end of the bar, and each next band from
/// its lower level to the high end, over the bands before it, so bands meet without a seam.
fn bands(color: &ResolvedColorScale, levels: &ColorLevels, placed: &PlacedColorbar) -> Vec<Item> {
    let bar = placed.bar;
    let values: Vec<f64> = levels.ticks.iter().map(|tick| tick.value).collect();
    (0..=values.len())
        .filter_map(|band| {
            let below = band.checked_sub(1).and_then(|i| values.get(i)).copied();
            let above = values.get(band).copied();
            // A level that maps to no position (not positive on a log scale) lies below the range.
            let start = below.map_or(0.0, |v| color.norm.unit(v).unwrap_or(0.0));
            if start >= 1.0 && below.is_some() {
                return None;
            }
            let start = NormalizedPos::new(start);
            let (x0, y0, x1, y1) = match placed.position {
                ColorbarSide::Right => (bar.x, bar.y, bar.right(), bar.y_at(start)),
                ColorbarSide::Bottom => (bar.x_at(start), bar.y, bar.right(), bar.bottom()),
            };
            let mut path = Path::new();
            path.move_to(point(x0, y0));
            path.line_to(point(x1, y0));
            path.line_to(point(x1, y1));
            path.line_to(point(x0, y1));
            path.close();
            Some(Item::Fill {
                path,
                paint: Paint::Solid(color.band_color(below, above)),
                rule: FillRule::NonZero,
            })
        })
        .collect()
}
