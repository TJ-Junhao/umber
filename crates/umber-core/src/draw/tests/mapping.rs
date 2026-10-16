//! Axis maps: category band centers, padded single-value data and log normalization shared
//! with the gridlines.

use super::{line_strokes, nums, plot_of, strokes, subpaths};
use crate::geometry::PathVerb;
use crate::layout::Area;
use crate::model::{AxisData, Figure, LineOptions};
use crate::resolve::NormalizedPos;
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, GridLines, ScaleKind, Theme};

#[test]
fn category_lines_map_to_band_centers() {
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Categories(vec!["a".into(), "b".into(), "c".into(), "d".into()]),
            &[1.0, 3.0, 2.0, 4.0],
            LineOptions::default(),
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let plot = plot_of(&f);
    let parts = subpaths(line_strokes(&scene)[0].0);
    let xs: Vec<f64> = parts[0].iter().map(|p| f64::from(p.x)).collect();
    for (x, center) in xs.iter().zip([0.125, 0.375, 0.625, 0.875]) {
        assert!(
            (x - plot.x_at(NormalizedPos::new(center))).abs() < 1e-3,
            "{xs:?}"
        );
    }
}

#[test]
fn too_close_limits_are_rejected_and_too_narrow_data_is_padded() {
    let next_up = |v: f64| f64::from_bits(v.to_bits() + 1);
    for (lo, hi) in [(1.0, next_up(1.0)), (0.0, 5e-324), (1e200, next_up(1e200))] {
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert!(matches!(
            axes.xlim(lo, hi),
            Err(crate::Error::Style {
                context: "Axes.xlim()",
                ..
            })
        ));
        assert!(matches!(
            axes.ylim(lo, hi),
            Err(crate::Error::Style {
                context: "Axes.ylim()",
                ..
            })
        ));

        // As data, the extent is a single value: a padded axis with the line visible,
        // vertical through the middle of the plot.
        axes.line(nums(&[lo, hi]), &[0.0, 1.0], LineOptions::default())
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        assert_scene_bounded(&scene);
        let plot = plot_of(&f);
        let parts = subpaths(line_strokes(&scene)[0].0);
        let [piece] = parts.as_slice() else {
            panic!("{lo}..{hi}: expected one piece, got {parts:?}")
        };
        assert_eq!(piece.len(), 2);
        for p in piece {
            assert!(
                (f64::from(p.x) - plot.x_at(NormalizedPos::new(0.5))).abs() < 1e-3,
                "{lo}..{hi}: {p:?}"
            );
            assert!(f64::from(p.y) >= plot.y - 1e-3 && f64::from(p.y) <= plot.bottom() + 1e-3);
        }
        assert!((piece[0].y - piece[1].y).abs() > 10.0);
    }
}

/// The scene x of the vertical line at `v` and the nearest vertical gridline, on a log x
/// axis with limits `lo..hi`, plus the expected x from `reference_pos`.
fn log_mark_and_grid(width_in: f64, lo: f64, hi: f64, v: f64) -> (f64, f64, Area) {
    let theme = Theme::graphite().with_grid_lines(GridLines::Both).unwrap();
    let mut f = Figure::new(width_in, 4.0, theme).unwrap();
    f.add_axes(
        ScaleKind::Log {
            base: crate::LogBase::TEN,
        },
        ScaleKind::Linear,
    )
    .unwrap()
    .xlim(lo, hi)
    .unwrap()
    .xticks(&[v])
    .unwrap()
    .line(nums(&[v, v]), &[0.0, 1.0], LineOptions::default())
    .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let mark = f64::from(subpaths(line_strokes(&scene)[0].0)[0][0].x);
    let grid = strokes(&scene)
        .iter()
        .filter_map(|(path, _)| match path.verbs() {
            [PathVerb::MoveTo(a), PathVerb::LineTo(b)] if a.x == b.x => Some(f64::from(a.x)),
            _ => None,
        })
        .min_by(|a, b| (a - mark).abs().total_cmp(&(b - mark).abs()))
        .unwrap();
    (mark, grid, plot_of(&f))
}

