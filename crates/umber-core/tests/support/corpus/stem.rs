//! Output-gate cases for stems, their markers, domains, clipping and errors.

use std::fmt::Write as _;

use umber_core::{
    AxisData, Figure, FontLibrary, Grid, GridIndex, LegendStyle, Marker, Placement, SizeSpec,
    StemOptions, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, linear, log, themes};

fn numbers(x: &[f64]) -> AxisData {
    AxisData::Numbers(x.to_vec())
}

fn names() -> AxisData {
    AxisData::Categories(
        ["North", "East", "South", "West"]
            .map(String::from)
            .to_vec(),
    )
}

fn labelled(label: &str) -> StemOptions {
    StemOptions {
        label: label.into(),
        ..StemOptions::default()
    }
}

fn response() -> (Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..24).map(f64::from).collect();
    let y = x
        .iter()
        .map(|n| (-n / 7.0).exp() * (n * 0.9).cos() * 10.0)
        .collect();
    (x, y)
}

/// Passes each scene and API record to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, f) in figures() {
        emit_scene(&name, &f.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "stem.txt".into(),
        bytes: errors().into_bytes(),
    });
    let mut widths = String::new();
    for (name, theme) in themes() {
        let _ = writeln!(widths, "{name} stem_width {:?}", theme.stem_width());
    }
    sink(Output {
        kind: "api",
        file: "constants-stem.txt".into(),
        bytes: widths.into_bytes(),
    });
}

fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();
    for (name, theme) in themes() {
        let (x, y) = response();
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .title("Stems")
            .subtitle("Damped impulse response")
            .stem(numbers(&x), &y, labelled("Response"))
            .unwrap();
        out.push((format!("stem-{name}"), f));

        let mut f = figure(
            6.0,
            4.0,
            theme.clone(),
            umber_core::ScaleKind::Category,
            linear(),
        );
        axes(&mut f)
            .title("Regional change")
            .stem(names(), &[4.0, -2.0, 7.0, -5.0], labelled("Change"))
            .unwrap();
        out.push((format!("stem-category-{name}"), f));

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), log(10.0));
        axes(&mut f)
            .title("Log y")
            .stem(
                numbers(&[1.0, 2.0, 3.0, 4.0]),
                &[2.0, 15.0, 80.0, 400.0],
                labelled("Amount"),
            )
            .unwrap();
        out.push((format!("stem-log-y-{name}"), f));

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .title("Explicit limits")
            .xlim(1.0, 5.0)
            .unwrap()
            .ylim(-3.0, 3.0)
            .unwrap()
            .stem(
                numbers(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
                &[2.0, 3.0, 12.0, -3.0, -10.0, 1.0, 2.0],
                labelled("Clipped"),
            )
            .unwrap();
        out.push((format!("stem-limits-{name}"), f));

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .title("Gaps and negative values")
            .stem(
                numbers(&[0.0, 1.0, f64::NAN, 3.0, 4.0, 5.0]),
                &[-2.0, f64::NAN, 1.0, -4.0, 0.0, 3.0],
                labelled("Samples"),
            )
            .unwrap();
        out.push((format!("stem-gaps-{name}"), f));

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f).title("Marker shapes");
        for (i, (title, marker)) in [
            ("Circle", Marker::Circle),
            ("Square", Marker::Square),
            ("Triangle", Marker::Triangle),
            ("Diamond", Marker::Diamond),
            ("Cross", Marker::Cross),
        ]
        .into_iter()
        .enumerate()
        {
            axes(&mut f)
                .stem(
                    numbers(&[i as f64]),
                    &[2.0 + i as f64],
                    StemOptions {
                        marker,
                        size: SizeSpec::times(1.5).unwrap(),
                        ..labelled(title)
                    },
                )
                .unwrap();
        }
        out.push((format!("stem-markers-{name}"), f));

        let grid = Grid::new(1, 2).unwrap();
        let (width, height) = grid.auto_size_in();
        let mut f = Figure::with_grid(width, height, theme, grid).unwrap();
        for col in 0..2 {
            let mut panel = f
                .add_axes_at(
                    Placement {
                        row: GridIndex::At(0),
                        col: GridIndex::At(col),
                        row_span: 1,
                        col_span: 1,
                    },
                    linear(),
                    linear(),
                )
                .unwrap();
            panel.title(if col == 0 { "Response" } else { "Decay" });
            let values = if col == 0 {
                y.clone()
            } else {
                y.iter().map(|v| v.abs()).collect()
            };
            panel
                .stem(numbers(&x), &values, labelled("Samples"))
                .unwrap();
        }
        out.push((format!("stem-grid-{name}"), f));
    }
    for (name, style) in [
        ("header", LegendStyle::Header),
        ("direct", LegendStyle::Direct),
        ("chips", LegendStyle::Chips),
        ("none", LegendStyle::None),
    ] {
        let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
        axes(&mut f)
            .legend(style, umber_core::SizeSpec::Auto)
            .unwrap()
            .stem(
                numbers(&[0.0, 1.0, 2.0]),
                &[1.0, 4.0, 2.0],
                labelled("Response"),
            )
            .unwrap();
        out.push((format!("stem-legend-{name}"), f));
    }
    let mut f = figure(6.0, 4.0, Theme::pigment(), log(10.0), linear());
    axes(&mut f)
        .title("Log x")
        .stem(
            numbers(&[1.0, 10.0, 100.0, 1000.0]),
            &[3.0, 1.0, -2.0, 4.0],
            labelled("Response"),
        )
        .unwrap();
    out.push(("stem-log-x".into(), f));
    out
}

