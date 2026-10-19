//! Pie panels in a grid lay out in their whole cell, whatever the other panels reserve.

use super::*;
use crate::figure::{AxisData, BandOptions, BarOptions, EcdfOptions, HexbinOptions, LineOptions};
use crate::layout::{Area, PADDING};
use crate::{
    CellRange, ColorbarOptions, ColorbarPosition, Grid, GridIndex, HeatmapOptions, LegendMode,
    LegendPosition, LegendStyle, LogBase, PieOptions, Placement, ScaleKind, Share, SizeSpec,
    Transform,
};

fn lib() -> &'static FontLibrary {
    FontLibrary::global()
}

/// A small deterministic generator of normal samples.
struct Normal(u64);

impl Normal {
    /// A value in `0.0..1.0`, never 0.
    fn unit(&mut self) -> f64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 + 0.5) / (1_u64 << 53) as f64
    }

    /// A standard normal sample (Box–Muller).
    fn sample(&mut self) -> f64 {
        let (u, v) = (self.unit(), self.unit());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    fn samples(&mut self, n: usize, mean: f64, sd: f64) -> Vec<f64> {
        (0..n).map(|_| mean + sd * self.sample()).collect()
    }
}

fn at(row: i64, col: i64) -> Placement {
    Placement {
        row: GridIndex::At(row),
        col: GridIndex::At(col),
        ..Placement::default()
    }
}

fn labelled(label: &str) -> LineOptions {
    LineOptions {
        label: label.into(),
        ..LineOptions::default()
    }
}

/// Adds the annotated donut at `row`, `col`, with the legend `style` at `position`.
fn donut(
    f: &mut Figure,
    (row, col): (i64, i64),
    style: LegendStyle,
    position: Option<LegendPosition>,
) {
    f.add_pie_axes_at(at(row, col))
        .unwrap()
        .title("Donut")
        .pie(
            ["Web", "App", "Shop"].map(String::from).to_vec(),
            &[52.0, 30.0, 18.0],
            PieOptions {
                hole: 0.55,
                annotate: true,
                ..PieOptions::default()
            },
        )
        .unwrap()
        .legend(style, position, SizeSpec::Auto)
        .unwrap();
}

