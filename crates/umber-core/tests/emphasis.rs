//! Emphasis: highlighted category tick labels, outlined heatmap cells and cell edges, and the
//! theme's emphasis color they share.

#![allow(clippy::unwrap_used, clippy::panic)]

#[path = "support/pdf_check.rs"]
mod pdf_check;

use std::sync::Arc;

use umber_core::{
    AxisData, BarOptions, ColorSpec, Error, Figure, FontLibrary, Grid, GridIndex, HeatmapOptions,
    Item, LineOptions, PathVerb, Placement, Render, Rgba, ScaleKind, Share, SvgText, Theme, Weight,
};

/// The one cell at `row`, `col`.
fn at(row: i64, col: i64) -> Placement {
    Placement {
        row: GridIndex::At(row),
        col: GridIndex::At(col),
        ..Placement::default()
    }
}

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn categories(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i:02}")).collect()
}

/// A figure of bars on a category x axis of `x`, `width` inches wide.
fn bars(x: &[String], width: f64, theme: Theme) -> Figure {
    let mut f = Figure::new(width, 3.0, theme).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .bar(x.to_vec(), &vec![1.0; x.len()], BarOptions::default())
        .unwrap();
    f
}

/// A figure of one heatmap of `x` by `y` with `options`, 5 by 4 inches.
fn heatmap(x: &[&str], y: &[&str], values: &[f64], options: HeatmapOptions) -> Figure {
    heatmap_sized(x, y, values, options, (5.0, 4.0))
}

fn heatmap_sized(
    x: &[&str],
    y: &[&str],
    values: &[f64],
    options: HeatmapOptions,
    (width, height): (f64, f64),
) -> Figure {
    let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(&names(x), &names(y), values, options)
        .unwrap();
    f
}

fn axes(f: &mut Figure) -> umber_core::AxesMut<'_> {
    f.axes_at_mut(GridIndex::Auto, GridIndex::Auto).unwrap()
}

/// The text items of the chrome group of the first axes: its tick labels and titles.
fn chrome_texts(f: &Figure) -> Vec<umber_core::scene::TextItem> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let Item::Group { clip: None, items } = &scene.items[0] else {
        panic!("the chrome group comes first: {:?}", scene.items[0])
    };
    items
        .iter()
        .filter_map(|item| match item {
            Item::Text(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The weight and color of the chrome text `text`.
fn label(f: &Figure, text: &str) -> (Weight, Rgba) {
    let texts = chrome_texts(f);
    let found = texts
        .iter()
        .find(|item| item.text == text)
        .unwrap_or_else(|| panic!("no label {text:?} among {texts:?}"));
    (found.style.weight, found.color)
}

/// The items of the clipped marks group of the first axes.
fn marks(f: &Figure) -> Vec<Item> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    match &scene.items[1] {
        Item::Group {
            clip: Some(_),
            items,
        } => items.clone(),
        other => panic!("expected the marks group, got {other:?}"),
    }
}

/// The corners of a path of straight edges, in order.
fn corners(verbs: &[PathVerb]) -> Vec<(f64, f64)> {
    verbs
        .iter()
        .filter_map(|verb| match *verb {
            PathVerb::MoveTo(p) | PathVerb::LineTo(p) => Some((f64::from(p.x), f64::from(p.y))),
            _ => None,
        })
        .collect()
}

/// A stroke: its corners, color and width.
type Stroke = (Vec<(f64, f64)>, Rgba, f32);

/// The strokes among `items`.
fn strokes(items: &[Item]) -> Vec<Stroke> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { path, style } => Some((corners(path.verbs()), style.color, style.width)),
            _ => None,
        })
        .collect()
}

/// The left, top, right and bottom edges of the cells of a fully drawn vector heatmap of
/// `columns` by `rows` cells: the first fill starts at the top left of the grid, and the last
/// one, which reaches under no neighbour, ends at its bottom right.
fn grid(items: &[Item], (columns, rows): (usize, usize)) -> (f64, f64, f64, f64) {
    let fills: Vec<Vec<(f64, f64)>> = items
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => Some(corners(path.verbs())),
            _ => None,
        })
        .collect();
    assert_eq!(fills.len(), columns * rows, "one fill per cell");
    let (left, top) = fills[0][0];
    let (right, bottom) = fills[fills.len() - 1][2];
    (left, top, right, bottom)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn highlighted_labels_are_semibold_in_the_emphasis_color() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let x = names(&["North", "South", "East"]);
        let mut f = bars(&x, 5.0, theme.clone());
        axes(&mut f)
            .xtick_highlight(names(&["South"]), ColorSpec::AUTO)
            .unwrap();
        assert_eq!(
            label(&f, "South"),
            (Weight::SemiBold, theme.emphasis_color())
        );
        for other in ["North", "East"] {
            assert_eq!(label(&f, other), (Weight::Regular, theme.muted_color()));
        }
    }
    // On a category y axis, with the x labels of the same axes unchanged.
    let mut f = Figure::new(5.0, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names(&["a", "b"]),
            &names(&["p", "q", "r"]),
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            HeatmapOptions::default(),
        )
        .unwrap()
        .ytick_highlight(names(&["q", "r"]), ColorSpec::AUTO)
        .unwrap();
    let emphasis = Theme::pigment().emphasis_color();
    assert_eq!(label(&f, "q"), (Weight::SemiBold, emphasis));
    assert_eq!(label(&f, "r"), (Weight::SemiBold, emphasis));
    assert_eq!(label(&f, "p").0, Weight::Regular);
    assert_eq!(label(&f, "a").0, Weight::Regular);
}

