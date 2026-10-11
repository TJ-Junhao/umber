//! Output-gate cases for per-series line widths and marker sizes.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered, in every theme: a thick emphasized line, a thin muted line and a line at the theme's
//! width, an area with its own dashed width, and scatters smaller and larger than the theme's
//! markers, once with the header legend set on the axes (each swatch drawn at its series' width
//! or size) and once in the theme's own legend style (direct labels end in dots of the theme's
//! marker size, chips keep their fixed dot). Also: widths and sizes in points next to the
//! theme's on a log y axis; the widest effective stroke widening the marks clip to the y tick
//! labels; a line thicker than its legend swatch is long; and every error branch of
//! `SizeSpec::times` and `SizeSpec::pt`, with the specs they return.

use std::fmt::Write as _;

use umber_core::figure::{AreaOptions, AxisData, LineOptions, ScatterOptions};
use umber_core::{
    Alpha, ColorSpec, Dash, Figure, FontLibrary, LegendStyle, Marker, Rgba, SizeSpec,
};

use super::super::{
    Log, Output, axes, emit_scene, figure, labelled, linear, log, num, numbers, record, themes,
    wobble,
};

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib), sink);
    }
    sink(Output {
        kind: "errors",
        file: "size.txt".into(),
        bytes: size_errors().into_bytes(),
    });
}

fn times(factor: f64) -> SizeSpec {
    SizeSpec::times(factor).unwrap()
}

fn pt(points: f64) -> SizeSpec {
    SizeSpec::pt(points).unwrap()
}

fn sized_line(label: &str, dash: Dash, width: SizeSpec) -> LineOptions {
    LineOptions {
        label: label.into(),
        dash,
        width,
        ..LineOptions::default()
    }
}

fn sized_scatter(label: &str, marker: Marker, size: SizeSpec) -> ScatterOptions {
    ScatterOptions {
        label: label.into(),
        marker,
        size,
        ..ScatterOptions::default()
    }
}

/// A ramp of `n` points with a deterministic wobble, lifted by `lift`.
fn lifted(n: usize, seed: u32, lift: f64) -> (Vec<f64>, Vec<f64>) {
    let (x, y) = wobble(n, seed);
    (x, y.iter().map(|v| v + lift).collect())
}

/// An emphasized line, a muted one, one at the theme's width, an area with its own width and
/// two scatters, in `figure`.
fn emphasis(figure: &mut Figure) {
    let (x1, y1) = lifted(30, 41, 8.0);
    let (x2, y2) = lifted(30, 42, 5.0);
    let (x3, y3) = lifted(30, 43, 6.5);
    let (x4, y4) = lifted(30, 44, 2.0);
    let muted = ColorSpec::Fixed(Rgba::GRAY).with_alpha(Alpha::new(0.7).unwrap());
    let (xs, ys) = lifted(12, 45, 9.0);
    let (xl, yl) = lifted(8, 46, 4.0);
    axes(figure)
        .line(
            numbers(x1),
            &y1,
            sized_line("Emphasized", Dash::Solid, times(2.5)),
        )
        .unwrap()
        .line(
            numbers(x2),
            &y2,
            LineOptions {
                color: muted,
                ..sized_line("Muted", Dash::Solid, times(0.5))
            },
        )
        .unwrap()
        .line(
            numbers(x3),
            &y3,
            sized_line("Default", Dash::Dotted, SizeSpec::Auto),
        )
        .unwrap()
        .area(
            numbers(x4),
            &y4,
            sized_line("Area", Dash::Dashed, times(1.5)),
        )
        .unwrap()
        .scatter(
            numbers(xs.iter().map(|v| v + 0.5).collect()),
            &ys,
            sized_scatter("Small", Marker::Circle, times(0.6)),
        )
        .unwrap()
        .scatter(
            numbers(xl.iter().map(|v| 3.0 * v + 1.0).collect()),
            &yl,
            sized_scatter("Large", Marker::Diamond, pt(10.0)),
        )
        .unwrap();
}

