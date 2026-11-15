//! Integer ticks for integer data, and limits fixed at one end with the other automatic.

use super::*;
use crate::model::{AxesMut, LineOptions};
use crate::pipeline::plan_figure;
use crate::{
    BarOptions, Figure, FontLibrary, Grid, GridIndex, LimitEnd, LogBase, Placement, ScaleKind,
    ScatterOptions, Share, Theme,
};

/// The microseconds in a day.
const DAY: i64 = 86_400_000_000;

fn figure(x: ScaleKind, y: ScaleKind, build: impl FnOnce(&mut AxesMut<'_>)) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    build(&mut f.add_axes(x, y).unwrap());
    f
}

fn line(x: &[f64], y: &[f64]) -> impl FnOnce(&mut AxesMut<'_>) {
    let (x, y) = (x.to_vec(), y.to_vec());
    move |axes| {
        axes.line(AxisData::Numbers(x), &y, LineOptions::default())
            .unwrap();
    }
}

fn plan(f: &Figure) -> Resolved {
    plan_figure(f, (0, 0), FontLibrary::global()).0
}

fn labels(axis: &ResolvedAxis) -> Vec<&str> {
    axis.ticks.iter().map(|tick| tick.label.as_str()).collect()
}

fn values(axis: &ResolvedAxis) -> Vec<f64> {
    axis.ticks
        .iter()
        .map(|tick| tick.label.replace('\u{2212}', "-").parse().unwrap())
        .collect()
}

fn log10() -> ScaleKind {
    ScaleKind::Log { base: LogBase::TEN }
}

#[test]
fn integer_years_take_whole_steps_with_labels_without_decimals() {
    let years: Vec<f64> = (2015..=2026).map(f64::from).collect();
    let rates = vec![1.0; years.len()];
    for width in [2.5, 4.0, 6.0, 9.0] {
        let mut f = Figure::new(width, 3.0, Theme::pigment()).unwrap();
        line(&years, &rates)(&mut f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap());
        let r = plan(&f);
        assert_eq!(r.x.steps, TickSteps::Whole);
        let ticks = values(&r.x);
        assert!(ticks.len() >= 2, "{width}: {ticks:?}");
        assert!(ticks.iter().all(|v| v.fract() == 0.0), "{width}: {ticks:?}");
        assert!(labels(&r.x).iter().all(|l| !l.contains('.')), "{width}");
        let step = ticks[1] - ticks[0];
        assert!([1.0, 2.0, 5.0, 10.0].contains(&step), "{width}: {ticks:?}");
    }
}

#[test]
fn counts_on_a_value_axis_take_whole_steps() {
    let f = figure(ScaleKind::Category, ScaleKind::Linear, |axes| {
        axes.bar(
            ["a", "b", "c"].map(String::from).to_vec(),
            &[1.0, 3.0, 2.0],
            BarOptions::default(),
        )
        .unwrap();
    });
    let r = plan(&f);
    assert_eq!(r.y.steps, TickSteps::Whole);
    assert_eq!(values(&r.y), [0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn integer_data_spanning_one_keeps_integer_ticks() {
    let f = figure(
        ScaleKind::Linear,
        ScaleKind::Linear,
        line(&[0.0, 1.0, 2.0], &[0.0, 1.0, 0.0]),
    );
    assert_eq!(labels(&plan(&f).y), ["0", "1"]);
}

#[test]
fn a_domain_holding_fewer_than_two_integers_allows_round_steps() {
    // From 0.2 to 1.04, the domain holds the integer 1 alone.
    let f = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 1.0, 2.0], &[0.0, 1.0, 0.0])(axes);
        axes.ylim_end(LimitEnd::Low, 0.2).unwrap();
    });
    let r = plan(&f);
    assert_eq!(r.y.steps, TickSteps::Whole);
    assert_eq!(
        r.y.continuous_scale().domain(),
        (0.2, 1.0 + 0.8 * AUTO_LIMIT_PADDING)
    );
    let ticks = values(&r.y);
    assert!(ticks.len() >= 2, "{ticks:?}");
    assert!(ticks.iter().any(|v| v.fract() != 0.0), "{ticks:?}");
}