#[test]
fn highlighted_labels_are_set_in_the_bold_face() {
    // The same label is wider in the SemiBold face, and the layout measures it so: a bold
    // label moves the plot's left edge by its extra width on a category y axis.
    let y = names(&["Washing machine", "Fan"]);
    let figure = |highlight: &[&str]| {
        let mut f = Figure::new(5.0, 3.0, Theme::pigment()).unwrap();
        let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
        ax.bar(y.clone(), &[3.0, 4.0], BarOptions::default())
            .unwrap();
        ax.ytick_highlight(names(highlight), ColorSpec::AUTO)
            .unwrap();
        f
    };
    let width = |f: &Figure| {
        chrome_texts(f)
            .into_iter()
            .find(|text| text.text == "Washing machine")
            .unwrap()
            .width
    };
    let (plain, bold) = (figure(&[]), figure(&["Washing machine"]));
    assert!(
        width(&bold) > width(&plain) * 1.02,
        "{} {}",
        width(&bold),
        width(&plain)
    );
    let svg = bold.to_svg(SvgText::Editable).unwrap();
    assert!(
        svg.contains(r#"font-weight="600""#) && svg.contains(">Washing machine</text>"),
        "the label stays editable text in the bold weight"
    );
}

#[test]
fn an_explicit_color_colors_the_labels() {
    let x = names(&["North", "South", "East"]);
    let mut f = bars(&x, 5.0, Theme::pigment());
    axes(&mut f)
        .xtick_highlight(names(&["North"]), ColorSpec::Fixed(Rgba::BLUE))
        .unwrap();
    assert_eq!(label(&f, "North"), (Weight::SemiBold, Rgba::BLUE));
    let palette = ColorSpec::palette(1, umber_core::Alpha::OPAQUE).unwrap();
    axes(&mut f)
        .xtick_highlight(names(&["North"]), palette)
        .unwrap();
    assert_eq!(
        label(&f, "North"),
        (Weight::SemiBold, Theme::pigment().palette()[1])
    );
    // A custom emphasis color is the automatic one.
    let theme = Theme::pigment().with_emphasis_color(Rgba::TEAL).unwrap();
    let mut f = bars(&x, 5.0, theme);
    axes(&mut f)
        .xtick_highlight(names(&["East"]), ColorSpec::AUTO)
        .unwrap();
    assert_eq!(label(&f, "East"), (Weight::SemiBold, Rgba::TEAL));
}

#[test]
fn a_second_call_replaces_the_highlight_and_an_empty_one_removes_it() {
    let x = names(&["North", "South", "East"]);
    let mut f = bars(&x, 5.0, Theme::pigment());
    axes(&mut f)
        .xtick_highlight(names(&["North", "South"]), ColorSpec::AUTO)
        .unwrap()
        .xtick_highlight(names(&["East"]), ColorSpec::AUTO)
        .unwrap();
    assert_eq!(
        f.axes_at(GridIndex::Auto, GridIndex::Auto)
            .unwrap()
            .x_tick_highlight()
            .categories(),
        ["East"]
    );
    assert_eq!(label(&f, "North").0, Weight::Regular);
    assert_eq!(label(&f, "East").0, Weight::SemiBold);
    axes(&mut f)
        .xtick_highlight(Vec::new(), ColorSpec::AUTO)
        .unwrap();
    let plain = bars(&x, 5.0, Theme::pigment());
    assert_eq!(
        f.to_scene(FontLibrary::global()).unwrap(),
        plain.to_scene(FontLibrary::global()).unwrap()
    );
}

#[test]
fn numeric_and_time_axes_refuse_a_highlight() {
    for (x, y, scale) in [
        (ScaleKind::Linear, ScaleKind::Linear, "linear"),
        (
            ScaleKind::Log {
                base: umber_core::LogBase::TEN,
            },
            ScaleKind::Log {
                base: umber_core::LogBase::TEN,
            },
            "log",
        ),
        (ScaleKind::Time, ScaleKind::Time, "time"),
    ] {
        let mut f = Figure::new(5.0, 3.0, Theme::pigment()).unwrap();
        f.add_axes(x, y).unwrap();
        let before = f.clone();
        let err = axes(&mut f)
            .xtick_highlight(names(&["a"]), ColorSpec::AUTO)
            .map(drop)
            .unwrap_err();
        assert!(matches!(err, Error::Usage { .. }), "{err:?}");
        assert_eq!(
            err.to_string(),
            format!(
                "Axes.xtick_highlight(): this is a {scale} axis; tick highlights apply to \
                 category axes only."
            )
        );
        assert_eq!(
            err.hint(),
            Some("pass x=Scale.category() to Figure.add_axes().")
        );
        let err = axes(&mut f)
            .ytick_highlight(names(&["a"]), ColorSpec::AUTO)
            .map(drop)
            .unwrap_err();
        assert!(
            err.to_string()
                .starts_with(&format!("Axes.ytick_highlight(): this is a {scale} axis")),
            "{err}"
        );
        assert_eq!(
            err.hint(),
            Some("pass y=Scale.category() to Figure.add_axes().")
        );
        assert_eq!(f, before, "a failed call changed the figure");
    }
}

#[test]
fn an_unknown_category_is_a_data_error_at_render() {
    let x = names(&["North", "South"]);
    let mut f = bars(&x, 5.0, Theme::pigment());
    axes(&mut f)
        .xtick_highlight(names(&["South", "Sout"]), ColorSpec::AUTO)
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Data { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): xtick_highlight() names \"Sout\", which is not a category of the x \
         axis."
    );
    assert_eq!(
        err.hint(),
        Some("pass names of the axis' categories, exactly as the marks give them.")
    );
    // On a grid, the error names the cell.
    let mut f = Figure::with_grid(8.0, 3.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    f.add_axes_at(at(0, 1), ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .bar(x.clone(), &[1.0, 2.0], BarOptions::default())
        .unwrap()
        .ytick_highlight(names(&["West"]), ColorSpec::AUTO)
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(
        err.to_string().starts_with(
            "Figure.to_scene(): row 0, col 1: ytick_highlight() names \"West\", which is not a \
             category of the y axis."
        ),
        "{err}"
    );
}

#[test]
fn a_highlight_left_out_by_crowded_labels_is_a_style_error() {
    let x = categories("class ", 40);
    // On a narrow figure the automatic labels show every k-th class only.
    let mut f = bars(&x, 4.0, Theme::pigment());
    let shown: Vec<String> = chrome_texts(&f)
        .into_iter()
        .map(|text| text.text)
        .filter(|text| text.starts_with("class "))
        .collect();
    assert!(shown.len() < x.len() && shown.len() > 1, "{shown:?}");
    let hidden = x.iter().find(|name| !shown.contains(name)).unwrap().clone();
    let step = x.iter().position(|name| *name == shown[1]).unwrap();
    axes(&mut f)
        .xtick_highlight(vec![shown[1].clone(), hidden.clone()], ColorSpec::AUTO)
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        format!(
            "Figure.to_scene(): the x axis labels every {step}th category to keep its labels \
             apart, which leaves out the highlighted \"{hidden}\"."
        )
    );
    assert_eq!(
        err.hint(),
        Some("enlarge the figure, or set a smaller tick label size with xtick_labels().")
    );
    // Control: a highlighted class whose label is shown renders, and so does the hidden one
    // on a figure wide enough to label every class.
    axes(&mut f)
        .xtick_highlight(vec![shown[1].clone()], ColorSpec::AUTO)
        .unwrap();
    assert_eq!(label(&f, &shown[1]).0, Weight::SemiBold);
    let mut wide = bars(&x, 24.0, Theme::pigment());
    axes(&mut wide)
        .xtick_highlight(vec![hidden.clone()], ColorSpec::AUTO)
        .unwrap();
    assert_eq!(label(&wide, &hidden).0, Weight::SemiBold);
}

#[test]
fn shared_axes_share_the_highlight_as_ticks_are() {
    let x = names(&["a", "b", "c"]);
    let grid = Grid::new(2, 1).unwrap().with_share_x(Share::All);
    let mut f = Figure::with_grid(5.0, 5.0, Theme::pigment(), grid).unwrap();
    for row in 0..2 {
        f.add_axes_at(at(row, 0), ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .bar(x.clone(), &[1.0, 2.0, 3.0], BarOptions::default())
            .unwrap();
    }
    // Set on the upper panel, whose labels are hidden: the lower panel draws it.
    f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .xtick_highlight(names(&["b"]), ColorSpec::AUTO)
        .unwrap();
    let lower = f.axes_at(GridIndex::At(1), GridIndex::At(0)).unwrap();
    assert_eq!(lower.x_tick_highlight().categories(), ["b"]);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let bold: Vec<_> = scene
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Group { clip: None, items } => Some(items),
            _ => None,
        })
        .flatten()
        .filter_map(|item| match item {
            Item::Text(text) if text.style.weight == Weight::SemiBold && text.text == "b" => {
                Some(text.color)
            }
            _ => None,
        })
        .collect();
    assert_eq!(bold, [Theme::pigment().emphasis_color()]);
    // Another member may not set a different highlight; the same one is fine.
    let before = f.clone();
    let err = f
        .axes_at_mut(GridIndex::At(1), GridIndex::At(0))
        .unwrap()
        .xtick_highlight(names(&["c"]), ColorSpec::AUTO)
        .map(drop)
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "Axes.xtick_highlight(): x tick highlight is shared with row 0, col 0, which set \
         another highlight."
    );
    assert_eq!(err.parameter(), Some("categories"));
    assert_eq!(f, before);
    f.axes_at_mut(GridIndex::At(1), GridIndex::At(0))
        .unwrap()
        .xtick_highlight(names(&["b"]), ColorSpec::AUTO)
        .unwrap();
}

