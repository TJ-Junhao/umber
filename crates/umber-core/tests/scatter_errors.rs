//! Error bars and a connecting line that belong to a scatter (`x_error`, `y_error`, `connect`),
//! through the public API: their color, palette slot and legend, drawing order, gaps, limits,
//! validation, axes kinds, and the point labels that avoid them.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use umber_core::scene::{Item, LineCap, Scene, StrokeStyle, TextItem};
use umber_core::{
    AxesMut, AxisData, ColorBy, ColorSpec, Error, ErrorBarOptions, ErrorRange, Figure, FontLibrary,
    GridIndex, Instant, LineOptions, Marker, Path, PathVerb, Place, PointColor, PointLabels, Rgba,
    ScaleKind, ScatterGroup, ScatterOptions, SizeSpec, SvgText, Theme,
};

fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure.add_axes(x, y).unwrap();
    figure
}

fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
}

fn render(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

fn nums(v: &[f64]) -> AxisData {
    AxisData::Numbers(v.to_vec())
}

fn names(v: &[&str]) -> AxisData {
    AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect())
}

fn ends(low: &[f64], high: &[f64]) -> Option<ErrorRange> {
    Some(ErrorRange::Numbers {
        low: Arc::from(low),
        high: Arc::from(high),
    })
}

fn with_errors(y_error: Option<ErrorRange>, connect: bool) -> ScatterOptions {
    ScatterOptions {
        y_error,
        connect,
        ..ScatterOptions::default()
    }
}

/// The items of every marks group of `scene`, in drawing order: the groups clipped to a plot.
fn mark_items(scene: &Scene) -> Vec<&Item> {
    fn leaves<'a>(items: &'a [Item], out: &mut Vec<&'a Item>) {
        for item in items {
            match item {
                Item::Group { items, .. } => leaves(items, out),
                other => out.push(other),
            }
        }
    }
    let mut out = Vec::new();
    for item in &scene.items {
        if let Item::Group {
            clip: Some(_),
            items,
        } = item
        {
            leaves(items, &mut out);
        }
    }
    out
}

/// The strokes among the marks of `scene`, in drawing order.
fn stroke_items(scene: &Scene) -> Vec<(&Path, &StrokeStyle)> {
    mark_items(scene)
        .into_iter()
        .filter_map(|item| match item {
            Item::Stroke { path, style } => Some((path, style)),
            _ => None,
        })
        .collect()
}

/// The fill of each batch of markers among the marks of `scene`, in drawing order.
fn marker_fills(scene: &Scene) -> Vec<Rgba> {
    mark_items(scene)
        .into_iter()
        .filter_map(|item| match item {
            Item::Markers { fill, .. } => *fill,
            _ => None,
        })
        .collect()
}

/// The number of contours of `path`: its move commands.
fn contours(path: &Path) -> usize {
    path.verbs()
        .iter()
        .filter(|verb| matches!(verb, PathVerb::MoveTo(_)))
        .count()
}

/// The width of a standalone line or error bar drawn as a scatter's own are: half the automatic.
fn half() -> SizeSpec {
    SizeSpec::times(0.5).unwrap()
}

fn solid(color: Rgba) -> Rgba {
    Rgba { a: 255, ..color }
}

/// Every text item of `scene`, in drawing order.
fn texts(scene: &Scene) -> Vec<&TextItem> {
    fn walk<'a>(items: &'a [Item], out: &mut Vec<&'a TextItem>) {
        for item in items {
            match item {
                Item::Group { items, .. } => walk(items, out),
                Item::Text(text) => out.push(text),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&scene.items, &mut out);
    out
}

// ---- Color, palette and legend --------------------------------------------------------------

#[test]
fn bars_and_line_take_the_series_color_at_full_opacity() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[2.0, 3.0, 2.5],
            with_errors(ends(&[1.5, 2.5, 2.0], &[2.5, 3.5, 3.0]), true),
        )
        .unwrap();
    let scene = render(&f);
    let fill = marker_fills(&scene)[0];
    let strokes = stroke_items(&scene);
    assert_eq!(strokes.len(), 2, "the line and the bars");
    for (_, style) in &strokes {
        assert_eq!(style.color, solid(fill));
    }
    // An explicit color, with an alpha of its own, is drawn opaque too.
    let red = Rgba {
        r: 200,
        g: 30,
        b: 40,
        a: 100,
    };
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0]),
            &[2.0, 3.0],
            ScatterOptions {
                color: PointColor::Uniform(ColorSpec::Fixed(red)),
                ..with_errors(ends(&[1.5, 2.5], &[2.5, 3.5]), true)
            },
        )
        .unwrap();
    for (_, style) in stroke_items(&render(&f)) {
        assert_eq!(style.color, solid(red));
    }
}

