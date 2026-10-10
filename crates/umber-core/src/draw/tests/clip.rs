//! Clipping in axis space: bounded output on every axis combination, subpath splitting,
//! crossings at extreme magnitudes, and agreement with the exact oracle.

use super::super::clip::{Bounds, clip_polyline, crossing, crossing_offset_fast};
use super::super::mapping::{AxisMaps, AxisPoint};
use super::{Pixels, line_figure, near, nums, oracle, plot_of, subpaths};
use crate::geometry::Point;
use crate::model::{AxisData, Figure, LineOptions, ScatterOptions};
use crate::resolve::NormalizedPos;
use crate::scale::log_offset;
use crate::scene::Item;
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, ScaleKind, Theme};

#[test]
fn narrow_limits_stay_bounded_on_every_axis_combination() {
    let log = ScaleKind::Log {
        base: crate::LogBase::TEN,
    };
    let mut state = 151_010_u64;
    let mut rng = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    for xkind in [ScaleKind::Linear, log, ScaleKind::Category] {
        for ykind in [ScaleKind::Linear, log] {
            let mut visible = 0;
            for _ in 0..50 {
                let (a, b) = (0.5 + rng() * 1.5, 0.5 + rng() * 1.5);
                let x = match xkind {
                    ScaleKind::Linear => nums(&[-1e60 * a, 1e60 * b]),
                    ScaleKind::Category => AxisData::Categories(vec!["a".into(), "b".into()]),
                    _ if ykind == ScaleKind::Linear => nums(&[1.0 + 0.3e-8, 1.0 + 0.7e-8]),
                    _ => nums(&[1e-300, 1e300]),
                };
                let y = match (xkind, ykind) {
                    (ScaleKind::Linear, ScaleKind::Linear) => [0.5e-50; 2],
                    (ScaleKind::Linear, _) => [1.0 + 0.5e-8; 2],
                    (_, ScaleKind::Linear) => [-1e60 * b, 1e60 * a],
                    _ => [1e-300, 1e300],
                };
                let mut f = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
                let axes = f.add_axes(xkind, ykind).unwrap();
                axes.line(x.clone(), &y, LineOptions::default())
                    .unwrap()
                    .scatter(x, &y, ScatterOptions::default())
                    .unwrap();
                match xkind {
                    ScaleKind::Linear => drop(axes.xlim(0.0, 1e-50).unwrap()),
                    ScaleKind::Category => {}
                    _ => drop(axes.xlim(1.0, 1.0 + 1e-8).unwrap()),
                }
                if ykind == ScaleKind::Linear {
                    axes.ylim(0.0, 1e-50).unwrap();
                } else {
                    axes.ylim(1.0, 1.0 + 1e-8).unwrap();
                }
                let scene = f.to_scene(FontLibrary::global());
                assert_scene_bounded(&scene);
                if scene.items.len() == 2 {
                    visible += 1;
                }
            }
            // Every one of these lines crosses the plot area.
            assert_eq!(visible, 50, "{xkind:?} / {ykind:?}");
        }
    }
}

#[test]
fn subpath_splitting_edge_cases() {
    let bounds = Bounds {
        left: 10.0,
        top: 20.0,
        right: 410.0,
        bottom: 260.0,
    };
    // A zero-length run on a corner, then a run that leaves through the right edge and
    // comes back.
    let parts = clip_polyline(
        &[10.0, 10.0, f64::NAN, 30.0, 500.0, 30.0],
        &[20.0, 20.0, 0.0, 100.0, 100.0, 200.0],
        &bounds,
    );
    let p = AxisPoint::new;
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], [p(10.0, 20.0), p(10.0, 20.0)]);
    assert_eq!(parts[1], [p(30.0, 100.0), p(410.0, 100.0)]);
    assert_eq!(parts[2].first().map(|p| p.x), Some(410.0));
    assert_eq!(parts[2].last(), Some(&p(30.0, 200.0)));
    // NaN in either coordinate breaks the run; a lone inside point survives.
    let parts = clip_polyline(&[10.0, f64::NAN, 10.0], &[f64::NAN, 30.0, 20.0], &bounds);
    assert_eq!(parts, [vec![p(10.0, 20.0)]]);
    // A lone point outside is dropped.
    assert!(clip_polyline(&[0.0], &[0.0], &bounds).is_empty());
}

