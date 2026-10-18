//! Automatic linear limits follow the data, independently of the tick grid.

use super::*;
use crate::layout::{PlotSize, TickSpacing, tick_rooms};
use crate::scale::AnySpacing;
use crate::{BarOptions, Figure, FontLibrary, ScaleKind, ScatterOptions, Theme};
use proptest::prelude::*;

fn assert_grid(values: &[f64], domain: (f64, f64), target: usize) {
    assert!(values.len() >= 2, "{domain:?}: {values:?}");
    assert!(values.len() <= (3 * target).div_ceil(2), "{values:?}");
    let step = values[1] - values[0];
    let mantissa = step / 10f64.powf(step.log10().floor());
    assert!(
        [1.0, 2.0, 2.5, 5.0, 10.0]
            .iter()
            .any(|m| (mantissa - m).abs() < 1e-7),
        "step {step}: {values:?}"
    );
    for &value in values {
        let index = value / step;
        assert!((index - index.round()).abs() < 1e-7 * index.abs().max(1.0));
        assert!(
            domain.0 <= value && value <= domain.1,
            "{domain:?}: {values:?}"
        );
    }
    if domain.0 <= 0.0 && domain.1 >= 0.0 {
        assert!(values.contains(&0.0), "{domain:?}: {values:?}");
    }
}

#[test]
fn comparison_ranges_use_data_padding_and_inside_ticks() {
    for (low, high, horizontal) in [
        (11.0, 65.0, false),
        (284.7, 820.1, false),
        (5302.4, 10651.9, false),
        (-101.8, 123.8, false),
        (3452.8, 5127.9, true),
        (-2.047, 0.154, true),
    ] {
        let mut f = Figure::new(if horizontal { 3.2 } else { 6.0 }, 2.6, Theme::pigment()).unwrap();
        let values = vec![low, (low + high) / 2.0, high];
        let positions = vec![0.0, 1.0, 2.0];
        let (xs, ys) = if horizontal {
            (values, positions)
        } else {
            (positions, values)
        };
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(AxisData::Numbers(xs), &ys, ScatterOptions::default())
            .unwrap();
        let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let axis = if horizontal { &resolved.x } else { &resolved.y };
        let pad = AUTO_LIMIT_PADDING * (high - low);
        assert_eq!(axis.continuous_scale().domain(), (low - pad, high + pad));
        assert!(axis.ticks.len() >= 2, "{low}..{high}: {:?}", axis.ticks);
        assert!(
            axis.ticks
                .iter()
                .all(|t| (0.0..=1.0).contains(&t.pos.get()))
        );
        let values: Vec<f64> = axis
            .ticks
            .iter()
            .map(|tick| tick.label.replace('−', "-").parse().unwrap())
            .collect();
        let (x_target, y_target) = crate::layout::tick_targets(layout.plot.size());
        assert_grid(
            &values,
            axis.continuous_scale().domain(),
            if horizontal { x_target } else { y_target },
        );
    }
}

#[test]
fn positive_and_negative_bars_pad_only_the_far_end() {
    for sign in [-1.0, 1.0] {
        for horizontal in [false, true] {
            let mut f = Figure::new(6.0, 2.6, Theme::pigment()).unwrap();
            let (x, y) = if horizontal {
                (ScaleKind::Linear, ScaleKind::Category)
            } else {
                (ScaleKind::Category, ScaleKind::Linear)
            };
            f.add_axes(x, y)
                .unwrap()
                .bar(
                    vec!["a".into(), "b".into()],
                    &[11.0 * sign, 65.0 * sign],
                    BarOptions::default(),
                )
                .unwrap();
            let resolved = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).0;
            let domain = if horizontal { &resolved.x } else { &resolved.y }
                .continuous_scale()
                .domain();
            assert_eq!(
                domain,
                if sign > 0.0 {
                    (0.0, 68.25)
                } else {
                    (-68.25, 0.0)
                }
            );
        }
    }
}

#[test]
fn zero_scatter_endpoint_has_room_for_its_marker() {
    let mut f = Figure::new(6.0, 2.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 65.0],
            ScatterOptions::default(),
        )
        .unwrap();
    let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
    let scale = resolved.y.continuous_scale();
    assert_eq!(scale.domain(), (-3.25, 68.25));
    // The default marker has radius 3 pt; its center is farther from the lower edge.
    assert!(scale.normalize(0.0) * layout.plot.height > 3.0);
}

