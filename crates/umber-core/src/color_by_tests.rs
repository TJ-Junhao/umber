//! Color-mapped scatter and bubble marks: mapped colors, the shared color scale of an axes,
//! color limits, legends and atomic errors.

use crate::draw::fill::marker_alpha;
use crate::legend::Legend;
use crate::pipeline::plan_figure;
use crate::resolve::{Limits, resolve_marks};
use crate::{
    AxisData, BubbleOptions, Cmap, CmapChoice, ColorBy, Error, Figure, FontLibrary, Item,
    LegendStyle, LineOptions, LogBase, PointColor, Rgba, ScaleKind, ScatterOptions, SvgText, Theme,
    Transform,
};

fn figure(theme: Theme) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    f
}

fn nums(v: &[f64]) -> AxisData {
    AxisData::Numbers(v.to_vec())
}

fn by(values: &[f64], cmap: Cmap) -> ColorBy {
    ColorBy {
        cmap: CmapChoice::Fixed(cmap),
        ..ColorBy::new(values.to_vec())
    }
}

fn mapped(by: ColorBy) -> ScatterOptions {
    ScatterOptions {
        color: PointColor::By(by),
        ..ScatterOptions::default()
    }
}

/// Every marker batch of the scene, in drawing order, as its fill and number of positions.
fn marker_fills(f: &Figure) -> Vec<(Rgba, usize)> {
    fn walk(item: &Item, out: &mut Vec<(Rgba, usize)>) {
        match item {
            Item::Markers {
                fill: Some(fill),
                positions,
                ..
            } => out.push((*fill, positions.len())),
            Item::Group { items, .. } => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let mut out = Vec::new();
    scene.items.iter().for_each(|item| walk(item, &mut out));
    out
}

/// The fills of the bubbles of the scene's marks group, in drawing order.
fn bubble_fills(f: &Figure) -> Vec<Rgba> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let Item::Group { items, .. } = &scene.items[1] else {
        panic!("marks")
    };
    items
        .iter()
        .filter_map(|item| match item {
            Item::Fill {
                paint: crate::Paint::Solid(color),
                ..
            } => Some(*color),
            _ => None,
        })
        .collect()
}

fn faded(color: Rgba, theme: &Theme) -> Rgba {
    color.scale_alpha(marker_alpha(theme))
}

#[test]
fn scatter_points_take_mapped_colors() {
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[1.0, 2.0, 3.0],
            mapped(by(&[0.0, 5.0, 10.0], Cmap::viridis())),
        )
        .unwrap();
    let viridis = Cmap::viridis();
    assert_eq!(
        marker_fills(&f),
        [
            (viridis.at(0.0), 1),
            (viridis.at(0.5), 1),
            (viridis.at(1.0), 1),
        ]
    );
    assert_eq!(viridis.at(0.0).to_hex(), "#440154");
    assert_eq!(viridis.at(1.0).to_hex(), "#FDE725");
    // Mapped fills are opaque: the theme's marker alpha does not apply to them.
    assert!(f.theme().marker_alpha() < 1.0);
    assert!(marker_fills(&f).iter().all(|(fill, _)| fill.a == 255));

    // Runs of one color share a batch; data order is kept; NaN values are not drawn.
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .scatter(
            nums(&[1.0, 2.0, 3.0, 4.0, 5.0]),
            &[1.0, 2.0, 3.0, 4.0, 5.0],
            mapped(by(&[0.0, 0.0, f64::NAN, 10.0, 0.0], Cmap::viridis())),
        )
        .unwrap();
    assert_eq!(
        marker_fills(&f),
        [
            (viridis.at(0.0), 2),
            (viridis.at(1.0), 1),
            (viridis.at(0.0), 1),
        ]
    );
}

