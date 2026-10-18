//! Automatic limits that fit whole bubble markers, and the data-position limits they fall back
//! to when the bubbles cannot all fit.

use crate::layout::Layout;
use crate::pipeline::{plan_figure, try_plan_panels};
use crate::resolve::{Resolved, ResolvedMark, resolve_marks};
use crate::{
    AxesMut, AxisData, BubbleOptions, Figure, FontLibrary, Grid, GridIndex, Instant, Item,
    LegendStyle, LineOptions, LogBase, Marker, Placement, ScaleKind, ScatterOptions, Share, Theme,
};

const MARKERS: [Marker; 5] = [
    Marker::Circle,
    Marker::Square,
    Marker::Triangle,
    Marker::Diamond,
    Marker::Cross,
];

fn bubbles(f: &mut Figure, x: &[f64], y: &[f64], sizes: &[f64], marker: Marker) {
    f.single_axes_mut()
        .bubbles(
            x,
            y,
            sizes,
            BubbleOptions {
                marker,
                ..BubbleOptions::default()
            },
        )
        .unwrap();
}

/// How far an outline of `width` reaches past the path of a `marker` shape, at most: half the
/// width along a circle, and at a vertex of interior angle `θ` the miter `width / 2 / sin(θ/2)`,
/// which is the whole width at a triangle's 60° corners and `width / √2` at the right-angled
/// corners of squares, diamonds and crosses.
fn outline_reach(marker: Marker, width: f32) -> f64 {
    let half = f64::from(width) / 2.0;
    match marker {
        Marker::Circle => half,
        Marker::Triangle => half / (std::f64::consts::PI / 6.0).sin(),
        Marker::Square | Marker::Diamond | Marker::Cross => {
            half / (std::f64::consts::PI / 4.0).sin()
        }
    }
}

/// Every panel of `f` as laid out, with its resolved marks.
fn plans(f: &Figure) -> Vec<((Resolved, Layout), Vec<ResolvedMark<'_>>)> {
    let planned = try_plan_panels(f, FontLibrary::global()).unwrap();
    planned
        .into_iter()
        .zip(f.cartesian_panels())
        .map(|(plan, axes)| (plan, resolve_marks(axes.marks(), f.theme())))
        .collect()
}

