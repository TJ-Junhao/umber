//! Figure headers, alignment and shared legend colors.

use super::*;
use crate::layout::PADDING;
use crate::legend::Swatch;
use crate::{
    Align, AxisData, BarOptions, Grid, GridIndex, LineOptions, Marker, Placement, ScaleKind,
};

#[test]
fn every_existing_mark_exposes_its_legend_swatch() {
    use crate::{
        AreaOptions, AxisValue, BandOptions, ErrorBarOptions, HistOptions, RuleOptions,
        ScatterOptions, Series, SpanOptions, StemOptions, StepOptions,
    };
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    let x = || AxisData::Numbers(vec![0.0, 1.0]);
    let mut a = f
        .add_axes_at(
            Placement {
                col: GridIndex::At(0),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap();
    a.line(
        x(),
        &[1.0, 2.0],
        LineOptions {
            label: "A".into(),
            ..LineOptions::default()
        },
    )
    .unwrap();
    a.scatter(
        x(),
        &[1.0, 2.0],
        ScatterOptions {
            label: "A".into(),
            ..ScatterOptions::default()
        },
    )
    .unwrap();
    a.stem(
        x(),
        &[1.0, 2.0],
        StemOptions {
            label: "A".into(),
            ..StemOptions::default()
        },
    )
    .unwrap();
    a.area(
        x(),
        &[1.0, 2.0],
        AreaOptions {
            label: "A".into(),
            ..AreaOptions::default()
        },
    )
    .unwrap();
    a.stacked_area(
        &[0.0, 1.0],
        vec![Series {
            label: "A".into(),
            values: vec![1.0, 2.0],
            ..Series::default()
        }],
    )
    .unwrap();
    a.hist(
        &[0.0, 1.0],
        HistOptions {
            label: "A".into(),
            ..HistOptions::default()
        },
    )
    .unwrap();
    a.step(
        x(),
        &[1.0, 2.0],
        StepOptions {
            label: "A".into(),
            ..StepOptions::default()
        },
    )
    .unwrap();
    a.hline(
        1.0,
        RuleOptions {
            label: "A".into(),
            ..RuleOptions::default()
        },
    )
    .unwrap();
    a.vline(
        AxisValue::Number(1.0),
        RuleOptions {
            label: "A".into(),
            ..RuleOptions::default()
        },
    )
    .unwrap();
    a.hspan(
        0.0,
        1.0,
        SpanOptions {
            label: "A".into(),
            ..SpanOptions::default()
        },
    )
    .unwrap();
    a.vspan(
        0.0,
        1.0,
        SpanOptions {
            label: "A".into(),
            ..SpanOptions::default()
        },
    )
    .unwrap();
    a.band(
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[1.0, 2.0],
        BandOptions {
            label: "A".into(),
            ..BandOptions::default()
        },
    )
    .unwrap();
    a.y_error_bars(
        x(),
        &[0.0, 1.0],
        &[1.0, 2.0],
        ErrorBarOptions {
            label: "A".into(),
            ..ErrorBarOptions::default()
        },
    )
    .unwrap();
    a.x_error_bars(
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[1.0, 2.0],
        ErrorBarOptions {
            label: "A".into(),
            ..ErrorBarOptions::default()
        },
    )
    .unwrap();
    let mut a = f
        .add_axes_at(
            Placement {
                col: GridIndex::At(1),
                ..Placement::default()
            },
            ScaleKind::Category,
            ScaleKind::Linear,
        )
        .unwrap();
    let categories = || vec!["x".into()];
    let series = || {
        ["A", "B"]
            .map(|label| Series {
                label: label.into(),
                values: vec![1.0],
                color: crate::ColorSpec::AUTO,
            })
            .to_vec()
    };
    a.bar(
        categories(),
        &[1.0],
        BarOptions {
            label: "A".into(),
            ..BarOptions::default()
        },
    )
    .unwrap();
    a.grouped_bar(categories(), series(), crate::ValueLabels::default())
        .unwrap();
    a.stacked_bar(categories(), series(), crate::ValueLabels::default())
        .unwrap();
    let marks = resolve::resolve_figure_marks(&f);
    for (axes, marks) in f.panels().zip(&marks) {
        let entries: Vec<_> = marks.iter().flat_map(ResolvedMark::legend_swatch).collect();
        assert_eq!(
            entries,
            Legend::new(crate::LegendStyle::Header, marks, &Limits::of(axes)).row()
        );
        for entry in entries {
            assert_eq!(
                entry.color(),
                f.theme().palette()[usize::from(entry.label() == "B")]
            );
        }
    }
    let merged = Legend::merged(&f, marks.iter().map(Vec::as_slice));
    assert_eq!(merged.row().len(), 5);
    assert!(matches!(
        merged.row()[3].swatch(),
        Swatch::ErrorBar { vertical: true, .. }
    ));
}

fn figure(theme: Theme) -> Figure {
    let mut figure = Figure::with_grid(8.0, 4.0, theme, Grid::new(1, 2).unwrap()).unwrap();
    for col in [1, 0] {
        let mut axes = figure
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.title("Panel").subtitle("Details");
        for label in if col == 0 {
            vec!["North"]
        } else {
            vec!["North", "South"]
        } {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: label.into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
    }
    figure
}

#[test]
fn one_label_one_color_across_panels() {
    let f = figure(Theme::pigment());
    let marks = resolve::resolve_figure_marks(&f);
    let colors: Vec<Vec<_>> = marks
        .iter()
        .map(|panel| {
            panel
                .iter()
                .flat_map(ResolvedMark::legend_swatch)
                .map(|e| e.color())
                .collect()
        })
        .collect();
    assert_eq!(colors[0], vec![f.theme().palette()[0]]);
    assert_eq!(
        colors[1],
        vec![f.theme().palette()[0], f.theme().palette()[1]]
    );
}

#[test]
fn stems_share_label_slots_before_unlabelled_colors_and_merge_marker_swatches() {
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in [1, 0] {
        let mut axes = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        for label in ["", "North", if col == 0 { "North" } else { "South" }] {
            axes.stem(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                crate::StemOptions {
                    label: label.into(),
                    ..crate::StemOptions::default()
                },
            )
            .unwrap();
        }
    }
    let marks = resolve::resolve_figure_marks(&f);
    let palette = f.theme().palette();
    for (panel, expected) in marks.iter().zip([[2, 0, 0], [2, 0, 1]]) {
        for (mark, slot) in panel.iter().zip(expected) {
            assert!(matches!(mark, ResolvedMark::Stem { color, .. } if *color == palette[slot]));
        }
    }
    let legend = Legend::merged(&f, marks.iter().map(Vec::as_slice));
    assert_eq!(legend.row().len(), 2);
    for (entry, (slot, label)) in legend
        .row()
        .iter()
        .zip(["North", "South"].into_iter().enumerate())
    {
        assert_eq!(entry.label(), label);
        assert_eq!(entry.color(), palette[slot]);
        assert_eq!(
            entry.swatch(),
            Swatch::Dot {
                diameter: f.theme().marker_size(),
                marker: Marker::Circle,
            }
        );
    }
}

#[test]
fn stacked_area_layers_share_colors_and_merge_swatches_but_keep_direct_labels() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = Figure::with_grid(8.0, 4.0, theme, Grid::new(1, 2).unwrap()).unwrap();
        for col in [1, 0] {
            let mut axes = f
                .add_axes_at(
                    Placement {
                        col: GridIndex::At(col),
                        ..Placement::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            let series = ["", "North", if col == 0 { "North" } else { "South" }, ""]
                .map(|label| crate::Series {
                    label: label.into(),
                    values: vec![1.0, 1.0],
                    ..crate::Series::default()
                })
                .to_vec();
            axes.stacked_area(&[0.0, 1.0], series).unwrap();
            for label in ["North", ""] {
                axes.line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1.0, 2.0],
                    LineOptions {
                        label: label.into(),
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            }
        }
        let marks = resolve::resolve_figure_marks(&f);
        let palette = f.theme().palette();
        for (col, (axes, panel)) in f.panels().zip(&marks).enumerate() {
            let colors: Vec<_> = panel
                .iter()
                .flat_map(|mark| mark.keys(&Limits::of(axes)))
                .map(|key| key.color)
                .collect();
            assert_eq!(
                colors,
                [
                    palette[2],
                    palette[0],
                    palette[col],
                    palette[3],
                    palette[0],
                    palette[4]
                ]
            );
            let legend = Legend::new(f.theme().legend_style(), panel, &Limits::of(axes));
            if f.theme().legend_style() == crate::LegendStyle::Direct {
                assert!(legend.row().is_empty());
                assert_eq!(legend.direct().len(), 3);
                for (entry, y) in legend.direct().iter().zip([2.0, 3.0, 2.0]) {
                    assert_eq!(entry.anchor().x, resolve::AnchorX::Number(1.0));
                    assert_eq!(entry.anchor().y, y);
                }
            }
        }
        let legend = Legend::merged(&f, marks.iter().map(Vec::as_slice));
        if f.theme().legend_style() == crate::LegendStyle::Direct {
            assert!(legend.is_empty());
        } else {
            // Layers and the line merge by label; each entry keeps the swatch of its first
            // mark, here a layer's area swatch.
            let area = Swatch::Area {
                dash: crate::Dash::Solid,
                width: f.theme().drawn_line_width(crate::SizeSpec::Auto),
            };
            let entries: Vec<_> = legend
                .row()
                .iter()
                .map(|entry| (entry.label(), entry.color(), entry.swatch()))
                .collect();
            assert_eq!(
                entries,
                [("North", palette[0], area), ("South", palette[1], area)]
            );
        }
        f.to_scene(FontLibrary::global()).unwrap();
    }
}

#[test]
fn merged_legend_merges_same_label_and_kind() {
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in 0..2 {
        let mut axes = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Category,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.line(
            AxisData::Categories(vec!["x".into()]),
            &[1.0],
            LineOptions {
                label: "A".into(),
                color: crate::ColorSpec::Fixed(crate::Rgba::opaque(col as u8, 2, 3)),
                dash: if col == 0 {
                    crate::Dash::Dashed
                } else {
                    crate::Dash::Dotted
                },
                ..LineOptions::default()
            },
        )
        .unwrap();
        if col == 1 {
            axes.bar(
                vec!["x".into()],
                &[2.0],
                BarOptions {
                    label: "A".into(),
                    ..BarOptions::default()
                },
            )
            .unwrap();
        }
    }
    let marks = resolve::resolve_figure_marks(&f);
    let legend = Legend::merged(&f, marks.iter().map(Vec::as_slice));
    assert_eq!(legend.row().len(), 2);
    assert_eq!(
        legend.row()[0].swatch(),
        Swatch::Line {
            dash: crate::Dash::Dashed,
            width: f.theme().line_width()
        }
    );
    assert_eq!(legend.row()[0].color(), crate::Rgba::opaque(0, 2, 3));
    assert_eq!(legend.row()[1].swatch(), Swatch::Square);
}

#[test]
fn a_line_and_an_area_of_one_label_merge_as_before_area_swatches() {
    let line = |a: &mut crate::AxesMut<'_>| {
        a.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            LineOptions {
                label: "North".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    };
    let area = |a: &mut crate::AxesMut<'_>| {
        a.area(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.5, 1.0],
            crate::AreaOptions {
                label: "North".into(),
                ..crate::AreaOptions::default()
            },
        )
        .unwrap();
    };
    let width = Theme::pigment().drawn_line_width(crate::SizeSpec::Auto);
    let line_swatch = Swatch::Line {
        dash: crate::Dash::Solid,
        width,
    };
    let area_swatch = Swatch::Area {
        dash: crate::Dash::Solid,
        width,
    };
    // One panel: a panel legend keeps repeated labels, one entry per series, as it always did.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    line(&mut a);
    area(&mut a);
    let marks = resolve::resolve_marks(f.single_axes().marks(), f.theme());
    let legend = Legend::new(
        crate::LegendStyle::Header,
        &marks,
        &Limits::of(f.single_axes()),
    );
    let swatches: Vec<_> = legend
        .row()
        .iter()
        .map(|e| (e.label(), e.swatch()))
        .collect();
    assert_eq!(swatches, [("North", line_swatch), ("North", area_swatch)]);
    // A 1 x 2 grid with the line in one panel and the area in the other: the merged legend has
    // one "North" entry, with the swatch of the first mark in panel order.
    for (first, swatch) in [(true, line_swatch), (false, area_swatch)] {
        let mut f = Figure::with_grid(
            8.0,
            4.0,
            Theme::pigment(),
            Grid::new(1, 2)
                .unwrap()
                .with_legend(crate::LegendMode::Merged),
        )
        .unwrap();
        for col in 0..2 {
            let mut a = f
                .add_axes_at(
                    Placement {
                        col: GridIndex::At(col),
                        ..Placement::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            if (col == 0) == first {
                line(&mut a);
            } else {
                area(&mut a);
            }
        }
        let marks = resolve::resolve_figure_marks(&f);
        let merged = Legend::merged(&f, marks.iter().map(Vec::as_slice));
        let swatches: Vec<_> = merged
            .row()
            .iter()
            .map(|e| (e.label(), e.swatch()))
            .collect();
        assert_eq!(swatches, [("North", swatch)], "line first: {first}");
        f.to_scene(FontLibrary::global()).unwrap();
    }
}

#[test]
fn merged_legend_follows_figure_title_alignment() {
    let lib = FontLibrary::with_fallbacks(Vec::new());
    for align in [Align::Center, Align::Right] {
        let mut f = figure(Theme::pigment().with_title_align(align).unwrap());
        f.title("Figure").subtitle("Summary");
        let groups = ShareGroups::new(&f);
        let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
        let panels = panels(&f, &groups, &fonts).unwrap();
        let planned = plan(
            &f,
            &panels,
            &groups,
            &fonts,
            "Figure.to_scene()",
            &layout::LabelCache::new(&fonts),
            crate::pipeline::Rooms::Solved,
        )
        .unwrap();
        let entries = planned.header.legend.entries();
        let left = entries.first().unwrap().swatch.x;
        let last = entries.last().unwrap();
        let right = last.label.x + last.label.width();
        let width = f.width_in() * 72.0;
        let expected = if align == Align::Center {
            width / 2.0
        } else {
            width - PADDING
        };
        let actual = if align == Align::Center {
            (left + right) / 2.0
        } else {
            right
        };
        assert!((actual - expected).abs() < 0.5);
        for (_, panel) in &planned.panels {
            assert!(panel.legend.is_empty());
            assert!(panel.plot.y > planned.header.bottom);
        }
    }
}

#[test]
fn figure_title_is_one_and_a_half_title_sizes() {
    let lib = FontLibrary::with_fallbacks(Vec::new());
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = figure(theme.with_font_scale(1.4).unwrap());
        f.title("Figure");
        let groups = ShareGroups::new(&f);
        let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
        let panels = panels(&f, &groups, &fonts).unwrap();
        let planned = plan(
            &f,
            &panels,
            &groups,
            &fonts,
            "Figure.to_scene()",
            &layout::LabelCache::new(&fonts),
            crate::pipeline::Rooms::Solved,
        )
        .unwrap();
        let title = planned.header.title.unwrap();
        assert_eq!(
            title.first().style.size,
            1.5 * f.theme().title_size() * f.theme().font_scale()
        );
    }
}

#[test]
fn single_panel_left_alignment_is_unchanged() {
    let lib = FontLibrary::with_fallbacks(Vec::new());
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let make = |theme| {
            let mut f = Figure::new(6.0, 4.0, theme).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .title("Panel")
                .subtitle("Details")
                .line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1.0, 2.0],
                    LineOptions {
                        label: "North".into(),
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            f
        };
        let original = make(theme.clone());
        let left = make(theme.with_title_align(Align::Left).unwrap());
        assert_eq!(
            original.to_scene(&lib).unwrap(),
            left.to_scene(&lib).unwrap()
        );
        let (_, layout) = plan_figure(&left, (0, 0), &lib);
        assert_eq!(layout.title.unwrap().first().x, PADDING);
        assert_eq!(layout.subtitle.unwrap().first().x, PADDING);
    }
}

#[test]
fn labels_precede_unlabelled_colors_and_explicit_colors_never_register() {
    use crate::{Alpha, ColorSpec, Rgba};
    let theme = Theme::pigment();
    let palette = theme.palette().to_vec();
    let fixed = Rgba::opaque(1, 2, 3);
    let mut f = Figure::with_grid(8.0, 4.0, theme, Grid::new(1, 2).unwrap()).unwrap();
    for col in [1, 0] {
        let mut a = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        for (label, color) in [
            ("", ColorSpec::AUTO),
            ("B", ColorSpec::Fixed(fixed)),
            (
                "explicit only",
                ColorSpec::Palette {
                    index: 6,
                    alpha: Alpha::OPAQUE,
                },
            ),
            (if col == 0 { "A" } else { "B" }, ColorSpec::AUTO),
            ("A", ColorSpec::AUTO),
            ("", ColorSpec::AUTO),
            ("A", ColorSpec::Auto(Alpha::new(0.4).unwrap())),
        ] {
            a.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: label.into(),
                    color,
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
    }
    for (i, (axes, marks)) in f
        .panels()
        .zip(resolve::resolve_figure_marks(&f))
        .enumerate()
    {
        let colors: Vec<_> = marks
            .iter()
            .flat_map(|m| m.keys(&Limits::of(axes)))
            .map(|k| k.color)
            .collect();
        assert_eq!(
            colors,
            [
                palette[2],
                fixed,
                palette[6],
                palette[i],
                palette[0],
                palette[3],
                ColorSpec::Auto(Alpha::new(0.4).unwrap()).resolve(&palette, 0)
            ]
        );
    }
}

#[test]
fn merged_rows_wrap_and_align_independently_in_both_styles() {
    let lib = FontLibrary::with_fallbacks(Vec::new());
    for theme in [Theme::pigment(), Theme::midnight()] {
        for align in [Align::Left, Align::Center, Align::Right] {
            let mut f = Figure::with_grid(
                3.5,
                5.0,
                theme.with_title_align(align).unwrap(),
                Grid::new(1, 2).unwrap(),
            )
            .unwrap();
            f.title("Figure").subtitle("Summary");
            let mut a = f
                .add_axes_at(
                    Placement {
                        col: GridIndex::At(0),
                        ..Placement::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            a.title("Panel").subtitle("Details");
            for i in 0..10 {
                a.line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1.0, 2.0],
                    LineOptions {
                        label: format!("Series {i}"),
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            }
            let groups = ShareGroups::new(&f);
            let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
            let planned = plan(
                &f,
                &panels(&f, &groups, &fonts).unwrap(),
                &groups,
                &fonts,
                "Figure.to_scene()",
                &layout::LabelCache::new(&fonts),
                crate::pipeline::Rooms::Solved,
            )
            .unwrap();
            let rows: Vec<_> = planned
                .header
                .legend
                .entries()
                .chunk_by(|a, b| a.swatch.center_y == b.swatch.center_y)
                .collect();
            assert!(rows.len() > 1);
            let right = f.width_in() * 72.0 - PADDING;
            for row in rows {
                let first = row.first().unwrap();
                let last = row.last().unwrap();
                let start = first.chip.map_or(first.swatch.x, |c| c.x);
                let end = last
                    .chip
                    .map_or(last.label.x + last.label.width(), |c| c.right());
                assert!((start - align.position(end - start, PADDING, right)).abs() < 1e-9);
                assert!(start >= PADDING - 1e-9 && end <= right + 1e-9);
            }
            for label in planned
                .header
                .title
                .iter()
                .chain(&planned.header.subtitle)
                .flat_map(|block| block.lines())
            {
                assert_eq!(label.x, align.position(label.width(), PADDING, right));
            }
            // A panel's header is aligned from the left edge of its y tick labels to the right
            // edge of its plot.
            let panel = &planned.panels[0].1;
            let left = panel
                .y_tick_labels
                .iter()
                .map(|label| label.first().x)
                .fold(f64::INFINITY, f64::min);
            assert!(left < panel.plot.x);
            for label in panel
                .title
                .iter()
                .chain(&panel.subtitle)
                .flat_map(|block| block.lines())
            {
                let x = align.position(label.width(), left, panel.plot.right());
                assert!((label.x - x).abs() < 1e-9, "{} != {x}", label.x);
            }
        }
    }
}

#[test]
fn merged_entry_wider_than_the_inner_canvas_is_a_style_error() {
    let lib = FontLibrary::with_fallbacks(Vec::new());
    for theme in [Theme::pigment(), Theme::midnight()] {
        for align in [Align::Left, Align::Center, Align::Right] {
            let mut f = figure(theme.with_title_align(align).unwrap());
            let label = "W".repeat(200);
            f.axes_at_mut(GridIndex::Auto, GridIndex::At(0))
                .unwrap()
                .line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1.0, 2.0],
                    LineOptions {
                        label,
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            let err = f.to_scene(&lib).unwrap_err();
            assert!(matches!(err, crate::Error::Style { .. }));
            assert!(
                err.to_string()
                    .starts_with("Figure.to_scene(): the legend label \"WW")
            );
            assert!(err.to_string().contains("the figure has"));
            assert_eq!(err.hint(), Some("shorten the label or widen the figure."));
        }
    }
}

#[test]
fn direct_lines_stay_on_panels_and_other_kinds_merge() {
    use crate::{LegendStyle, ScatterOptions};
    let lib = FontLibrary::with_fallbacks(Vec::new());
    let mut f = figure(Theme::graphite());
    for col in 0..2 {
        f.axes_at_mut(GridIndex::Auto, GridIndex::At(col))
            .unwrap()
            .scatter(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                ScatterOptions {
                    label: "Samples".into(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
    }
    let groups = ShareGroups::new(&f);
    let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
    let planned = plan(
        &f,
        &panels(&f, &groups, &fonts).unwrap(),
        &groups,
        &fonts,
        "Figure.to_scene()",
        &layout::LabelCache::new(&fonts),
        crate::pipeline::Rooms::Solved,
    )
    .unwrap();
    assert_eq!(planned.header.legend.entries().len(), 1);
    assert_eq!(planned.header.legend.entries()[0].label.text, "Samples");
    assert_eq!(planned.panels[0].1.direct.len(), 1);
    assert_eq!(planned.panels[1].1.direct.len(), 2);
    assert!(planned.panels.iter().all(|(_, p)| p.legend.is_empty()));
    f.axes_at_mut(GridIndex::Auto, GridIndex::At(0))
        .unwrap()
        .legend(LegendStyle::Header, crate::SizeSpec::Auto)
        .unwrap();
    f.axes_at_mut(GridIndex::Auto, GridIndex::At(1))
        .unwrap()
        .legend(LegendStyle::None, crate::SizeSpec::Auto)
        .unwrap();
    let groups = ShareGroups::new(&f);
    let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
    let planned = plan(
        &f,
        &panels(&f, &groups, &fonts).unwrap(),
        &groups,
        &fonts,
        "Figure.to_scene()",
        &layout::LabelCache::new(&fonts),
        crate::pipeline::Rooms::Solved,
    )
    .unwrap();
    let labels: Vec<_> = planned
        .header
        .legend
        .entries()
        .iter()
        .map(|e| e.label.text.as_str())
        .collect();
    assert_eq!(labels, ["North", "Samples"]);
    assert!(planned.panels.iter().all(|(_, p)| p.direct.is_empty()));
}

#[test]
fn per_panel_legends_do_not_follow_title_alignment() {
    use crate::LegendMode;
    let lib = FontLibrary::with_fallbacks(Vec::new());
    for cols in [1, 2] {
        for align in [Align::Center, Align::Right] {
            let grid = Grid::new(1, cols)
                .unwrap()
                .with_legend(LegendMode::PerPanel);
            let mut f = Figure::with_grid(
                8.0,
                4.0,
                Theme::pigment().with_title_align(align).unwrap(),
                grid,
            )
            .unwrap();
            f.title("Figure");
            f.add_axes_at(
                Placement {
                    col: GridIndex::At(0),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .title("Panel")
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: "North".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
            let groups = ShareGroups::new(&f);
            let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
            let planned = plan(
                &f,
                &panels(&f, &groups, &fonts).unwrap(),
                &groups,
                &fonts,
                "Figure.to_scene()",
                &layout::LabelCache::new(&fonts),
                crate::pipeline::Rooms::Solved,
            )
            .unwrap();
            assert!(planned.header.legend.is_empty());
            let p = &planned.panels[0].1;
            // On a grid the header runs from the left edge of the y tick labels to the plot's
            // right edge.
            let (left, right) = if cols == 1 {
                (PADDING, f.width_in() * 72.0 - PADDING)
            } else {
                let left = p
                    .y_tick_labels
                    .iter()
                    .map(|label| label.first().x)
                    .fold(f64::INFINITY, f64::min);
                (left, p.plot.right())
            };
            assert!((p.legend.entries()[0].swatch.x - left).abs() < 1e-9);
            let title = p.title.as_ref().unwrap().first();
            let x = align.position(title.width(), left, right);
            assert!((title.x - x).abs() < 1e-9, "{} != {x}", title.x);
        }
    }
}

#[test]
fn figure_header_joins_single_and_grid_height_needs() {
    let lib = FontLibrary::with_fallbacks(Vec::new());
    for cols in [1, 2] {
        let make = |height, title| {
            let mut f = Figure::with_grid(
                8.0,
                height / 72.0,
                Theme::pigment(),
                Grid::new(1, cols).unwrap(),
            )
            .unwrap();
            f.title(title);
            f.add_axes_at(
                Placement {
                    col: GridIndex::At(0),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap();
            f
        };
        assert!(make(25.0, "").to_scene(&lib).is_ok());
        let err = make(60.0, "Figure").to_scene(&lib).unwrap_err();
        assert!(matches!(err, crate::Error::Style { .. }));
        assert!(err.to_string().contains("at least 62 pt"), "{err}");
        let f = make(62.0, "Figure");
        let groups = ShareGroups::new(&f);
        let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
        let planned = plan(
            &f,
            &panels(&f, &groups, &fonts).unwrap(),
            &groups,
            &fonts,
            "Figure.to_scene()",
            &layout::LabelCache::new(&fonts),
            crate::pipeline::Rooms::Solved,
        )
        .unwrap();
        assert_eq!(planned.header.bottom, PADDING + 22.5 * 1.25 + 8.0);
        assert_eq!(planned.panels[0].1.plot.y, planned.header.bottom);
        assert!(planned.panels[0].1.plot.height >= 1.0);
    }
}

#[test]
fn figure_titles_are_independent_cloneable_and_can_be_cleared() {
    let lib = FontLibrary::with_fallbacks(Vec::new());
    let mut f = figure(Theme::pigment());
    f.title("Figure").subtitle("Summary");
    let mut copy = f.clone();
    copy.title("").subtitle("");
    assert_eq!((f.title_text(), f.subtitle_text()), ("Figure", "Summary"));
    assert_eq!((copy.title_text(), copy.subtitle_text()), ("", ""));
    assert!(f.panels().all(|a| a.title_text() == "Panel"));
    let mut empty = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let blank = empty.to_scene(&lib).unwrap();
    empty.title("Figure");
    assert_ne!(empty.to_scene(&lib).unwrap(), blank);
    empty.title("");
    assert_eq!(empty.to_scene(&lib).unwrap(), blank);
}

/// The left edge of the y tick label column of `layout`: where its widest label starts.
fn tick_column_left(layout: &crate::layout::Layout) -> f64 {
    layout
        .y_tick_labels
        .iter()
        .flat_map(crate::layout::TextBlock::lines)
        .map(|label| label.x)
        .fold(f64::INFINITY, f64::min)
}

/// Asserts that `a` and `b` agree to within rounding.
#[track_caller]
fn assert_near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

/// A `width` by 4 in figure of `cols` panels with per-panel header legends, each titled,
/// subtitled and with a labelled line; `setup` finishes the panel in each column.
fn headed_grid(
    theme: Theme,
    width: f64,
    cols: i64,
    setup: impl Fn(i64, &mut crate::AxesMut<'_>),
) -> Figure {
    let grid = Grid::new(1, cols)
        .unwrap()
        .with_legend(crate::LegendMode::PerPanel);
    let mut f = Figure::with_grid(width, 4.0, theme, grid).unwrap();
    for col in 0..cols {
        let mut axes = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.title("Days")
            .subtitle("By type")
            .legend(crate::LegendStyle::Header, crate::SizeSpec::Auto)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[10.0, 2000.0],
                LineOptions {
                    label: "Sunny".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        setup(col, &mut axes);
    }
    f
}

#[test]
fn panel_headers_start_where_the_panel_content_starts() {
    // A panel's title, subtitle and header legend start at its y axis label, which starts
    // its y tick label column; without a label, at the column; without either, at the plot.
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for align in [Align::Left, Align::Center] {
            let theme = theme.with_title_align(align).unwrap();
            let f = headed_grid(theme, 10.0, 3, |col, axes| match col {
                0 => {
                    axes.ylabel("Share of days (%)");
                }
                1 => {}
                _ => {
                    axes.yticks(&[]).unwrap();
                }
            });
            let planned = try_plan_panels(&f, FontLibrary::global()).unwrap();
            for (col, (_, layout)) in planned.iter().enumerate() {
                let left = match col {
                    0 => {
                        let label = layout.ylabel.as_ref().unwrap();
                        assert_near(label.x, tick_column_left(layout));
                        label.x
                    }
                    1 => tick_column_left(layout),
                    _ => {
                        assert!(layout.y_tick_labels.is_empty());
                        layout.plot.x
                    }
                };
                if col < 2 {
                    assert!(left < layout.plot.x - 6.0);
                }
                let right = layout.plot.right();
                for block in [&layout.title, &layout.subtitle] {
                    let line = block.as_ref().unwrap().first();
                    assert_near(line.x, align.position(line.width(), left, right));
                }
                // The legend row starts there whatever the title alignment.
                assert_near(layout.legend.entries()[0].swatch.x, left);
            }
        }
    }
}

#[test]
fn a_twin_panel_header_starts_at_the_left_y_tick_labels() {
    let f = headed_grid(Theme::pigment(), 9.0, 2, |col, axes| {
        if col == 1 {
            axes.twin_y(ScaleKind::Linear)
                .unwrap()
                .ylabel("Right")
                .line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[0.0, 1.0],
                    LineOptions::default(),
                )
                .unwrap();
        }
    });
    let planned = try_plan_panels(&f, FontLibrary::global()).unwrap();
    let (_, panel) = &planned[1];
    let title = panel.title.as_ref().unwrap().first();
    assert_near(title.x, tick_column_left(panel));
    assert_near(panel.legend.entries()[0].swatch.x, tick_column_left(panel));
}

#[test]
fn figure_texts_stay_at_the_figure_margin() {
    // The figure title, subtitle, caption and merged legend start at the left padding, which
    // is where the panels of the first column start their content.
    let mut f = figure(Theme::pigment());
    f.title("Weather")
        .subtitle("Two stations")
        .caption("Source: survey");
    f.axes_at_mut(GridIndex::Auto, GridIndex::At(0))
        .unwrap()
        .ylabel("Share of days (%)");
    let lib = FontLibrary::global();
    let groups = ShareGroups::new(&f);
    let fonts = layout::figure_fonts(lib, f.theme()).unwrap();
    let planned = plan(
        &f,
        &panels(&f, &groups, &fonts).unwrap(),
        &groups,
        &fonts,
        "Figure.to_scene()",
        &layout::LabelCache::new(&fonts),
        crate::pipeline::Rooms::Solved,
    )
    .unwrap();
    let header = &planned.header;
    for block in [&header.title, &header.subtitle, &header.caption.text] {
        assert_eq!(block.as_ref().unwrap().first().x, PADDING);
    }
    assert_eq!(header.legend.entries()[0].swatch.x, PADDING);
    let first = &planned.panels[0].1;
    assert_near(first.ylabel.as_ref().unwrap().x, PADDING);
    assert_near(first.title.as_ref().unwrap().first().x, PADDING);
}

#[test]
fn a_pie_header_keeps_the_left_edge_of_its_cell() {
    let mut f = Figure::with_grid(9.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    f.add_axes_at(
        Placement {
            col: GridIndex::At(0),
            ..Placement::default()
        },
        ScaleKind::Linear,
        ScaleKind::Linear,
    )
    .unwrap()
    .title("Days")
    .ylabel("Share of days (%)");
    f.add_pie_axes_at(Placement {
        col: GridIndex::At(1),
        ..Placement::default()
    })
    .unwrap()
    .title("Orders")
    .pie(
        vec!["A".into(), "B".into()],
        &[1.0, 2.0],
        crate::PieOptions::default(),
    )
    .unwrap();
    let planned = try_plan_panels(&f, FontLibrary::global()).unwrap();
    let (_, pie) = &planned[1];
    assert_eq!(pie.title.as_ref().unwrap().first().x, pie.plot.x);
}

#[test]
fn a_single_panel_header_moves_in_with_a_plot_inset_by_x_tick_labels() {
    for legend in [
        crate::LegendPosition::TopLeft,
        crate::LegendPosition::Bottom,
    ] {
        let build = |inset: bool| {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            let first = if inset { "W".repeat(10) } else { "0".into() };
            axes.title("Days")
                .ylabel("Share")
                .legend_at(crate::LegendStyle::Header, legend, crate::SizeSpec::Auto)
                .unwrap()
                .xlim(0.0, 1.0)
                .unwrap()
                .xticks_labeled(&[0.0, 1.0], vec![first, "1".into()])
                .unwrap()
                .yticks_labeled(&[0.0, 1.0], vec!["0".into(), "1".into()])
                .unwrap();
            for i in 0..8 {
                axes.line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[0.0, 1.0],
                    LineOptions {
                        label: format!("Station {i}"),
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            }
            f
        };
        // Without an inset the header starts at the padding, as the y axis label does.
        let (_, plain) = &try_plan_panels(&build(false), FontLibrary::global()).unwrap()[0];
        assert_eq!(plain.title.as_ref().unwrap().first().x, PADDING);
        assert_near(plain.ylabel.as_ref().unwrap().x, PADDING);
        // A wide first x tick label moves the plot, its y labels and the header in.
        let f = build(true);
        f.to_scene(FontLibrary::global()).unwrap();
        let (_, moved) = &try_plan_panels(&f, FontLibrary::global()).unwrap()[0];
        let label = moved.ylabel.as_ref().unwrap();
        assert!(label.x > PADDING + 1.0, "{}", label.x);
        assert_near(moved.title.as_ref().unwrap().first().x, label.x);
        let entries = moved.legend.entries();
        assert_near(entries[0].swatch.x, label.x);
        let right = 6.0 * 72.0 - PADDING;
        for entry in entries {
            assert!(entry.swatch.x >= label.x - 1e-9);
            assert!(entry.label.x + entry.label.width() <= right + 1e-9);
        }
    }
}

#[test]
fn headers_over_equal_aspect_plots_start_at_their_own_y_axis_label() {
    // The rooms come from the same figure without equal aspect, whose y tick labels differ;
    // the header still starts where this figure's y axis label does.
    for cols in [1, 2] {
        let mut f = Figure::with_grid(
            6.0 * cols as f64,
            4.0,
            Theme::pigment(),
            Grid::new(1, cols).unwrap(),
        )
        .unwrap();
        for col in 0..cols {
            f.add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .xlim(0.0, 20.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap()
            .title("T")
            .ylabel("Y")
            .equal_aspect()
            .unwrap();
        }
        f.to_scene(FontLibrary::global()).unwrap();
        for (_, layout) in try_plan_panels(&f, FontLibrary::global()).unwrap() {
            let label = layout.ylabel.as_ref().unwrap();
            assert_near(label.x, tick_column_left(&layout));
            assert_near(layout.title.as_ref().unwrap().first().x, label.x);
        }
    }
}

/// A 3.5 in wide figure `height` pt tall whose wide first x tick label insets the plot, with a
/// header legend at `position` of eight entries.
fn inset_legend(height: f64, position: crate::LegendPosition) -> Figure {
    let mut f = Figure::new(3.5, height / 72.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes.title("Days")
        .ylabel("Share")
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .xticks_labeled(&[0.0, 1.0], vec!["W".repeat(10), "1".into()])
        .unwrap()
        .yticks_labeled(&[0.0, 1.0], vec!["0".into(), "1".into()])
        .unwrap()
        .legend_at(crate::LegendStyle::Header, position, crate::SizeSpec::Auto)
        .unwrap();
    for i in 0..8 {
        axes.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions {
                label: format!("RRR{i}"),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    f
}

#[test]
fn a_short_figure_with_an_inset_legend_names_its_height_not_a_colorbar() {
    for (height, position) in [
        (119.0, crate::LegendPosition::TopLeft),
        (121.0, crate::LegendPosition::Bottom),
    ] {
        let error = inset_legend(height, position)
            .to_scene(FontLibrary::global())
            .unwrap_err();
        let message = error.to_string();
        assert!(!message.contains("colorbar"), "{message}");
        let needed: f64 = message
            .split_once("at least ")
            .and_then(|(_, rest)| rest.split_once(" pt"))
            .and_then(|(number, _)| number.parse().ok())
            .unwrap_or_else(|| panic!("{message}"));
        inset_legend(needed, position)
            .to_scene(FontLibrary::global())
            .unwrap();
    }
}

#[test]
fn an_equal_aspect_header_moves_in_only_to_its_y_axis_label() {
    // The plot moves right in its room for its wider y tick labels; the header starts with
    // the y axis label, at the padding, not that far further in.
    let mut f = Figure::new(3.5, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 20.0)
        .unwrap()
        .ylim(-1.0, 9.0)
        .unwrap()
        .title("T")
        .ylabel("Y")
        .equal_aspect()
        .unwrap();
    f.to_scene(FontLibrary::global()).unwrap();
    let (_, layout) = &try_plan_panels(&f, FontLibrary::global()).unwrap()[0];
    let label = layout.ylabel.as_ref().unwrap();
    assert_near(layout.title.as_ref().unwrap().first().x, label.x);
}

#[test]
fn an_equal_aspect_header_legend_keeps_the_rows_it_reserved() {
    // The legend row wraps within the reference's header box, whose height the rows reserve,
    // so it ends above the y axis label.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes.xlim(0.0, 20.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .title("T")
        .ylabel("Y axis label (units)")
        .equal_aspect()
        .unwrap()
        .legend(crate::LegendStyle::Header, crate::SizeSpec::Auto)
        .unwrap();
    for i in 0..3 {
        axes.line(
            AxisData::Numbers(vec![0.0, 20.0]),
            &[0.0, 1.0],
            LineOptions {
                label: format!("{}{i}", "R".repeat(18)),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    f.to_scene(FontLibrary::global()).unwrap();
    let (_, layout) = &try_plan_panels(&f, FontLibrary::global()).unwrap()[0];
    let label = layout.ylabel.as_ref().unwrap();
    assert!(
        layout.header_bottom <= label.y - 0.5 * f64::from(label.style.size) + 1e-9,
        "{} {}",
        layout.header_bottom,
        label.y
    );
    let last = layout
        .legend
        .entries()
        .iter()
        .map(|entry| entry.label.y)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(last < label.y - 8.0, "{last} {}", label.y);
}