/// Every size figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();

    // Emphasis with the header legend set on the axes, then in the theme's own legend style.
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Emphasis, header legend");
        emphasis(&mut f);
        axes(&mut f).legend(LegendStyle::Header);
        out.push((format!("fig-size-header-{theme_name}"), f));

        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Emphasis, theme legend");
        emphasis(&mut f);
        out.push((format!("fig-size-theme-legend-{theme_name}"), f));
    }

    // Absolute sizes next to the theme's, on a log y axis, with every marker shape.
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme, linear(), log(10.0));
        labelled(&mut f, "Sizes in points");
        let x: Vec<f64> = (0..10).map(f64::from).collect();
        let a = axes(&mut f);
        a.line(
            numbers(x.clone()),
            &x.iter().map(|v| 2.0 + v * v).collect::<Vec<_>>(),
            sized_line("Hairline", Dash::Solid, pt(0.25)),
        )
        .unwrap()
        .line(
            numbers(x.clone()),
            &x.iter().map(|v| 5.0 * (v + 1.0)).collect::<Vec<_>>(),
            sized_line("Bold", Dash::Dashed, pt(6.0)),
        )
        .unwrap();
        for (i, marker) in [
            Marker::Circle,
            Marker::Square,
            Marker::Triangle,
            Marker::Diamond,
            Marker::Cross,
        ]
        .into_iter()
        .enumerate()
        {
            let k = i as f64;
            a.scatter(
                numbers(x.clone()),
                &x.iter().map(|v| (k + 1.0) * 8.0 + v).collect::<Vec<_>>(),
                sized_scatter(&format!("{marker:?}"), marker, pt(3.0 + 3.0 * k)),
            )
            .unwrap();
        }
        a.legend(LegendStyle::Header);
        out.push((format!("fig-size-points-{theme_name}"), f));
    }

    // The widest effective stroke widens the marks clip, up to the y tick labels, and a line
    // in points thicker than ten theme widths keeps the swatch length of the others.
    let mut f = figure(6.0, 4.0, umber_core::Theme::graphite(), linear(), linear());
    labelled(&mut f, "Thick line ends");
    axes(&mut f)
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .line(
            numbers(vec![0.0, 0.5, 1.0]),
            &[0.2, 0.6, 0.4],
            sized_line("Wide", Dash::Solid, pt(24.0)),
        )
        .unwrap()
        .line(
            numbers(vec![0.0, 1.0]),
            &[0.8, 0.7],
            sized_line("Thin", Dash::Dashed, times(0.5)),
        )
        .unwrap()
        .legend(LegendStyle::Header);
    out.push(("fig-size-wide-line-ends".to_owned(), f));

    // A category x axis: an emphasized area and line, and a large scatter, under chips.
    let months: Vec<String> = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let mut f = figure(
        6.0,
        4.0,
        umber_core::Theme::midnight(),
        umber_core::ScaleKind::Category,
        linear(),
    );
    labelled(&mut f, "Category axis");
    axes(&mut f)
        .area(
            AxisData::Categories(months.clone()),
            &[1.0, 2.0, 3.5, 4.0, 3.0, 2.5],
            sized_line("Solar", Dash::Solid, times(2.0)),
        )
        .unwrap()
        .line(
            AxisData::Categories(months.clone()),
            &[3.0, 2.5, 2.0, 2.5, 3.5, 4.0],
            sized_line("Wind", Dash::Dotted, times(0.75)),
        )
        .unwrap()
        .scatter(
            AxisData::Categories(months),
            &[2.0, 3.0, 1.5, 3.0, 2.0, 1.0],
            sized_scatter("Sites", Marker::Square, times(2.0)),
        )
        .unwrap()
        .legend(LegendStyle::Chips);
    out.push(("fig-size-category-chips".to_owned(), f));

    out
}

/// A size spec as recorded: its variant and value bits.
fn spec(s: SizeSpec) -> String {
    match s {
        SizeSpec::Auto => "auto".into(),
        SizeSpec::Times(factor) => format!("times {}", record::f64(factor.get())),
        SizeSpec::Points(points) => format!("points {}", record::f64(points.get())),
    }
}

/// The results of `SizeSpec::times` and `SizeSpec::pt` across and beyond their bounds, and the
/// state of axes given lines, areas and scatters with every kind of size.
fn size_errors() -> String {
    let mut rec = Log::default();
    let values = [
        f64::NEG_INFINITY,
        -1.0,
        -0.0,
        0.0,
        5e-324,
        1e-300,
        0.5,
        1.0,
        10.0,
        10.000_001,
        72.0,
        72.1,
        1e301,
        f64::INFINITY,
        f64::NAN,
    ];
    rec.section("SizeSpec::times");
    for v in values {
        rec.result(&format!("times({})", num(v)), SizeSpec::times(v), spec);
    }
    rec.section("SizeSpec::pt");
    for v in values {
        rec.result(&format!("pt({})", num(v)), SizeSpec::pt(v), spec);
    }
    rec.section("SizeSpec defaults");
    let mut out = String::new();
    let _ = writeln!(out, "default: {}", spec(SizeSpec::default()));
    let _ = writeln!(out, "line: {}", spec(LineOptions::default().width));
    let _ = writeln!(out, "area: {}", spec(AreaOptions::default().width));
    let _ = writeln!(out, "scatter: {}", spec(ScatterOptions::default().size));
    rec.raw(&out);

    rec.section("Axes with sized marks, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    for (name, size) in [
        ("auto", SizeSpec::Auto),
        ("times", times(3.0)),
        ("points", pt(0.5)),
    ] {
        rec.on_axes(&mut f, &format!("line, area and scatter {name}"), |a| {
            a.line(
                numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                sized_line(name, Dash::Solid, size),
            )?
            .area(
                numbers(vec![0.0, 1.0]),
                &[0.5, 1.0],
                sized_line(name, Dash::Dashed, size),
            )?
            .scatter(
                numbers(vec![0.5]),
                &[1.5],
                sized_scatter(name, Marker::Cross, size),
            )
            .map(drop)
        });
        let marks = f.axes().unwrap().marks();
        let widths: Vec<String> = marks
            .iter()
            .map(|mark| match mark {
                umber_core::Mark::Line(line) => format!("line {}", spec(line.options().width)),
                umber_core::Mark::Area(area) => format!("area {}", spec(area.options().width)),
                umber_core::Mark::Scatter(scatter) => {
                    format!("scatter {}", spec(scatter.options().size))
                }
                _ => "other".into(),
            })
            .collect();
        rec.raw(&format!("  sizes [{}]\n", widths.join(", ")));
    }
    rec.out
}
