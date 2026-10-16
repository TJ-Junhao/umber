//! Colorbars: one gradient rectangle, its outline, ticks and measured texts.

use super::chrome::{segment, text};
use super::mapping::point;
use crate::color_scale::ResolvedColorScale;
use crate::layout::{ColorbarSide, PlacedColorbar};
use crate::{FillRule, Item, LineCap, LineJoin, Paint, Path, Rgba, StrokeStyle, Theme};

/// Draws the placed colorbar as one unclipped group.
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
    let mut items = vec![
        Item::Fill {
            path: path.clone(),
            paint: Paint::Sampled {
                cmap: color.cmap.clone(),
                from,
                to,
            },
            rule: FillRule::NonZero,
        },
        Item::Stroke {
            path,
            style: style(theme.grid_color(), 0.5),
        },
    ];
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