#[test]
fn one_value_off_an_integer_keeps_the_round_steps() {
    // A rounding error away from 2015 is not an integer, and a reference line counts.
    let near = figure(
        ScaleKind::Linear,
        ScaleKind::Linear,
        line(&[2_015.000_000_1, 2026.0], &[1.0, 2.0]),
    );
    assert_eq!(plan(&near).x.steps, TickSteps::Any);
    let reference = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 10.0], &[1.0, 2.0])(axes);
        axes.vline(AxisValue::Number(2.5), Default::default())
            .unwrap();
    });
    assert_eq!(plan(&reference).x.steps, TickSteps::Any);
}

#[test]
fn non_integer_data_takes_any_round_step() {
    let f = figure(
        ScaleKind::Linear,
        ScaleKind::Linear,
        line(&[0.0, 1.0, 2.0], &[0.25, 9.75, 4.5]),
    );
    let r = plan(&f);
    assert_eq!((r.x.steps, r.y.steps), (TickSteps::Whole, TickSteps::Any));
}

#[test]
fn shared_axes_take_whole_steps_only_when_every_panel_holds_integers() {
    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut f = Figure::with_grid(8.0, 3.0, Theme::pigment(), grid).unwrap();
    let at = |col| Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        ..Placement::default()
    };
    line(&[0.0, 7.0], &[1.0, 2.0])(
        &mut f
            .add_axes_at(at(0), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap(),
    );
    line(&[0.5, 7.25], &[1.0, 2.0])(
        &mut f
            .add_axes_at(at(1), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap(),
    );
    assert_eq!(plan(&f).x.steps, TickSteps::Any);
}

#[test]
fn explicit_limits_keep_any_steps_and_values_past_a_fixed_end_do_not_count() {
    let both = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 1.0, 2.0], &[0.0, 10.0, 5.0])(axes);
        axes.ylim(0.0, 10.0).unwrap();
    });
    assert_eq!(plan(&both).y.steps, TickSteps::Any);
    // 0.5 lies below the fixed end, so only the integers above it count.
    let one = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 1.0, 2.0], &[0.5, 10.0, 5.0])(axes);
        axes.ylim_end(LimitEnd::Low, 1.0).unwrap();
    });
    assert_eq!(plan(&one).y.steps, TickSteps::Whole);
}

#[test]
fn log_and_category_axes_keep_any_steps() {
    let f = figure(
        ScaleKind::Linear,
        log10(),
        line(&[0.0, 1.0, 2.0], &[1.0, 100.0, 1000.0]),
    );
    assert_eq!(plan(&f).y.steps, TickSteps::Any);
    let bars = figure(ScaleKind::Category, ScaleKind::Linear, |axes| {
        axes.bar(vec!["a".into()], &[1.0], BarOptions::default())
            .unwrap();
    });
    assert_eq!(plan(&bars).x.steps, TickSteps::Any);
}

#[test]
fn a_fixed_low_end_pads_only_the_automatic_end() {
    let f = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 1.0, 2.0], &[41.5, 47.8, 49.3])(axes);
        axes.ylim_end(LimitEnd::Low, 0.0).unwrap();
    });
    let pad = AUTO_LIMIT_PADDING * 49.3;
    assert_eq!(
        plan(&f).y.continuous_scale().domain(),
        (0.0, 49.3 + pad),
        "the span runs from the fixed end"
    );
}

#[test]
fn a_fixed_high_end_pads_only_the_automatic_end() {
    let f = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        axes.scatter(
            AxisData::Numbers(vec![2.0, 6.0]),
            &[1.0, 2.0],
            ScatterOptions::default(),
        )
        .unwrap()
        .xlim_end(LimitEnd::High, 4.0)
        .unwrap();
    });
    // The data beyond the fixed end do not count: the span is 2 to 4.
    let pad = AUTO_LIMIT_PADDING * 2.0;
    assert_eq!(plan(&f).x.continuous_scale().domain(), (2.0 - pad, 4.0));
}