#[test]
fn theme_cmap_follows_the_center() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = figure(theme.clone());
        f.single_axes_mut()
            .scatter(
                nums(&[1.0, 2.0]),
                &[1.0, 2.0],
                mapped(ColorBy::new(vec![0.0, 10.0])),
            )
            .unwrap();
        let ends = [theme.cmap().at(0.0), theme.cmap().at(1.0)];
        assert_eq!(marker_fills(&f), ends.map(|color| (color, 1)));
        let mut f = figure(theme.clone());
        let centered = ColorBy {
            center: Some(0.0),
            ..ColorBy::new(vec![-4.0, 0.0, 8.0])
        };
        f.single_axes_mut()
            .scatter(nums(&[1.0, 2.0, 3.0]), &[1.0, 2.0, 3.0], mapped(centered))
            .unwrap();
        let diverging = theme.diverging_cmap();
        assert_eq!(
            marker_fills(&f),
            [0.25, 0.5, 1.0].map(|t| (diverging.at(t), 1))
        );
    }
}

#[test]
fn equal_values_map_to_middle() {
    let mut f = figure(Theme::graphite());
    f.single_axes_mut()
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[1.0, 2.0, 3.0],
            mapped(by(&[7.0, 7.0, 7.0], Cmap::magma())),
        )
        .unwrap();
    assert_eq!(marker_fills(&f), [(Cmap::magma().at(0.5), 3)]);
    let resolved = plan_figure(&f, (0, 0), FontLibrary::global()).0;
    let scale = resolved.color.expect("a color scale");
    let ticks: Vec<_> = scale
        .ticks
        .iter()
        .map(|t| (t.pos.get(), t.label.as_str()))
        .collect();
    assert_eq!(ticks, [(0.5, "7")]);
}

#[test]
fn all_nan_draws_no_colorbar() {
    for clim in [None, Some((0.0, 10.0))] {
        let mut f = figure(Theme::pigment());
        let mut axes = f.single_axes_mut();
        if let Some((low, high)) = clim {
            axes.clim(low, high).unwrap();
        }
        axes.scatter(
            nums(&[1.0, 2.0]),
            &[1.0, 2.0],
            mapped(ColorBy::new(vec![f64::NAN, f64::NAN])),
        )
        .unwrap()
        .bubbles(
            &[1.0],
            &[1.0],
            &[5.0],
            BubbleOptions {
                color: PointColor::By(ColorBy::new(vec![f64::NAN])),
                ..BubbleOptions::default()
            },
        )
        .unwrap();
        let resolved = plan_figure(&f, (0, 0), FontLibrary::global()).0;
        assert_eq!(resolved.color, None, "{clim:?}");
        assert!(marker_fills(&f).is_empty());
        assert!(bubble_fills(&f).is_empty());
    }
}

#[test]
fn clim_clamps_values_outside_it() {
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .clim(0.0, 10.0)
        .unwrap()
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[1.0, 2.0, 3.0],
            mapped(by(&[-5.0, 5.0, 50.0], Cmap::cividis())),
        )
        .unwrap();
    let cividis = Cmap::cividis();
    assert_eq!(
        marker_fills(&f),
        [0.0, 0.5, 1.0].map(|t| (cividis.at(t), 1))
    );
}

#[test]
fn log_scale_maps_by_the_logarithm() {
    let mut f = figure(Theme::pigment());
    let log = ColorBy {
        scale: Transform::Log(LogBase::TEN),
        ..by(&[1.0, 10.0, 100.0], Cmap::viridis())
    };
    f.single_axes_mut()
        .scatter(nums(&[1.0, 2.0, 3.0]), &[1.0, 2.0, 3.0], mapped(log))
        .unwrap();
    assert_eq!(
        marker_fills(&f),
        [0.0, 0.5, 1.0].map(|t| (Cmap::viridis().at(t), 1))
    );
}

#[test]
fn mapped_marks_take_no_palette_color() {
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .scatter(nums(&[1.0]), &[1.0], mapped(ColorBy::new(vec![1.0])))
        .unwrap()
        .scatter(nums(&[2.0]), &[2.0], ScatterOptions::default())
        .unwrap();
    let theme = f.theme().clone();
    let fills = marker_fills(&f);
    assert_eq!(fills[1], (faded(theme.palette()[0], &theme), 1));
}

