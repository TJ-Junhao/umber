//! Output cases for category y axes: horizontal bars, grouped and stacked bars, boxes and
//! violins on a shared category y axis, dot plots, and the errors of marks on these axes.

use std::sync::Arc;

use super::super::{Log, Output, axes, emit_scene, lcg, linear, log, themes};
use umber_core::figure::{AxisData, BarOptions, ScatterOptions, Series};
use umber_core::{
    AxisValue, BoxOptions, Distribution, ErrorBarOptions, Figure, FontLibrary, Grid, GridIndex,
    LineOptions, Placement, RuleOptions, ScaleKind, Share, SpanOptions, Theme, ViolinOptions,
};

const COUNTRIES: [&str; 8] = [
    "United States",
    "Germany",
    "Brazil",
    "India",
    "South Korea",
    "Nigeria",
    "Japan",
    "New Zealand",
];

fn countries(n: usize) -> Vec<String> {
    COUNTRIES[..n].iter().map(|s| (*s).to_owned()).collect()
}

fn series(label: &str, values: &[f64]) -> Series {
    Series {
        label: label.into(),
        values: values.to_vec(),
        ..Series::default()
    }
}

/// One axes with a linear x axis beside a category y axis.
fn horizontal(width: f64, height: f64, theme: Theme) -> Figure {
    let mut f = Figure::new(width, height, theme).unwrap();
    f.add_axes(linear(), ScaleKind::Category).unwrap();
    f
}

/// Samples around `mean` with spread `sd`, one outlier in every other group.
fn samples(seed: u32, means: &[f64], shift: f64) -> Vec<Vec<f64>> {
    let mut next = lcg(seed);
    means
        .iter()
        .enumerate()
        .map(|(i, &mean)| {
            let sd = [6.0, 9.0, 5.0, 11.0, 7.0, 8.0, 6.5, 4.5][i];
            let mut group: Vec<f64> = (0..40)
                .map(|_| mean + shift + sd * (next() + next() + next() - 1.5) * 2.0)
                .collect();
            if i % 2 == 0 {
                group.push(mean + shift + 4.0 * sd);
            }
            group
        })
        .collect()
}