/// Where the bubbles of one panel reach past its plot along x and along y, in points: the
/// largest overshoot of any fill or outline, 0 or less when they all stay inside. Every drawn
/// bubble must be in the scene, none culled.
fn overshoot(
    f: &Figure,
    (resolved, layout): &(Resolved, Layout),
    marks: &[ResolvedMark<'_>],
) -> (f64, f64) {
    let (mut x, mut y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for mark in marks {
        let ResolvedMark::Bubbles {
            mark: data,
            maximum,
            radius,
            ..
        } = mark
        else {
            continue;
        };
        let Item::Group { items, .. } =
            crate::draw::marks(f.theme(), resolved, layout, std::slice::from_ref(mark)).unwrap()
        else {
            panic!("a mark group")
        };
        let drawn = data
            .sizes()
            .iter()
            .enumerate()
            .filter(|(i, size)| {
                !data.x_values().is_gap(*i)
                    && !data.y_values().is_gap(*i)
                    && crate::bubble_scale::radius(**size, *maximum, *radius) > 0.0
            })
            .count();
        assert_eq!(items.len(), 2 * drawn, "no bubble may be culled");
        let marker = data.options().marker;
        for item in &items {
            let (path, reach) = match item {
                Item::Fill { path, .. } => (path, 0.0),
                Item::Stroke { path, style } => (path, outline_reach(marker, style.width)),
                other => panic!("a bubble path: {other:?}"),
            };
            let (lo, hi) = path.bounds().unwrap();
            let p = layout.plot;
            x = x
                .max(p.x - (f64::from(lo.x) - reach))
                .max(f64::from(hi.x) + reach - p.right());
            y = y
                .max(p.y - (f64::from(lo.y) - reach))
                .max(f64::from(hi.y) + reach - p.bottom());
        }
    }
    (x, y)
}

/// Asserts that every bubble of `f`, fill and outline, stays inside its panel's plot; `case`
/// names the figure.
fn assert_bubbles_inside(f: &Figure, case: &str) {
    for (i, (plan, marks)) in plans(f).iter().enumerate() {
        let (x, y) = overshoot(f, plan, marks);
        assert!(
            x <= 1e-4 && y <= 1e-4,
            "{case}, panel {i}: {x} pt past x, {y} pt past y"
        );
    }
}

/// A diagonal line with two corner bubbles, and the population bubbles of the cities chart.
fn corner_and_population(marker: Marker) -> [Figure; 2] {
    let mut corner = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
    corner
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions::default(),
        )
        .unwrap();
    bubbles(&mut corner, &[0.0, 1.0], &[1.0, 0.0], &[1.0, 1.0], marker);
    let mut cities = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    cities
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Cities")
        .subtitle("Marker area shows population, in thousands")
        .xlabel("Income")
        .ylabel("Output")
        .bubbles(
            &[12.0, 18.0, 24.0, 35.0, 42.0, 55.0, 67.0, 78.0],
            &[3.2, 4.8, 2.9, 6.4, 5.7, 7.1, 4.2, 8.3],
            &[80.0, 140.0, 220.0, 500.0, 330.0, 870.0, 190.0, 650.0],
            BubbleOptions {
                label: "Cities".into(),
                marker,
                ..BubbleOptions::default()
            },
        )
        .unwrap();
    [corner, cities]
}

#[test]
fn automatic_limits_fit_every_marker_shape_with_its_outline() {
    for marker in MARKERS {
        for (f, case) in corner_and_population(marker)
            .iter()
            .zip(["corner", "cities"])
        {
            assert_bubbles_inside(f, &format!("{marker:?} {case}"));
        }
    }
}

#[test]
fn automatic_limits_fit_a_single_bubble_with_and_without_explicit_ticks() {
    for marker in MARKERS {
        for explicit_ticks in [false, true] {
            let mut f = Figure::new(3.0, 2.0, Theme::pigment()).unwrap();
            let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            if explicit_ticks {
                a.xticks(&[0.0, 1.0]).unwrap().yticks(&[0.0, 1.0]).unwrap();
            }
            bubbles(&mut f, &[1.0], &[1.0], &[1.0], marker);
            assert_bubbles_inside(&f, &format!("{marker:?}, explicit ticks: {explicit_ticks}"));
        }
    }
}

#[test]
fn automatic_limits_fit_bubbles_on_log_axes() {
    for marker in [Marker::Circle, Marker::Triangle] {
        for (x, y) in [
            (ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear),
            (ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN }),
            (
                ScaleKind::Log {
                    base: LogBase::new(2.0).unwrap(),
                },
                ScaleKind::Log { base: LogBase::TEN },
            ),
        ] {
            let mut f = Figure::new(3.0, 2.5, Theme::midnight()).unwrap();
            f.add_axes(x, y)
                .unwrap()
                .xticks(&[1.0, 100.0])
                .unwrap()
                .yticks(&[1.0, 100.0])
                .unwrap();
            bubbles(&mut f, &[1.0, 100.0], &[100.0, 1.0], &[1.0, 1.0], marker);
            assert_bubbles_inside(&f, &format!("{marker:?} on {x:?} and {y:?}"));
        }
    }
}