#[test]
fn mismatched_cmap_rejected_atomically() {
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .scatter(
            nums(&[1.0, 2.0]),
            &[1.0, 2.0],
            mapped(by(&[0.0, 1.0], Cmap::viridis())),
        )
        .unwrap();
    let before = f.to_svg(SvgText::Editable).unwrap();
    let marks = f.single_axes().marks().to_vec();

    let err = f
        .single_axes_mut()
        .scatter(nums(&[3.0]), &[3.0], mapped(by(&[2.0], Cmap::magma())))
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }));
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): color uses Cmap.MAGMA; this axes already maps colors with Cmap.VIRIDIS."
    );
    assert_eq!(err.parameter(), Some("color"));
    assert_eq!(
        err.hint(),
        Some("use the same cmap, scale and center for every color-mapped mark of an axes.")
    );

    let theme_cmap = BubbleOptions {
        color: PointColor::By(ColorBy::new(vec![2.0])),
        ..BubbleOptions::default()
    };
    let err = f
        .single_axes_mut()
        .bubbles(&[3.0], &[3.0], &[1.0], theme_cmap)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.bubbles(): color uses the theme's colormap; this axes already maps colors with \
         Cmap.VIRIDIS."
    );
    let centered = ColorBy {
        center: Some(0.5),
        ..by(&[2.0], Cmap::viridis())
    };
    let err = f
        .single_axes_mut()
        .scatter(nums(&[3.0]), &[3.0], mapped(centered))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): color uses center 0.5; this axes already maps colors with no center."
    );
    let err = f
        .single_axes_mut()
        .scatter(
            nums(&[3.0, 4.0]),
            &[3.0, 4.0],
            mapped(by(&[2.0], Cmap::viridis())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::Data { .. }));
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): x has 2 values but color has 1; x and color must have the same length."
    );
    assert_eq!(err.parameter(), Some("color"));
    // A color-mapped mark with the same spec is accepted; one color always is.
    f.clone()
        .single_axes_mut()
        .scatter(nums(&[3.0]), &[3.0], mapped(by(&[2.0], Cmap::viridis())))
        .unwrap()
        .scatter(nums(&[4.0]), &[4.0], ScatterOptions::default())
        .unwrap();

    assert_eq!(f.single_axes().marks(), marks);
    assert_eq!(f.to_svg(SvgText::Editable).unwrap(), before);
}

#[test]
fn clim_conflicts_are_rejected_atomically() {
    let mut f = figure(Theme::pigment());
    let centered = ColorBy {
        center: Some(5.0),
        ..ColorBy::new(vec![0.0, 10.0])
    };
    f.single_axes_mut()
        .scatter(nums(&[1.0, 2.0]), &[1.0, 2.0], mapped(centered))
        .unwrap();
    let before = f.to_svg(SvgText::Editable).unwrap();
    let err = f.single_axes_mut().clim(6.0, 10.0).unwrap_err();
    assert!(matches!(err, Error::Style { .. }));
    assert_eq!(
        err.to_string(),
        "Axes.clim(): center is 5; it must lie inside the color range 6 to 10."
    );
    assert_eq!(err.parameter(), Some("low"));
    let err = f.single_axes_mut().clim(5.0, 2.0).unwrap_err();
    assert_eq!(err.hint(), Some("swap the two values."));
    assert_eq!(f.single_axes().color_limits(), None);
    assert_eq!(f.to_svg(SvgText::Editable).unwrap(), before);

    // Limits set first are checked when the mark arrives.
    let mut f = figure(Theme::pigment());
    f.single_axes_mut().clim(0.0, 10.0).unwrap();
    let before = f.to_svg(SvgText::Editable).unwrap();
    let log = ColorBy {
        scale: Transform::Log(LogBase::TEN),
        ..ColorBy::new(vec![1.0])
    };
    let err = f
        .single_axes_mut()
        .scatter(nums(&[1.0]), &[1.0], mapped(log))
        .unwrap_err();
    assert!(matches!(err, Error::Style { .. }));
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): the color limits are 0 and 10; a log color scale needs positive limits."
    );
    assert_eq!(err.parameter(), Some("color"));
    let centered = ColorBy {
        center: Some(12.0),
        ..ColorBy::new(vec![1.0])
    };
    let err = f
        .single_axes_mut()
        .scatter(nums(&[1.0]), &[1.0], mapped(centered))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): center is 12; it must lie inside the color range 0 to 10."
    );
    assert!(f.single_axes().marks().is_empty());
    assert_eq!(f.to_svg(SvgText::Editable).unwrap(), before);
}

