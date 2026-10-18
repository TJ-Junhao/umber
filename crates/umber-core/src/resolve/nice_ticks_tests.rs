//! Automatic linear ticks keep a zero-aligned grid at every screen size.

use super::*;
use crate::layout::{PlotSize, TickSpacing, tick_rooms};
use crate::{
    BoxOptions, Figure, FontLibrary, Grid, GridIndex, Placement, ScaleKind, ScatterOptions, Theme,
    ViolinOptions,
};
use proptest::prelude::*;

fn assert_grid(values: &[f64], domain: (f64, f64)) {
    assert!(!values.is_empty(), "{values:?}");
    if values.len() == 1 {
        if domain.0 <= 0.0 && domain.1 >= 0.0 {
            assert!(
                values[0].abs() < (domain.1 - domain.0) * 1e-9,
                "{domain:?}: {values:?}"
            );
        }
        return;
    }
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
    assert!(values[0] >= -1e-9 * (values[1] - values[0]), "{values:?}");
    if starts_at_zero {
        assert!(scale.domain().0 < 0.0, "{values:?}");
        assert_eq!(resolved.y.ticks[0].label.parse::<f64>().unwrap(), 0.0);
        assert!(resolved.y.ticks[0].pos.get() > 0.0);
    }
}

#[test]
fn short_positive_scatter_ranges_start_on_whole_steps() {
    for (low, high, starts_at_zero) in [
        (11.0, 65.0, false),
        (0.11, 0.65, false),
        (1100.0, 6500.0, false),
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
fn short_four_column_box_violin_and_scatter_follow_the_data() {
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
        check_panel(&f, col, false);
        let resolved = crate::pipeline::plan_figure(&f, (0, col), FontLibrary::global()).0;
        assert_eq!(resolved.y.continuous_scale().domain(), (8.3, 67.7));
    }
}

#[test]
fn zero_endpoint_scatter_domains_are_mirrored_and_padded() {
    let domains: Vec<_> = [[0.0, 17.0, 31.0], [-31.0, -17.0, 0.0]]
        .into_iter()
        .map(|ys| {
            let mut f = Figure::new(6.0, 2.6, Theme::pigment()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .scatter(
                    AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                    &ys,
                    ScatterOptions::default(),
                )
                .unwrap();
            let resolved = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).0;
            resolved.y.continuous_scale().domain()
        })
        .collect();
    assert!(domains[0].0 < 0.0, "{domains:?}");
    assert_eq!(domains[0], (-domains[1].1, -domains[1].0));
}

#[test]
fn scatter_positions_and_values_keep_padding_below_zero() {
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
fn single_values_keep_padding_and_whole_step_ticks() {
    let settings = AxisTicks::default();
    let limits = ContinuousLimits {
        transform: Transform::Linear,
        explicit: None,
        ticks: &settings,
    };
    let tick = crate::test_support::tick_font(&crate::Theme::pigment());
    let (_, room) = TickRoom::unlimited(5, 5, tick);
    for value in [-1e295, -3.0, -1e-295, 0.0, 1e-295, 1e-290, 3.0, 1e295] {
        for auto in [AutoLimits::Exact, AutoLimits::Padded(Padding::BothSides)] {
            let axis = continuous(limits, Some((value, value)), auto, &room);
            let domain = axis.scale.domain();
            assert!(domain.0 < value && value < domain.1, "{value}: {domain:?}");
            let values: Vec<_> = axis
                .ticks
                .iter()
                .map(|tick| domain.0 + tick.pos.get() * (domain.1 - domain.0))
                .collect();
            assert_grid(&values, domain);
            if value.abs() < 1e-290 {
                assert!(domain.0 < 0.0 && domain.1 > 0.0, "{value}: {domain:?}");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn automatic_linear_axes_keep_whole_steps_and_data_padding(
        start in -100i32..100,
        width in prop_oneof![Just(0i32), 1i32..100],
        exponent in -295i32..=295,
        length in 40.0f64..1200.0,
    ) {
        let factor = 10f64.powi(exponent);
        let low = f64::from(start) * factor;
        let high = f64::from(start + width) * factor;
        let settings = AxisTicks::default();
        let limits = ContinuousLimits { transform: Transform::Linear, explicit: None, ticks: &settings };
        let theme = Theme::pigment();
        for spacing in [TickSpacing::Target, TickSpacing::Deferred, TickSpacing::LabelsApart] {
            let (x, y) = tick_rooms(PlotSize { width: length, height: length }, crate::test_support::tick_font(&theme), spacing);
            for room in [&x, &y] {
                for auto in [AutoLimits::Exact, AutoLimits::Padded(Padding::BothSides)] {
                    let axis = continuous(limits, Some((low, high)), auto, room);
                    let domain = axis.scale.domain();
                    let values: Vec<_> = axis.ticks.iter().map(|tick| domain.0 + tick.pos.get() * (domain.1 - domain.0)).collect();
                    assert_grid(&values, domain);
                    let expanded = ContinuousScale::from_extent(Transform::Linear, Some((low, high)), CONTEXT).unwrap();
                    let expected = if auto == AutoLimits::Exact || is_single_value(Transform::Linear, low, high) {
                        expanded
                    } else {
                        expanded.padded(Padding::BothSides)
                    };
                    prop_assert_eq!(domain, expected.domain());
                }
            }
        }
    }
}
