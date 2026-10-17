//! Automatic linear ticks keep a zero-aligned grid at every screen size.

use super::*;
use crate::layout::{PlotSize, TickSpacing, tick_rooms};
use crate::{
    BoxOptions, Figure, FontLibrary, Grid, GridIndex, Placement, ScaleKind, ScatterOptions, Theme,
    ViolinOptions,
};
use proptest::prelude::*;

fn assert_grid(values: &[f64], domain: (f64, f64)) {
    assert!(values.len() >= 2, "{values:?}");
    let step = values[1] - values[0];
    assert!(step.is_finite() && step > 0.0, "{values:?}");
    for &value in values {
        let index = value / step;
        assert!(
            (index - index.round()).abs() < 1e-8 * index.abs().max(1.0),
            "{values:?}, step {step}"
        );
    }
    if domain.0 <= 0.0 && domain.1 >= 0.0 {
        assert!(
            values.iter().any(|value| value.abs() < step * 1e-9),
            "{domain:?}: {values:?}"
        );
    }
}

fn check_panel(f: &Figure, col: usize, starts_at_zero: bool) {
    let resolved = crate::pipeline::plan_figure(f, (0, col), FontLibrary::global()).0;
    let scale = resolved.y.continuous_scale();
    let values: Vec<_> = resolved
        .y
        .ticks
        .iter()
        .map(|tick| scale.domain().0 + tick.pos.get() * (scale.domain().1 - scale.domain().0))
        .collect();
    assert_grid(&values, scale.domain());
    assert!(scale.domain().0 >= 0.0, "{:?}: {values:?}", scale.domain());
    if starts_at_zero {
        assert_eq!(scale.domain().0, 0.0, "{values:?}");
        assert_eq!(resolved.y.ticks[0].label.parse::<f64>().unwrap(), 0.0);
        assert_eq!(resolved.baseline.pos.get(), 0.0);
    }
}

#[test]
fn short_positive_scatter_ranges_start_on_whole_steps() {
    for (low, high, starts_at_zero) in [
        (11.0, 65.0, true),
        (0.11, 0.65, true),
        (1100.0, 6500.0, true),
        (21.0, 95.0, false),
        (51.0, 95.0, false),
    ] {
        let mut f = Figure::new(6.0, 2.6, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[low, (low + high) / 2.0, high],
                ScatterOptions::default(),
            )
            .unwrap();
        check_panel(&f, 0, starts_at_zero);
    }
}

#[test]
fn short_four_column_box_violin_and_scatter_start_at_zero() {
    let mut f = Figure::with_grid(16.0, 2.6, Theme::pigment(), Grid::new(1, 4).unwrap()).unwrap();
    for col in 0..4 {
        let mut a = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                if col < 2 {
                    ScaleKind::Category
                } else {
                    ScaleKind::Linear
                },
                ScaleKind::Linear,
            )
            .unwrap();
        let names = vec!["A".to_owned()];
        let groups = vec![vec![11.0, 20.0, 30.0, 40.0, 50.0, 65.0]];
        match col {
            0 => a.box_plot(names, groups, BoxOptions::default()).unwrap(),
            1 => a.violin(names, groups, ViolinOptions::default()).unwrap(),
            _ => a
                .scatter(
                    AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                    &[11.0, 40.0, 65.0],
                    ScatterOptions::default(),
                )
                .unwrap(),
        };
    }
    for col in 0..4 {
        check_panel(&f, col, true);
    }
}

#[test]
fn scatter_positions_keep_padding_while_nonnegative_values_stop_at_zero() {
    for start in [0.0, 0.1] {
        let mut f = Figure::new(6.0, 2.6, Theme::pigment()).unwrap();
        let xs: Vec<_> = (0..40).map(|i| f64::from(i) + start).collect();
        let ys: Vec<_> = (0..40).map(f64::from).collect();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(AxisData::Numbers(xs), &ys, ScatterOptions::default())
            .unwrap();
        let resolved = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).0;
        let domain = resolved.x.continuous_scale().domain();
        assert!(domain.0 < 0.0, "{domain:?}");
        let values: Vec<_> = resolved
            .x
            .ticks
            .iter()
            .map(|tick| domain.0 + tick.pos.get() * (domain.1 - domain.0))
            .collect();
        assert_grid(&values, domain);
        assert_eq!(resolved.x.ticks[0].label.parse::<f64>().unwrap(), 0.0);
        assert!(resolved.x.ticks[0].pos.get() > 0.0);
        check_panel(&f, 0, true);
    }
}

#[test]
fn only_value_axes_clamp_nonnegative_single_values() {
    let settings = AxisTicks::default();
    let limits = ContinuousLimits {
        transform: Transform::Linear,
        explicit: None,
        ticks: &settings,
    };
    let (_, room) = TickRoom::unlimited(5, 5, FontLibrary::global());
    for value in [0.0, 1e-290, 3.0, 1e290] {
        for auto in [
            AutoLimits::Exact,
            AutoLimits::Nice(Padding::BothSides),
            AutoLimits::NiceBelowData(Padding::BothSides),
        ] {
            let axis = continuous(limits, Some((value, value)), auto, &room);
            if matches!(auto, AutoLimits::NiceBelowData(_)) {
                assert!(
                    axis.scale.domain().0 >= 0.0,
                    "{value}: {:?}",
                    axis.scale.domain()
                );
            } else if value == 0.0 {
                assert!(
                    axis.scale.domain().0 < 0.0,
                    "{value}: {:?}",
                    axis.scale.domain()
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn automatic_linear_axes_keep_whole_steps_and_nonnegative_limits(
        start in -100i32..100,
        width in 1i32..100,
        exponent in -9i32..10,
        length in 40.0f64..1200.0,
    ) {
        let factor = 10f64.powi(exponent);
        let low = f64::from(start) * factor;
        let high = f64::from(start + width) * factor;
        let settings = AxisTicks::default();
        let limits = ContinuousLimits { transform: Transform::Linear, explicit: None, ticks: &settings };
        let theme = Theme::pigment();
        for spacing in [TickSpacing::Target, TickSpacing::Deferred, TickSpacing::LabelsApart] {
            let (x, y) = tick_rooms(PlotSize { width: length, height: length }, &theme, FontLibrary::global(), spacing);
            for room in [&x, &y] {
                for auto in [AutoLimits::Exact, AutoLimits::Nice(Padding::BothSides), AutoLimits::NiceBelowData(Padding::BothSides)] {
                    let axis = continuous(limits, Some((low, high)), auto, room);
                    let domain = axis.scale.domain();
                    let values: Vec<_> = axis.ticks.iter().map(|tick| domain.0 + tick.pos.get() * (domain.1 - domain.0)).collect();
                    assert_grid(&values, domain);
                    if low >= 0.0 && matches!(auto, AutoLimits::NiceBelowData(_)) {
                        prop_assert!(domain.0 >= 0.0, "data {low}..{high}, domain {domain:?}, ticks {values:?}");
                    }
                }
            }
        }
    }
}
