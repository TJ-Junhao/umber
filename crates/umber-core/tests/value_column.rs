//! The values that scatters write in a column beside the plot (`Place::Column`), through the
//! public API: where the column goes, its sub-columns and their alignment, its title, the room
//! the layout reserves for it, what it sits beside, and the errors.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    AxesMut, AxisData, ColorBy, Error, Figure, FontLibrary, Grid, GridIndex, Instant,
    LegendPosition, LegendStyle, Marker, Place, Placement, PointColor, PointLabels, Rgba,
    ScaleKind, ScatterGroup, ScatterOptions, SizeSpec, SvgText, Theme, TickFormat,
};

/// Positions agree to this many points.
const TOLERANCE: f64 = 2e-3;
/// The gap between the plot side and a column right of it, and between sub-columns.
const GAP: f64 = 6.0;
/// The gap between the top y tick label and a row above the plot.
const ROW_GAP: f64 = 4.0;

const MODELS: [&str; 4] = ["Baseline", "Dropout", "Gate", "Gate + aug"];

fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
}

fn render(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

fn render_err(figure: &Figure) -> Error {
    figure.to_scene(FontLibrary::global()).unwrap_err()
}

fn nums(v: &[f64]) -> AxisData {
    AxisData::Numbers(v.to_vec())
}

fn names(v: &[&str]) -> AxisData {
    AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect())
}

fn column_labels(format: Option<TickFormat>) -> PointLabels {
    PointLabels {
        annotate: true,
        format,
        place: Place::Column,
        ..PointLabels::default()
    }
}

/// Scatter options that write the values in the column, in `format`, under `label`.
fn in_column(label: &str, format: Option<TickFormat>) -> ScatterOptions {
    ScatterOptions {
        label: label.to_owned(),
        labels: column_labels(format),
        ..ScatterOptions::default()
    }
}

fn fixed(decimals: i64) -> Option<TickFormat> {
    Some(TickFormat::fixed(decimals).unwrap())
}

/// A forest plot of `series` (a label and one mean per model), each writing its values in the
/// column with one decimal, with the column titled `title` when given.
fn forest(series: &[(&str, [f64; 4])], title: Option<&str>) -> Figure {
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
    for (label, means) in series {
        ax.scatter_shared(nums(means), names(&MODELS), in_column(label, fixed(1)))
            .unwrap();
    }
    if let Some(title) = title {
        ax.value_column(title, SizeSpec::Auto);
    }
    f
}

const LONG: (&str, [f64; 4]) = ("Long training", [68.08, 70.7, 74.0, 75.4]);
const SHORT: (&str, [f64; 4]) = ("80 epochs", [66.5, 69.14, 71.64, 73.06]);

/// Every leaf item of `items`, groups flattened, in drawing order.
fn leaves(items: &[Item]) -> Vec<&Item> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Group { items, .. } => out.extend(leaves(items)),
            other => out.push(other),
        }
    }
    out
}

/// The text items of `scene`, in drawing order.
fn texts(scene: &Scene) -> Vec<&TextItem> {
    leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Text(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn contents(scene: &Scene) -> Vec<&str> {
    texts(scene).into_iter().map(|t| t.text.as_str()).collect()
}

/// The last text item of `scene` that reads `content`: values are drawn after the tick labels.
fn text<'a>(scene: &'a Scene, content: &str) -> &'a TextItem {
    texts(scene)
        .into_iter()
        .rfind(|t| t.text == content)
        .unwrap_or_else(|| panic!("no text {content:?} in {:?}", contents(scene)))
}

/// The box of `item`: left, top, right, bottom, from its advance width and font metrics.
fn text_box(scene: &Scene, item: &TextItem) -> [f64; 4] {
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    let layout = fonts.layout(&item.text, &item.style).unwrap();
    let (ascent, descent) = (f64::from(layout.ascent()), f64::from(layout.descent()));
    let (x, y) = (f64::from(item.origin.x), f64::from(item.origin.y));
    [x, y - ascent, x + f64::from(item.width), y + descent]
}

/// The box of the last text of `scene` that reads `content`.
fn boxed(scene: &Scene, content: &str) -> [f64; 4] {
    text_box(scene, text(scene, content))
}

/// The advance width of `prefix` set as `item` is.
fn width_of(scene: &Scene, item: &TextItem, prefix: &str) -> f64 {
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    f64::from(fonts.layout(prefix, &item.style).unwrap().width())
}