#[test]
fn bars_under_a_fixed_top_keep_their_zero_rule() {
    let positive = figure(ScaleKind::Category, ScaleKind::Linear, |axes| {
        axes.bar(vec!["a".into()], &[5.0], BarOptions::default())
            .unwrap()
            .ylim_end(LimitEnd::High, 20.0)
            .unwrap();
    });
    assert_eq!(plan(&positive).y.continuous_scale().domain(), (0.0, 20.0));
    let mixed = figure(ScaleKind::Category, ScaleKind::Linear, |axes| {
        axes.bar(
            vec!["a".into(), "b".into()],
            &[12.5, -4.0],
            BarOptions::default(),
        )
        .unwrap()
        .ylim_end(LimitEnd::High, 20.0)
        .unwrap();
    });
    let pad = AUTO_LIMIT_PADDING * 24.0;
    assert_eq!(
        plan(&mixed).y.continuous_scale().domain(),
        (-4.0 - pad, 20.0)
    );
}

#[test]
fn lines_keep_their_exact_extent_from_a_fixed_end() {
    let f = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 10.0], &[1.0, 2.0])(axes);
        axes.xlim_end(LimitEnd::Low, -1.0).unwrap();
    });
    assert_eq!(plan(&f).x.continuous_scale().domain(), (-1.0, 10.0));
}

#[test]
fn no_value_beyond_the_fixed_end_reaches_as_a_single_value() {
    let flat = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 1.0], &[0.0, 0.0])(axes);
        axes.ylim_end(LimitEnd::Low, 0.0).unwrap();
    });
    assert_eq!(plan(&flat).y.continuous_scale().domain(), (0.0, 0.5));
    let empty = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        axes.ylim_end(LimitEnd::High, 20.0).unwrap();
    });
    assert_eq!(plan(&empty).y.continuous_scale().domain(), (19.0, 20.0));
}

#[test]
fn a_log_axis_keeps_its_fixed_end_and_rounds_the_other() {
    let f = figure(ScaleKind::Linear, log10(), |axes| {
        line(&[0.0, 1.0, 2.0], &[12.0, 900.0, 23_000.0])(axes);
        axes.ylim_end(LimitEnd::Low, 1.0).unwrap();
    });
    let r = plan(&f);
    let (low, high) = r.y.continuous_scale().domain();
    assert_eq!(low, 1.0);
    assert!(high >= 100_000.0, "{high}");
    assert_eq!(labels(&r.y)[0], "1");
}

#[test]
fn a_time_axis_runs_from_its_fixed_start() {
    let start = Instant::from_micros(20_454 * DAY).unwrap();
    let days: Vec<Option<Instant>> = (3..=9)
        .map(|k| Some(Instant::from_micros((20_454 + k) * DAY).unwrap()))
        .collect();
    let last = days[6].unwrap();
    let lines = figure(ScaleKind::Time, ScaleKind::Linear, |axes| {
        axes.line_shared(
            AxisData::Times(days.clone()),
            AxisData::Numbers(vec![1.0; 7]),
            LineOptions::default(),
        )
        .unwrap()
        .xlim_time_end(LimitEnd::Low, start)
        .unwrap();
    });
    let AxisScale::Time(scale) = &plan(&lines).x.scale else {
        panic!("a time axis")
    };
    assert_eq!(scale.instants(), (start, last), "lines keep their extent");

    let points = figure(ScaleKind::Time, ScaleKind::Linear, |axes| {
        axes.scatter_shared(
            AxisData::Times(days.clone()),
            AxisData::Numbers(vec![1.0; 7]),
            ScatterOptions::default(),
        )
        .unwrap()
        .xlim_time_end(LimitEnd::High, last)
        .unwrap();
    });
    let AxisScale::Time(scale) = &plan(&points).x.scale else {
        panic!("a time axis")
    };
    let (low, high) = scale.instants();
    assert_eq!(high, last);
    let span = last.micros() - days[0].unwrap().micros();
    let padded = last.micros() - span - span / 20;
    assert!((low.micros() - padded).abs() <= 1, "{low:?}");
}