#[test]
fn a_zero_tick_is_not_snapped_to_a_nearby_padding_endpoint() {
    for (low, high) in [(-1e-76, 2.2e-59), (-2.2e-59, 1e-76)] {
        let scale = ContinuousScale::new(Transform::Linear, low, high, CONTEXT).unwrap();
        let ticks = scale.auto_ticks(3, &AnySpacing);
        let values: Vec<_> = ticks.ticks.iter().map(|tick| tick.value).collect();
        assert_grid(&values, scale.domain(), 3);
    }
}

#[test]
fn single_tick_labels_describe_their_own_value() {
    let theme = Theme::pigment();
    let (room, _) = tick_rooms(
        PlotSize {
            width: 1.0,
            height: 100.0,
        },
        crate::test_support::tick_font(&theme),
        TickSpacing::LabelsApart,
    );
    let judge = DrawnTicks {
        format: &TickFormat::AUTO,
        room: &room,
    };
    for (low, high, label) in [(-0.0004, 0.0004, "0"), (0.00151, 0.00169, "0.0016")] {
        let scale = ContinuousScale::new(Transform::Linear, low, high, CONTEXT).unwrap();
        let ticks = scale.auto_ticks(5, &judge);
        assert_eq!(ticks.ticks.len(), 1);
        assert_eq!(ticks.ticks[0].label, label);
    }
}

#[test]
fn fixed_decimals_choose_exactly_representable_steps() {
    let theme = Theme::pigment();
    let (room, _) = tick_rooms(
        PlotSize {
            width: 800.0,
            height: 200.0,
        },
        crate::test_support::tick_font(&theme),
        TickSpacing::LabelsApart,
    );
    for (factor, format) in [
        (1.0, TickFormat::fixed(0).unwrap()),
        (0.1, TickFormat::fixed(1).unwrap()),
        (0.01, TickFormat::percent(0).unwrap()),
        (0.001, TickFormat::percent(1).unwrap()),
    ] {
        let scale =
            ContinuousScale::new(Transform::Linear, -7.5 * factor, 7.5 * factor, CONTEXT).unwrap();
        let judge = DrawnTicks {
            format: &format,
            room: &room,
        };
        let ticks = scale.auto_ticks(5, &judge);
        assert!(ticks.ticks.len() >= 2);
        for tick in &ticks.ticks {
            let units = tick.value / factor;
            assert!(
                (units - units.round()).abs() < 1e-9,
                "{format:?}: {ticks:?}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn random_linear_limits_are_padded_data_with_inside_whole_ticks(
        start in -100.0f64..100.0,
        width in 0.01f64..100.0,
        exponent in -295i32..=295,
        length in prop_oneof![40.0f64..120.0, 180.0f64..1200.0],
    ) {
        let factor = 10f64.powi(exponent);
        let (low, high) = (start * factor, (start + width) * factor);
        let data = Some((low, high));
        let initial = ContinuousScale::from_extent(Transform::Linear, data, CONTEXT).unwrap();
        let expected = if is_single_value(Transform::Linear, low, high) {
            initial.domain()
        } else {
            let pad = AUTO_LIMIT_PADDING * (high - low);
            (low - pad, high + pad)
        };
        let theme = Theme::pigment();
        for spacing in [TickSpacing::Target, TickSpacing::LabelsApart] {
            let (x, y) = tick_rooms(PlotSize { width: length, height: length }, crate::test_support::tick_font(&theme), spacing);
            for room in [&x, &y] {
                let judge = DrawnTicks { format: &TickFormat::AUTO, room };
                let (scale, _, ticks) = automatic(Transform::Linear, None, data, AutoLimits::Padded(Padding::BothSides), room.target, &judge);
                prop_assert_eq!(scale.domain(), expected);
                let values: Vec<_> = ticks.ticks.iter().map(|tick| tick.value).collect();
                prop_assert!(judge.slack(&scale, &ticks) >= 0.0);
                if values.len() >= 2 {
                    assert_grid(&values, scale.domain(), room.target);
                } else {
                    prop_assert_eq!(values.len(), 1);
                    prop_assert!(expected.0 <= values[0] && values[0] <= expected.1);
                    if expected.0 <= 0.0 && expected.1 >= 0.0 { prop_assert_eq!(values[0], 0.0); }
                    // A single label is justified only when no whole-step pair fits this room.
                    for pair in scale.tick_pairs(&TickFormat::AUTO) {
                        prop_assert!(room.slack(&positioned(&scale, pair)) < 0.0);
                    }
                }
            }
        }
    }
}
