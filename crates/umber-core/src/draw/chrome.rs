//! The axes chrome: gridlines, tick labels, the header and the axis labels, and the baseline,
//! which is drawn on its own over the marks.

use super::mapping::point;
use super::stroke::stroke_width;
use crate::geometry::{Path, Point};
use crate::layout::{Label, Layout};
use crate::resolve::ValueAxis;
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle, TextItem};
use crate::{Dash, GridLines, Rgba, Theme};

/// The straight path from `from` to `to`.
pub(super) fn segment(from: Point, to: Point) -> Path {
    let mut path = Path::new();
    path.move_to(from);
    path.line_to(to);
    path
}

/// The gridline stroke: `Dotted` is round dots every 3.2 pt, `Dashed` 2 on / 4 off.
fn grid_style(theme: &Theme) -> StrokeStyle {
    let (dash, cap) = match theme.grid_dash() {
        Dash::Solid => (Vec::new(), LineCap::Butt),
        Dash::Dashed => (vec![2.0, 4.0], LineCap::Butt),
        Dash::Dotted => (vec![0.0, 3.2], LineCap::Round),
    };
    StrokeStyle {
        color: theme.grid_color(),
        width: stroke_width(theme.grid_width()),
        dash,
        cap,
        join: LineJoin::Miter,
    }
}

/// `label` as a text item in `color`, reusing its shaped layout.
pub(super) fn text(label: &Label, color: Rgba) -> Item {
    let origin = point(label.x, label.y);
    Item::Text(TextItem::from_layout(
        label.text.clone(),
        label.style,
        origin,
        color,
        &label.layout,
    ))
}

/// The figure title, subtitle and merged legend as one unclipped group, omitted when empty.
pub(crate) fn figure_header(theme: &Theme, header: &crate::layout::FigureHeader) -> Option<Item> {
    let mut items = Vec::new();
    if let Some(title) = &header.title {
        items.push(text(title, theme.text_color()));
    }
    if let Some(subtitle) = &header.subtitle {
        items.push(text(subtitle, theme.muted_color()));
    }
    super::legend::row(theme, header.legend.entries(), &mut items);
    (!items.is_empty()).then_some(Item::Group { clip: None, items })
}

/// Which gridlines the theme's `lines` draws on a plot whose values lie along `value_axis`. The
/// theme's choice describes a chart with its values on y; a chart with its values on x, one with
/// a category y axis beside a numeric x axis, mirrors it, so that its lines across the values
/// run vertically: horizontal lines become vertical ones and vertical lines horizontal ones.
fn oriented_grid(lines: GridLines, value_axis: ValueAxis) -> GridLines {
    match (value_axis, lines) {
        (ValueAxis::Y, lines) => lines,
        (ValueAxis::X, GridLines::Horizontal) => GridLines::Vertical,
        (ValueAxis::X, GridLines::Vertical) => GridLines::Horizontal,
        (ValueAxis::X, lines @ (GridLines::Both | GridLines::None)) => lines,
    }
}

/// The axes chrome as one unclipped group: gridlines, tick labels, header and axis labels, in
/// that order. The baseline is not part of it; see [`baseline`].
pub(crate) fn chrome(theme: &Theme, layout: &Layout) -> Item {
    let plot = layout.plot;
    let mut items = Vec::new();

    let grid = grid_style(theme);
    // The baseline lies on the value axis.
    let lines = oriented_grid(theme.grid_lines(), layout.baseline.axis);
    if matches!(lines, GridLines::Horizontal | GridLines::Both) {
        for &y in &layout.y_grid {
            items.push(Item::Stroke {
                path: segment(point(plot.x, y), point(plot.right(), y)),
                style: grid.clone(),
            });
        }
    }
    if matches!(lines, GridLines::Vertical | GridLines::Both) {
        for &x in &layout.x_grid {
            items.push(Item::Stroke {
                path: segment(point(x, plot.y), point(x, plot.bottom())),
                style: grid.clone(),
            });
        }
    }

    let muted = theme.muted_color();
    for label in layout.y_tick_labels.iter().chain(&layout.x_tick_labels) {
        items.push(text(label, muted));
    }
    if let Some(label) = &layout.title {
        items.push(text(label, theme.text_color()));
    }
    if let Some(label) = &layout.subtitle {
        items.push(text(label, muted));
    }
    for label in layout.ylabel.iter().chain(&layout.xlabel) {
        items.push(text(label, theme.text_color()));
    }

    Item::Group { clip: None, items }
}

/// The baseline as its own unclipped group, drawn after the marks so that it stays visible
/// where marks reach it: a solid line across the plot area at the layout's baseline, in the
/// theme's baseline color and width. A baseline on the y axis runs from left to right, one on
/// the x axis from top to bottom.
pub(crate) fn baseline(theme: &Theme, layout: &Layout) -> Item {
    let plot = layout.plot;
    let at = layout.baseline.at;
    let path = match layout.baseline.axis {
        ValueAxis::Y => segment(point(plot.x, at), point(plot.right(), at)),
        ValueAxis::X => segment(point(at, plot.y), point(at, plot.bottom())),
    };
    let stroke = Item::Stroke {
        path,
        style: StrokeStyle {
            color: theme.baseline_color(),
            width: stroke_width(theme.baseline_width()),
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        },
    };
    Item::Group {
        clip: None,
        items: vec![stroke],
    }
}
