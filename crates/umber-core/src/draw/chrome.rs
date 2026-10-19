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
        AxesMut, AxisData, AxisValue, Distribution, Figure, FontLibrary, HexbinOptions,
        Hist2dOptions, Instant, LineOptions, MeshOptions, ScaleKind, Series,
    };

    fn numbers() -> AxisData {
        AxisData::Numbers(vec![0.0, 1.0])
    }

    fn categories() -> Vec<String> {
        vec!["a".into(), "b".into()]
    }

    fn series() -> Vec<Series> {
        vec![Series {
            values: vec![1.0, 2.0],
            ..Series::default()
        }]
    }

    fn groups() -> Vec<Vec<f64>> {
        vec![vec![1.0, 2.0, 3.0], vec![2.0, 3.0, 4.0]]
    }

    fn distributions() -> Vec<Distribution> {
        vec![Distribution {
            groups: groups(),
            ..Distribution::default()
        }]
    }

    #[test]
    fn baseline_is_drawn_only_for_zero_anchored_marks() {
        use ScaleKind::{Category, Linear};
        type Case = (
            &'static str,
            ScaleKind,
            ScaleKind,
            bool,
            fn(&mut AxesMut<'_>),
        );
        let cases: &[Case] = &[
            ("Line", Linear, Linear, false, |a| {
                a.line(numbers(), &[1.0, 2.0], Default::default()).unwrap();
            }),
            ("Step", Linear, Linear, false, |a| {
                a.step(numbers(), &[1.0, 2.0], Default::default()).unwrap();
            }),
            ("Scatter", Linear, Linear, false, |a| {
                a.scatter(numbers(), &[1.0, 2.0], Default::default())
                    .unwrap();
            }),
            ("Bubbles", Linear, Linear, false, |a| {
                a.bubbles(&[0.0, 1.0], &[1.0, 2.0], &[1.0, 2.0], Default::default())
                    .unwrap();
            }),
            ("Box", Category, Linear, false, |a| {
                a.box_plot(categories(), groups(), Default::default())
                    .unwrap();
            }),
            ("GroupedBox", Category, Linear, false, |a| {
                a.grouped_box_plot(categories(), distributions()).unwrap();
            }),
            ("Violin", Category, Linear, false, |a| {
                a.violin(categories(), groups(), Default::default())
                    .unwrap();
            }),
            ("GroupedViolin", Category, Linear, false, |a| {
                a.grouped_violin(categories(), distributions()).unwrap();
            }),
            ("ErrorBars", Linear, Linear, false, |a| {
                a.y_error_bars(numbers(), &[1.0, 2.0], &[2.0, 3.0], Default::default())
                    .unwrap();
            }),
            ("Band", Linear, Linear, false, |a| {
                a.band(&[0.0, 1.0], &[1.0, 2.0], &[2.0, 3.0], Default::default())
                    .unwrap();
            }),
            ("HLine", Linear, Linear, false, |a| {
                a.hline(1.0, Default::default()).unwrap();
            }),
            ("VLine", Linear, Linear, false, |a| {
                a.vline(AxisValue::Number(1.0), Default::default()).unwrap();
            }),
            ("HSpan", Linear, Linear, false, |a| {
                a.hspan(1.0, 2.0, Default::default()).unwrap();
            }),
            ("VSpan", Linear, Linear, false, |a| {
                a.vspan(1.0, 2.0, Default::default()).unwrap();
            }),
            ("Mesh", Linear, Linear, false, |a| {
                Cells::Mesh.add(a, -1.0, 1.0);
            }),
            ("Hist2d", Linear, Linear, false, |a| {
                Cells::Hist2d.add(a, -1.0, 1.0);
            }),
            ("Hexbin", Linear, Linear, false, |a| {
                Cells::Hexbin.add(a, -1.0, 1.0);
            }),
            ("Heatmap", Category, Category, false, |a| {
                a.heatmap(
                    &categories(),
                    &categories(),
                    &[1.0, 2.0, 3.0, 4.0],
                    Default::default(),
                )
                .unwrap();
            }),
            ("Bar", Category, Linear, true, |a| {
                a.bar(categories(), &[1.0, 2.0], Default::default())
                    .unwrap();
            }),
            ("GroupedBar", Category, Linear, true, |a| {
                a.grouped_bar(categories(), series(), crate::ValueLabels::default())
                    .unwrap();
            }),
            ("StackedBar", Category, Linear, true, |a| {
                a.stacked_bar(categories(), series(), crate::ValueLabels::default())
                    .unwrap();
            }),
            ("Hist", Linear, Linear, true, |a| {
                a.hist(&[0.0, 1.0, 2.0], Default::default()).unwrap();
            }),
            ("Area", Linear, Linear, true, |a| {
                a.area(numbers(), &[1.0, 2.0], Default::default()).unwrap();
            }),
            ("StackedArea", Linear, Linear, true, |a| {
                a.stacked_area(&[0.0, 1.0], series()).unwrap();
            }),
            ("Stem", Linear, Linear, true, |a| {
                a.stem(numbers(), &[1.0, 2.0], Default::default()).unwrap();
            }),
            ("horizontal Bar", Linear, Category, true, |a| {
                a.bar(categories(), &[1.0, 2.0], Default::default())
                    .unwrap();
            }),
            ("horizontal GroupedBar", Linear, Category, true, |a| {
                a.grouped_bar(categories(), series(), crate::ValueLabels::default())
                    .unwrap();
            }),
            ("horizontal StackedBar", Linear, Category, true, |a| {
                a.stacked_bar(categories(), series(), crate::ValueLabels::default())
                    .unwrap();
            }),
            ("dot plot", Category, Linear, false, |a| {
                a.scatter(
                    AxisData::Categories(categories()),
                    &[1.0, 2.0],
                    Default::default(),
                )
                .unwrap();
            }),
            ("horizontal dot plot", Linear, Category, false, |a| {
                a.scatter_shared(
                    numbers(),
                    AxisData::Categories(categories()),
                    Default::default(),
                )
                .unwrap();
            }),
        ];
        let lib = FontLibrary::global();
        let mut wrong = Vec::new();
        for &(name, x, y, expected, add) in cases {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = f.add_axes(x, y).unwrap();
            add(&mut axes);
            assert_eq!(axes.marks().len(), 1, "{name}");
            let (_, layout) = plan_figure(&f, (0, 0), lib);
            let scene = f.to_scene(lib).unwrap();
            if scene.items.contains(&baseline(f.theme(), &layout)) != expected {
                wrong.push(name);
            }
        }
        assert!(wrong.is_empty(), "wrong baseline visibility: {wrong:?}");
    }

    #[test]
    fn empty_axes_draw_no_baseline() {
        for (x, y) in [
            (ScaleKind::Linear, ScaleKind::Linear),
            (ScaleKind::Category, ScaleKind::Linear),
            (ScaleKind::Linear, ScaleKind::Category),
            (ScaleKind::Category, ScaleKind::Category),
        ] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            f.add_axes(x, y).unwrap();
            no_baseline(&f);
        }
    }

    #[test]
    fn line_over_bars_draws_baseline_in_either_order() {
        for line_first in [false, true] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
            let add_line = |a: &mut AxesMut<'_>| {
                a.line(
                    AxisData::Categories(categories()),
                    &[1.0, 2.0],
                    Default::default(),
                )
                .unwrap();
            };
            if line_first {
                add_line(&mut axes);
            }
            axes.bar(categories(), &[1.0, 2.0], Default::default())
                .unwrap();
            if !line_first {
                add_line(&mut axes);
            }
            let lib = FontLibrary::global();
            let (_, layout) = plan_figure(&f, (0, 0), lib);
            assert!(
                f.to_scene(lib)
                    .unwrap()
                    .items
                    .contains(&baseline(f.theme(), &layout))
            );
        }
    }

    #[test]
    fn mesh_beside_bar_keeps_baseline() {
        let mut bars = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        bars.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .bar(categories(), &[1.0, 2.0], Default::default())
            .unwrap();
        let mesh = cells(Cells::Mesh, -1.0, 1.0, GridLines::Horizontal);
        let lib = FontLibrary::global();
        let (resolved, _) = plan_figure(&bars, (0, 0), lib);
        // Bar needs a category axis and Mesh forbids one; test the rule with validated marks.
        let bar = bars.single_axes().marks()[0].clone();
        let mesh = mesh.single_axes().marks()[0].clone();
        assert!(resolved.draws_baseline(
            &[bar.clone(), mesh.clone()],
            crate::BaselineVisibility::Auto,
        ));
        assert!(resolved.draws_baseline(&[mesh, bar], crate::BaselineVisibility::Auto));
    }

    #[test]
    fn mesh_beside_histogram_bars_draws_baseline_in_either_order() {
        for hist_first in [false, true] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            if hist_first {
                axes.hist(&[0.0, 1.0, 2.0], Default::default()).unwrap();
            }
            Cells::Mesh.add(&mut axes, -1.0, 1.0);
            if !hist_first {
                axes.hist(&[0.0, 1.0, 2.0], Default::default()).unwrap();
            }
            let lib = FontLibrary::global();
            let (_, layout) = plan_figure(&f, (0, 0), lib);
            assert!(
                f.to_scene(lib)
                    .unwrap()
                    .items
                    .contains(&baseline(f.theme(), &layout))
            );
        }
    }

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
    fn line_keeps_gridline_at_zero_or_lower_edge_but_bars_draw_baseline() {
        for bars in [false, true] {
            for (low, high) in [(-1.0, 1.0), (1.0, 3.0)] {
                let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
                let x = if bars {
                    ScaleKind::Category
                } else {
                    ScaleKind::Linear
                };
                let mut axes = f.add_axes(x, ScaleKind::Linear).unwrap();
                axes.ylim(low, high).unwrap();
                axes.yticks(&[low, (low + high) / 2.0, high]).unwrap();
                if bars {
                    axes.bar(categories(), &[low, high], Default::default())
                        .unwrap();
                } else {
                    line(&mut axes, low, high);
                }
                let lib = FontLibrary::global();
                let (_, layout) = plan_figure(&f, (0, 0), lib);
                let scene = f.to_scene(lib).unwrap();
                assert_eq!(scene.items.contains(&baseline(f.theme(), &layout)), bars);
                assert_eq!(layout.y_grid.contains(&layout.baseline.at), !bars);
                assert_eq!(layout.y_grid.len(), if bars { 2 } else { 3 });
                let Item::Group { items, clip: None } = &scene.items[0] else {
                    panic!("expected unclipped chrome");
                };
                let y = layout.baseline.at;
                let gridline = Item::Stroke {
                    path: segment(point(layout.plot.x, y), point(layout.plot.right(), y)),
                    style: grid_style(f.theme()),
                };
                assert_eq!(items.contains(&gridline), !bars);
            }
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