#[test]
fn a_twin_highlights_the_x_axis_it_shares() {
    let x = names(&["Jan", "Feb", "Mar"]);
    let mut f = Figure::new(5.0, 3.0, Theme::pigment()).unwrap();
    let mut left = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    left.bar(x.clone(), &[40.0, 55.0, 30.0], BarOptions::default())
        .unwrap();
    let mut right = left.twin_y(ScaleKind::Linear).unwrap();
    right
        .line(
            AxisData::Categories(x.clone()),
            &[7.0, 9.0, 8.0],
            LineOptions::default(),
        )
        .unwrap()
        .xtick_highlight(names(&["Feb"]), ColorSpec::AUTO)
        .unwrap();
    let err = right
        .ytick_highlight(names(&["Feb"]), ColorSpec::AUTO)
        .map(drop)
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    assert_eq!(
        f.axes_at(GridIndex::Auto, GridIndex::Auto)
            .unwrap()
            .x_tick_highlight()
            .categories(),
        ["Feb"]
    );
    let febs: Vec<_> = chrome_texts(&f)
        .into_iter()
        .filter(|text| text.text == "Feb")
        .map(|text| (text.style.weight, text.color))
        .collect();
    assert_eq!(
        febs,
        [(Weight::SemiBold, Theme::pigment().emphasis_color())]
    );
}