/// The axis maps and the line clip box (in axis space) of `figure`.
fn axis_clip(figure: &Figure) -> (AxisMaps, Bounds) {
    let (resolved, layout) = crate::pipeline::plan(
        figure.width_in() * 72.0,
        figure.height_in() * 72.0,
        figure.theme(),
        figure.axes().unwrap(),
        FontLibrary::global(),
    );
    let maps = AxisMaps::new(&resolved.x, &resolved.y, layout.plot);
    let margin = f64::from(figure.theme().line_width()).max(2.0);
    let ((left, right), (top, bottom)) = (maps.x.expanded(margin), maps.y.expanded(margin));
    let bounds = Bounds {
        left,
        top,
        right,
        bottom,
    };
    // The clip box always contains the axis domain.
    assert!(left <= maps.x.lo && right >= maps.x.hi);
    assert!(top <= maps.y.lo && bottom >= maps.y.hi);
    (maps, bounds)
}

/// The axis-space value of a log or linear datum, on an axis whose lower limit is `lo`.
fn axis_value(kind: ScaleKind, v: f64, lo: f64) -> f64 {
    if matches!(kind, ScaleKind::Log { .. }) {
        log_offset(v, lo)
    } else {
        v
    }
}

/// Checks the one visible piece of the 2-point line in `figure` against the exact oracle,
/// given the line's axis-space endpoints. Returns the largest endpoint error in points when
/// it is visible.
#[track_caller]
fn check_against_oracle(figure: &Figure, a: AxisPoint, b: AxisPoint) -> Option<f64> {
    let scene = figure.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let (maps, bounds) = axis_clip(figure);
    let want = oracle::exact_plot(a, b, &bounds, maps);
    let got: Vec<Vec<Point>> = match scene.items.as_slice() {
        [_, Item::Group { items, .. }] => items
            .iter()
            .filter_map(|item| match item {
                Item::Stroke { path, .. } => Some(subpaths(path)),
                _ => None,
            })
            .flatten()
            .collect(),
        _ => Vec::new(),
    };
    match (want, got.as_slice()) {
        (None, []) => None,
        // A rounding-level corner touch of the expanded box: a single point, outside the
        // plot clip.
        (None, [piece]) if piece.windows(2).all(|w| w[0] == w[1]) => None,
        (Some((start, end)), [piece]) => {
            let [p, q] = piece.as_slice() else {
                panic!("expected a 2-point piece, got {piece:?}")
            };
            let mut worst = 0.0_f64;
            for (got, exact) in [(p, start), (q, end)] {
                let (ex, ey) = (exact.x, exact.y);
                let error = (f64::from(got.x) - ex)
                    .abs()
                    .max((f64::from(got.y) - ey).abs());
                assert!(
                    error < 1e-3,
                    "{a:?}..{b:?}: got {got:?}, exact ({ex}, {ey})"
                );
                worst = worst.max(error);
            }
            Some(worst)
        }
        (want, got) => panic!("{a:?}..{b:?}: exact {want:?}, got {got:?}"),
    }
}