#[test]
fn groups_and_color_maps_color_their_own_bars_and_lines() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let group = |label: &str, y: f64| ScatterGroup {
        y_error: ends(&[y - 0.5, y - 0.5], &[y + 0.5, y + 0.5]),
        ..ScatterGroup::new(label, nums(&[1.0, 2.0]), nums(&[y, y + 1.0]))
    };
    axes(&mut f)
        .grouped_scatter(
            vec![group("North", 1.0), group("South", 5.0)],
            Marker::Circle,
            SizeSpec::Auto,
            PointLabels::default(),
            true,
        )
        .unwrap();
    let scene = render(&f);
    let fills = marker_fills(&scene);
    let strokes = stroke_items(&scene);
    assert_eq!(strokes.len(), 4, "a line and bars per group");
    assert_ne!(fills[0], fills[1]);
    for (i, (path, style)) in strokes.iter().enumerate() {
        assert_eq!(style.color, solid(fills[i / 2]), "stroke {i}");
        if i % 2 == 0 {
            // Each group's line joins its own two points only.
            assert_eq!(contours(path), 1);
        }
    }
    // A color-mapped scatter's bars and line take the theme's text color.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0]),
            &[2.0, 3.0],
            ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![0.1, 0.9])),
                ..with_errors(ends(&[1.5, 2.5], &[2.5, 3.5]), true)
            },
        )
        .unwrap();
    let mapped = render(&f);
    let strokes = stroke_items(&mapped);
    assert_eq!(strokes.len(), 2);
    for (_, style) in strokes {
        assert_eq!(style.color, Theme::pigment().text_color());
    }
}

#[test]
fn bars_and_line_take_no_palette_slot_and_no_legend_entry() {
    let build = |y_error: Option<ErrorRange>, connect: bool| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .scatter(
                nums(&[1.0, 2.0]),
                &[2.0, 3.0],
                ScatterOptions {
                    label: "Means".into(),
                    ..with_errors(y_error, connect)
                },
            )
            .unwrap()
            .scatter(
                nums(&[1.0, 2.0]),
                &[1.0, 1.5],
                ScatterOptions {
                    label: "Others".into(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        render(&f)
    };
    let plain = build(None, false);
    let own = build(ends(&[1.5, 2.5], &[2.5, 3.5]), true);
    // The following series keeps its color.
    assert_eq!(marker_fills(&own), marker_fills(&plain));
    // The legend is the same: one entry per series, with unchanged swatches.
    let legend = |scene: &Scene| -> Vec<String> {
        texts(scene)
            .into_iter()
            .filter(|t| t.text == "Means" || t.text == "Others")
            .map(|t| t.text.clone())
            .collect()
    };
    assert_eq!(legend(&own), ["Means", "Others"]);
    let swatches = |scene: &Scene| marker_fills(scene).len();
    assert_eq!(swatches(&own), swatches(&plain));
    // The legend group is identical item by item.
    let last = |scene: &Scene| {
        scene
            .items
            .iter()
            .rfind(|item| matches!(item, Item::Group { clip: None, .. }))
            .cloned()
    };
    assert_eq!(last(&own), last(&plain));
}

// ---- Drawing --------------------------------------------------------------------------------

#[test]
fn the_line_then_the_bars_lie_under_the_markers() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[2.0, 3.0, 2.5],
            ScatterOptions {
                x_error: ends(&[0.8, 1.8, 2.8], &[1.2, 2.2, 3.2]),
                ..with_errors(ends(&[1.5, 2.5, 2.0], &[2.5, 3.5, 3.0]), true)
            },
        )
        .unwrap();
    let scene = render(&f);
    let kinds: Vec<&str> = mark_items(&scene)
        .into_iter()
        .map(|item| match item {
            Item::Stroke { style, .. } if style.cap == LineCap::Round => "line",
            Item::Stroke { .. } => "bars",
            Item::Markers { .. } => "markers",
            _ => "other",
        })
        .collect();
    // Both errors at once: the horizontal bars, then the vertical ones.
    assert_eq!(kinds, ["line", "bars", "bars", "markers"]);
}