/// A 3 by 2 heatmap with every cell drawn, outlined where `outline` says.
fn outlined(outline: &[bool], options: HeatmapOptions) -> Figure {
    heatmap(
        &["a", "b", "c"],
        &["p", "q"],
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        HeatmapOptions {
            outline: Some(Arc::from(outline)),
            ..options
        },
    )
}

#[test]
fn outlines_lie_inside_their_cells() {
    // Cells much wider than tall: a rectangle follows the cell's shape.
    let f = heatmap_sized(
        &["a", "b", "c"],
        &["p", "q"],
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        HeatmapOptions {
            outline: Some(Arc::from(
                [false, true, false, false, false, true].as_slice(),
            )),
            ..HeatmapOptions::default()
        },
        (9.0, 2.5),
    );
    let items = marks(&f);
    let (left, top, right, bottom) = grid(&items, (3, 2));
    let (w, h) = ((right - left) / 3.0, (bottom - top) / 2.0);
    assert!(w > 1.5 * h, "{w} {h}");
    let outlines = strokes(&items);
    assert_eq!(outlines.len(), 2, "{outlines:?}");
    let emphasis = Theme::pigment().emphasis_color();
    for ((corners, color, width), (row, column)) in outlines.iter().zip([(0, 1), (1, 2)]) {
        assert_eq!((*color, *width), (emphasis, 1.5));
        let (x0, y0) = (left + column as f64 * w, top + row as f64 * h);
        let expected = [
            (x0 + 0.75, y0 + 0.75),
            (x0 + w - 0.75, y0 + 0.75),
            (x0 + w - 0.75, y0 + h - 0.75),
            (x0 + 0.75, y0 + h - 0.75),
        ];
        assert_eq!(corners.len(), 4, "{corners:?}");
        for (got, want) in corners.iter().zip(expected) {
            assert!(
                close(got.0, want.0) && close(got.1, want.1),
                "{corners:?}, expected {expected:?}"
            );
        }
    }
}