/// A 1 by 2 grid sharing its category y axis, with the plain mark of a family on the left and
/// the grouped one on the right.
fn distributions_figure(
    theme: Theme,
    title: &str,
    n: usize,
    add: impl Fn(umber_core::AxesMut<'_>, bool, Vec<String>, Vec<Vec<f64>>, Vec<Distribution>),
) -> Figure {
    let grid = Grid::new(1, 2).unwrap().with_share_y(Share::All);
    let mut f = Figure::with_grid(10.0, 5.2, theme, grid).unwrap();
    f.title(title);
    let means = &[31.0, 42.0, 28.0, 52.0, 38.0, 45.0, 35.0, 24.0][..n];
    for (col, grouped) in [(0, false), (1, true)] {
        let mut a = f
            .add_axes_at(
                Placement {
                    row: GridIndex::At(0),
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                linear(),
                ScaleKind::Category,
            )
            .unwrap();
        a.xlabel("Minutes");
        let distributions = vec![
            Distribution {
                label: "Car".into(),
                groups: samples(82, means, -4.0),
                ..Distribution::default()
            },
            Distribution {
                label: "Transit".into(),
                groups: samples(83, means, 6.0),
                ..Distribution::default()
            },
        ];
        add(
            a,
            grouped,
            countries(n),
            samples(81, means, 0.0),
            distributions,
        );
    }
    f
}

/// Emits horizontal bar, box, violin and dot plot figures in all themes, then the errors.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let share_2015 = [13.0, 30.0, 75.0, 16.0, 3.0, 17.0, 15.0, 80.0];
    let share_2023 = [23.0, 54.0, 89.0, 21.0, 9.0, 20.0, 22.0, 82.0];
    for (name, theme) in themes() {
        let mut f = horizontal(6.4, 4.4, theme.clone());
        axes(&mut f)
            .title("Renewable share of electricity")
            .subtitle("Percent of generation, 2023")
            .xlabel("Share, %")
            .bar(
                countries(8),
                &share_2023,
                BarOptions {
                    label: "Renewable share".into(),
                    ..BarOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("horizontal-bar-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = horizontal(6.4, 4.8, theme.clone());
        axes(&mut f)
            .title("Renewable share, then and now")
            .subtitle("Percent of generation")
            .xlabel("Share, %")
            .grouped_bar(
                countries(8),
                vec![series("2015", &share_2015), series("2023", &share_2023)],
                umber_core::ValueLabels::default(),
            )
            .unwrap();
        emit_scene(
            &format!("horizontal-grouped-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = horizontal(6.4, 4.8, theme.clone());
        axes(&mut f)
            .title("Renewable mix")
            .subtitle("TWh by source, 2023")
            .xlabel("Generation, TWh")
            .stacked_bar(
                countries(8),
                vec![
                    series("Hydro", &[240.0, 25.0, 420.0, 160.0, 7.0, 30.0, 85.0, 24.0]),
                    series("Wind", &[425.0, 140.0, 100.0, 80.0, 5.0, 1.0, 10.0, 3.0]),
                    series("Solar", &[240.0, 60.0, 50.0, 140.0, 28.0, 1.0, 100.0, 1.0]),
                ],
                umber_core::ValueLabels::default(),
            )
            .unwrap();
        emit_scene(
            &format!("horizontal-stacked-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let f = distributions_figure(
            theme.clone(),
            "Commute time by country: box plots",
            8,
            |mut a, grouped, names, groups, distributions| {
                if grouped {
                    a.title("grouped_box")
                        .grouped_box_plot(names, distributions)
                        .unwrap();
                } else {
                    let options = BoxOptions {
                        label: "All commuters".into(),
                        ..BoxOptions::default()
                    };
                    a.title("box").box_plot(names, groups, options).unwrap();
                }
            },
        );
        emit_scene(
            &format!("horizontal-box-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let f = distributions_figure(
            theme.clone(),
            "Commute time by country: violins",
            7,
            |mut a, grouped, names, groups, distributions| {
                if grouped {
                    a.title("grouped_violin")
                        .grouped_violin(names, distributions)
                        .unwrap();
                } else {
                    let options = ViolinOptions {
                        label: "All commuters".into(),
                        ..ViolinOptions::default()
                    };
                    a.title("violin").violin(names, groups, options).unwrap();
                }
            },
        );
        emit_scene(
            &format!("horizontal-violin-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = horizontal(6.4, 4.4, theme.clone());
        let names = || AxisData::Categories(countries(8));
        let dots = |label: &str| ScatterOptions {
            label: label.into(),
            ..ScatterOptions::default()
        };
        axes(&mut f)
            .title("Life expectancy by country")
            .subtitle("Years at birth")
            .xlabel("Years")
            .scatter_shared(
                AxisData::Numbers(vec![78.7, 81.0, 75.3, 68.3, 82.0, 54.5, 83.7, 81.5]),
                names(),
                dots("2015"),
            )
            .unwrap()
            .scatter_shared(
                AxisData::Numbers(vec![77.5, 80.8, 75.9, 70.8, 83.5, 54.7, 84.8, 82.5]),
                names(),
                dots("2023"),
            )
            .unwrap()
            .x_error_bars_shared(
                AxisData::Categories(countries(2)),
                Arc::from([76.0, 79.5]),
                Arc::from([79.0, 82.1]),
                ErrorBarOptions::default(),
            )
            .unwrap()
            .vline(AxisValue::Number(73.0), RuleOptions::default())
            .unwrap()
            .vspan(60.0, 65.0, SpanOptions::default())
            .unwrap();
        emit_scene(
            &format!("horizontal-dotplot-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        // Negative bars on explicit limits, and a log x axis, whose bars grow from the
        // baseline at its lower limit.
        let mut f = horizontal(6.0, 3.0, theme.clone());
        axes(&mut f)
            .xlim(-30.0, 60.0)
            .unwrap()
            .bar(
                countries(4),
                &[-12.0, 45.0, 75.0, f64::NAN],
                BarOptions::default(),
            )
            .unwrap();
        emit_scene(
            &format!("horizontal-limits-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
        let mut f = Figure::new(6.0, 3.0, theme).unwrap();
        f.add_axes(log(10.0), ScaleKind::Category)
            .unwrap()
            .stacked_bar(
                countries(3),
                vec![
                    series("Hydro", &[240.0, 25.0, 420.0]),
                    series("Wind", &[425.0, 140.0, 100.0]),
                ],
                umber_core::ValueLabels::default(),
            )
            .unwrap();
        emit_scene(
            &format!("horizontal-log-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
    sink(Output {
        kind: "errors",
        file: "horizontal.txt".into(),
        bytes: errors().into_bytes(),
    });
}

/// The errors of mark methods beside a category y axis, and on two category axes, with the
/// state each failed call leaves.
fn errors() -> String {
    let mut rec = Log::default();
    let names = || vec!["A".to_owned(), "B".to_owned()];
    let mut f = rec.new_figure("linear/category", linear(), ScaleKind::Category);
    rec.on_axes(&mut f, "bar ok", |a| {
        a.bar(names(), &[1.0, -2.0], BarOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "bar infinite height", |a| {
        a.bar(names(), &[1.0, f64::INFINITY], BarOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "scatter numbers on y", |a| {
        a.scatter(
            AxisData::Numbers(vec![1.0]),
            &[1.0],
            ScatterOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "line", |a| {
        a.line(AxisData::Numbers(vec![1.0]), &[1.0], LineOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "x_error_bars numbers on y", |a| {
        a.x_error_bars(&[1.0], &[0.0], &[1.0], ErrorBarOptions::default())
            .map(drop)
    });
    let mut f = rec.new_figure("log/category", log(10.0), ScaleKind::Category);
    rec.on_axes(&mut f, "bar not positive on log x", |a| {
        a.bar(names(), &[1.0, 0.0], BarOptions::default()).map(drop)
    });
    let mut f = rec.new_figure(
        "category/category",
        ScaleKind::Category,
        ScaleKind::Category,
    );
    rec.on_axes(&mut f, "bar", |a| {
        a.bar(names(), &[1.0, 2.0], BarOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "violin", |a| {
        a.violin(
            names(),
            vec![vec![1.0], vec![2.0]],
            ViolinOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "scatter", |a| {
        a.scatter_shared(
            AxisData::Categories(names()),
            AxisData::Categories(names()),
            ScatterOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "vline", |a| {
        a.vline(AxisValue::Category("A".into()), RuleOptions::default())
            .map(drop)
    });
    rec.out
}