#[test]
fn own_bars_look_like_standalone_error_bars_and_skip_nan_ends() {
    let (low, high) = ([1.5, f64::NAN, 2.0], [2.5, f64::NAN, 3.0]);
    let mut own = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut own)
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[2.0, 3.0, 2.5],
            with_errors(ends(&low, &high), false),
        )
        .unwrap();
    let mut alone = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut alone)
        .y_error_bars(
            nums(&[1.0, 2.0, 3.0]),
            &low,
            &high,
            ErrorBarOptions {
                width: half(),
                ..ErrorBarOptions::default()
            },
        )
        .unwrap()
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[2.0, 3.0, 2.5],
            ScatterOptions::default(),
        )
        .unwrap();
    let (own, alone) = (render(&own), render(&alone));
    let (own_bars, alone_bars) = (stroke_items(&own)[0], stroke_items(&alone)[0]);
    // The same bars and caps, at the same width; two bars of three caps' worth of contours.
    assert_eq!(own_bars.0, alone_bars.0);
    assert_eq!(own_bars.1.width, alone_bars.1.width);
    assert_eq!(own_bars.1.cap, alone_bars.1.cap);
    assert_eq!(contours(own_bars.0), 6);
    // The point whose bar is skipped is still drawn.
    let markers = mark_items(&own)
        .into_iter()
        .find_map(|item| match item {
            Item::Markers { positions, .. } => Some(positions.len()),
            _ => None,
        })
        .unwrap();
    assert_eq!(markers, 3);
}

#[test]
fn a_point_left_out_has_no_bar_and_breaks_the_line() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0, 3.0, 4.0, 5.0]),
            &[2.0, 3.0, f64::NAN, 2.5, 3.0],
            with_errors(
                ends(&[1.5, 2.5, 2.0, 2.0, 2.5], &[2.5, 3.5, 3.0, 3.0, 3.5]),
                true,
            ),
        )
        .unwrap();
    let scene = render(&f);
    let strokes = stroke_items(&scene);
    let (line, bars) = (strokes[0].0, strokes[1].0);
    assert_eq!(contours(line), 2);
    // Four bars, each a stem and two caps.
    assert_eq!(contours(bars), 12);
}

#[test]
fn bars_extend_automatic_limits() {
    let y_top = |y_error: Option<ErrorRange>| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .scatter(nums(&[1.0, 2.0]), &[2.0, 3.0], with_errors(y_error, false))
            .unwrap();
        let scene = render(&f);
        let item = mark_items(&scene)
            .into_iter()
            .find_map(|item| match item {
                Item::Markers { positions, .. } => Some(positions.clone()),
                _ => None,
            })
            .unwrap();
        (scene, f64::from(item[1].y))
    };
    let (_, plain) = y_top(None);
    let (scene, grown) = y_top(ends(&[1.0, 2.0], &[3.0, 10.0]));
    // The highest point sits lower once the bar's top end counts.
    assert!(grown > plain + 10.0, "{grown} <= {plain}");
    let clip = scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group { clip: Some(r), .. } => Some(*r),
            _ => None,
        })
        .unwrap();
    let (low, _) = stroke_items(&scene)[0].0.bounds().unwrap();
    assert!(
        low.y >= clip.y - 1.0,
        "the bar's top end {low:?} stays inside {clip:?}"
    );
}

// ---- Validation -----------------------------------------------------------------------------