#[test]
fn outlines_of_small_cells_stay_inside_them() {
    // 1 pt tall cells: the stroke narrows to half the cell, so it covers no neighbour and the
    // rectangle it follows keeps half the cell's height.
    let y: Vec<String> = (0..200).map(|i| format!("r{i}")).collect();
    let y: Vec<&str> = y.iter().map(String::as_str).collect();
    let mut outline = vec![false; 200];
    outline[50] = true;
    let f = heatmap_sized(
        &["a"],
        &y,
        &[1.0; 200],
        HeatmapOptions {
            outline: Some(Arc::from(outline)),
            render: Render::Vector,
            ..HeatmapOptions::default()
        },
        (3.0, 3.6),
    );
    let items = marks(&f);
    let (_, top, _, bottom) = grid(&items, (1, 200));
    let h = (bottom - top) / 200.0;
    assert!(h < 1.5, "{h}");
    let outlines = strokes(&items);
    assert_eq!(outlines.len(), 1);
    let (corners, _, width) = &outlines[0];
    assert!((f64::from(*width) - h / 2.0).abs() < 1e-3, "{width} {h}");
    let ys: Vec<f64> = corners.iter().map(|p| p.1).collect();
    let span =
        ys.iter().copied().fold(f64::MIN, f64::max) - ys.iter().copied().fold(f64::MAX, f64::min);
    assert!((span - h / 2.0).abs() < 1e-3, "{corners:?}");
    let cell_top = top + 50.0 * h;
    for (_, y) in corners {
        assert!(
            *y >= cell_top + f64::from(*width) / 2.0 - 1e-3
                && *y <= cell_top + h - f64::from(*width) / 2.0 + 1e-3,
            "{corners:?} in {cell_top}..{}",
            cell_top + h
        );
    }
}

/// The figure of a 200 by 200 heatmap of ones with 1.05 pt cells and no colorbar, the cell at
/// row 100, column 100 outlined if `outlined`.
fn tiny_cells(outlined: bool) -> Figure {
    let names: Vec<String> = (0..200).map(|i| format!("c{i}")).collect();
    let mut outline = vec![false; 200 * 200];
    outline[100 * 200 + 100] = outlined;
    let mut f = Figure::new(3.6, 3.533486111111111, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names,
            &names,
            &vec![1.0; 200 * 200],
            HeatmapOptions {
                outline: Some(Arc::from(outline)),
                ..HeatmapOptions::default()
            },
        )
        .unwrap()
        .colorbar(umber_core::ColorbarOptions {
            position: Some(umber_core::ColorbarPosition::None),
            ..umber_core::ColorbarOptions::default()
        })
        .unwrap();
    f
}

#[test]
fn an_outline_on_a_tiny_cell_shows_in_the_outline_color() {
    let f = tiny_cells(true);
    let items = marks(&f);
    let outline: Vec<_> = strokes(&items);
    assert_eq!(outline.len(), 1);
    let (corners, color, width) = &outline[0];
    let xs: Vec<f64> = corners.iter().map(|p| p.0).collect();
    let (x0, x1) = (
        xs.iter().copied().fold(f64::MAX, f64::min),
        xs.iter().copied().fold(f64::MIN, f64::max),
    );
    // The cells are about 1.05 pt: the rectangle keeps half of that, and the stroke of half the
    // cell covers the whole cell.
    let side = 2.0 * f64::from(*width);
    assert!(side > 1.0 && side < 1.1, "{side}");
    assert!((x1 - x0 - side / 2.0).abs() < 1e-3, "{corners:?}");
    let ys: Vec<f64> = corners.iter().map(|p| p.1).collect();
    let center = (
        (x0 + x1) / 2.0,
        (ys.iter().copied().fold(f64::MAX, f64::min) + ys.iter().copied().fold(f64::MIN, f64::max))
            / 2.0,
    );
    assert_ne!(f.to_png(72).unwrap(), tiny_cells(false).to_png(72).unwrap());
    // At 1200 dpi the cell is about 17 pixels across; its center pixel is the outline color.
    let dpi = 1200;
    let pixels = pdf_check::image::decode_rgba8(&f.to_png(dpi).unwrap()).unwrap();
    let scale = dpi as f64 / 72.0;
    let actual = pixels.at((center.0 * scale) as u32, (center.1 * scale) as u32);
    let expected = [color.r, color.g, color.b, 255];
    assert_eq!(*color, Theme::pigment().emphasis_color());
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
        "outline pixel {actual:?}, expected {expected:?}"
    );
}

/// The strokes of every group of `scene`, depth first.
fn all_strokes(items: &[Item]) -> Vec<Stroke> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Group { items, .. } => out.extend(all_strokes(items)),
            other => out.extend(strokes(std::slice::from_ref(other))),
        }
    }
    out
}

#[test]
fn an_all_nan_heatmap_keeps_its_edges_and_outlines() {
    for render in [Render::Vector, Render::Raster] {
        let f = heatmap(
            &["a", "b"],
            &["p", "q"],
            &[f64::NAN; 4],
            HeatmapOptions {
                outline: Some(Arc::from([true, false, false, false].as_slice())),
                edges: true,
                render,
                ..HeatmapOptions::default()
            },
        );
        let items = marks(&f);
        assert!(
            !items
                .iter()
                .any(|item| matches!(item, Item::Fill { .. } | Item::Image { .. })),
            "{render:?}: no cell is drawn"
        );
        let kinds: Vec<_> = strokes(&items)
            .iter()
            .map(|(_, color, width)| (*color, *width))
            .collect();
        let theme = Theme::pigment();
        assert_eq!(
            kinds,
            [
                (theme.grid_color(), 0.5),
                (theme.grid_color(), 0.5),
                (theme.emphasis_color(), 1.5)
            ],
            "{render:?}"
        );
    }
}