/// The centres of every marker of `scene`, in drawing order.
fn markers(scene: &Scene) -> Vec<(f64, f64)> {
    leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Markers { positions, .. } => Some(
                positions
                    .iter()
                    .map(|p| (f64::from(p.x), f64::from(p.y)))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

/// The plot areas of `scene`, left, top, right, bottom: the clips of the marks of each panel.
fn plots(scene: &Scene) -> Vec<[f64; 4]> {
    fn walk(items: &[Item], out: &mut Vec<[f64; 4]>) {
        for item in items {
            if let Item::Group { clip, items } = item {
                if let Some(r) = clip {
                    out.push([
                        f64::from(r.x),
                        f64::from(r.y),
                        f64::from(r.x + r.width),
                        f64::from(r.y + r.height),
                    ]);
                } else {
                    walk(items, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&scene.items, &mut out);
    out
}

fn plot(scene: &Scene) -> [f64; 4] {
    plots(scene)[0]
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < TOLERANCE
}

fn assert_close(a: f64, b: f64, what: &str) {
    assert!(close(a, b), "{what}: {a} != {b}");
}

fn solid(color: Rgba) -> Rgba {
    Rgba { a: 255, ..color }
}

/// The text color of Pigment's first series color, which is a little under 4.5:1 on the
/// background and so is moved darker for text (`tests/series_text.rs` covers the rule).
const FIRST_TEXT: Rgba = Rgba::opaque(189, 79, 52);

// ---- A column right of the plot ------------------------------------------------------------

#[test]
fn a_right_column_writes_each_value_level_with_its_point() {
    let scene = render(&forest(&[LONG], None));
    let plot = plot(&scene);
    let centres = markers(&scene);
    for (value, (_, y)) in ["68.1", "70.7", "74.0", "75.4"].into_iter().zip(centres) {
        let [left, top, _, bottom] = boxed(&scene, value);
        assert!(left >= plot[2] + GAP - TOLERANCE, "{value} at {left}");
        assert_close((top + bottom) / 2.0, y, value);
        assert_eq!(text(&scene, value).color, FIRST_TEXT);
    }
}

#[test]
fn each_series_has_its_own_sub_column_in_mark_order() {
    let scene = render(&forest(&[SHORT, LONG], None));
    let plot = plot(&scene);
    let first = boxed(&scene, "66.5");
    let second = boxed(&scene, "68.1");
    // Left to right in mark order, the first next to the plot, a fixed gap between them.
    assert_close(first[0], plot[2] + GAP, "first sub-column");
    assert_close(second[0], first[2] + GAP, "second sub-column");
    // Both rows of a model are level.
    assert_close(first[1], second[1], "one row");
    let theme = Theme::pigment();
    let palette = theme.palette();
    assert_eq!(text(&scene, "66.5").color, FIRST_TEXT);
    assert_eq!(text(&scene, "68.1").color, solid(palette[1]));
}

#[test]
fn each_group_has_its_own_sub_column() {
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .grouped_scatter(
            vec![
                ScatterGroup::new("80 epochs", nums(&SHORT.1), names(&MODELS)),
                ScatterGroup::new("Long training", nums(&LONG.1), names(&MODELS)),
            ],
            Marker::Circle,
            SizeSpec::Auto,
            column_labels(fixed(1)),
            false,
            umber_core::MarkRender::Vector,
        )
        .unwrap();
    let scene = render(&f);
    let (first, second) = (boxed(&scene, "69.1"), boxed(&scene, "70.7"));
    assert_close(second[0], first[2] + GAP, "group sub-columns");
    let theme = Theme::pigment();
    let palette = theme.palette();
    assert_eq!(text(&scene, "69.1").color, FIRST_TEXT);
    assert_eq!(text(&scene, "70.7").color, solid(palette[1]));
}

#[test]
fn color_mapped_values_take_the_text_color() {
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .scatter_shared(
            nums(&LONG.1),
            names(&MODELS),
            ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![0.1, 0.4, 0.6, 0.9])),
                ..in_column("", fixed(1))
            },
        )
        .unwrap();
    let scene = render(&f);
    assert_eq!(
        text(&scene, "74.0").color,
        solid(Theme::pigment().text_color())
    );
}

#[test]
fn values_in_one_notation_align_on_the_decimal_point() {
    // Plain decimals of different lengths: the points (or where they would be) line up.
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .scatter_shared(
            nums(&[5.25, 12.5, 100.0, 7.0]),
            names(&MODELS),
            in_column("", None),
        )
        .unwrap();
    let scene = render(&f);
    let anchor = |value: &str| {
        let item = text(&scene, value);
        let before = value.split('.').next().unwrap();
        f64::from(item.origin.x) + width_of(&scene, item, before)
    };
    let at = anchor("5.25");
    for value in ["12.5", "100", "7"] {
        assert_close(anchor(value), at, value);
    }
    // Plain and scientific values mixed: right-aligned.
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(
        ScaleKind::Log {
            base: umber_core::LogBase::TEN,
        },
        ScaleKind::Category,
    )
    .unwrap()
    .scatter_shared(
        nums(&[5.25, 2.5e6, 12.5, 7.0]),
        names(&MODELS),
        in_column("", None),
    )
    .unwrap();
    let scene = render(&f);
    let right = boxed(&scene, "5.25")[2];
    for value in ["2.5e6", "12.5", "7"] {
        assert_close(boxed(&scene, value)[2], right, value);
    }
}

#[test]
fn times_in_a_row_are_centred_on_their_points() {
    // A scatter on a category y axis takes numbers on x, so times are values only on a time y
    // axis, whose values go in a row above the plot.
    let day = |d: i64| Instant::from_micros((20_515 + d) * 86_400_000_000).unwrap();
    let times = AxisData::Times(vec![Some(day(0)), Some(day(14)), Some(day(40))]);
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Time)
        .unwrap()
        .scatter_shared(names(&["A", "B", "C"]), times, in_column("", None))
        .unwrap();
    let scene = render(&f);
    let plot = plot(&scene);
    for ((x, _), value) in markers(&scene)
        .into_iter()
        .zip(["Mar 3", "Mar 17", "Apr 12"])
    {
        let [left, _, right, bottom] = boxed(&scene, value);
        assert_close((left + right) / 2.0, x, value);
        assert!(bottom < plot[1], "{value}");
    }
}

#[test]
fn a_log_value_axis_writes_the_data_values() {
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(
        ScaleKind::Log {
            base: umber_core::LogBase::TEN,
        },
        ScaleKind::Category,
    )
    .unwrap()
    .scatter_shared(
        nums(&[10.0, 1000.0, 250.0, 3.5]),
        names(&MODELS),
        in_column("", None),
    )
    .unwrap();
    let scene = render(&f);
    let plot = plot(&scene);
    for ((_, y), value) in markers(&scene)
        .into_iter()
        .zip(["10", "1000", "250", "3.5"])
    {
        let [left, top, _, bottom] = boxed(&scene, value);
        assert!(left > plot[2], "{value}");
        assert_close((top + bottom) / 2.0, y, value);
    }
}

// ---- A row above the plot ------------------------------------------------------------------

/// How far a y tick label at the top of the plot `plot` of `scene` rises above it: its ascent
/// less the 0.35 sizes its baseline sits below its tick.
fn tick_rise(scene: &Scene, plot: [f64; 4]) -> f64 {
    let tick = texts(scene)
        .into_iter()
        .find(|t| f64::from(t.origin.x) < plot[0] && t.text.parse::<f64>().is_ok())
        .unwrap();
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    let layout = fonts.layout(&tick.text, &tick.style).unwrap();
    (f64::from(layout.ascent()) - 0.35 * f64::from(tick.style.size)).max(0.0)
}

/// A matched-folds chart with the values of `series` in a row above the plot, titled `title`
/// when given.
fn folds(series: &[[f64; 3]], title: Option<&str>) -> Figure {
    let mut f = Figure::new(6.4, 3.8, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    ax.ylabel("Accuracy (%)");
    for (i, values) in series.iter().enumerate() {
        ax.scatter_shared(
            names(&["Fold 1", "Fold 2", "Fold 3"]),
            nums(values),
            in_column(&format!("Series {i}"), fixed(1)),
        )
        .unwrap();
    }
    if let Some(title) = title {
        ax.value_column(title, SizeSpec::Auto);
    }
    f
}

#[test]
fn a_row_above_centres_each_value_on_its_point_in_sub_rows_top_down() {
    let scene = render(&folds(&[[66.8, 68.1, 65.4], [78.4, 80.2, 76.9]], None));
    let plot = plot(&scene);
    let centres = markers(&scene);
    let top_tick = plot[1] - tick_rise(&scene, plot);
    for (values, row) in [(["66.8", "68.1", "65.4"], 0), (["78.4", "80.2", "76.9"], 1)] {
        for (i, value) in values.into_iter().enumerate() {
            let [left, _, right, bottom] = boxed(&scene, value);
            assert_close((left + right) / 2.0, centres[3 * row + i].0, value);
            assert!(
                bottom <= top_tick - ROW_GAP + TOLERANCE,
                "{value} above the plot"
            );
        }
    }
    // Top down in mark order, a fixed gap between the sub-rows.
    let (first, second) = (boxed(&scene, "66.8"), boxed(&scene, "78.4"));
    assert_close(second[1], first[3] + GAP, "second sub-row");
    // The lower sub-row sits on the rise of the top y tick label.
    assert_close(second[3], top_tick - ROW_GAP, "row bottom");
    // The y axis label sits above the row.
    assert!(boxed(&scene, "Accuracy (%)")[3] <= first[1]);
}

#[test]
fn values_on_two_numeric_axes_go_in_a_row_above() {
    let mut f = Figure::new(6.4, 3.8, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            nums(&[0.0, 5.0, 10.0]),
            &[1.5, 2.5, 3.5],
            in_column("", None),
        )
        .unwrap();
    let scene = render(&f);
    let plot = plot(&scene);
    for ((x, _), value) in markers(&scene).into_iter().zip(["1.5", "2.5", "3.5"]) {
        let [left, _, right, bottom] = boxed(&scene, value);
        assert_close((left + right) / 2.0, x, value);
        assert!(bottom < plot[1], "{value}");
    }
}

// ---- The title -----------------------------------------------------------------------------

#[test]
fn the_title_is_centred_over_a_column_level_with_the_y_label() {
    let mut f = forest(&[SHORT, LONG], Some("Mean (%)"));
    axes(&mut f).ylabel("Model").title("Accuracy");
    let scene = render(&f);
    let title = text(&scene, "Mean (%)");
    let [left, _, right, bottom] = text_box(&scene, title);
    let (first, second) = (boxed(&scene, "66.5"), boxed(&scene, "68.1"));
    assert_close(
        (left + right) / 2.0,
        (first[0] + second[2]) / 2.0,
        "centred",
    );
    assert_close(
        f64::from(title.origin.y),
        f64::from(text(&scene, "Model").origin.y),
        "level",
    );
    assert!(bottom <= plot(&scene)[1], "above the plot");
    // In the axis label style, in the text color, under the panel title.
    let model = text(&scene, "Model");
    assert_eq!(title.style, model.style);
    assert_eq!(title.color, Theme::pigment().text_color());
    assert!(boxed(&scene, "Accuracy")[3] < text_box(&scene, title)[1]);
}

#[test]
fn the_title_of_a_row_sits_before_it_in_the_y_tick_label_column() {
    let scene = render(&folds(
        &[[66.8, 68.1, 65.4], [78.4, 80.2, 76.9]],
        Some("Mean"),
    ));
    let plot = plot(&scene);
    let [_, top, right, bottom] = boxed(&scene, "Mean");
    assert_close(right, plot[0] - GAP, "right-aligned to the plot edge");
    let (first, second) = (boxed(&scene, "66.8"), boxed(&scene, "78.4"));
    let item = text(&scene, "Mean");
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    let layout = fonts.layout("Mean", &item.style).unwrap();
    let centre =
        f64::from(item.origin.y) - (f64::from(layout.ascent()) - f64::from(layout.descent())) / 2.0;
    assert_close(centre, (first[1] + second[3]) / 2.0, "centred on the row");
    assert!(top < bottom);
}

#[test]
fn a_second_call_replaces_the_title_and_its_size() {
    let mut f = forest(&[LONG], Some("Mean"));
    axes(&mut f).value_column("Mean (%)", SizeSpec::pt(14.0).unwrap());
    let scene = render(&f);
    assert!(!contents(&scene).contains(&"Mean"));
    assert_eq!(text(&scene, "Mean (%)").style.size, 14.0);
    let options = f
        .axes_at(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
        .value_column_options()
        .unwrap();
    assert_eq!(options.title(), "Mean (%)");
}

// ---- The room the layout reserves ----------------------------------------------------------

#[test]
fn the_plot_narrows_by_exactly_the_room_of_the_column() {
    let plain = {
        let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
        let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
        for (label, means) in [SHORT, LONG] {
            ax.scatter_shared(
                nums(&means),
                names(&MODELS),
                ScatterOptions {
                    label: label.to_owned(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        }
        plot(&render(&f))
    };
    // Two sub-columns: the gap, each sub-column and the gap between them.
    let scene = render(&forest(&[SHORT, LONG], None));
    let width = |v: &str| boxed(&scene, v)[2] - boxed(&scene, v)[0];
    let room = GAP + width("66.5") + GAP + width("68.1");
    let narrowed = plot(&scene);
    assert_close(plain[2] - narrowed[2], room, "two sub-columns");
    assert_close(plain[0], narrowed[0], "left edge");
    // A title wider than the sub-columns widens the column to its width.
    let title = "Mean accuracy over five folds (%)";
    let scene = render(&forest(&[SHORT, LONG], Some(title)));
    let title_width = boxed(&scene, title)[2] - boxed(&scene, title)[0];
    assert!(title_width > room - GAP);
    assert_close(plain[2] - plot(&scene)[2], GAP + title_width, "title width");
}

#[test]
fn the_plot_gets_shorter_by_exactly_the_room_of_the_row() {
    let plain = {
        let mut f = Figure::new(6.4, 3.8, Theme::pigment()).unwrap();
        let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        ax.ylabel("Accuracy (%)");
        for (i, values) in [[66.8, 68.1, 65.4], [78.4, 80.2, 76.9]].iter().enumerate() {
            ax.scatter_shared(
                names(&["Fold 1", "Fold 2", "Fold 3"]),
                nums(values),
                ScatterOptions {
                    label: format!("Series {i}"),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        }
        render(&f)
    };
    let scene = render(&folds(&[[66.8, 68.1, 65.4], [78.4, 80.2, 76.9]], None));
    let height = |v: &str| boxed(&scene, v)[3] - boxed(&scene, v)[1];
    let room = ROW_GAP + height("66.8") + GAP + height("78.4");
    assert_close(plot(&scene)[1] - plot(&plain)[1], room, "shorter");
    assert_close(plot(&scene)[3], plot(&plain)[3], "bottom edge");
    assert_close(plot(&scene)[0], plot(&plain)[0], "left edge");
    // A row title wider than the y tick labels widens their column to its width: it starts
    // at the outer padding, and the plot a gap after it.
    let title = "Accuracy on each fold";
    let scene = render(&folds(
        &[[66.8, 68.1, 65.4], [78.4, 80.2, 76.9]],
        Some(title),
    ));
    let plot = plot(&scene);
    let [left, _, right, _] = boxed(&scene, title);
    assert_close(left, 12.0, "title at the padding");
    assert_close(right, plot[0] - GAP, "plot after the title");
    for tick in texts(&scene)
        .into_iter()
        .filter(|t| f64::from(t.origin.x) < plot[0] && t.text.parse::<f64>().is_ok())
    {
        assert!(f64::from(tick.width) < right - left, "{}", tick.text);
    }
}

#[test]
fn column_values_never_change_the_limits() {
    // The points sit at the same fractions of the plot with and without the column.
    let fractions = |f: &Figure| {
        let scene = render(f);
        let [left, _, right, _] = plot(&scene);
        markers(&scene)
            .into_iter()
            .map(|(x, _)| (x - left) / (right - left))
            .collect::<Vec<_>>()
    };
    let mut plain = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    plain
        .add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .scatter_shared(nums(&LONG.1), names(&MODELS), ScatterOptions::default())
        .unwrap();
    let (a, b) = (fractions(&plain), fractions(&forest(&[LONG], Some("Mean"))));
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(&b) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }
}

#[test]
fn the_column_sits_outside_a_colorbar() {
    let mut f = Figure::new(7.0, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .scatter_shared(
            nums(&LONG.1),
            names(&MODELS),
            ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![0.1, 0.4, 0.6, 0.9])),
                ..in_column("", fixed(1))
            },
        )
        .unwrap()
        .value_column("Mean (%)", SizeSpec::Auto);
    let scene = render(&f);
    let plot = plot(&scene);
    let column = ["68.1", "70.7", "74.0", "75.4", "Mean (%)"];
    let left = column
        .into_iter()
        .map(|v| boxed(&scene, v)[0])
        .fold(f64::INFINITY, f64::min);
    // The colorbar's tick labels, right of the plot, end a gap before the column starts.
    let ticks: Vec<_> = texts(&scene)
        .into_iter()
        .filter(|t| f64::from(t.origin.x) > plot[2] && !column.contains(&t.text.as_str()))
        .collect();
    assert!(ticks.len() >= 2, "{:?}", contents(&scene));
    for tick in ticks {
        let right = text_box(&scene, tick)[2];
        assert!(
            right + GAP <= left + TOLERANCE,
            "{} ends at {right}",
            tick.text
        );
    }
}

#[test]
fn a_row_sits_under_the_y_labels_of_both_axes_beside_twin_tick_labels() {
    let mut f = Figure::new(6.4, 3.8, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    ax.ylabel("Accuracy (%)")
        .scatter_shared(
            names(&["Fold 1", "Fold 2", "Fold 3"]),
            nums(&[66.8, 68.1, 65.4]),
            in_column("", fixed(1)),
        )
        .unwrap()
        .value_column("Mean", SizeSpec::Auto);
    ax.twin_y(ScaleKind::Linear)
        .unwrap()
        .ylabel("Wall time (h)")
        .scatter_shared(
            names(&["Fold 1", "Fold 2", "Fold 3"]),
            nums(&[1200.0, 1500.0, 1800.0]),
            ScatterOptions::default(),
        )
        .unwrap();
    let scene = render(&f);
    let plot = plot(&scene);
    let row_top = ["66.8", "68.1", "65.4"]
        .into_iter()
        .map(|v| boxed(&scene, v)[1])
        .fold(f64::INFINITY, f64::min);
    for label in ["Accuracy (%)", "Wall time (h)"] {
        assert!(boxed(&scene, label)[3] <= row_top, "{label} above the row");
    }
    // Both y axis labels stay level, the twin's at the plot's right edge.
    assert_close(
        f64::from(text(&scene, "Accuracy (%)").origin.y),
        f64::from(text(&scene, "Wall time (h)").origin.y),
        "level",
    );
    assert_close(boxed(&scene, "Wall time (h)")[2], plot[2], "twin label");
    // The twin's tick labels stay right of the plot.
    assert!(boxed(&scene, "1400")[0] > plot[2]);
}

#[test]
fn a_right_legend_stays_outside_the_column() {
    let mut f = forest(&[SHORT, LONG], Some("Mean (%)"));
    axes(&mut f)
        .legend_at(LegendStyle::Header, LegendPosition::Right, SizeSpec::Auto)
        .unwrap();
    let scene = render(&f);
    let column_right = ["68.1", "70.7", "74.0", "75.4", "Mean (%)"]
        .into_iter()
        .map(|v| boxed(&scene, v)[2])
        .fold(f64::NEG_INFINITY, f64::max);
    for entry in ["80 epochs", "Long training"] {
        assert!(boxed(&scene, entry)[0] > column_right, "{entry}");
    }
}

#[test]
fn each_panel_of_a_grid_reserves_its_own_column() {
    let mut f = Figure::with_grid(10.0, 3.6, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for (col, (label, means)) in [SHORT, LONG].into_iter().enumerate() {
        let at = Placement {
            row: GridIndex::At(0),
            col: GridIndex::At(col as i64),
            ..Placement::default()
        };
        f.add_axes_at(at, ScaleKind::Linear, ScaleKind::Category)
            .unwrap()
            .scatter_shared(nums(&means), names(&MODELS), in_column(label, fixed(1)))
            .unwrap()
            .value_column(&format!("Mean {col}"), SizeSpec::Auto);
    }
    let scene = render(&f);
    let plots = plots(&scene);
    assert_eq!(plots.len(), 2);
    // Each title, wider than its values, starts a gap right of its own plot.
    for (plot, (title, value)) in plots.iter().zip([("Mean 0", "73.1"), ("Mean 1", "75.4")]) {
        assert_close(boxed(&scene, title)[0], plot[2] + GAP, title);
        assert!(boxed(&scene, value)[0] > plot[2] + GAP, "{value}");
    }
    // The left panel's column ends before the right panel's y tick labels.
    let right_ticks = texts(&scene)
        .into_iter()
        .filter(|t| t.text == "Baseline")
        .map(|t| text_box(&scene, t)[0])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(boxed(&scene, "Mean 0")[2] < right_ticks);
    assert!(boxed(&scene, "73.1")[2] < right_ticks);
}

// ---- Which points ----------------------------------------------------------------------

#[test]
fn points_outside_explicit_limits_nan_and_unmapped_points_get_no_value() {
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .xlim(60.0, 75.0)
        .unwrap()
        .scatter_shared(
            nums(&[68.08, f64::NAN, 74.0, 75.4]),
            names(&MODELS),
            ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![0.1, 0.4, f64::NAN, 0.9])),
                ..in_column("", fixed(1))
            },
        )
        .unwrap();
    let scene = render(&f);
    text(&scene, "68.1");
    for value in ["74.0", "75.4"] {
        assert!(!contents(&scene).contains(&value), "{value}");
    }
}

// ---- Errors --------------------------------------------------------------------------------

#[test]
fn a_title_without_a_column_scatter_is_a_usage_error() {
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .scatter_shared(
            nums(&LONG.1),
            names(&MODELS),
            ScatterOptions {
                labels: PointLabels {
                    annotate: true,
                    ..PointLabels::default()
                },
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .value_column("Mean (%)", SizeSpec::Auto);
    let err = render_err(&f);
    assert!(matches!(err, Error::Usage { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): value_column() is set on these axes, but none of their scatters \
         writes its values in the column."
    );
    assert_eq!(
        err.hint(),
        Some(
            "pass annotate=True and place=Place.COLUMN to the scatter whose values the column \
             shows, or remove the value_column() call."
        )
    );
    // On a grid the message names the cell.
    let mut f = Figure::with_grid(10.0, 3.6, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in 0..2 {
        let at = Placement {
            row: GridIndex::At(0),
            col: GridIndex::At(col),
            ..Placement::default()
        };
        let mut ax = f
            .add_axes_at(at, ScaleKind::Linear, ScaleKind::Category)
            .unwrap();
        ax.scatter_shared(nums(&LONG.1), names(&MODELS), ScatterOptions::default())
            .unwrap();
        if col == 1 {
            ax.value_column("", SizeSpec::Auto);
        }
    }
    let err = render_err(&f);
    assert!(
        err.to_string()
            .starts_with("Figure.to_scene(): row 0, col 1: value_column() is set"),
        "{err}"
    );
}

#[test]
fn values_that_overlap_in_a_sub_column_are_a_style_error() {
    // Two points of one series on one category.
    let mut f = Figure::new(6.4, 3.6, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .scatter_shared(
            nums(&[66.5, 70.25, 71.0]),
            names(&["Baseline", "Gate", "Baseline"]),
            in_column("80 epochs", None),
        )
        .unwrap();
    let err = render_err(&f);
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the values \"66.5\" and \"71\" of the series \"80 epochs\" overlap \
         in the value column: their points lie too close along the y axis."
    );
    assert_eq!(
        err.hint(),
        Some("make the figure taller, pass a smaller label_size=, or use another place.")
    );
    // Two numeric axes: the row above, whose values overlap along x.
    let mut f = Figure::new(4.0, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            nums(&[0.0, 0.1, 10.0]),
            &[1.25, 2.5, 3.0],
            in_column("", None),
        )
        .unwrap();
    let err = render_err(&f);
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the values \"1.25\" and \"2.5\" of scatter 1 overlap in the value \
         row: their points lie too close along the x axis."
    );
    assert_eq!(
        err.hint(),
        Some("make the figure wider, pass a smaller label_size=, or use another place.")
    );
    // Sub-columns of two series never meet: each has its own.
    render(&forest(&[SHORT, LONG], Some("Mean (%)")));
}

#[test]
fn a_column_on_twin_axes_is_refused() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    let mut twin = ax.twin_y(ScaleKind::Linear).unwrap();
    let err = twin
        .scatter(nums(&[1.0]), &[2.0], in_column("", None))
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err}");
    assert_eq!(err.parameter(), Some("place"));
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): place is Place.COLUMN on twin axes; the value column belongs to the \
         primary axes of the plot."
    );
    let err = twin
        .grouped_scatter(
            vec![ScatterGroup::new("A", nums(&[1.0]), nums(&[2.0]))],
            Marker::Circle,
            SizeSpec::Auto,
            column_labels(None),
            false,
            umber_core::MarkRender::Vector,
        )
        .unwrap_err();
    assert_eq!(err.parameter(), Some("place"));
    assert!(twin.marks().is_empty());
}

#[test]
fn the_title_is_one_line() {
    let f = forest(&[LONG], Some("Mean\n(%)"));
    let err = render_err(&f);
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert!(
        err.to_string()
            .contains("the value column title \"Mean\\n(%)\" holds a line break"),
        "{err}"
    );
}

#[test]
fn the_no_room_hint_of_labels_in_the_plot_suggests_a_column() {
    // A label right of the last category leaves the plot: category axes never grow.
    let mut f = Figure::new(2.0, 2.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .scatter_shared(
            names(&["A", "B"]),
            nums(&[1.0, 123_456.5]),
            ScatterOptions {
                labels: PointLabels {
                    annotate: true,
                    place: Place::Right,
                    ..PointLabels::default()
                },
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    let err = render_err(&f);
    assert_eq!(
        err.hint(),
        Some(
            "enlarge the figure, use place=Place.COLUMN or another place, or a smaller label size."
        ),
        "{err}"
    );
}

/// Row values written with a long prefix and suffix, which reach far past their points.
fn measured() -> ScatterOptions {
    ScatterOptions {
        labels: PointLabels {
            annotate: true,
            format: Some(
                TickFormat::fixed(3)
                    .unwrap()
                    .affix("Measured = ", " units")
                    .unwrap(),
            ),
            place: Place::Column,
            ..PointLabels::default()
        },
        ..ScatterOptions::default()
    }
}

#[test]
fn row_values_that_leave_the_figure_are_a_style_error() {
    let mut f = Figure::new(4.0, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .scatter(nums(&[0.0, 1.0]), &[123.456, 789.012], measured())
        .unwrap();
    let err = render_err(&f);
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the value \"Measured = 123.456 units\" of scatter 1 reaches outside \
         the figure, or less than 6 pt from its left or right edge."
    );
    assert_eq!(
        err.hint(),
        Some("make the figure wider, pass a smaller label_size=, or use another place.")
    );
    // The same figure and limits with the points moved in from the plot's edges: the values
    // fit, so the error is about where the values sit, not about the values themselves.
    let mut f = Figure::new(4.0, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .scatter(nums(&[0.25, 0.75]), &[123.456, 789.012], measured())
        .unwrap();
    let scene = render(&f);
    assert!(boxed(&scene, "Measured = 123.456 units")[0] >= 6.0);
    assert!(boxed(&scene, "Measured = 789.012 units")[2] <= 4.0 * 72.0 - 6.0);
}

/// One row value at `x` between explicit limits 0 and 1, on a figure 4 in wide.
fn edge_value(x: f64) -> Figure {
    let mut f = Figure::new(4.0, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1000.0)
        .unwrap()
        .scatter(nums(&[x]), &[123.456], measured())
        .unwrap();
    f
}

#[test]
fn row_values_keep_6_pt_from_the_canvas_edge() {
    // Where the value's left end falls for a point at `x`: the plot and the value's width do
    // not depend on `x`, since the limits are explicit.
    let scene = render(&edge_value(0.5));
    let [plot_left, _, plot_right, _] = plot(&scene);
    let [left, _, right, _] = boxed(&scene, "Measured = 123.456 units");
    let half = (right - left) / 2.0;
    let at = |edge: f64| (edge + half - plot_left) / (plot_right - plot_left);
    // Just inside the 6 pt inset at the left edge of the canvas renders there.
    let scene = render(&edge_value(at(6.05)));
    assert_close(boxed(&scene, "Measured = 123.456 units")[0], 6.05, "inside");
    // Just outside it, though still on the canvas, raises.
    let err = render_err(&edge_value(at(5.95)));
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert!(
        err.to_string()
            .ends_with("less than 6 pt from its left or right edge."),
        "{err}"
    );
}

#[test]
fn row_values_that_reach_into_a_neighbouring_panel_are_a_style_error() {
    let mut f = Figure::with_grid(8.0, 3.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    let at = |col: i64| Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        ..Placement::default()
    };
    f.add_axes_at(at(0), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .scatter(nums(&[0.0, 1.0]), &[123.456, 789.012], measured())
        .unwrap();
    f.add_axes_at(at(1), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(nums(&[0.0, 1.0]), &[3.0, 4.0], in_column("", None))
        .unwrap()
        .value_column("Mean", SizeSpec::Auto);
    let err = render_err(&f);
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): row 0, col 0: the value \"Measured = 123.456 units\" of scatter 1 \
         reaches outside the panel, or less than 6 pt from the figure's left or right edge."
    );
}

#[test]
fn a_column_overlap_is_judged_against_the_settled_layout() {
    // An automatic label above a point on a line first leaves the plot, so the first pass
    // lays the plot out before the limits grow, narrower than the settled one; the row's
    // values fit the settled plot.
    let categories = ["A", "B", "C", "D"];
    let mut f = Figure::new(2.1, 1.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .line(
            names(&categories),
            &[1.0; 4],
            umber_core::LineOptions::default(),
        )
        .unwrap()
        .scatter_shared(
            names(&["B"]),
            nums(&[1.0]),
            ScatterOptions {
                labels: PointLabels {
                    annotate: true,
                    format: fixed(4),
                    ..PointLabels::default()
                },
                y_error: Some(umber_core::ErrorRange::Numbers {
                    low: std::sync::Arc::from(&[0.0][..]),
                    high: std::sync::Arc::from(&[1.0][..]),
                }),
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .scatter_shared(
            names(&categories),
            nums(&[0.2, 0.3, 0.4, 0.5]),
            in_column("", fixed(4)),
        )
        .unwrap();
    let scene = render(&f);
    for value in ["1.0000", "0.2000", "0.3000", "0.4000", "0.5000"] {
        text(&scene, value);
    }
    for pair in ["0.2000", "0.3000", "0.4000", "0.5000"].windows(2) {
        assert!(
            boxed(&scene, pair[0])[2] <= boxed(&scene, pair[1])[0],
            "{pair:?}"
        );
    }
}

#[test]
fn a_tall_row_title_gets_room_and_is_centred_with_the_row() {
    let mut f = Figure::new(5.0, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .scatter_shared(
            names(&["A", "B", "C"]),
            nums(&[1.0, 2.0, 3.0]),
            in_column("", None),
        )
        .unwrap()
        .ylabel("Response")
        .title("Panel title")
        .value_column("Mean", SizeSpec::pt(48.0).unwrap());
    let scene = render(&f);
    let plot = plot(&scene);
    let title = boxed(&scene, "Mean");
    let label = boxed(&scene, "Response");
    assert!(label[3] <= title[1] + TOLERANCE, "{label:?} over {title:?}");
    assert!(boxed(&scene, "Panel title")[3] <= label[1] + TOLERANCE);
    assert!(title[3] <= plot[1] - tick_rise(&scene, plot) - ROW_GAP + TOLERANCE);
    // The values sit at the title's vertical centre.
    let item = text(&scene, "Mean");
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    let layout = fonts.layout("Mean", &item.style).unwrap();
    let centre =
        f64::from(item.origin.y) - (f64::from(layout.ascent()) - f64::from(layout.descent())) / 2.0;
    let value = boxed(&scene, "2");
    assert_close((value[1] + value[3]) / 2.0, centre, "centred");
}

// ---- Output --------------------------------------------------------------------------------

#[test]
fn png_svg_and_pdf_render_the_column() {
    let f = forest(&[SHORT, LONG], Some("Mean (%)"));
    let svg = f.to_svg(SvgText::Editable, 300).unwrap();
    for value in [">66.5</text>", ">75.4</text>", ">Mean (%)</text>"] {
        assert!(svg.contains(value), "{value} in {svg}");
    }
    let png = f.to_png(72).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let pdf = f.to_pdf(300).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    // All three come from one scene, which holds each value once.
    let scene = render(&f);
    for value in ["66.5", "75.4", "Mean (%)"] {
        assert_eq!(contents(&scene).iter().filter(|t| **t == value).count(), 1);
    }
}
