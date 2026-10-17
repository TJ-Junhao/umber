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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::plan_figure;
    use crate::{
        AxesMut, AxisData, Figure, FontLibrary, HexbinOptions, Hist2dOptions, Instant, LineOptions,
        MeshOptions, ScaleKind,
    };

    #[derive(Clone, Copy)]
    enum Cells {
        Mesh,
        TimeMesh,
        Hist2d,
        Hexbin,
    }

    impl Cells {
        fn add(self, axes: &mut AxesMut<'_>, low: f64, high: f64) {
            match self {
                Self::Mesh => axes
                    .mesh(&[0.0, 1.0], &[low, high], &[1.0], MeshOptions::default())
                    .unwrap(),
                Self::TimeMesh => axes
                    .mesh_time(
                        AxisData::Times(vec![
                            Some(Instant::from_micros(0).unwrap()),
                            Some(Instant::from_micros(86_400_000_000).unwrap()),
                        ]),
                        AxisData::Numbers(vec![low, high]),
                        &[1.0],
                        MeshOptions::default(),
                    )
                    .unwrap(),
                Self::Hist2d => axes
                    .hist2d(&[0.0, 1.0], &[low, high], Hist2dOptions::default())
                    .unwrap(),
                Self::Hexbin => axes
                    .hexbin(&[0.0, 1.0], &[low, high], HexbinOptions::default())
                    .unwrap(),
            };
        }
    }

    fn cells(kind: Cells, low: f64, high: f64, grid: GridLines) -> Figure {
        let theme = Theme::pigment().with_grid_lines(grid).unwrap();
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        let x = if matches!(kind, Cells::TimeMesh) {
            ScaleKind::Time
        } else {
            ScaleKind::Linear
        };
        let mut axes = f.add_axes(x, ScaleKind::Linear).unwrap();
        axes.ylim(low, high).unwrap();
        axes.yticks(&[low, (low + high) / 2.0, high]).unwrap();
        kind.add(&mut axes, low, high);
        f
    }

    fn no_baseline(f: &Figure) {
        let lib = FontLibrary::global();
        let (_, layout) = plan_figure(f, (0, 0), lib);
        let scene = f.to_scene(lib).unwrap();
        assert!(!scene.items.contains(&baseline(f.theme(), &layout)));
    }

    fn cells_have_no_baseline(kind: Cells, low: f64, high: f64) {
        let f = cells(kind, low, high, GridLines::Horizontal);
        no_baseline(&f);
        // Chrome, marks and the colorbar, with no baseline group.
        assert_eq!(f.to_scene(FontLibrary::global()).unwrap().items.len(), 3);
    }

    #[test]
    fn mesh_draws_no_baseline_with_zero_inside() {
        cells_have_no_baseline(Cells::Mesh, -1.0, 1.0);
    }

    #[test]
    fn mesh_draws_no_baseline_with_zero_outside() {
        cells_have_no_baseline(Cells::Mesh, 1.0, 3.0);
    }

    #[test]
    fn time_mesh_draws_no_baseline_with_zero_inside() {
        cells_have_no_baseline(Cells::TimeMesh, -1.0, 1.0);
    }

    #[test]
    fn time_mesh_draws_no_baseline_with_zero_outside() {
        cells_have_no_baseline(Cells::TimeMesh, 1.0, 3.0);
    }

    #[test]
    fn hist2d_draws_no_baseline_with_zero_inside() {
        cells_have_no_baseline(Cells::Hist2d, -1.0, 1.0);
    }

    #[test]
    fn hist2d_draws_no_baseline_with_zero_outside() {
        cells_have_no_baseline(Cells::Hist2d, 1.0, 3.0);
    }

    #[test]
    fn hexbin_draws_no_baseline_with_zero_inside() {
        cells_have_no_baseline(Cells::Hexbin, -1.0, 1.0);
    }

    #[test]
    fn hexbin_draws_no_baseline_with_zero_outside() {
        cells_have_no_baseline(Cells::Hexbin, 1.0, 3.0);
    }

    fn cell_ticks_have_gridlines(kind: Cells) {
        for (low, high) in [(-1.0, 1.0), (1.0, 3.0)] {
            let f = cells(kind, low, high, GridLines::Horizontal);
            let lib = FontLibrary::global();
            let (resolved, layout) = plan_figure(&f, (0, 0), lib);
            let expected: Vec<_> = resolved
                .y
                .ticks
                .iter()
                .map(|tick| layout.plot.y_at(tick.pos))
                .collect();
            assert_eq!(layout.y_grid, expected);
            let scene = f.to_scene(lib).unwrap();
            let Item::Group { items, clip: None } = &scene.items[0] else {
                panic!("expected unclipped chrome");
            };
            for y in expected {
                assert!(items.contains(&Item::Stroke {
                    path: segment(point(layout.plot.x, y), point(layout.plot.right(), y)),
                    style: grid_style(f.theme()),
                }));
            }
        }
    }

    #[test]
    fn mesh_ticks_at_zero_and_lower_edge_have_gridlines() {
        cell_ticks_have_gridlines(Cells::Mesh);
    }

    #[test]
    fn time_mesh_ticks_at_zero_and_lower_edge_have_gridlines() {
        cell_ticks_have_gridlines(Cells::TimeMesh);
    }

    #[test]
    fn hist2d_ticks_at_zero_and_lower_edge_have_gridlines() {
        cell_ticks_have_gridlines(Cells::Hist2d);
    }

    #[test]
    fn hexbin_ticks_at_zero_and_lower_edge_have_gridlines() {
        cell_ticks_have_gridlines(Cells::Hexbin);
    }

    fn line(axes: &mut AxesMut<'_>, low: f64, high: f64) {
        axes.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[low, high],
            LineOptions::default(),
        )
        .unwrap();
    }

    #[test]
    fn mesh_and_line_draw_no_baseline_in_either_order() {
        for line_first in [false, true] {
            for (low, high) in [(-1.0, 1.0), (1.0, 3.0)] {
                let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
                let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
                axes.ylim(low, high).unwrap();
                if line_first {
                    line(&mut axes, low, high);
                }
                Cells::Mesh.add(&mut axes, low, high);
                if !line_first {
                    line(&mut axes, low, high);
                }
                no_baseline(&f);
            }
        }
    }

    #[test]
    fn line_alone_keeps_baseline_instead_of_its_gridline() {
        for (low, high) in [(-1.0, 1.0), (1.0, 3.0)] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            axes.ylim(low, high).unwrap();
            axes.yticks(&[low, (low + high) / 2.0, high]).unwrap();
            line(&mut axes, low, high);
            let lib = FontLibrary::global();
            let (_, layout) = plan_figure(&f, (0, 0), lib);
            let scene = f.to_scene(lib).unwrap();
            assert!(scene.items.contains(&baseline(f.theme(), &layout)));
            assert!(!layout.y_grid.contains(&layout.baseline.at));
            assert_eq!(layout.y_grid.len(), 2);
        }
    }

    #[test]
    fn cell_gridlines_still_follow_the_theme() {
        let f = cells(Cells::Mesh, -1.0, 1.0, GridLines::None);
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let Item::Group { items, clip: None } = &scene.items[0] else {
            panic!("expected unclipped chrome");
        };
        assert!(!items.iter().any(|item| matches!(item, Item::Stroke { .. })));
    }

    #[test]
    fn blank_mesh_still_draws_no_baseline() {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .mesh(
                &[0.0, 1.0],
                &[-1.0, 1.0],
                &[f64::NAN],
                MeshOptions::default(),
            )
            .unwrap();
        no_baseline(&f);
    }
}