#[test]
fn an_all_nan_heatmap_keeps_its_outline_whatever_its_color_group_holds() {
    // The outline of the blank heatmap on the right does not depend on whether the heatmap it
    // shares colors with on the left has finite values.
    let outlines = |left: f64| {
        let grid = Grid::new(1, 2).unwrap().with_share_color(Share::All);
        let mut f = Figure::with_grid(8.0, 3.0, Theme::pigment(), grid).unwrap();
        f.add_axes_at(at(0, 0), ScaleKind::Category, ScaleKind::Category)
            .unwrap()
            .heatmap(
                &names(&["a"]),
                &names(&["p"]),
                &[left],
                HeatmapOptions::default(),
            )
            .unwrap();
        f.add_axes_at(at(0, 1), ScaleKind::Category, ScaleKind::Category)
            .unwrap()
            .heatmap(
                &names(&["a", "b"]),
                &names(&["p"]),
                &[f64::NAN; 2],
                HeatmapOptions {
                    outline: Some(Arc::from([false, true].as_slice())),
                    ..HeatmapOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        all_strokes(&scene.items)
            .into_iter()
            .filter(|(_, color, width)| {
                *color == Theme::pigment().emphasis_color() && *width == 1.5
            })
            .collect::<Vec<_>>()
    };
    let (finite, blank) = (outlines(1.0), outlines(f64::NAN));
    assert_eq!(finite.len(), 1, "{finite:?}");
    assert_eq!(blank.len(), 1, "{blank:?}");
}

#[test]
fn a_nan_cell_can_be_outlined_and_outline_color_overrides_the_emphasis_color() {
    let f = heatmap(
        &["a", "b"],
        &["p"],
        &[f64::NAN, 2.0],
        HeatmapOptions {
            outline: Some(Arc::from([true, false].as_slice())),
            outline_color: ColorSpec::Fixed(Rgba::BLUE),
            ..HeatmapOptions::default()
        },
    );
    let items = marks(&f);
    let fills = items
        .iter()
        .filter(|item| matches!(item, Item::Fill { .. }))
        .count();
    assert_eq!(fills, 1, "the NaN cell stays empty");
    let outlines = strokes(&items);
    assert_eq!(outlines.len(), 1);
    assert_eq!((outlines[0].1, outlines[0].2), (Rgba::BLUE, 1.5));
    // The outline is over the empty cell, left of the drawn one.
    let Some(Item::Fill { path, .. }) = items.iter().find(|i| matches!(i, Item::Fill { .. }))
    else {
        unreachable!()
    };
    let cell_left = corners(path.verbs())[0].0;
    assert!(
        outlines[0].0.iter().all(|p| p.0 < cell_left),
        "{outlines:?}"
    );
}

#[test]
fn cells_edges_outlines_and_values_are_drawn_in_that_order() {
    let f = outlined(
        &[true, false, false, false, false, false],
        HeatmapOptions {
            annotate: true,
            edges: true,
            ..HeatmapOptions::default()
        },
    );
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let Item::Group { items, .. } = &scene.items[1] else {
        panic!("the marks group")
    };
    let kinds: Vec<&str> = items
        .iter()
        .map(|item| match item {
            Item::Fill { .. } => "cell",
            Item::Stroke { style, .. } if style.width == 0.5 => "edge",
            Item::Stroke { .. } => "outline",
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "cell", "cell", "cell", "cell", "cell", "cell", "edge", "edge", "edge", "outline"
        ]
    );
    // The values follow in their own group, over everything the marks group holds.
    let Item::Group {
        clip: Some(_),
        items: values,
    } = &scene.items[2]
    else {
        panic!("the values group follows the marks")
    };
    assert_eq!(values.len(), 6);
    assert!(values.iter().all(|item| matches!(item, Item::Text(_))));
}

#[test]
fn edges_lie_between_cells_only() {
    for (columns, rows) in [(3usize, 2usize), (5, 4), (1, 3), (1, 1)] {
        let x: Vec<String> = (0..columns).map(|i| format!("c{i}")).collect();
        let y: Vec<String> = (0..rows).map(|i| format!("r{i}")).collect();
        let x: Vec<&str> = x.iter().map(String::as_str).collect();
        let y: Vec<&str> = y.iter().map(String::as_str).collect();
        for theme in [Theme::pigment(), Theme::midnight()] {
            let mut f = Figure::new(5.0, 4.0, theme.clone()).unwrap();
            f.add_axes(ScaleKind::Category, ScaleKind::Category)
                .unwrap()
                .heatmap(
                    &names(&x),
                    &names(&y),
                    &vec![1.0; columns * rows],
                    HeatmapOptions {
                        edges: true,
                        ..HeatmapOptions::default()
                    },
                )
                .unwrap();
            let items = marks(&f);
            let (left, top, right, bottom) = grid(&items, (columns, rows));
            let edges = strokes(&items);
            assert_eq!(edges.len(), columns - 1 + rows - 1, "{columns} x {rows}");
            let (w, h) = (
                (right - left) / columns as f64,
                (bottom - top) / rows as f64,
            );
            for (i, (corners, color, width)) in edges.iter().enumerate() {
                assert_eq!((*color, *width), (theme.grid_color(), 0.5));
                let [(x0, y0), (x1, y1)] = corners[..] else {
                    panic!("{corners:?}")
                };
                if i < columns - 1 {
                    // Vertical, left to right, from the top to the bottom of the grid.
                    let x = left + (i + 1) as f64 * w;
                    assert!(close(x0, x) && close(x1, x), "{corners:?}");
                    assert!(close(y0, top) && close(y1, bottom), "{corners:?}");
                } else {
                    let y = top + (i - (columns - 1) + 1) as f64 * h;
                    assert!(close(y0, y) && close(y1, y), "{corners:?}");
                    assert!(close(x0, left) && close(x1, right), "{corners:?}");
                }
            }
        }
    }
}

#[test]
fn raster_cells_keep_vector_edges_and_outlines() {
    let f = outlined(
        &[false, false, true, false, false, false],
        HeatmapOptions {
            render: Render::Raster,
            edges: true,
            ..HeatmapOptions::default()
        },
    );
    let items = marks(&f);
    assert!(matches!(items[0], Item::Image { .. }), "{:?}", items[0]);
    let kinds: Vec<_> = strokes(&items[1..])
        .iter()
        .map(|(_, color, width)| (*color, *width))
        .collect();
    let (grid_color, emphasis) = (
        Theme::pigment().grid_color(),
        Theme::pigment().emphasis_color(),
    );
    assert_eq!(
        kinds,
        [
            (grid_color, 0.5),
            (grid_color, 0.5),
            (grid_color, 0.5),
            (emphasis, 1.5)
        ]
    );
    assert_eq!(items.len(), 5);
    // The same geometry as the vector cells give.
    let vector = outlined(
        &[false, false, true, false, false, false],
        HeatmapOptions {
            render: Render::Vector,
            edges: true,
            ..HeatmapOptions::default()
        },
    );
    assert_eq!(strokes(&items), strokes(&marks(&vector)));
}

#[test]
fn outline_shape_and_color_are_checked_at_the_call() {
    let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap();
    let before = f.clone();
    let err = axes(&mut f)
        .heatmap(
            &names(&["a", "b", "c"]),
            &names(&["p", "q"]),
            &[1.0; 6],
            HeatmapOptions {
                outline: Some(Arc::from([true; 5].as_slice())),
                ..HeatmapOptions::default()
            },
        )
        .map(drop)
        .unwrap_err();
    assert!(matches!(err, Error::Data { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "Axes.heatmap(): outline has 5 flags; it must have one per value, len(y_categories) * \
         len(x_categories) = 2 * 3 = 6."
    );
    assert_eq!(err.parameter(), Some("outline"));
    let err = axes(&mut f)
        .heatmap(
            &names(&["a"]),
            &names(&["p"]),
            &[1.0],
            HeatmapOptions {
                outline_color: ColorSpec::Fixed(Rgba::BLUE),
                ..HeatmapOptions::default()
            },
        )
        .map(drop)
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "Axes.heatmap(): outline_color is given but outline is not; only outlined cells take it."
    );
    assert_eq!(err.parameter(), Some("outline_color"));
    assert_eq!(f, before, "a failed call changed the figure");
}

#[test]
fn the_emphasis_color_is_shared_by_labels_and_outlines() {
    let theme = Theme::midnight().with_emphasis_color(Rgba::PINK).unwrap();
    let mut f = Figure::new(5.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names(&["a", "b"]),
            &names(&["a", "b"]),
            &[1.0, 2.0, 3.0, 4.0],
            HeatmapOptions {
                outline: Some(Arc::from([false, true, false, false].as_slice())),
                ..HeatmapOptions::default()
            },
        )
        .unwrap()
        .xtick_highlight(names(&["b"]), ColorSpec::AUTO)
        .unwrap();
    assert_eq!(strokes(&marks(&f))[0].1, Rgba::PINK);
    let pinks = chrome_texts(&f)
        .iter()
        .filter(|text| text.color == Rgba::PINK)
        .count();
    assert_eq!(pinks, 1);
}

/// The confusion matrix of four classes, the last two a group: highlighted on both axes, its
/// within-group confusion outlined, zero cells empty, with values, and with edges if `edges`.
fn confusion(render: Render, edges: bool) -> Figure {
    let classes = names(&["bird", "dog", "drill", "saw"]);
    let counts = [
        9.0, 1.0, 0.0, 0.0, //
        2.0, 7.0, 0.0, 1.0, //
        0.0, 0.0, 6.0, 3.0, //
        0.0, 1.0, 2.0, 8.0,
    ];
    let values: Vec<f64> = counts
        .iter()
        .map(|&c| if c == 0.0 { f64::NAN } else { c })
        .collect();
    let group = |i: usize| i >= 2;
    let outline: Vec<bool> = (0..16)
        .map(|i| {
            let (row, column) = (i / 4, i % 4);
            group(row) && group(column) && row != column && counts[i] > 0.0
        })
        .collect();
    let mut f = Figure::new(4.0, 3.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &classes,
            &classes,
            &values,
            HeatmapOptions {
                annotate: true,
                edges,
                outline: Some(Arc::from(outline)),
                render,
                ..HeatmapOptions::default()
            },
        )
        .unwrap()
        .xtick_highlight(names(&["drill", "saw"]), ColorSpec::AUTO)
        .unwrap()
        .ytick_highlight(names(&["drill", "saw"]), ColorSpec::AUTO)
        .unwrap();
    f
}

#[test]
fn png_svg_and_pdf_agree() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!(
            "skipped the PDF/PNG raster comparison: pdftoppm is not installed; SVG text, PDF \
             fonts and PNG pixels are still checked"
        );
    }
    let emphasis = Theme::pigment().emphasis_color();
    for (name, render) in [("vector", Render::Vector), ("raster", Render::Raster)] {
        let f = confusion(render, true);
        let without_edges = confusion(render, false);
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let Item::Group { items, .. } = &scene.items[1] else {
            panic!("the marks group")
        };
        let outlines: Vec<_> = strokes(items)
            .into_iter()
            .filter(|(_, _, width)| *width == 1.5)
            .collect();
        assert_eq!(outlines.len(), 2, "{name}");

        // SVG: the four highlighted labels stay text in the bold weight and emphasis color,
        // in editable mode; both modes stroke the outlines and edges.
        let editable = f.to_svg(SvgText::Editable).unwrap();
        let fill = format!(r#"fill="{}""#, emphasis.to_hex());
        let bold_labels = editable
            .lines()
            .filter(|line| {
                line.starts_with("<text")
                    && line.contains(r#"font-weight="600""#)
                    && line.contains(&fill)
            })
            .count();
        assert_eq!(bold_labels, 4, "{name}");
        for mode in [SvgText::Editable, SvgText::Outlines] {
            let svg = f.to_svg(mode).unwrap();
            let outline = format!(r#"stroke="{}" stroke-width="1.5""#, emphasis.to_hex());
            assert_eq!(svg.matches(&outline).count(), 2, "{name} {mode:?}");
            // Six edges between the four rows and the four columns; the colorbar may draw
            // others of the same style.
            let edge = format!(
                r#"stroke="{}" stroke-width="0.5""#,
                Theme::pigment().grid_color().to_hex()
            );
            let others = without_edges.to_svg(mode).unwrap().matches(&edge).count();
            assert_eq!(svg.matches(&edge).count(), others + 6, "{name} {mode:?}");
        }

        // PDF: the same scene, the bold face embedded for the labels.
        let pdf = f.to_pdf().unwrap();
        assert_eq!(pdf, umber_core::render::render_pdf(&scene).unwrap());
        let body = String::from_utf8_lossy(&pdf);
        let base_fonts: Vec<&str> = body
            .match_indices("/BaseFont /")
            .map(|(at, m)| body[at + m.len()..].split_whitespace().next().unwrap())
            .collect();
        // The figure has no title: the SemiBold face of the body font is the labels'.
        assert!(
            base_fonts
                .iter()
                .any(|font| font.ends_with("+IBMPlexSans-SmBld")),
            "{name}: no SemiBold face embedded: {base_fonts:?}"
        );

        // PNG: the middle of an outline's top side is the emphasis color.
        let (corners, ..) = &outlines[0];
        let at = (
            (corners[0].0 + corners[1].0) / 2.0,
            (corners[0].1 + corners[1].1) / 2.0,
        );
        let dpi = 288;
        let png = f.to_png(dpi).unwrap();
        let pixels = pdf_check::image::decode_rgba8(&png).unwrap();
        let scale = dpi as f64 / 72.0;
        let actual = pixels.at((at.0 * scale) as u32, (at.1 * scale) as u32);
        let expected = [emphasis.r, emphasis.g, emphasis.b, 255];
        assert!(
            actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
            "{name}: outline pixel {actual:?}, expected {expected:?}"
        );
        if poppler {
            let dir = pdf_check::scratch(&format!("emphasis-{name}"));
            let path = pdf_check::write(&dir, "confusion.pdf", &pdf);
            pdf_check::assert_matches_png(&path, &f.to_png(144).unwrap(), 144);
        }
    }
}