#[test]
fn error_ends_are_checked_and_name_the_argument_and_the_index() {
    let attempt = |kinds: (ScaleKind, ScaleKind), x: AxisData, options: ScatterOptions| {
        let mut f = figure(kinds.0, kinds.1);
        let mut ax = axes(&mut f);
        let err = ax.scatter(x, &[1.0, 2.0], options).unwrap_err();
        assert!(ax.marks().is_empty(), "a failed call adds nothing");
        err
    };
    let linear = (ScaleKind::Linear, ScaleKind::Linear);
    let cases = [
        (
            linear,
            with_errors(ends(&[1.0], &[2.0, 3.0]), false),
            "Figure.to_scene()",
            "y_error low has 1 values but the scatter has 2 points; y_error needs one low and \
             one high value per point.",
        ),
        (
            linear,
            with_errors(ends(&[1.0, f64::INFINITY], &[2.0, 3.0]), false),
            "",
            "y_error low[1] is inf; values must be finite.",
        ),
        (
            linear,
            with_errors(ends(&[1.0, 4.0], &[2.0, 3.0]), false),
            "",
            "y_error[1] has low 4 above high 3; low must not exceed high.",
        ),
        (
            (
                ScaleKind::Linear,
                ScaleKind::Log {
                    base: umber_core::LogBase::TEN,
                },
            ),
            with_errors(ends(&[0.0, 1.0], &[2.0, 3.0]), false),
            "",
            "y_error low[0] is 0 on a log axis; values must be positive.",
        ),
        (
            linear,
            ScatterOptions {
                x_error: ends(&[1.0, 2.0], &[0.5, 3.0]),
                ..ScatterOptions::default()
            },
            "",
            "x_error[0] has low 1 above high 0.5; low must not exceed high.",
        ),
    ];
    for (kinds, options, _, message) in cases {
        let err = attempt(kinds, nums(&[1.0, 2.0]), options);
        assert!(matches!(err, Error::Data { .. }), "{err}");
        assert!(err.to_string().ends_with(message), "{err}");
        assert!(
            matches!(err.parameter(), Some("y_error" | "x_error")),
            "{err}"
        );
    }
    // Times on a linear axis, numbers on a time axis, and a category axis are usage errors.
    let t = |micros: i64| Some(Instant::from_micros(micros).unwrap());
    let times = Some(ErrorRange::Times {
        low: Arc::from([t(0), t(1)]),
        high: Arc::from([t(2), t(3)]),
    });
    let usage = [
        (linear, nums(&[1.0, 2.0]), with_errors(times.clone(), false)),
        (
            (ScaleKind::Linear, ScaleKind::Time),
            nums(&[1.0, 2.0]),
            with_errors(ends(&[1.0, 2.0], &[2.0, 3.0]), false),
        ),
        (
            (ScaleKind::Category, ScaleKind::Linear),
            names(&["a", "b"]),
            ScatterOptions {
                x_error: ends(&[1.0, 2.0], &[2.0, 3.0]),
                ..ScatterOptions::default()
            },
        ),
    ];
    for (kinds, x, options) in usage {
        let mut f = figure(kinds.0, kinds.1);
        let mut ax = axes(&mut f);
        let err = if kinds.1 == ScaleKind::Time {
            ax.scatter_shared(x, AxisData::Times(vec![t(5), t(6)]), options)
                .unwrap_err()
        } else {
            ax.scatter(x, &[1.0, 2.0], options).unwrap_err()
        };
        assert!(matches!(err, Error::Usage { .. }), "{err}");
        assert!(ax.marks().is_empty());
    }
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let err = axes(&mut f)
        .scatter(
            names(&["a"]),
            &[1.0],
            ScatterOptions {
                x_error: ends(&[1.0], &[2.0]),
                ..ScatterOptions::default()
            },
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): the x axis is a category axis; x_error needs a linear, log or time x \
         axis."
    );
    assert_eq!(err.parameter(), Some("x_error"));
}

// ---- Axes kinds -----------------------------------------------------------------------------