#[test]
fn marks_and_gridlines_share_the_log_normalization() {
    // Edge cases: the narrowest allowed log ratio at 1e299, with an explicit tick at `v`
    // so the normalization check is independent of automatic tick choice. The positions are
    // 100-digit references.
    let cases = [
        (
            200.0,
            1.385e299,
            1.385_000_001_385_000_2e299,
            1.385_000_000_989_286e299,
            0.714_285_829_422_576_1,
        ),
        (
            6.0,
            1.308_000_000_000_000_1e299,
            1.308_000_001_308_000_2e299,
            1.308_000_000_981_000_2e299,
            0.750_000_000_093_75,
        ),
    ];
    for (width_in, lo, hi, v, reference) in cases {
        let (mark, grid, plot) = log_mark_and_grid(width_in, lo, hi, v);
        assert!(
            (mark - grid).abs() <= 1e-3,
            "{width_in} in: mark {mark}, grid {grid}"
        );
        let expected = plot.x_at(NormalizedPos::new(reference));
        assert!(
            (mark - expected).abs() <= 1e-3,
            "{width_in} in: mark {mark}, expected {expected}"
        );
    }
}

#[test]
fn both_axes_map_through_their_scales() {
    use super::super::mapping::{AxisMaps, AxisPoint, axis_numbers, axis_value, axis_values};
    use crate::resolve::{AxisScale, ResolvedAxis};
    use crate::{CategoryScale, ContinuousScale, Transform};
    let plot = Area {
        x: 10.0,
        y: 20.0,
        width: 200.0,
        height: 100.0,
    };
    let names = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
    let category = ResolvedAxis {
        scale: AxisScale::Category(CategoryScale::from_first_appearance(["a", "b"])),
        ticks: Vec::new(),
    };
    let linear = ResolvedAxis {
        scale: AxisScale::Continuous(
            ContinuousScale::new(Transform::Linear, 0.0, 4.0, "test").unwrap(),
        ),
        ticks: Vec::new(),
    };
    // A category x axis maps band centers left to right, a category y axis top down, so that
    // its first category is at the top, and a linear axis maps its values the same way on
    // either side: left to right, or bottom up.
    let names_ab = AxisData::Categories(names(&["a", "b", "z"]));
    let centers = axis_values(&category, &names_ab);
    assert_eq!(&centers[..2], &[0.25, 0.75]);
    assert!(centers[2].is_nan());
    let maps = AxisMaps::new(&category, &linear, plot);
    let low = maps.to_plot(AxisPoint::new(maps.x.lo, maps.y.lo));
    let high = maps.to_plot(AxisPoint::new(maps.x.hi, maps.y.hi));
    assert_eq!((low.x, low.y), (plot.x, plot.bottom()));
    assert_eq!((high.x, high.y), (plot.right(), plot.y));
    let maps = AxisMaps::new(&linear, &category, plot);
    let low = maps.to_plot(AxisPoint::new(maps.x.lo, maps.y.lo));
    let high = maps.to_plot(AxisPoint::new(maps.x.hi, maps.y.hi));
    assert_eq!((low.x, low.y), (plot.x, plot.y));
    assert_eq!((high.x, high.y), (plot.right(), plot.bottom()));
    let point = maps.to_plot(AxisPoint::new(axis_value(&linear, 1.0), centers[1]));
    assert_eq!((point.x, point.y), (60.0, 95.0));
    // In the frame of horizontal marks the category axis comes first: a mark drawn there as
    // on a category x axis lands at the same plot points, its coordinates swapped.
    let frame = maps.oriented(crate::resolve::Orientation::Horizontal);
    assert_eq!((frame.x, frame.y), (maps.y, maps.x));
    let swapped = frame.to_plot(AxisPoint::new(centers[1], axis_value(&linear, 1.0)));
    assert_eq!((swapped.x, swapped.y), (60.0, 95.0));
    assert_eq!(maps.oriented(crate::resolve::Orientation::Vertical), maps);
    // Numbers have no place on a category axis.
    assert!(axis_value(&category, 1.0).is_nan());
    assert!(
        axis_numbers(&category, &[1.0, 2.0])
            .iter()
            .all(|v| v.is_nan())
    );
    assert_eq!(axis_numbers(&linear, &[1.0, f64::NAN])[0], 1.0);
}