/// The 13.5 × 9 in, 3 × 4 grid with the donut at row 0, col 3 next to a line panel with
/// direct labels at (0, 0), a twin y panel at (2, 1), a hexbin with a colorbar at (2, 2) and
/// grouped ECDFs with direct labels at (2, 3).
fn crowded_grid() -> Figure {
    let grid = Grid::new(3, 4).unwrap().with_legend(LegendMode::PerPanel);
    let mut f = Figure::with_grid(13.5, 9.0, Theme::pigment(), grid).unwrap();
    f.title("umber")
        .subtitle("Plotting for Python, rendered in Rust");
    let mut rng = Normal(12);

    let x: Vec<f64> = (0..200).map(|i| 12.0 * f64::from(i) / 199.0).collect();
    let sin: Vec<f64> = x.iter().map(|x| x.sin()).collect();
    let low: Vec<f64> = sin.iter().map(|y| y - 0.3).collect();
    let high: Vec<f64> = sin.iter().map(|y| y + 0.3).collect();
    let trend: Vec<f64> = x.iter().map(|x| 0.8 * x.cos()).collect();
    f.add_axes_at(at(0, 0), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Lines and bands")
        .band(&x, &low, &high, BandOptions::default())
        .unwrap()
        .line(AxisData::Numbers(x.clone()), &sin, labelled("mean"))
        .unwrap()
        .line(AxisData::Numbers(x), &trend, labelled("trend"))
        .unwrap()
        .legend(LegendStyle::Direct, SizeSpec::Auto)
        .unwrap();

    let months: Vec<String> = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
        .map(String::from)
        .to_vec();
    let mut axes = f
        .add_axes_at(at(2, 1), ScaleKind::Category, ScaleKind::Linear)
        .unwrap();
    axes.title("Two y axes")
        .bar(
            months.clone(),
            &[52.0, 48.0, 55.0, 61.0, 72.0, 84.0],
            BarOptions {
                label: "rain".into(),
                ..BarOptions::default()
            },
        )
        .unwrap()
        .legend(LegendStyle::None, SizeSpec::Auto)
        .unwrap();
    axes.twin_y(ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Categories(months),
            &[4.0, 5.0, 9.0, 13.0, 18.0, 22.0],
            labelled("°C"),
        )
        .unwrap();

    let hx = rng.samples(20_000, 0.0, 1.0);
    let hy: Vec<f64> = hx.iter().map(|x| 0.6 * x + 0.7 * rng.sample()).collect();
    f.add_axes_at(at(2, 2), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Hexbin, log color")
        .hexbin(
            &hx,
            &hy,
            HexbinOptions {
                gridsize: 25,
                scale: Transform::Log(LogBase::TEN),
                ..HexbinOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "count".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();

    let mut axes = f
        .add_axes_at(at(2, 3), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("ECDF, grouped by=");
    for (mean, sd, label) in [(3.0, 0.4, "before"), (2.7, 0.3, "after")] {
        let values = rng
            .samples(400, mean, sd)
            .into_iter()
            .map(f64::exp)
            .collect();
        axes.ecdf(
            AxisData::Numbers(values),
            EcdfOptions {
                label: label.into(),
                ..EcdfOptions::default()
            },
        )
        .unwrap();
    }
    axes.legend(LegendStyle::Direct, SizeSpec::Auto).unwrap();

    donut(&mut f, (0, 3), LegendStyle::Direct, None);
    f
}

/// The planned layout of every panel of `f`, and its shared colorbars.
fn planned(f: &Figure) -> Result<FigurePlan> {
    let fonts = crate::test_support::figure_fonts(f.theme());
    let groups = ShareGroups::new(f);
    let mut panels = panels(f, &groups, &fonts)?;
    plan_cells(
        f,
        &mut panels,
        &groups,
        &fonts,
        "Figure.to_scene()",
        &layout::LabelCache::new(&fonts),
    )
}

/// The cells and layout of the only pie panel of `f`, planned in `plan`, and its pie placed.
fn pie_of(f: &Figure, plan: &FigurePlan) -> (CellRange, Layout, layout::pie::PieLayout) {
    let (index, panel) = f
        .layers()
        .enumerate()
        .find_map(|(i, layer)| match layer {
            Layer::Primary(crate::Panel::Pie(panel)) => Some((i, panel)),
            _ => None,
        })
        .expect("the figure has a pie panel");
    let layout = plan.panels[index].1.clone();
    let fonts = crate::test_support::figure_fonts(f.theme());
    let pie = panel.pie.as_ref().expect("the panel holds a pie");
    let room = layout::pie::PieRoom::new(layout.plot, layout.aligned);
    let placed = layout::pie::place(pie, panel.legend, room, f.theme(), &fonts)
        .expect("the pie fits its plot area");
    (panel.cells, layout, placed)
}

/// The cell of the panel covering `cells` of `f`'s grid, from its frame: inside the gaps
/// between cells and the canvas padding. The figure has no figure header.
fn cell(f: &Figure, cells: CellRange, frame: layout::Frame) -> Area {
    let gap = f64::from(f.theme().panel_gap());
    let grid = f.grid();
    let inset = |outer: bool| if outer { PADDING } else { gap / 2.0 };
    let left = frame.x + inset(cells.col() == 0);
    let right = frame.right() - inset(cells.col() + cells.col_span() == grid.cols());
    let top = frame.y + inset(cells.row() == 0);
    let bottom = frame.bottom() - inset(cells.row() + cells.row_span() == grid.rows());
    Area {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

/// The radius of the donut alone in a figure whose canvas inside the padding is `cell`.
fn alone(theme: &Theme, cell: Area, style: LegendStyle, position: Option<LegendPosition>) -> f64 {
    let mut f = Figure::new(
        (cell.width + 2.0 * PADDING) / 72.0,
        (cell.height + 2.0 * PADDING) / 72.0,
        theme.clone(),
    )
    .unwrap();
    donut(&mut f, (0, 0), style, position);
    let plan = planned(&f).unwrap();
    pie_of(&f, &plan).2.circle.radius
}

#[test]
fn a_donut_next_to_crowded_panels_lays_out_in_its_whole_cell() {
    let f = crowded_grid();
    let plan = planned(&f).unwrap();
    let (cells, layout, placed) = pie_of(&f, &plan);
    let gap = f64::from(f.theme().panel_gap());
    // Column 3 of 4: from the gap on its left to the canvas padding on its right.
    assert!((layout.plot.x - (layout.frame.x + gap / 2.0)).abs() < 1e-9);
    assert!((layout.plot.right() - (layout.frame.right() - PADDING)).abs() < 1e-9);
    // Row 0 of 3: down to the gap below it.
    assert!((layout.plot.bottom() - (layout.frame.bottom() - gap / 2.0)).abs() < 1e-9);
    let header = layout.title.as_ref().expect("a title").first();
    assert_eq!(header.x, layout.plot.x);
    assert!(placed.circle.radius > layout.plot.height / 4.0);
    // The Cartesian panels keep their aligned plots.
    let cartesian: Vec<_> = f
        .layers()
        .zip(&plan.panels)
        .filter(|(layer, _)| layer.cells() != cells)
        .map(|(_, (_, layout))| layout.plot)
        .collect();
    for plot in &cartesian {
        assert!((plot.width - cartesian[0].width).abs() < 1e-9);
        assert!((plot.height - cartesian[0].height).abs() < 1e-9);
    }
    assert!(cartesian.iter().all(|plot| plot.width < layout.plot.width));
    f.to_scene(lib()).unwrap();
}

#[test]
fn grid_pies_get_the_circle_of_a_figure_of_their_cell_size() {
    let wide_y = |f: &mut Figure, (row, col): (i64, i64)| {
        let x = ["Mon", "Tue", "Wed"].map(String::from);
        let y = ["Northern stations", "Southern stations"].map(String::from);
        f.add_axes_at(at(row, col), ScaleKind::Category, ScaleKind::Category)
            .unwrap()
            .title("Load")
            .heatmap(
                &x,
                &y,
                &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
                HeatmapOptions::default(),
            )
            .unwrap()
            .colorbar(ColorbarOptions {
                label: "Load per station".into(),
                position: Some(ColorbarPosition::Right),
                ..ColorbarOptions::default()
            })
            .unwrap();
    };
    let direct = |f: &mut Figure, (row, col): (i64, i64)| {
        let mut axes = f
            .add_axes_at(at(row, col), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.title("Output");
        for (i, label) in ["Station output", "Mean output"].into_iter().enumerate() {
            let y = [1_000_000.0, 2_500_000.0 + 400_000.0 * i as f64];
            axes.line(AxisData::Numbers(vec![0.0, 1.0]), &y, labelled(label))
                .unwrap();
        }
        axes.legend(LegendStyle::Direct, SizeSpec::Auto).unwrap();
    };
    let legends = [
        (LegendStyle::Direct, None),
        (LegendStyle::Header, None),
        (LegendStyle::Chips, Some(LegendPosition::TopRight)),
        (LegendStyle::Chips, Some(LegendPosition::Right)),
        (LegendStyle::Header, Some(LegendPosition::Bottom)),
    ];
    let cells = [(0, 0), (0, 1), (1, 0), (1, 1)];
    for (&pie, (style, position)) in cells
        .iter()
        .flat_map(|c| legends.iter().map(move |l| (c, *l)))
    {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let grid = Grid::new(2, 2).unwrap().with_legend(LegendMode::PerPanel);
            let mut f = Figure::with_grid(11.0, 7.0, theme.clone(), grid).unwrap();
            let others = cells.iter().filter(|&&c| c != pie);
            for (j, &other) in others.enumerate() {
                if j % 2 == 0 {
                    wide_y(&mut f, other);
                } else {
                    direct(&mut f, other);
                }
            }
            donut(&mut f, pie, style, position);
            let plan = planned(&f).unwrap();
            let (cells, layout, placed) = pie_of(&f, &plan);
            // The pie's header fits the band the row reserved above the aligned plots.
            assert!(layout.header_bottom <= layout.aligned.y + 1e-9);
            let cell = cell(&f, cells, layout.frame);
            let expected = alone(&theme, cell, style, position);
            assert!(
                (placed.circle.radius - expected).abs() < 1e-6,
                "{pie:?} {style:?} {position:?}: {} in the grid, {expected} alone",
                placed.circle.radius
            );
            f.to_scene(lib()).unwrap();
        }
    }
}

#[test]
fn a_donut_renders_in_a_grid_cell_the_size_of_a_figure_it_renders_in() {
    // A 3 × 2.6 in figure has 192 × 163.2 pt inside its padding; so do the cells of a
    // 6 × 2.6 in, 1 × 2 grid with a 24 pt gap, here holding the donut and an empty cell.
    let theme = Theme::pigment();
    assert_eq!(theme.panel_gap(), 24.0);
    let mut single = Figure::new(3.0, 2.6, theme.clone()).unwrap();
    donut(&mut single, (0, 0), LegendStyle::Direct, None);
    let radius = pie_of(&single, &planned(&single).unwrap()).2.circle.radius;
    let mut f = Figure::with_grid(6.0, 2.6, theme, Grid::new(1, 2).unwrap()).unwrap();
    donut(&mut f, (0, 0), LegendStyle::Direct, None);
    let plan = planned(&f).unwrap();
    let (_, layout, placed) = pie_of(&f, &plan);
    assert!((layout.plot.width - 192.0).abs() < 1e-9);
    assert!((placed.circle.radius - radius).abs() < 1e-9);
    f.to_scene(lib()).unwrap();
}

#[test]
fn pies_keep_clear_of_shared_colorbars() {
    for (position, pie) in [
        (ColorbarPosition::Right, (0, 1)),
        (ColorbarPosition::Bottom, (1, 0)),
    ] {
        let grid = Grid::new(2, 2).unwrap().with_share_color(Share::All);
        let mut f = Figure::with_grid(10.0, 7.0, Theme::graphite(), grid).unwrap();
        f.colorbar(ColorbarOptions {
            label: "Shared intensity".into(),
            position: Some(position),
            ..ColorbarOptions::default()
        })
        .unwrap();
        for cell in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            if cell == pie {
                continue;
            }
            f.add_axes_at(at(cell.0, cell.1), ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .scatter(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[0.0, 1.0],
                    crate::ScatterOptions {
                        color: crate::PointColor::By(crate::ColorBy::new(vec![1.0, 9.0])),
                        ..crate::ScatterOptions::default()
                    },
                )
                .unwrap();
        }
        donut(&mut f, pie, LegendStyle::Direct, None);
        let plan = planned(&f).unwrap();
        assert_eq!(plan.colorbars.len(), 1);
        let bar = &plan.colorbars[0].1;
        let (_, layout, placed) = pie_of(&f, &plan);
        let mut texts = placed.labels.iter().map(|label| &label.text);
        match position {
            ColorbarPosition::Right => {
                assert!(layout.plot.right() < bar.bar.x);
                assert!(texts.all(|text| text.x + text.width() < bar.bar.x));
            }
            _ => {
                let top = bar.label.as_ref().map_or(bar.bar.y, |label| {
                    label.y - f64::from(label.layout.ascent())
                });
                assert!(layout.plot.bottom() < top);
                assert!(texts.all(|text| text.y + f64::from(text.layout.descent()) < top));
            }
        }
        f.to_scene(lib()).unwrap();
    }
}

/// The width a grid error `the figure is … pt wide; at least {n} pt is needed …` names.
fn named_width(err: &Error) -> Option<f64> {
    let message = err.to_string();
    let (_, rest) = message.split_once(" pt wide; at least ")?;
    rest.split_once(" pt is needed for ")?.0.parse().ok()
}

/// The height a grid error `the figure is … pt tall; at least {n} pt is needed …` names.
fn named_height(err: &Error) -> Option<f64> {
    let message = err.to_string();
    let (_, rest) = message.split_once(" pt tall; at least ")?;
    rest.split_once(" pt is needed for ")?.0.parse().ok()
}

#[test]
fn too_small_pie_cells_fail_and_name_sizes_that_suffice() {
    // Labels that cannot fit around the circle in a small cell.
    let grid = Grid::new(2, 2).unwrap();
    let mut f = Figure::with_grid(4.0, 2.4, Theme::pigment(), grid).unwrap();
    for cell in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        donut(&mut f, cell, LegendStyle::Direct, None);
    }
    let err = f.to_scene(lib()).unwrap_err();
    assert!(
        err.to_string()
            .contains("cannot be placed outside the circle"),
        "{err}"
    );
    assert!(
        err.hint()
            .is_some_and(|hint| hint.contains("larger figure"))
    );

    // Heights: pies with two-line titles and legend rows next to a line panel.
    let tall = |height: f64| {
        let grid = Grid::new(2, 2).unwrap().with_legend(LegendMode::PerPanel);
        let mut f = Figure::with_grid(6.0, height / 72.0, Theme::midnight(), grid).unwrap();
        for cell in [(0, 0), (0, 1), (1, 1)] {
            f.add_pie_axes_at(at(cell.0, cell.1))
                .unwrap()
                .title("Orders\nby channel")
                .pie(
                    ["Web", "App", "Shop", "Other"].map(String::from).to_vec(),
                    &[40.0, 30.0, 20.0, 10.0],
                    PieOptions::default(),
                )
                .unwrap()
                .legend(LegendStyle::Header, None, SizeSpec::Auto)
                .unwrap();
        }
        f.add_axes_at(at(1, 0), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title("Load")
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                labelled("Station"),
            )
            .unwrap();
        f
    };
    let need = named_height(&tall(100.0).to_scene(lib()).unwrap_err()).unwrap();
    tall(need).to_scene(lib()).unwrap();
    let shorter = tall(need - 1.0).to_scene(lib()).unwrap_err();
    assert_eq!(named_height(&shorter), Some(need), "{shorter}");

    // Widths: pies with legends beside them next to a panel with wide y tick labels.
    let wide = |width: f64| {
        let grid = Grid::new(1, 3).unwrap().with_legend(LegendMode::PerPanel);
        let mut f = Figure::with_grid(width / 72.0, 4.0, Theme::pigment(), grid).unwrap();
        f.add_axes_at(at(0, 0), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1_000_000.0, 2_000_000.0],
                LineOptions::default(),
            )
            .unwrap();
        for col in [1, 2] {
            f.add_pie_axes_at(at(0, col))
                .unwrap()
                .pie(
                    ["Web", "App", "Shop"].map(String::from).to_vec(),
                    &[50.0, 30.0, 20.0],
                    PieOptions::default(),
                )
                .unwrap()
                .legend(
                    LegendStyle::Chips,
                    Some(LegendPosition::Right),
                    SizeSpec::Auto,
                )
                .unwrap();
        }
        f
    };
    // The width named renders; a point less does not, though below it a legend that no
    // longer fits beside its plot may be reported first.
    let need = named_width(&wide(100.0).to_scene(lib()).unwrap_err()).unwrap();
    wide(need).to_scene(lib()).unwrap();
    wide(need - 1.0).to_scene(lib()).unwrap_err();
}

#[test]
fn a_donut_that_fits_its_aligned_plot_also_fits_its_cell() {
    // The cell is taller than the aligned plot; the circle the labels need is smaller than
    // half the largest circle in the cell, but not than half the one in the aligned plot.
    let mut f = Figure::with_grid(5.0, 1.8, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    f.add_pie_axes_at(at(0, 0))
        .unwrap()
        .pie(
            ["Web", "App", "Shop"].map(String::from).to_vec(),
            &[52.0, 30.0, 18.0],
            PieOptions {
                hole: 0.55,
                annotate: true,
                ..PieOptions::default()
            },
        )
        .unwrap();
    f.add_axes_at(at(0, 1), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions::default(),
        )
        .unwrap()
        .xlabel("Distance")
        .ylabel("Volume");
    let plan = planned(&f).unwrap();
    let (_, layout, placed) = pie_of(&f, &plan);
    let least = |area: Area| area.width.min(area.height) / 4.0;
    assert!(layout.plot.height > layout.aligned.height);
    assert!(placed.circle.radius < least(layout.plot));
    assert!(placed.circle.radius >= least(layout.aligned));
    f.to_scene(lib()).unwrap();
}

#[test]
fn pie_legends_leave_the_cartesian_plots_where_they_were() {
    let grid = Grid::new(2, 2).unwrap().with_legend(LegendMode::PerPanel);
    let mut f = Figure::with_grid(8.0, 5.0, Theme::pigment(), grid).unwrap();
    f.add_pie_axes_at(at(0, 0))
        .unwrap()
        .title("Pie")
        .pie(
            ["North", "South", "East", "West", "Other"]
                .map(String::from)
                .to_vec(),
            &[60.0, 20.0, 10.0, 5.0, 5.0],
            PieOptions {
                hole: 0.55,
                ..PieOptions::default()
            },
        )
        .unwrap()
        .legend(LegendStyle::Header, None, SizeSpec::Auto)
        .unwrap();
    for (row, col) in [(0, 1), (1, 0), (1, 1)] {
        f.add_axes_at(at(row, col), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title(&format!("Line {row}{col}"))
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1_000_000.0, 2_000_000.0],
                labelled("Many outputs"),
            )
            .unwrap()
            .legend(LegendStyle::Direct, SizeSpec::Auto)
            .unwrap();
    }
    let plan = planned(&f).unwrap();
    let (_, pie, _) = pie_of(&f, &plan);
    assert!(pie.header_bottom <= pie.aligned.y + 1e-9);
    // The plot of (0, 1) where the pie's legend row is measured across the aligned plot.
    let line = f
        .layers()
        .zip(&plan.panels)
        .find(|(layer, _)| layer.cells().row() == 0 && layer.cells().col() == 1)
        .map(|(_, (_, layout))| layout.plot)
        .unwrap();
    assert!((line.y - 71.700).abs() < 5e-4, "{line:?}");
    assert!((line.height - 94.856).abs() < 5e-4, "{line:?}");
    f.to_scene(lib()).unwrap();
}
