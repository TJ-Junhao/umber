//! Figure headers, alignment and shared legend colors.

use super::*;
use crate::layout::PADDING;
use crate::legend::Swatch;
use crate::{Align, AxisData, BarOptions, Grid, GridIndex, LineOptions, Placement, ScaleKind};

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
    a.grouped_bar(categories(), series()).unwrap();
    a.stacked_bar(categories(), series()).unwrap();
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
        let panels = panels(&f, &groups, &lib);
        let planned = plan(
            &f,
            &panels,
            &groups,
            &lib,
            "Figure.to_scene()",
            &layout::LabelCache::new(&lib),
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
        let panels = panels(&f, &groups, &lib);
        let planned = plan(
            &f,
            &panels,
            &groups,
            &lib,
            "Figure.to_scene()",
            &layout::LabelCache::new(&lib),
        )
        .unwrap();
        let title = planned.header.title.unwrap();
        assert_eq!(
            title.style.size,
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
        assert_eq!(layout.title.unwrap().x, PADDING);
        assert_eq!(layout.subtitle.unwrap().x, PADDING);
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
            let planned = plan(
                &f,
                &panels(&f, &groups, &lib),
                &groups,
                &lib,
                "Figure.to_scene()",
                &layout::LabelCache::new(&lib),
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
            for label in planned.header.title.iter().chain(&planned.header.subtitle) {
                assert_eq!(label.x, align.position(label.width(), PADDING, right));
            }
            let panel = &planned.panels[0].1;
            for label in panel.title.iter().chain(&panel.subtitle) {
                assert_eq!(
                    label.x,
                    align.position(label.width(), panel.plot.x, panel.plot.right())
                );
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
    let planned = plan(
        &f,
        &panels(&f, &groups, &lib),
        &groups,
        &lib,
        "Figure.to_scene()",
        &layout::LabelCache::new(&lib),
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
    let planned = plan(
        &f,
        &panels(&f, &groups, &lib),
        &groups,
        &lib,
        "Figure.to_scene()",
        &layout::LabelCache::new(&lib),
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
            let planned = plan(
                &f,
                &panels(&f, &groups, &lib),
                &groups,
                &lib,
                "Figure.to_scene()",
                &layout::LabelCache::new(&lib),
            )
            .unwrap();
            assert!(planned.header.legend.is_empty());
            let p = &planned.panels[0].1;
            let (left, right) = if cols == 1 {
                (PADDING, f.width_in() * 72.0 - PADDING)
            } else {
                (p.plot.x, p.plot.right())
            };
            assert_eq!(p.legend.entries()[0].swatch.x, left);
            let title = p.title.as_ref().unwrap();
            assert_eq!(title.x, align.position(title.width(), left, right));
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
        let planned = plan(
            &f,
            &panels(&f, &groups, &lib),
            &groups,
            &lib,
            "Figure.to_scene()",
            &layout::LabelCache::new(&lib),
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