#[test]
fn crossings_survive_underflow() {
    let next_up = |v: f64| f64::from_bits(v.to_bits() + 1);
    let next_down = |v: f64| f64::from_bits(v.to_bits() - 1);
    // Underflow cases (exact on the per-axis-scaled fast path),
    // and a case only the exact path can do: an edge 1e-200 against x data at 1e300.
    let cases = [
        (
            (10.0, 100.0),
            (next_up(next_up(10.0)), 1e308),
            10.0,
            140.0,
            100.0,
        ),
        (
            (next_down(10.0), -1e308),
            (next_up(10.0), 1e308),
            10.0,
            140.0,
            0.0,
        ),
        // Limits 1e-200 wide against data at 1e300.
        ((-1e300, -1e299), (1e300, 1e299), 1e-200, 0.0, 1e-201),
    ];
    for (a, b, edge, r, expected) in cases {
        let y = crossing(a, b, edge, r).unwrap();
        assert!(
            (y - expected).abs() <= 1e-15 * expected.abs().max(1e-300),
            "{y} vs {expected}"
        );
    }
    let (a, b, edge, r, _) = cases[2];
    assert!(crossing_offset_fast(a, b, edge, r).is_none());
    // Ordinary operands take the fast path.
    assert!(crossing_offset_fast((0.0, 0.0), (10.0, 5.0), 4.0, 1.0).is_some());
}

#[test]
fn overflowing_maps_keep_the_true_entry() {
    // Data at 1e296 to 1e300 with limits 0 to 1e-10: mapping the far end to points
    // overflows x only (1e296), or x and y (1e300). The line is y = x / 10 far out and
    // passes (5e-11, 5e-11).
    for (x0, y0) in [(-1e295, -1e294), (-1e296, -1e295), (-1e300, -1e299)] {
        let theme = Theme::graphite();
        let mut f = line_figure(
            theme.clone(),
            &[x0, 5e-11],
            &[y0, 5e-11],
            LineOptions::default(),
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1e-10)
            .unwrap()
            .ylim(0.0, 1e-10)
            .unwrap();
        assert!(
            check_against_oracle(&f, AxisPoint::new(x0, y0), AxisPoint::new(5e-11, 5e-11))
                .is_some()
        );

        // The true line shows at normalized (0.1, 0.46) in a 300 dpi render.
        let plot = plot_of(&f);
        let pixels = Pixels::decode(&f.to_png(300).unwrap());
        let at = pixels.at_point(
            (
                plot.x_at(NormalizedPos::new(0.1)),
                plot.y_at(NormalizedPos::new(0.46)),
            ),
            300.0,
        );
        assert!(near(at, theme.palette()[0], 8), "{x0}: {at:?}");
    }
}