fn errors() -> String {
    let mut rec = Log::default();
    let options = StemOptions::default;
    rec.section("Axes.stem, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    for (name, x, y) in [
        ("ok", vec![0.0, 1.0], vec![2.0, -1.0]),
        ("unsorted", vec![2.0, 1.0], vec![1.0, 2.0]),
        ("lengths", vec![0.0, 1.0], vec![1.0]),
        ("length before value", vec![f64::INFINITY], vec![]),
        ("infinite x", vec![f64::NEG_INFINITY], vec![1.0]),
        ("infinite y", vec![0.0], vec![f64::INFINITY]),
        ("x before y", vec![f64::INFINITY], vec![f64::INFINITY]),
        ("huge x", vec![1e301], vec![1.0]),
        ("huge y", vec![0.0], vec![-1e301]),
        ("NaN", vec![f64::NAN, 1.0], vec![1.0, f64::NAN]),
        ("empty", vec![], vec![]),
    ] {
        rec.on_axes(&mut f, name, |a| {
            a.stem(numbers(&x), &y, options()).map(drop)
        });
    }
    rec.on_axes(&mut f, "names on numeric x", |a| {
        a.stem(names(), &[1.0; 4], options()).map(drop)
    });
    let mut f = rec.new_figure("category/linear", umber_core::ScaleKind::Category, linear());
    rec.on_axes(&mut f, "numbers on category x", |a| {
        a.stem(numbers(&[1.0]), &[1.0], options()).map(drop)
    });
    rec.on_axes(&mut f, "category ok", |a| {
        a.stem(names(), &[1.0; 4], options()).map(drop)
    });
    for (name, x_kind, y_kind, along_x) in [
        ("log/linear", log(10.0), linear(), true),
        ("linear/log", linear(), log(10.0), false),
    ] {
        let mut f = rec.new_figure(name, x_kind, y_kind);
        for value in [0.0, -1.0, 1e-310] {
            rec.on_axes(&mut f, &format!("invalid log value {value:e}"), |a| {
                let (x, y) = if along_x { (value, 1.0) } else { (1.0, value) };
                a.stem(numbers(&[x]), &[y], options()).map(drop)
            });
        }
    }
    rec.out
}
