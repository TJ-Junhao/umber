//! Output-gate cases for step lines.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Scenes, in the three built-in themes: the three step positions side by side on a 1 × 3 grid,
//! each over the points it steps through; labelled steps with NaN gaps, a dashed step beside a
//! solid one and a thick one, under each theme's legend style (so with direct labels at the
//! ends of the steps in Graphite). In single themes: a category x axis; a log x axis with the
//! middle position; explicit limits that cut a step (a direct label at the edge, at the level
//! the step has there, and a step that ends inside the limits, with its end dot); a step
//! beside a line and an area; and 200 000 points, downsampled before they are stepped.
//!
//! In `errors/step.txt`: every error branch of `Axes::step` in the order its checks run, which
//! fault is reported when an input has several, and the state a failed call leaves.

use umber_core::figure::{AreaOptions, AxisData, LineOptions, ScatterOptions, Step, StepOptions};
use umber_core::{
    Alpha, ColorSpec, Dash, Figure, FontLibrary, Grid, GridIndex, LegendStyle, Placement,
    ScaleKind, SizeSpec, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, linear, log, themes, wobble};

/// The first palette color, so that a step and its points share one.
fn first_color() -> ColorSpec {
    ColorSpec::Palette {
        index: 0,
        alpha: Alpha::new(1.0).unwrap(),
    }
}

fn labelled_step(label: &str, position: Step) -> StepOptions {
    StepOptions {
        position,
        label: label.into(),
        ..StepOptions::default()
    }
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "step.txt".into(),
        bytes: step_errors().into_bytes(),
    });
}