#[test]
fn every_value_past_the_fixed_end_fails_the_render() {
    let low = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 1.0], &[2.0, 3.0])(axes);
        axes.ylim_end(LimitEnd::Low, 10.0).unwrap();
    });
    let err = low.to_scene(FontLibrary::global()).unwrap_err();
    match &err {
        Error::Style {
            context, message, ..
        } => {
            assert_eq!(*context, "Figure.to_scene()");
            assert_eq!(
                message,
                "the y limits are (10, AUTO), and every y value lies below 10, the nearest at 3; \
                 the lower limit must leave some data on the plot."
            );
        }
        other => panic!("expected a style error, got {other:?}"),
    }
    assert_eq!(
        err.hint(),
        Some("lower the limit to reach the data, or set both limits with ylim().")
    );

    let high = figure(ScaleKind::Time, ScaleKind::Linear, |axes| {
        axes.line_shared(
            AxisData::Times(vec![Some(Instant::from_micros(0).unwrap())]),
            AxisData::Numbers(vec![1.0]),
            LineOptions::default(),
        )
        .unwrap()
        .xlim_time_end(LimitEnd::High, Instant::from_micros(-DAY).unwrap())
        .unwrap();
    });
    let err = high.to_scene(FontLibrary::global()).unwrap_err();
    assert!(
        err.to_string().contains("every x value lies above"),
        "{err}"
    );
    // A value at the fixed end leaves the plot a single value's reach.
    let at_end = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        line(&[0.0, 1.0], &[2.0, 3.0])(axes);
        axes.ylim_end(LimitEnd::Low, 3.0).unwrap();
    });
    assert!(at_end.to_scene(FontLibrary::global()).is_ok());
}

#[test]
fn a_grid_names_the_cell_whose_fixed_end_leaves_no_data() {
    let grid = Grid::new(1, 2).unwrap();
    let mut f = Figure::with_grid(8.0, 3.0, Theme::pigment(), grid).unwrap();
    let at = |col| Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        ..Placement::default()
    };
    line(&[0.0, 1.0], &[1.0, 2.0])(
        &mut f
            .add_axes_at(at(0), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap(),
    );
    let mut axes = f
        .add_axes_at(at(1), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    line(&[0.0, 1.0], &[1.0, 2.0])(&mut axes);
    axes.xlim_end(LimitEnd::High, -5.0).unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(
        err.to_string()
            .contains("row 0, col 1: the x limits are (AUTO, -5)"),
        "{err}"
    );
}

#[test]
fn bar_labels_widen_the_automatic_end_of_a_one_sided_limit() {
    let f = figure(ScaleKind::Category, ScaleKind::Linear, |axes| {
        axes.bar(
            ["a", "b", "c"].map(String::from).to_vec(),
            &[1.0, 3.0, 2.0],
            BarOptions {
                annotation: crate::ValueLabels {
                    annotate: true,
                    ..crate::ValueLabels::default()
                },
                ..BarOptions::default()
            },
        )
        .unwrap()
        .ylim_end(LimitEnd::Low, 0.0)
        .unwrap();
    });
    assert!(f.to_scene(FontLibrary::global()).is_ok());
    let (low, high) = plan(&f).y.continuous_scale().domain();
    assert_eq!(low, 0.0);
    assert!(high > 3.0 * (1.0 + AUTO_LIMIT_PADDING), "{high}");
}

#[test]
fn bubbles_widen_only_the_automatic_end() {
    let f = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        axes.bubbles(
            &[0.0, 5.0, 10.0],
            &[1.0, 2.0, 3.0],
            &[400.0, 400.0, 400.0],
            Default::default(),
        )
        .unwrap()
        .xlim_end(LimitEnd::Low, 0.0)
        .unwrap();
    });
    let (low, high) = plan(&f).x.continuous_scale().domain();
    // The bubble at the fixed end is clipped; the one at 10 is held whole.
    assert_eq!(low, 0.0);
    assert!(high > 10.0 * (1.0 + AUTO_LIMIT_PADDING), "{high}");
    assert!(f.to_scene(FontLibrary::global()).is_ok());
}

#[test]
fn a_hexbin_keeps_its_fixed_end_and_whole_cells_at_the_other() {
    let x: Vec<f64> = (0..20).map(f64::from).collect();
    let y: Vec<f64> = (0..20).map(|i| f64::from(i % 7)).collect();
    for end in [LimitEnd::Low, LimitEnd::High] {
        let f = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
            axes.hexbin(
                &x,
                &y,
                crate::HexbinOptions {
                    gridsize: 7,
                    ..crate::HexbinOptions::default()
                },
            )
            .unwrap();
            match end {
                LimitEnd::Low => axes.xlim_end(LimitEnd::Low, 3.0).unwrap(),
                LimitEnd::High => axes.xlim_end(LimitEnd::High, 15.0).unwrap(),
            };
        });
        let (low, high) = plan(&f).x.continuous_scale().domain();
        // A lattice unit is 19 / 7 along x; the outer cells reach half a unit past their
        // points.
        let half = 0.5 * 19.0 / 7.0;
        match end {
            LimitEnd::Low => {
                assert_eq!(low, 3.0);
                assert!(high >= 19.0 + half - 1e-9, "{high}");
            }
            LimitEnd::High => {
                assert_eq!(high, 15.0);
                assert!(low <= -half + 1e-9, "{low}");
            }
        }
        assert!(f.to_scene(FontLibrary::global()).is_ok());
    }
}