#[test]
fn axis_space_clipping_matches_the_oracle_on_every_axis_combination() {
    let log = ScaleKind::Log {
        base: crate::LogBase::TEN,
    };
    let mut state = 20_261_011_u64;
    let mut rng = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    let mut u = move |lo: f64, hi: f64| lo + (hi - lo) * rng();
    let mut summary = Vec::new();
    for xkind in [ScaleKind::Linear, log, ScaleKind::Category] {
        for ykind in [ScaleKind::Linear, log] {
            let (mut cases, mut visible, mut max_error) = (0, 0, 0.0_f64);
            for case in 0..250 {
                // Limits: linear ones as narrow as 1e-200 (around 0 when that narrow, so
                // they are representable), log ones with ratios down to 1 + 1e-8.
                // Limits down to the narrowest allowed: linear ranges of 1e-10 of their
                // magnitude (or 1e-290 around 0), log ratios of 1 + 1e-9.
                let limits = |kind: ScaleKind, u: &mut dyn FnMut(f64, f64) -> f64| {
                    if matches!(kind, ScaleKind::Log { .. }) {
                        let lo = 10f64.powf(u(-200.0, 200.0));
                        (lo, lo * (1.0 + 1.01 * 10f64.powf(u(-9.0, 3.0))))
                    } else if u(0.0, 1.0) < 0.3 {
                        let width = 10f64.powf(u(-290.0, 3.0));
                        (-width * u(0.5, 1.0), width * u(0.5, 1.0))
                    } else {
                        let sign = if u(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
                        let center = sign * 10f64.powf(u(-280.0, 280.0));
                        let relative = 1.01 * 10f64.powf(u(-10.0, 0.0));
                        let half = center.abs() * relative / 2.0;
                        (center - half, center + half)
                    }
                };
                let xlim = limits(xkind, &mut u);
                let ylim = limits(ykind, &mut u);
                // A datum inside the limits, and one anywhere in the valid range.
                let inside = |kind: ScaleKind, (lo, hi): (f64, f64), t: f64| {
                    if matches!(kind, ScaleKind::Log { .. }) {
                        (lo.ln() + t * (hi.ln() - lo.ln())).exp().clamp(lo, hi)
                    } else {
                        lo + t * (hi - lo)
                    }
                };
                let far = |kind: ScaleKind, u: &mut dyn FnMut(f64, f64) -> f64| {
                    let magnitude = 10f64.powf(u(-300.0, 300.0));
                    if matches!(kind, ScaleKind::Log { .. }) {
                        magnitude
                    } else if u(0.0, 1.0) < 0.5 {
                        -magnitude
                    } else {
                        magnitude
                    }
                };
                let (x, y): ([f64; 2], [f64; 2]) = match case % 3 {
                    // Inside to far.
                    0 => (
                        [inside(xkind, xlim, u(0.0, 1.0)), far(xkind, &mut u)],
                        [inside(ykind, ylim, u(0.0, 1.0)), far(ykind, &mut u)],
                    ),
                    // Both far, anywhere.
                    1 => (
                        [far(xkind, &mut u), far(xkind, &mut u)],
                        [far(ykind, &mut u), far(ykind, &mut u)],
                    ),
                    // Far in y only: a steep line across the plot.
                    _ => (
                        [
                            inside(xkind, xlim, u(0.0, 1.0)),
                            inside(xkind, xlim, u(0.0, 1.0)),
                        ],
                        [
                            -far(ScaleKind::Linear, &mut u).abs().min(1e300),
                            far(ykind, &mut u).abs(),
                        ],
                    ),
                };
                let y = if matches!(ykind, ScaleKind::Log { .. }) {
                    y.map(|v| v.abs().clamp(1e-300, 1e300))
                } else {
                    y
                };
                let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
                let axes = f.add_axes(xkind, ykind).unwrap();
                let (xdata, xa, xb) = if xkind == ScaleKind::Category {
                    // Two categories: band centers 0.25 and 0.75.
                    (
                        AxisData::Categories(vec!["a".into(), "b".into()]),
                        0.25,
                        0.75,
                    )
                } else {
                    let x = if matches!(xkind, ScaleKind::Log { .. }) {
                        x.map(|v| v.clamp(1e-300, 1e300))
                    } else {
                        x
                    };
                    (
                        nums(&x),
                        axis_value(xkind, x[0], xlim.0),
                        axis_value(xkind, x[1], xlim.0),
                    )
                };
                if axes.line(xdata, &y, LineOptions::default()).is_err()
                    || (xkind != ScaleKind::Category && axes.xlim(xlim.0, xlim.1).is_err())
                    || axes.ylim(ylim.0, ylim.1).is_err()
                {
                    continue;
                }
                let a = AxisPoint::new(xa, axis_value(ykind, y[0], ylim.0));
                let b = AxisPoint::new(xb, axis_value(ykind, y[1], ylim.0));
                cases += 1;
                if let Some(error) = check_against_oracle(&f, a, b) {
                    visible += 1;
                    max_error = max_error.max(error);
                }
            }
            assert!(
                cases >= 225,
                "{xkind:?}/{ykind:?}: only {cases} valid cases"
            );
            assert!(visible >= 50, "{xkind:?}/{ykind:?}: only {visible} visible");
            summary.push(format!(
                "{xkind:?}/{ykind:?}: {cases} cases, {visible} visible, max error {max_error:.2e} pt"
            ));
        }
    }
    println!("{}", summary.join("\n"));
}

#[test]
fn clipping_matches_an_exact_rational_oracle() {
    let stats = oracle::run(10_000);
    println!("{stats}");
}