#[test]
fn log_time_and_twin_axes_draw_their_own_bars_and_lines() {
    // A log y axis.
    let mut f = figure(
        ScaleKind::Linear,
        ScaleKind::Log {
            base: umber_core::LogBase::TEN,
        },
    );
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0]),
            &[10.0, 100.0],
            with_errors(ends(&[5.0, 50.0], &[20.0, 200.0]), true),
        )
        .unwrap();
    assert_eq!(stroke_items(&render(&f)).len(), 2);
    // A time y axis, with times for the ends and a line along the times.
    let day = 86_400_000_000;
    let t = |days: i64| Some(Instant::from_micros(days * day).unwrap());
    let build = |y_error: Option<ErrorRange>| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Time);
        axes(&mut f)
            .scatter_shared(
                nums(&[1.0, 2.0]),
                AxisData::Times(vec![t(10), t(12)]),
                with_errors(y_error, true),
            )
            .unwrap();
        render(&f)
    };
    let timed = build(Some(ErrorRange::Times {
        low: Arc::from([t(9), t(11)]),
        high: Arc::from([t(11), t(30)]),
    }));
    assert_eq!(stroke_items(&timed).len(), 2);
    // The late end extends the time axis: the points sit lower than without bars.
    let plain = build(None);
    let lowest = |scene: &Scene| {
        mark_items(scene)
            .into_iter()
            .find_map(|item| match item {
                Item::Markers { positions, .. } => Some(f64::from(positions[1].y)),
                _ => None,
            })
            .unwrap()
    };
    assert!(lowest(&timed) > lowest(&plain) + 10.0);
    // A time x axis with a connecting line.
    let mut f = figure(ScaleKind::Time, ScaleKind::Linear);
    axes(&mut f)
        .scatter_shared(
            AxisData::Times(vec![t(1), t(2), t(3)]),
            nums(&[1.0, 2.0, 1.5]),
            ScatterOptions {
                connect: true,
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    assert_eq!(contours(stroke_items(&render(&f))[0].0), 1);
    // The right axes of a twin pair.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut left = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    left.line(nums(&[0.0, 3.0]), &[0.0, 1.0], LineOptions::default())
        .unwrap();
    left.twin_y(ScaleKind::Linear)
        .unwrap()
        .scatter(
            nums(&[1.0, 2.0]),
            &[100.0, 300.0],
            with_errors(ends(&[50.0, 250.0], &[150.0, 350.0]), true),
        )
        .unwrap();
    let scene = render(&f);
    let fills = marker_fills(&scene);
    let strokes = stroke_items(&scene);
    // The left line, then the twin's own line and bars in the twin series color.
    assert_eq!(strokes.len(), 3);
    for (_, style) in &strokes[1..] {
        assert_eq!(style.color, solid(fills[0]));
    }
}

// ---- Point labels ---------------------------------------------------------------------------

/// The side of its point at `(x, y)` that the value label `content` of `scene` lies on.
fn label_side(scene: &Scene, content: &str, (x, y): (f64, f64)) -> Place {
    let label = texts(scene)
        .into_iter()
        .rfind(|t| t.text == content)
        .unwrap();
    let (left, baseline) = (f64::from(label.origin.x), f64::from(label.origin.y));
    let right = left + f64::from(label.width);
    if baseline < y && left < x && x < right {
        Place::Above
    } else if left > x {
        Place::Right
    } else if right < x {
        Place::Left
    } else {
        Place::Below
    }
}

fn labelled(options: ScatterOptions) -> ScatterOptions {
    ScatterOptions {
        labels: PointLabels {
            annotate: true,
            ..PointLabels::default()
        },
        ..options
    }
}

/// The centre of the first marker of `scene`.
fn first_centre(scene: &Scene) -> (f64, f64) {
    mark_items(scene)
        .into_iter()
        .find_map(|item| match item {
            Item::Markers { positions, .. } => {
                Some((f64::from(positions[0].x), f64::from(positions[0].y)))
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn point_labels_avoid_the_scatters_own_bars_and_line() {
    let place = |options: ScatterOptions, xs: &[f64], ys: &[f64]| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap()
            .scatter(nums(xs), ys, labelled(options))
            .unwrap();
        let scene = render(&f);
        let centre = first_centre(&scene);
        label_side(&scene, &format!("{}", ys[0]), centre)
    };
    assert_eq!(
        place(ScatterOptions::default(), &[5.0], &[5.0]),
        Place::Above
    );
    // Its own vertical bar blocks above and below.
    assert_eq!(
        place(with_errors(ends(&[3.0], &[7.0]), false), &[5.0], &[5.0]),
        Place::Right
    );
    // Its own steep line through the point blocks above and below as a stroke.
    assert_eq!(
        place(with_errors(None, true), &[5.0, 4.6, 5.4], &[5.0, 1.0, 9.0]),
        Place::Right
    );
}

// ---- Formats --------------------------------------------------------------------------------

#[test]
fn png_svg_and_pdf_draw_the_own_bars_and_line() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let nan = f64::NAN;
    axes(&mut f)
        .scatter(
            names(&["Fold 1", "Fold 2", "Mean"]),
            &[66.8, 68.1, 67.45],
            ScatterOptions {
                label: "80 epochs".into(),
                ..labelled(with_errors(
                    ends(&[nan, nan, 66.8], &[nan, nan, 68.1]),
                    true,
                ))
            },
        )
        .unwrap();
    let scene = render(&f);
    let color = stroke_items(&scene)[0].1.color;
    let hex = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    let svg = f.to_svg(SvgText::Editable).unwrap();
    assert!(svg.matches(&hex).count() >= 2, "{hex} in {svg}");
    assert!(f.to_png(72).unwrap().starts_with(b"\x89PNG"));
    assert!(f.to_pdf().unwrap().starts_with(b"%PDF"));
    // All three come from one scene, which draws the line and one bar.
    let strokes = stroke_items(&scene);
    assert_eq!((contours(strokes[0].0), contours(strokes[1].0)), (1, 3));
}

// ---- Byte identity ---------------------------------------------------------------------------

#[test]
fn without_bars_or_line_a_scatter_draws_only_its_markers() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(nums(&[1.0, 2.0]), &[2.0, 3.0], ScatterOptions::default())
        .unwrap();
    let scene = render(&f);
    assert!(stroke_items(&scene).is_empty());
    assert_eq!(mark_items(&scene).len(), 1);
}

// ---- Automatic limits -----------------------------------------------------------------------

#[test]
fn labels_of_lines_drawn_by_the_scatter_settle_as_limits_grow() {
    // The lowest point of a line has lines on both sides: above is just blocked before the
    // limits grow to hold its label below, and just clear after. The label keeps the side the
    // limits were grown for, rather than alternate between the two.
    let folds = ["Fold 1", "Fold 2", "Fold 3", "Fold 4", "Fold 5", "Mean"];
    let mut f = Figure::new(6.4, 3.8, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    ax.title("Matched folds: 80 epochs against long training")
        .ylabel("Accuracy (%)");
    let nan = f64::NAN;
    for (label, marker, scores) in [
        ("80 epochs", Marker::Square, [66.8, 68.1, 65.4, 69.0, 67.3]),
        (
            "Long training",
            Marker::Circle,
            [78.4, 80.2, 76.9, 81.7, 79.5],
        ),
    ] {
        // The mean and the population standard deviation of the folds.
        let mean = scores.iter().sum::<f64>() / 5.0;
        let sd = (scores.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / 5.0).sqrt();
        let mut values = scores.to_vec();
        values.push(mean);
        ax.scatter_shared(
            names(&folds),
            nums(&values),
            ScatterOptions {
                label: label.into(),
                marker,
                labels: PointLabels {
                    annotate: true,
                    format: Some(umber_core::TickFormat::fixed(1).unwrap()),
                    ..PointLabels::default()
                },
                ..with_errors(
                    ends(
                        &[nan, nan, nan, nan, nan, mean - sd],
                        &[nan, nan, nan, nan, nan, mean + sd],
                    ),
                    true,
                )
            },
        )
        .unwrap();
    }
    let scene = render(&f);
    let lowest = mark_items(&scene)
        .into_iter()
        .find_map(|item| match item {
            Item::Markers { positions, .. } => {
                Some((f64::from(positions[2].x), f64::from(positions[2].y)))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(label_side(&scene, "65.4", lowest), Place::Below);
    let markers: Vec<_> = mark_items(&scene)
        .into_iter()
        .filter_map(|item| match item {
            Item::Markers { positions, .. } => Some(positions.clone()),
            _ => None,
        })
        .collect();
    let peak = (f64::from(markers[1][3].x), f64::from(markers[1][3].y));
    assert_eq!(label_side(&scene, "81.7", peak), Place::Above);
}

// ---- Points that are not drawn, and the clip of the own line ---------------------------------

/// The marker centres of `scene`, in drawing order.
fn centres(scene: &Scene) -> Vec<(f32, f32)> {
    mark_items(scene)
        .into_iter()
        .filter_map(|item| match item {
            Item::Markers { positions, .. } => {
                Some(positions.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn bars_that_are_not_drawn_leave_the_limits_alone() {
    let build = |x: &[f64], y: &[f64], y_error: Option<ErrorRange>| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .scatter(nums(x), y, with_errors(y_error, false))
            .unwrap();
        render(&f)
    };
    let nan = f64::NAN;
    // A point left out for a missing x keeps no bar, and its ends count nowhere.
    let (x, y) = ([0.0, 1.0, nan], [0.0, 1.0, 0.0]);
    let plain = build(&x, &y, None);
    let gapped = build(&x, &y, ends(&[nan, nan, 1000.0], &[nan, nan, 2000.0]));
    assert_eq!(centres(&gapped), centres(&plain));
    assert!(stroke_items(&gapped).is_empty());
    // Nor does the end of a bar whose other end is missing.
    let (x, y) = ([0.0, 1.0], [0.0, 1.0]);
    let plain = build(&x, &y, None);
    let half = build(&x, &y, ends(&[nan, nan], &[nan, 2000.0]));
    assert_eq!(centres(&half), centres(&plain));
    assert!(stroke_items(&half).is_empty());
}

#[test]
fn a_point_hidden_by_its_mapped_value_loses_its_bar_and_label_and_breaks_the_line() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0, 3.0, 4.0, 5.0]),
            &[2.0, 3.0, 2.25, 2.5, 3.5],
            labelled(ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![0.1, 0.3, f64::NAN, 0.7, 0.9])),
                ..with_errors(
                    ends(&[1.5, 2.5, 2.0, 2.0, 3.0], &[2.5, 3.5, 2.5, 3.0, 4.0]),
                    true,
                )
            }),
        )
        .unwrap();
    let scene = render(&f);
    let strokes = stroke_items(&scene);
    let (line, bars) = (strokes[0].0, strokes[1].0);
    assert_eq!(contours(line), 2);
    assert_eq!(contours(bars), 12, "four bars, each a stem and two caps");
    assert!(!texts(&scene).iter().any(|t| t.text == "2.25"));
}

#[test]
fn the_own_line_widens_the_marks_clip_as_a_line_does() {
    // Explicit x limits at the end points: the round ends of the line reach past the plot.
    let clip_and_line = |connect: bool| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let mut ax = axes(&mut f);
        ax.xlim(0.0, 4.0).unwrap();
        if !connect {
            ax.line(
                nums(&[0.0, 2.0, 4.0]),
                &[1.0, 3.0, 2.0],
                LineOptions {
                    color: ColorSpec::Palette {
                        index: 0,
                        alpha: umber_core::Alpha::OPAQUE,
                    },
                    width: half(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        ax.scatter(
            nums(&[0.0, 2.0, 4.0]),
            &[1.0, 3.0, 2.0],
            with_errors(None, connect),
        )
        .unwrap();
        let scene = render(&f);
        let clip = scene
            .items
            .iter()
            .find_map(|item| match item {
                Item::Group { clip: Some(r), .. } => Some(*r),
                _ => None,
            })
            .unwrap();
        let (path, style) = stroke_items(&scene)[0];
        (clip, path.clone(), style.clone())
    };
    let (own, alone) = (clip_and_line(true), clip_and_line(false));
    assert_eq!(own.2.cap, LineCap::Round);
    assert_eq!(own, alone);
}

#[test]
fn own_bars_and_line_are_half_as_wide_as_standalone_ones() {
    let themes = [
        Theme::pigment(),
        Theme::graphite(),
        Theme::midnight(),
        Theme::pigment().with_line_width(3.5).unwrap(),
    ];
    for theme in themes {
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(nums(&[0.0, 3.0]), &[0.0, 1.0], LineOptions::default())
            .unwrap()
            .y_error_bars(nums(&[0.5]), &[0.0], &[1.0], ErrorBarOptions::default())
            .unwrap()
            .scatter(
                nums(&[1.0, 2.0]),
                &[2.0, 3.0],
                with_errors(ends(&[1.5, 2.5], &[2.5, 3.5]), true),
            )
            .unwrap();
        let scene = render(&f);
        let widths: Vec<f32> = stroke_items(&scene).iter().map(|(_, s)| s.width).collect();
        // The standalone line and bars, then the scatter's own line and bars.
        assert_eq!(widths.len(), 4);
        assert_eq!(widths[0], theme.line_width());
        assert_eq!(widths[1], theme.line_width());
        assert_eq!(widths[2], widths[0] / 2.0);
        assert_eq!(widths[3], widths[1] / 2.0);
    }
}