#[test]
fn a_hexbin_counts_only_its_paired_points_for_steps() {
    let paired = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        axes.hexbin(&[0.0, 1.0], &[0.0, 1.0], crate::HexbinOptions::default())
            .unwrap();
    });
    assert_eq!(plan(&paired).x.steps, TickSteps::Whole);
    let unpaired = figure(ScaleKind::Linear, ScaleKind::Linear, |axes| {
        axes.hexbin(
            &[0.0, 1.0, 0.25],
            &[0.0, 1.0, f64::NAN],
            crate::HexbinOptions::default(),
        )
        .unwrap();
    });
    assert_eq!(plan(&unpaired).x.steps, TickSteps::Whole);
}

#[test]
fn time_cells_keep_their_exact_end_beside_other_marks_from_a_fixed_end() {
    let day = |k: i64| Instant::from_micros((20_454 + k) * DAY).unwrap();
    let build = |fixed: bool| {
        figure(ScaleKind::Time, ScaleKind::Linear, |axes| {
            axes.mesh_time(
                AxisData::Times(vec![Some(day(0)), Some(day(10))]),
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0],
                crate::MeshOptions::default(),
            )
            .unwrap()
            .line_shared(
                AxisData::Times(vec![Some(day(2)), Some(day(8))]),
                AxisData::Numbers(vec![0.25, 0.75]),
                LineOptions::default(),
            )
            .unwrap();
            if fixed {
                axes.xlim_time_end(LimitEnd::Low, day(0)).unwrap();
            }
        })
    };
    let instants = |f: &Figure| match &plan(f).x.scale {
        AxisScale::Time(scale) => scale.instants(),
        other => panic!("a time axis, not {other:?}"),
    };
    assert_eq!(instants(&build(false)), (day(0), day(10)));
    assert_eq!(instants(&build(true)), (day(0), day(10)));
}

#[test]
fn a_fixed_end_without_room_for_the_other_is_refused() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, log10()).unwrap();
    for (end, value) in [(LimitEnd::Low, 1e300), (LimitEnd::High, -1e300)] {
        let err = axes.xlim_end(end, value).unwrap_err();
        assert!(matches!(err, Error::Style { .. }), "{err}");
        assert_eq!(err.parameter(), Some(end.name()));
        assert_eq!(
            err.hint(),
            Some("set both limits, or move this one away from the end of the range.")
        );
    }
    assert_eq!(
        axes.xlim_end(LimitEnd::Low, 1e300).unwrap_err().to_string(),
        "Axes.xlim(): low is 1e300; it leaves no room for an automatic limit above it within \
         the supported values."
    );
    for (end, value) in [(LimitEnd::Low, 1e300), (LimitEnd::High, 1e-300)] {
        assert!(axes.ylim_end(end, value).is_err());
    }
    // A little room is enough: the fixed end is always kept.
    axes.xlim_end(LimitEnd::Low, 9.9e299).unwrap();
    axes.line(
        AxisData::Numbers(vec![1e300, 1e300]),
        &[1.0, 2.0],
        LineOptions::default(),
    )
    .unwrap();
    let (low, high) = plan(&f).x.continuous_scale().domain();
    assert_eq!((low, high), (9.9e299, crate::MAX_MAGNITUDE));
    let mut t = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut time = t.add_axes(ScaleKind::Time, ScaleKind::Linear).unwrap();
    assert!(time.xlim_time_end(LimitEnd::Low, Instant::MAX).is_err());
    assert!(time.xlim_time_end(LimitEnd::High, Instant::MIN).is_err());
    assert!(time.xlim_time_end(LimitEnd::High, Instant::MAX).is_ok());
}