/// Every step figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();

    // The three positions over the points they step through, in every theme.
    let x: Vec<f64> = (0..8).map(f64::from).collect();
    let y = [2.0, 5.0, 4.0, 7.0, 6.0, 3.0, 5.0, 8.0];
    for (theme_name, theme) in themes() {
        let grid = Grid::new(1, 3).unwrap();
        let (width, height) = grid.auto_size_in();
        let mut f = Figure::with_grid(width, height, theme, grid).unwrap();
        for (col, (title, position)) in
            [("Post", Step::Post), ("Pre", Step::Pre), ("Mid", Step::Mid)]
                .into_iter()
                .enumerate()
        {
            let at = Placement {
                row: GridIndex::At(0),
                col: GridIndex::At(col as i64),
                row_span: 1,
                col_span: 1,
            };
            f.add_axes_at(at, linear(), linear())
                .unwrap()
                .title(title)
                .step(
                    AxisData::Numbers(x.clone()),
                    &y,
                    StepOptions {
                        position,
                        color: first_color(),
                        ..StepOptions::default()
                    },
                )
                .unwrap()
                .scatter(
                    AxisData::Numbers(x.clone()),
                    &y,
                    ScatterOptions {
                        color: first_color(),
                        ..ScatterOptions::default()
                    },
                )
                .unwrap();
        }
        out.push((format!("step-positions-{theme_name}"), f));
    }

    // Labelled steps with a NaN gap and an isolated point, a dashed step and a thick one.
    let nan = f64::NAN;
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Steps and gaps");
        let x: Vec<f64> = (0..14).map(f64::from).collect();
        axes(&mut f)
            .step(
                AxisData::Numbers(x.clone()),
                &[
                    3.0, 3.0, 4.5, 4.5, nan, 6.0, 5.0, nan, 2.0, nan, 4.0, 4.0, 5.5, 7.0,
                ],
                labelled_step("Demand", Step::Post),
            )
            .unwrap()
            .step(
                AxisData::Numbers(x.clone()),
                &[
                    1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 3.0, 2.5, 2.0, 1.5, 2.0, 2.5, 3.0, 3.5,
                ],
                StepOptions {
                    dash: Dash::Dashed,
                    ..labelled_step("Supply", Step::Mid)
                },
            )
            .unwrap()
            .step(
                AxisData::Numbers(x),
                &[
                    6.0, 6.5, 5.5, 5.0, 5.0, 4.5, 4.0, 4.5, 5.5, 6.5, 7.0, 7.5, 7.0, 6.0,
                ],
                StepOptions {
                    width: SizeSpec::times(2.0).unwrap(),
                    ..labelled_step("Price", Step::Pre)
                },
            )
            .unwrap();
        out.push((format!("step-gaps-{theme_name}"), f));
    }

    // A category x axis: the steps run between band centres, in the order given.
    let quarters = AxisData::Categories(["Q1", "Q2", "Q3", "Q4", "Q5"].map(String::from).to_vec());
    let mut f = figure(6.0, 4.0, Theme::graphite(), ScaleKind::Category, linear());
    labelled(&mut f, "Categories");
    axes(&mut f)
        .step(
            quarters.clone(),
            &[4.0, 6.0, 5.0, 8.0, 7.0],
            labelled_step("Budget", Step::Post),
        )
        .unwrap()
        .step(
            quarters,
            &[3.0, 3.5, 6.5, 6.0, 9.0],
            labelled_step("Actual", Step::Mid),
        )
        .unwrap();
    out.push(("step-category".to_owned(), f));

    // Steps that end in a NaN y: the line, its direct label and its end dot stop where the last
    // level stops, at the next x; on a category axis at the next band, or midway to it.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    labelled(&mut f, "Trailing gaps");
    for (k, (label, position)) in [("Post", Step::Post), ("Pre", Step::Pre), ("Mid", Step::Mid)]
        .into_iter()
        .enumerate()
    {
        axes(&mut f)
            .step(
                AxisData::Numbers(vec![0.0, 1.0, 4.0, 6.0]),
                &[
                    1.0 + 2.0 * k as f64,
                    2.0 + 2.0 * k as f64,
                    1.5 + 2.0 * k as f64,
                    nan,
                ],
                labelled_step(label, position),
            )
            .unwrap();
    }
    out.push(("step-trailing-gap".to_owned(), f));
    let mut f = figure(6.0, 4.0, Theme::graphite(), ScaleKind::Category, linear());
    labelled(&mut f, "Trailing gaps");
    for (k, (label, position)) in [("Post", Step::Post), ("Mid", Step::Mid)]
        .into_iter()
        .enumerate()
    {
        axes(&mut f)
            .step(
                AxisData::Categories(["a", "b", "c", "d"].map(String::from).to_vec()),
                &[
                    1.0 + 2.0 * k as f64,
                    2.0 + 2.0 * k as f64,
                    1.5 + 2.0 * k as f64,
                    nan,
                ],
                labelled_step(label, position),
            )
            .unwrap();
    }
    out.push(("step-trailing-gap-category".to_owned(), f));
    // The same cut by explicit limits: the held level crosses the high end.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    labelled(&mut f, "Trailing gap and limits");
    axes(&mut f)
        .step(
            AxisData::Numbers(vec![0.0, 1.0, 4.0]),
            &[1.0, 2.0, nan],
            labelled_step("Post", Step::Post),
        )
        .unwrap()
        .xlim(0.0, 3.0)
        .unwrap()
        .legend(LegendStyle::Direct);
    out.push(("step-trailing-gap-limits".to_owned(), f));

    // A log x axis: the middle position changes level midway in log space.
    let mut f = figure(6.0, 4.0, Theme::pigment(), log(10.0), linear());
    labelled(&mut f, "Log x");
    axes(&mut f)
        .step(
            AxisData::Numbers(vec![1.0, 10.0, 100.0, 1000.0]),
            &[1.0, 3.0, 2.0, 5.0],
            labelled_step("Tiers", Step::Mid),
        )
        .unwrap();
    out.push(("step-log-x".to_owned(), f));

    // Explicit limits: each position's step is cut at x = 5.5 by the limit, so its direct label
    // sits at the edge, at the level the step has there; the last step ends inside the limits.
    for (theme_name, theme) in [
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Explicit limits");
        let x: Vec<f64> = (0..10).map(|i| f64::from(i) * 1.2).collect();
        for (k, (label, position)) in [("Post", Step::Post), ("Pre", Step::Pre), ("Mid", Step::Mid)]
            .into_iter()
            .enumerate()
        {
            let y: Vec<f64> = (0..10)
                .map(|i| 2.0 * k as f64 + [1.0, 3.0, 2.0, 4.0, 3.5][i % 5])
                .collect();
            axes(&mut f)
                .step(
                    AxisData::Numbers(x.clone()),
                    &y,
                    labelled_step(label, position),
                )
                .unwrap();
        }
        axes(&mut f)
            .step(
                AxisData::Numbers(vec![0.0, 2.0, 4.5]),
                &[0.5, 1.0, 0.0],
                labelled_step("Ends", Step::Post),
            )
            .unwrap()
            .xlim(0.0, 5.5)
            .unwrap()
            .ylim(0.0, 12.0)
            .unwrap();
        out.push((format!("step-direct-limits-{theme_name}"), f));
    }

    // Beside a line and an area: the steps take palette colors in mark order.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    labelled(&mut f, "With a line and an area");
    let (x, y) = wobble(30, 71);
    let y: Vec<f64> = y.iter().map(|v| v + 6.0).collect();
    axes(&mut f)
        .area(
            AxisData::Numbers(x.clone()),
            &y,
            AreaOptions {
                label: "Level".into(),
                ..AreaOptions::default()
            },
        )
        .unwrap()
        .step(
            AxisData::Numbers(x.clone()),
            &y.iter().map(|v| v.round()).collect::<Vec<_>>(),
            labelled_step("Rounded", Step::Mid),
        )
        .unwrap()
        .line(
            AxisData::Numbers(x),
            &y.iter().map(|v| v - 2.0).collect::<Vec<_>>(),
            LineOptions {
                label: "Offset".into(),
                dash: Dash::Dotted,
                ..LineOptions::default()
            },
        )
        .unwrap()
        .legend(LegendStyle::Chips);
    out.push(("step-with-line-and-area".to_owned(), f));

    // Many points: reduced per pixel column first, then stepped.
    for (name, position) in [("post", Step::Post), ("mid", Step::Mid)] {
        let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
        let (x, y) = wobble(200_000, 81);
        axes(&mut f)
            .step(
                AxisData::Numbers(x),
                &y,
                StepOptions {
                    position,
                    ..StepOptions::default()
                },
            )
            .unwrap();
        out.push((format!("step-200k-points-{name}"), f));
    }

    out
}