#[test]
fn automatic_limits_fit_the_bubbles_of_every_shared_panel() {
    let grid = Grid::new(1, 2)
        .unwrap()
        .with_share_x(Share::All)
        .with_share_y(Share::All);
    let mut f = Figure::with_grid(6.0, 2.5, Theme::pigment(), grid).unwrap();
    for col in 0..2 {
        f.add_axes_at(
            Placement {
                row: GridIndex::At(0),
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .bubbles(
            &[col as f64],
            &[col as f64],
            &[1.0],
            BubbleOptions::default(),
        )
        .unwrap();
    }
    assert_bubbles_inside(&f, "shared");
    let plans = try_plan_panels(&f, FontLibrary::global()).unwrap();
    assert_eq!(plans[0].0.x, plans[1].0.x);
    assert_eq!(plans[0].0.y, plans[1].0.y);
}

#[test]
fn explicit_limits_are_kept_and_clip_the_bubbles() {
    let mut f = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    bubbles(&mut f, &[1.0], &[1.0], &[1.0], Marker::Circle);
    let (r, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(r.x.continuous().unwrap().domain(), (0.0, 1.0));
    assert_eq!(r.y.continuous_scale().domain(), (0.0, 1.0));
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    let Item::Group {
        items,
        clip: Some(clip),
    } = crate::draw::marks(f.theme(), &r, &layout, &marks).unwrap()
    else {
        panic!("clipped marks")
    };
    let Item::Fill { path, .. } = &items[0] else {
        panic!("bubble fill")
    };
    let (lo, hi) = path.bounds().unwrap();
    assert!(lo.y < clip.y && hi.x > clip.x + clip.width);
}

#[test]
fn only_the_axis_without_explicit_limits_is_fitted() {
    let mut f = Figure::new(0.75, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap();
    bubbles(&mut f, &[1.0], &[1.0], &[1.0], Marker::Circle);
    let (r, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(r.x.continuous().unwrap().domain(), (0.0, 1.0));
    assert!(layout.plot.width < 36.8);
    let position = r.y.continuous_scale().normalize(1.0);
    assert!(position * layout.plot.height >= 18.4);
    assert!((1.0 - position) * layout.plot.height >= 18.4);
    assert!(f.to_scene(FontLibrary::global()).is_ok());
}

/// How a test figure draws its points: as bubbles, or as a scatter of the same positions,
/// whose axes have the limits of the data positions alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Draw {
    Bubbles,
    Scatter,
}

fn points(a: &mut AxesMut<'_>, draw: Draw, x: &[f64], y: &[f64], sizes: &[f64]) {
    match draw {
        Draw::Bubbles => a.bubbles(x, y, sizes, BubbleOptions::default()).map(|_| ()),
        Draw::Scatter => a
            .scatter(AxisData::Numbers(x.to_vec()), y, ScatterOptions::default())
            .map(|_| ()),
    }
    .unwrap();
}

/// Asserts that every axis of the figure `make` draws with bubbles either holds all its
/// bubbles or has the limits of its data positions, as the same figure drawn as a scatter has,
/// and returns how many x and y axes have those limits.
fn fitted_or_data_limits(make: impl Fn(Draw) -> Figure) -> (usize, usize) {
    let f = make(Draw::Bubbles);
    f.to_scene(FontLibrary::global()).unwrap();
    let twin = try_plan_panels(&make(Draw::Scatter), FontLibrary::global()).unwrap();
    let mut data_limits = (0, 0);
    for (i, ((plan, marks), (twin, _))) in plans(&f).iter().zip(&twin).enumerate() {
        let (x, y) = overshoot(&f, plan, marks);
        let (resolved, _) = plan;
        if x > 1e-4 {
            assert_eq!(
                resolved.x, twin.x,
                "panel {i}: x neither fits nor has the data limits"
            );
            data_limits.0 += 1;
        }
        if y > 1e-4 {
            assert_eq!(
                resolved.y, twin.y,
                "panel {i}: y neither fits nor has the data limits"
            );
            data_limits.1 += 1;
        }
    }
    data_limits
}

#[test]
fn bubbles_larger_than_the_plot_keep_the_data_limits_and_are_clipped() {
    let make = |draw| {
        let mut f = Figure::new(0.75, 0.75, Theme::pigment()).unwrap();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.legend(LegendStyle::None, crate::SizeSpec::Auto)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap();
        points(&mut a, draw, &[0.0, 1.0], &[0.0, 1.0], &[1.0, 1.0]);
        f
    };
    assert_eq!(fitted_or_data_limits(make), (1, 1));
}

#[test]
fn a_grid_of_small_panels_renders_with_fitted_or_data_limits() {
    let make = |draw| {
        let grid = Grid::new(4, 4).unwrap();
        let mut f = Figure::with_grid(6.0, 4.0, Theme::pigment(), grid).unwrap();
        for row in 0..4 {
            for col in 0..4 {
                let mut a = f
                    .add_axes_at(
                        Placement {
                            row: GridIndex::At(row),
                            col: GridIndex::At(col),
                            ..Placement::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap();
                points(
                    &mut a,
                    draw,
                    &[1.0, 2.0, 3.0],
                    &[1.0, 3.0, 2.0],
                    &[1.0, 4.0, 9.0],
                );
            }
        }
        f
    };
    // The plots are wide enough for the bubbles, but shorter than the largest one.
    assert_eq!(fitted_or_data_limits(make), (0, 16));
}

#[test]
fn refitted_limits_are_padded_once() {
    // On this short plot the fit starts from the raw extent and is padded once, rather than
    // rounding to ticks or repeatedly widening an already padded domain.
    let mut f = Figure::new(3.951, 2.231, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Title")
        .bubbles(
            &[
                0.600_000_000_000_000_1,
                -1.8,
                1.200_000_000_000_000_2,
                6.699_999_999_999_999,
                6.6,
            ],
            &[-1.1, 3.0, -1.0, 5.9, 2.099_999_999_999_999_6],
            &[4.0, 8.0, 9.0, 6.0, 5.0],
            BubbleOptions {
                label: "B".into(),
                ..BubbleOptions::default()
            },
        )
        .unwrap();
    assert_bubbles_inside(&f, "refitted");
    let (r, _) = plan_figure(&f, (0, 0), FontLibrary::global());
    let (lo, hi) = r.y.continuous_scale().domain();
    assert!(
        (lo + 7.651_399_814_281_006).abs() < 1e-9 && (hi - 11.513_767_384_600_04).abs() < 1e-9,
        "{:?}",
        (lo, hi)
    );
    let ticks: Vec<_> = r.y.ticks.iter().map(|tick| tick.label.as_str()).collect();
    assert_eq!(ticks.first().zip(ticks.last()), Some((&"0", &"10")));
}

/// The microseconds in a day.
const DAY: i64 = 86_400_000_000;

/// The time `n` days after 1970-01-01, present.
fn day(n: i64) -> Option<Instant> {
    Some(Instant::from_micros(n * DAY).unwrap())
}

/// Adds bubbles to the axes of `f`, whose coordinates are each numbers or times.
fn time_bubbles(f: &mut Figure, x: AxisData, y: AxisData, sizes: &[f64], marker: Marker) {
    f.single_axes_mut()
        .bubbles_time(
            x,
            y,
            sizes.into(),
            BubbleOptions {
                marker,
                ..BubbleOptions::default()
            },
        )
        .unwrap();
}

/// The three axes pairs with a time axis.
const TIME_PAIRS: [(ScaleKind, ScaleKind); 3] = [
    (ScaleKind::Time, ScaleKind::Linear),
    (ScaleKind::Linear, ScaleKind::Time),
    (ScaleKind::Time, ScaleKind::Time),
];

/// The coordinates of two corner bubbles and a line between them, each a time on a time axis
/// and a number on a numeric one.
fn corner_data(x: ScaleKind, y: ScaleKind) -> ([AxisData; 2], [AxisData; 2]) {
    let along = |kind: ScaleKind, times: [Option<Instant>; 2], numbers: [f64; 2]| {
        if kind == ScaleKind::Time {
            AxisData::Times(times.to_vec())
        } else {
            AxisData::Numbers(numbers.to_vec())
        }
    };
    (
        [
            along(x, [day(0), day(10)], [0.0, 1.0]),
            along(y, [day(10), day(0)], [1.0, 0.0]),
        ],
        [
            along(x, [day(0), day(10)], [0.0, 1.0]),
            along(y, [day(0), day(10)], [0.0, 1.0]),
        ],
    )
}

#[test]
fn automatic_limits_fit_bubbles_on_time_axes() {
    for marker in MARKERS {
        for (x, y) in TIME_PAIRS {
            let mut f = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
            f.add_axes(x, y).unwrap();
            let (points, line) = corner_data(x, y);
            let [line_x, line_y] = line;
            f.single_axes_mut()
                .line_shared(line_x, line_y, LineOptions::default())
                .unwrap();
            let [bubble_x, bubble_y] = points;
            time_bubbles(&mut f, bubble_x, bubble_y, &[1.0, 1.0], marker);
            assert_bubbles_inside(&f, &format!("{marker:?} on {x:?} and {y:?}"));
        }
    }
}

#[test]
fn time_axes_widen_beyond_the_times_of_their_bubbles() {
    let mut f = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Time, ScaleKind::Linear).unwrap();
    time_bubbles(
        &mut f,
        AxisData::Times(vec![day(0), day(10)]),
        AxisData::Numbers(vec![1.0, 2.0]),
        &[1.0, 1.0],
        Marker::Circle,
    );
    let (r, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    let crate::resolve::AxisScale::Time(scale) = &r.x.scale else {
        panic!("a time axis")
    };
    let (low, high) = scale.instants();
    assert!(
        low < day(0).unwrap() && high > day(10).unwrap(),
        "{low:?} {high:?}"
    );
    // Each circle's radius is 18 pt: the fitted domain keeps its centre that far from the edges.
    let seconds = |t: Option<Instant>| scale.seconds(t.unwrap());
    let position = |t| scale.seconds_scale().normalize(seconds(t)) * layout.plot.width;
    assert!(position(day(0)) >= 18.0 - 1e-3, "{}", position(day(0)));
    assert!(
        layout.plot.width - position(day(10)) >= 18.0 - 1e-3,
        "{}",
        position(day(10))
    );
}

#[test]
fn automatic_limits_fit_a_single_time_bubble_with_and_without_explicit_ticks() {
    for (x, y) in TIME_PAIRS {
        for explicit_ticks in [false, true] {
            let mut f = Figure::new(3.0, 2.0, Theme::pigment()).unwrap();
            let mut a = f.add_axes(x, y).unwrap();
            if explicit_ticks {
                if x == ScaleKind::Time {
                    a.xticks_time(&[Instant::from_micros(0).unwrap()]).unwrap();
                }
                if y == ScaleKind::Time {
                    a.yticks_time(&[Instant::from_micros(0).unwrap()]).unwrap();
                }
            }
            let at = |kind| {
                if kind == ScaleKind::Time {
                    AxisData::Times(vec![day(2)])
                } else {
                    AxisData::Numbers(vec![1.0])
                }
            };
            time_bubbles(&mut f, at(x), at(y), &[1.0], Marker::Circle);
            assert_bubbles_inside(
                &f,
                &format!("{x:?} {y:?}, explicit ticks: {explicit_ticks}"),
            );
        }
    }
}

#[test]
fn automatic_limits_fit_the_bubbles_of_every_shared_time_panel() {
    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut f = Figure::with_grid(6.0, 2.5, Theme::pigment(), grid).unwrap();
    for col in 0..2 {
        f.add_axes_at(
            Placement {
                row: GridIndex::At(0),
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Time,
            ScaleKind::Linear,
        )
        .unwrap()
        .bubbles_time(
            AxisData::Times(vec![day(10 * col)]),
            AxisData::Numbers(vec![col as f64]),
            [1.0].into(),
            BubbleOptions::default(),
        )
        .unwrap();
    }
    assert_bubbles_inside(&f, "shared time");
    let plans = try_plan_panels(&f, FontLibrary::global()).unwrap();
    assert_eq!(plans[0].0.x, plans[1].0.x);
}

#[test]
fn explicit_time_limits_are_kept_and_clip_the_bubbles() {
    let mut f = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Time, ScaleKind::Linear)
        .unwrap()
        .xlim_time(
            Instant::from_micros(0).unwrap(),
            Instant::from_micros(DAY).unwrap(),
        )
        .unwrap();
    time_bubbles(
        &mut f,
        AxisData::Times(vec![day(1)]),
        AxisData::Numbers(vec![1.0]),
        &[1.0],
        Marker::Circle,
    );
    let (r, _) = plan_figure(&f, (0, 0), FontLibrary::global());
    let crate::resolve::AxisScale::Time(scale) = &r.x.scale else {
        panic!("a time axis")
    };
    assert_eq!(
        scale.instants(),
        (day(0).unwrap(), day(1).unwrap()),
        "explicit limits stay"
    );
    assert!(f.to_scene(FontLibrary::global()).is_ok());
}

#[test]
fn missing_times_draw_no_bubble() {
    let mut f = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Time, ScaleKind::Time).unwrap();
    time_bubbles(
        &mut f,
        AxisData::Times(vec![day(0), None, day(4), day(2)]),
        AxisData::Times(vec![day(1), day(2), None, day(5)]),
        &[1.0, 1.0, 1.0, 1.0],
        Marker::Circle,
    );
    assert_bubbles_inside(&f, "gaps");
}