#[test]
fn scatter_colorby_with_label_rejected() {
    let mut f = figure(Theme::pigment());
    let options = ScatterOptions {
        label: "a".into(),
        ..mapped(ColorBy::new(vec![1.0]))
    };
    let err = f
        .single_axes_mut()
        .scatter(nums(&[1.0]), &[1.0], options)
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }));
    assert_eq!(
        err.to_string(),
        "Axes.scatter(): label is \"a\"; a color-mapped scatter has no legend entry."
    );
    assert_eq!(err.parameter(), Some("label"));
    assert_eq!(
        err.hint(),
        Some("remove the label; the colorbar explains the colors.")
    );
    assert!(f.single_axes().marks().is_empty());
}

#[test]
fn bubbles_colorby_keeps_size_legend_neutral() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = figure(theme.clone());
        let options = BubbleOptions {
            label: "Population".into(),
            color: PointColor::By(by(&[20.0, 60.0, 100.0], Cmap::viridis())),
            ..BubbleOptions::default()
        };
        f.single_axes_mut()
            .bubbles(
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[100.0, 400.0, 900.0],
                options,
            )
            .unwrap();
        let viridis = Cmap::viridis();
        assert_eq!(bubble_fills(&f), [0.0, 0.5, 1.0].map(|t| viridis.at(t)));
        let marks = resolve_marks(f.single_axes().marks(), &theme);
        let legend = Legend::new(LegendStyle::Header, &marks, &Limits::of(f.single_axes()));
        let neutral = faded(theme.text_color(), &theme).over(theme.background());
        let labels: Vec<_> = legend.row().iter().map(|e| e.label()).collect();
        assert_eq!(labels[0], "Population");
        assert!(legend.row().len() > 1, "{labels:?}");
        for entry in legend.row() {
            assert_eq!(entry.color(), theme.text_color(), "{}", entry.label());
            assert_eq!(entry.fill_color(&theme), neutral, "{}", entry.label());
        }
    }
}

#[test]
fn color_mapped_scatter_has_no_legend_entry() {
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .scatter(nums(&[1.0]), &[1.0], mapped(ColorBy::new(vec![1.0])))
        .unwrap()
        .line(
            nums(&[0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions {
                label: "Trend".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    let legend = Legend::new(LegendStyle::Header, &marks, &Limits::of(f.single_axes()));
    let labels: Vec<_> = legend.row().iter().map(|e| e.label()).collect();
    assert_eq!(labels, ["Trend"]);
}

#[test]
fn narrow_ranges_keep_distinct_colors() {
    let viridis = Cmap::viridis();
    // An automatic linear range of 1 at 1e12.
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .scatter(
            nums(&[1.0, 2.0]),
            &[1.0, 2.0],
            mapped(by(&[1e12, 1e12 + 1.0], Cmap::viridis())),
        )
        .unwrap();
    assert_eq!(
        marker_fills(&f),
        [(viridis.at(0.0), 1), (viridis.at(1.0), 1)]
    );
    // Narrow color limits on a log scale.
    let mut f = figure(Theme::pigment());
    let log = ColorBy {
        scale: Transform::Log(LogBase::TEN),
        ..by(&[1.0, 1.0 + 5e-10], Cmap::viridis())
    };
    f.single_axes_mut()
        .clim(1.0, 1.0 + 5e-10)
        .unwrap()
        .scatter(nums(&[1.0, 2.0]), &[1.0, 2.0], mapped(log))
        .unwrap();
    assert_eq!(
        marker_fills(&f),
        [(viridis.at(0.0), 1), (viridis.at(1.0), 1)]
    );
}

#[test]
fn centers_of_either_zero_sign_agree() {
    let mut f = figure(Theme::pigment());
    let centered = |center: f64, value: f64| ColorBy {
        center: Some(center),
        ..ColorBy::new(vec![value])
    };
    f.single_axes_mut()
        .scatter(nums(&[1.0]), &[1.0], mapped(centered(0.0, 1.0)))
        .unwrap()
        .scatter(nums(&[2.0]), &[2.0], mapped(centered(-0.0, -1.0)))
        .unwrap();
    assert_eq!(f.single_axes().marks().len(), 2);
}