/// Every error branch of `Axes::step`, in the order its checks run, and the state after each
/// call.
fn step_errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let nums = |v: &[f64]| AxisData::Numbers(v.to_vec());
    let cats = |v: &[&str]| AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect());
    let step = StepOptions::default;
    let mut rec = Log::default();

    rec.section("Axes.step, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "step ok", |a| {
        a.step(nums(&[0.0, 1.0, 3.0]), &[1.0, 2.0, 0.0], step())
            .map(drop)
    });
    let cases: Vec<(&str, AxisData, Vec<f64>)> = vec![
        ("length mismatch", nums(&[0.0]), vec![1.0, 2.0]),
        (
            "length mismatch and unsorted x",
            nums(&[2.0, 1.0]),
            vec![1.0],
        ),
        ("infinite y", nums(&[0.0]), vec![inf]),
        ("negative infinite x", nums(&[f64::NEG_INFINITY]), vec![1.0]),
        ("huge x", nums(&[1.0e301]), vec![1.0]),
        ("huge negative y", nums(&[0.0]), vec![-1.0e301]),
        ("categories on number axis", cats(&["a"]), vec![1.0]),
        (
            "unsorted x and infinite y",
            nums(&[2.0, 1.0]),
            vec![inf, 1.0],
        ),
        ("unsorted x", nums(&[0.0, 3.0, 2.0, 5.0, 1.0]), vec![0.0; 5]),
        (
            "unsorted x after a gap",
            nums(&[0.0, nan, -1.5]),
            vec![0.0; 3],
        ),
        ("unsorted x, large", nums(&[1.0e7, 1.0e6]), vec![0.0; 2]),
        (
            "first of two unsorted pairs",
            nums(&[1.0, 0.5, 0.25]),
            vec![0.0; 3],
        ),
    ];
    for (name, x, y) in cases {
        rec.on_axes(&mut f, &format!("step {name}"), |a| {
            a.step(x, &y, step()).map(drop)
        });
    }
    // Equal x, gaps in x and y, empty data, with non-canonical NaN payloads and signed zeros:
    // storage keeps every bit.
    let p0 = f64::from_bits(0x7ff8_0000_0000_0001);
    let p1 = f64::from_bits(0xfff0_0000_dead_beef);
    rec.on_axes(&mut f, "step with equal x and NaN payloads", |a| {
        a.step(
            nums(&[-0.0, 0.0, p0, 2.0, 2.0]),
            &[p1, 1.0, 0.0, -0.0, 3.0],
            step(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "step empty", |a| {
        a.step(nums(&[]), &[], step()).map(drop)
    });
    for (name, position) in [("post", Step::Post), ("pre", Step::Pre), ("mid", Step::Mid)] {
        rec.on_axes(&mut f, &format!("step with options {name}"), |a| {
            a.step(
                nums(&[umber_core::MAX_MAGNITUDE, umber_core::MAX_MAGNITUDE]),
                &[-1.0, 1.0],
                StepOptions {
                    position,
                    label: "edge".into(),
                    color: ColorSpec::Palette {
                        index: 21,
                        alpha: Alpha::new(0.25).unwrap(),
                    },
                    dash: Dash::Dashed,
                    width: SizeSpec::pt(3.0).unwrap(),
                },
            )
            .map(drop)
        });
    }

    rec.section("Axes.step, category/log10");
    let mut f = rec.new_figure("category/log10", ScaleKind::Category, log(10.0));
    for (name, x, y) in [
        ("numbers on category axis", nums(&[1.0]), vec![1.0]),
        ("zero y", cats(&["a"]), vec![0.0]),
        ("negative y", cats(&["a", "b"]), vec![5.0, -1.0]),
        ("NaN y", cats(&["a", "b"]), vec![nan, 3.0]),
        (
            "names in any order",
            cats(&["b", "a", "b"]),
            vec![1.0, 2.0, 3.0],
        ),
    ] {
        rec.on_axes(&mut f, &format!("step {name}"), |a| {
            a.step(x, &y, step()).map(drop)
        });
    }

    rec.section("Axes.step, log2/linear");
    let mut f = rec.new_figure("log2/linear", log(2.0), linear());
    for (name, x, y) in [
        ("zero x", nums(&[0.0]), vec![1.0]),
        ("unsorted positive x", nums(&[4.0, 2.0]), vec![1.0, 2.0]),
        ("sorted positive x", nums(&[2.0, 4.0]), vec![1.0, 2.0]),
    ] {
        rec.on_axes(&mut f, &format!("step {name}"), |a| {
            a.step(x, &y, step()).map(drop)
        });
    }
    rec.out
}
