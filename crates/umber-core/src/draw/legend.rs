//! Legend drawing: swatches, chips, labels and the end dots of direct labels.
//!
//! Everything the legend draws is one unclipped group: the legend row sits in the header, and
//! direct labels sit right of the ends of their lines, mostly outside the plot area, so neither
//! is cut by the plot clip.

use super::bar_shape::rounded_rect;
use super::chrome::{segment, text};
use super::error_bars::{bar_style, push_bar};
use super::line::line_style;
use super::mapping::point;
use super::marker::{marker_shape, polygon};
use crate::geometry::Path;
use crate::layout::{Area, EndDot, Layout, RowEntry};
use crate::legend::Swatch;
use crate::scene::{FillRule, Item, LineCap, Paint};
use crate::{Marker, Rgba, Theme};

/// The opacity of a chip's tint, as a share of its series color's own.
const CHIP_TINT: f32 = 0.14;

/// The legend of `layout` as one unclipped group: for each row entry in order, its chip when it
/// has one, its swatch, and then its label in the theme's text color; then for each direct
/// label in order, its end dot and its label when it has them, both in the line color. `None`
/// when the legend has neither, so a figure without a legend gets no group.
pub(crate) fn legend(theme: &Theme, layout: &Layout) -> Option<Item> {
    let (entries, direct) = (layout.legend.entries(), &layout.direct);
    if entries.is_empty() && direct.is_empty() {
        return None;
    }
    let mut items = Vec::with_capacity(3 * entries.len() + 2 * direct.len());
    if let Some(box_) = layout.legend.background {
        items.push(Item::Fill {
            path: rounded_rect(box_.x, box_.right(), box_.bottom(), box_.y, 0.0, 0.0),
            paint: Paint::Solid(
                theme
                    .background()
                    .scale_alpha(theme.legend_background_alpha()),
            ),
            rule: FillRule::NonZero,
        });
    }
    row(theme, entries, &mut items);
    for label in direct {
        items.extend(label.dot.map(|d| end_dot(d, label.color)));
        items.extend(label.label.as_ref().map(|l| text(l, label.color)));
    }
    Some(Item::Group { clip: None, items })
}

/// Appends row entries, in their original drawing order, to a panel or figure header.
pub(super) fn row(theme: &Theme, entries: &[RowEntry], items: &mut Vec<Item>) {
    for entry in entries {
        if let Some(pill) = entry.chip {
            items.push(chip(pill, entry.swatch.color));
        }
        items.push(swatch(theme, entry));
        items.push(text(&entry.label, theme.text_color()));
    }
}

/// The end dot `d` of a directly labelled line: a filled circle in the line `color`.
fn end_dot(d: EndDot, color: Rgba) -> Item {
    dot(d.x, d.y, d.diameter, color)
}

/// A filled circle of `diameter` centered on `(x, y)`, in `color`.
fn dot(x: f64, y: f64, diameter: f64, color: Rgba) -> Item {
    let center = point(x, y);
    Item::Fill {
        path: marker_shape(Marker::Circle, diameter as f32).translated(center.x, center.y),
        paint: Paint::Solid(color),
        rule: FillRule::NonZero,
    }
}

/// The pill of a chip, filling the box `pill`: a rounded rectangle whose radius is half its height,
/// filled with [`chip_tint`] of the series `color`.
fn chip(pill: Area, color: Rgba) -> Item {
    let radius = pill.height / 2.0;
    Item::Fill {
        path: rounded_rect(pill.x, pill.right(), pill.bottom(), pill.y, radius, radius),
        paint: Paint::Solid(chip_tint(color)),
        rule: FillRule::NonZero,
    }
}

/// The tint of a chip: `color` at `0.14 ×` its own alpha.
fn chip_tint(color: Rgba) -> Rgba {
    color.scale_alpha(CHIP_TINT)
}

/// The swatch of `entry`, in its series color, inside its swatch box: a line in the series dash
/// and width, a filled dot, a filled square, or a bar with caps.
fn swatch(theme: &Theme, entry: &RowEntry) -> Item {
    let b = entry.swatch;
    match b.kind {
        Swatch::Bubble { radius } => Item::Markers {
            shape: marker_shape(Marker::Circle, 2.0 * radius),
            positions: vec![point(b.x + b.width / 2.0, b.center_y)],
            fill: Some(b.color.scale_alpha(theme.marker_alpha())),
            stroke: Some(crate::scene::StrokeStyle {
                color: theme.background(),
                width: super::MARKER_OUTLINE,
                dash: Vec::new(),
                cap: LineCap::Butt,
                join: crate::scene::LineJoin::Miter,
            }),
        },
        Swatch::Line { dash, width } => {
            let style = line_style(width, b.color, dash);
            // Round caps reach half the stroke width past the ends; keep them inside the box.
            let inset = match style.cap {
                LineCap::Butt => 0.0,
                LineCap::Round | LineCap::Square => {
                    (f64::from(style.width) / 2.0).min(b.width / 2.0)
                }
            };
            Item::Stroke {
                path: segment(
                    point(b.x + inset, b.center_y),
                    point(b.x + b.width - inset, b.center_y),
                ),
                style,
            }
        }
        Swatch::Dot { .. } => dot(b.x + b.width / 2.0, b.center_y, b.width, b.color),
        Swatch::ErrorBar { vertical, width } => {
            // Each end cap reaches half a stroke beyond its centre line along the bar.
            let inset = f64::from(width) / 2.0;
            let (start, end) = if vertical {
                let x = b.x + b.width / 2.0;
                (
                    point(x, b.center_y - b.height / 2.0 + inset),
                    point(x, b.center_y + b.height / 2.0 - inset),
                )
            } else {
                (
                    point(b.x + inset, b.center_y),
                    point(b.x + b.width - inset, b.center_y),
                )
            };
            let mut path = Path::new();
            push_bar(&mut path, vertical, (start, end), Some(theme.cap_width()));
            Item::Stroke {
                path,
                style: bar_style(width, b.color),
            }
        }
        Swatch::Box { width } => {
            // The outline reaches width/4 beyond its path. Keep its entire envelope inside
            // the measured box; the median's butt ends stop at that inset too.
            let inset = f64::from(width) / 4.0;
            let (left, right) = (b.x + inset, b.x + b.width - inset);
            let (top, bottom) = (
                b.center_y - b.height / 2.0 + inset,
                b.center_y + b.height / 2.0 - inset,
            );
            let path = polygon(&[
                (left as f32, top as f32),
                (right as f32, top as f32),
                (right as f32, bottom as f32),
                (left as f32, bottom as f32),
            ]);
            Item::Group {
                clip: None,
                items: vec![
                    Item::Fill {
                        path: path.clone(),
                        paint: Paint::Solid(b.color.scale_alpha(super::area::AREA_ALPHA)),
                        rule: FillRule::NonZero,
                    },
                    Item::Stroke {
                        path,
                        style: bar_style(width / 2.0, b.color),
                    },
                    Item::Stroke {
                        path: segment(point(left, b.center_y), point(right, b.center_y)),
                        style: bar_style(width, b.color),
                    },
                ],
            }
        }
        Swatch::Violin { width } => {
            let inset = f64::from(width) / 4.0;
            let center = b.x + b.width / 2.0;
            let (left, right) = (b.x + inset, b.x + b.width - inset);
            let (top, bottom) = (
                b.center_y - b.height / 2.0 + inset,
                b.center_y + b.height / 2.0 - inset,
            );
            let mut path = Path::new();
            path.move_to(point(center, top));
            // Control points stay inside the stroke-inset box, as does the whole curve.
            path.cubic_to(
                point(right, top),
                point(right, bottom),
                point(center, bottom),
            );
            path.cubic_to(point(left, bottom), point(left, top), point(center, top));
            path.close();
            Item::Group {
                clip: None,
                items: vec![
                    Item::Fill {
                        path: path.clone(),
                        paint: Paint::Solid(b.color.scale_alpha(super::area::AREA_ALPHA)),
                        rule: FillRule::NonZero,
                    },
                    Item::Stroke {
                        path,
                        style: bar_style(width / 2.0, b.color),
                    },
                ],
            }
        }
        Swatch::Square => {
            let (top_left, bottom_right) = (
                point(b.x, b.center_y - b.height / 2.0),
                point(b.x + b.width, b.center_y + b.height / 2.0),
            );
            Item::Fill {
                path: polygon(&[
                    (top_left.x, top_left.y),
                    (bottom_right.x, top_left.y),
                    (bottom_right.x, bottom_right.y),
                    (top_left.x, bottom_right.y),
                ]),
                paint: Paint::Solid(b.color),
                rule: FillRule::NonZero,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions, ScatterOptions};
    use crate::{Dash, FontLibrary, ScaleKind};

    fn layout(theme: Theme) -> (Theme, Layout) {
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        for (label, dash) in [("Solid", Dash::Solid), ("Dashed", Dash::Dashed)] {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: label.to_owned(),
                    dash,
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        axes.scatter(
            AxisData::Numbers(vec![0.5]),
            &[1.5],
            ScatterOptions {
                label: "Points".to_owned(),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
        (
            theme,
            crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).1,
        )
    }

    fn bounds(item: &Item) -> (f32, f32, f32, f32) {
        let path = match item {
            Item::Fill { path, .. } | Item::Stroke { path, .. } => path,
            other => panic!("not a path item: {other:?}"),
        };
        let (min, max) = path.bounds().unwrap();
        (min.x, min.y, max.x, max.y)
    }

    #[test]
    fn each_entry_draws_its_swatch_then_its_label() {
        let (theme, layout) = layout(Theme::pigment());
        let Some(Item::Group { clip: None, items }) = legend(&theme, &layout) else {
            panic!("an unclipped legend group");
        };
        let entries = layout.legend.entries();
        assert_eq!(items.len(), 2 * entries.len());
        for (pair, entry) in items.chunks(2).zip(entries) {
            let Item::Text(label) = &pair[1] else {
                panic!("a label after each swatch");
            };
            assert_eq!(label.text, entry.label.text);
            assert_eq!(label.color, theme.text_color());
        }
        // Solid line: round caps, inset by half the line width so the cap ends at the box.
        let solid = &entries[0].swatch;
        let Item::Stroke { style, .. } = &items[0] else {
            panic!("a line swatch is a stroke");
        };
        assert_eq!(style.cap, LineCap::Round);
        assert_eq!(style.color, solid.color);
        let half = f64::from(style.width) / 2.0;
        let (x0, y0, x1, y1) = bounds(&items[0]);
        assert!((f64::from(x0) - (solid.x + half)).abs() < 1e-3);
        assert!((f64::from(x1) - (solid.x + solid.width - half)).abs() < 1e-3);
        assert!(y0 == y1 && (f64::from(y0) - solid.center_y).abs() < 1e-3);
        // Dashed line: butt caps, the full box.
        let dashed = &entries[1].swatch;
        let Item::Stroke { style, .. } = &items[2] else {
            panic!("a line swatch is a stroke");
        };
        assert_eq!(style.cap, LineCap::Butt);
        assert!(!style.dash.is_empty());
        let (x0, _, x1, _) = bounds(&items[2]);
        assert!((f64::from(x0) - dashed.x).abs() < 1e-3);
        assert!((f64::from(x1) - (dashed.x + dashed.width)).abs() < 1e-3);
        // Dot: a filled circle filling its box.
        let dot = &entries[2].swatch;
        assert!(matches!(&items[4], Item::Fill { paint: Paint::Solid(c), .. } if *c == dot.color));
        let (x0, y0, x1, y1) = bounds(&items[4]);
        assert!((f64::from(x0) - dot.x).abs() < 1e-3);
        assert!((f64::from(x1 - x0) - dot.width).abs() < 1e-3);
        assert!((f64::from(y0 + y1) / 2.0 - dot.center_y).abs() < 1e-3);
    }

    #[test]
    fn square_swatch_fills_its_box() {
        let (theme, layout) = layout(Theme::pigment());
        let mut entry = layout.legend.entries()[2].clone();
        entry.swatch.kind = Swatch::Square;
        entry.swatch.height = entry.swatch.width;
        let item = swatch(&theme, &entry);
        assert!(
            matches!(&item, Item::Fill { paint: Paint::Solid(c), .. } if *c == entry.swatch.color)
        );
        let b = entry.swatch;
        let (x0, y0, x1, y1) = bounds(&item);
        assert!(
            (f64::from(x0) - b.x).abs() < 1e-3 && (f64::from(x1) - (b.x + b.width)).abs() < 1e-3
        );
        assert!((f64::from(y0) - (b.center_y - b.height / 2.0)).abs() < 1e-3);
        assert!((f64::from(y1) - (b.center_y + b.height / 2.0)).abs() < 1e-3);
    }

    /// The legend of a figure with vertical and horizontal error bars labelled, in `theme`.
    fn error_bar_layout(theme: Theme) -> (Theme, Layout) {
        use crate::model::ErrorBarOptions;
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let options = |label: &str| ErrorBarOptions {
            label: label.to_owned(),
            ..ErrorBarOptions::default()
        };
        axes.y_error_bars(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.5, 1.0],
            &[1.5, 2.0],
            options("Down"),
        )
        .unwrap()
        .x_error_bars(&[0.5], &[0.0], &[1.0], options("Across"))
        .unwrap();
        (
            theme,
            crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).1,
        )
    }

    #[test]
    fn an_error_bar_swatch_is_a_bar_with_a_cap_at_each_end() {
        let (theme, layout) = error_bar_layout(Theme::pigment());
        let Some(Item::Group { clip: None, items }) = legend(&theme, &layout) else {
            panic!("an unclipped legend group");
        };
        let entries = layout.legend.entries();
        assert_eq!(items.len(), 2 * entries.len());
        let cap = f64::from(theme.cap_width());
        let size = f64::from(theme.body_size() * theme.font_scale());
        for (pair, entry, vertical) in [
            (&items[0], &entries[0], true),
            (&items[2], &entries[1], false),
        ] {
            let b = entry.swatch;
            let Item::Stroke { path, style } = pair else {
                panic!("an error bar swatch is a stroke");
            };
            assert_eq!(style.cap, LineCap::Butt);
            assert_eq!(style.color, b.color);
            // The box includes the stroke of the end caps and of the stem. Caps keep the
            // theme's cap width, and the line is at most a quarter of the label size thick.
            let thickness = f64::from(style.width);
            let (long, across) = if vertical {
                (b.height, b.width)
            } else {
                (b.width, b.height)
            };
            assert!((long - (1.1 * size + thickness)).abs() < 1e-3);
            assert!((across - cap.max(thickness)).abs() < 1e-3);
            assert!(thickness <= 0.25 * size + 1e-6);
            let parts: Vec<_> = path.verbs().iter().collect();
            assert_eq!(parts.len(), 6, "a bar and two caps: {parts:?}");
            let (min, max) = path.bounds().unwrap();
            let (w, h) = (f64::from(max.x - min.x), f64::from(max.y - min.y));
            let (expected_w, expected_h) = if vertical {
                (cap, 1.1 * size)
            } else {
                (1.1 * size, cap)
            };
            assert!((w - expected_w).abs() < 1e-3 && (h - expected_h).abs() < 1e-3);
            assert!(
                (f64::from(min.x + max.x) / 2.0 - (b.x + b.width / 2.0)).abs() < 1e-3
                    && (f64::from(min.y + max.y) / 2.0 - b.center_y).abs() < 1e-3
            );
        }
        // A chips row draws a dot for them as for any series.
        let (theme, layout) = error_bar_layout(Theme::midnight());
        let Some(Item::Group { items, .. }) = legend(&theme, &layout) else {
            panic!("a legend group");
        };
        assert_eq!(items.len(), 3 * layout.legend.entries().len());
        assert!(
            layout
                .legend
                .entries()
                .iter()
                .all(|entry| matches!(entry.swatch.kind, Swatch::Dot { .. }))
        );
    }

    /// Checks the stem and both caps against the measured box, including their stroke width.
    fn error_bar_envelope(vertical: bool) {
        use crate::geometry::PathVerb;
        use crate::model::ErrorBarOptions;
        use crate::{LegendStyle, SizeSpec};

        for base in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for scale in [0.5, 1.0, 4.0, 10.0] {
                for width in [0.5, 8.0, 72.0] {
                    for row_style in [LegendStyle::Header, LegendStyle::Direct] {
                        let theme = base
                            .with_font_scale(scale)
                            .unwrap()
                            .with_legend_style(row_style)
                            .unwrap();
                        let mut f = Figure::new(20.0, 12.0, theme.clone()).unwrap();
                        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
                        let options = ErrorBarOptions {
                            label: "C".into(),
                            width: SizeSpec::pt(width).unwrap(),
                            ..ErrorBarOptions::default()
                        };
                        if vertical {
                            axes.y_error_bars(
                                AxisData::Numbers(vec![1.0]),
                                &[0.0],
                                &[2.0],
                                options,
                            )
                            .unwrap();
                        } else {
                            axes.x_error_bars(&[1.0], &[0.0], &[2.0], options).unwrap();
                        }
                        let layout =
                            crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).1;
                        let entry = &layout.legend.entries()[0];
                        let b = entry.swatch;
                        let Item::Stroke { path, style } = swatch(&theme, entry) else {
                            panic!("an error bar swatch is a stroke");
                        };
                        assert_eq!(style.cap, LineCap::Butt);
                        assert_eq!(path.verbs().len(), 6);
                        let half = f64::from(style.width) / 2.0;
                        for (i, segment) in path.verbs().chunks_exact(2).enumerate() {
                            let [PathVerb::MoveTo(a), PathVerb::LineTo(z)] = segment else {
                                panic!("a stem or cap is a segment");
                            };
                            let (dx, dy) = if a.x == z.x {
                                (half, 0.0)
                            } else {
                                assert_eq!(a.y, z.y);
                                (0.0, half)
                            };
                            let left = f64::from(a.x.min(z.x)) - dx;
                            let right = f64::from(a.x.max(z.x)) + dx;
                            let top = f64::from(a.y.min(z.y)) - dy;
                            let bottom = f64::from(a.y.max(z.y)) + dy;
                            assert!(
                                left >= b.x - 1e-3
                                    && right <= b.x + b.width + 1e-3
                                    && top >= b.center_y - b.height / 2.0 - 1e-3
                                    && bottom <= b.center_y + b.height / 2.0 + 1e-3,
                                "vertical={vertical} scale={scale} width={width} segment={i}: \
                                 ({left}, {top})..({right}, {bottom}) outside {b:?}"
                            );
                            assert!(entry.label.x - right >= 4.0 - 1e-3);
                            if i > 0 {
                                let length = f64::from((z.x - a.x).abs() + (z.y - a.y).abs());
                                assert!((length - f64::from(theme.cap_width())).abs() < 1e-3);
                            }
                        }
                        assert!((entry.label.x - (b.x + b.width) - 4.0).abs() < 1e-3);
                    }
                }
            }
        }
    }

    #[test]
    fn horizontal_error_bar_strokes_fit_the_swatch_box_before_the_label_gap() {
        error_bar_envelope(false);
    }

    #[test]
    fn vertical_error_bar_strokes_fit_the_swatch_box_before_the_label_gap() {
        error_bar_envelope(true);
    }

    #[test]
    fn box_swatch_strokes_fit_the_measured_bounds() {
        use crate::LegendStyle;
        use crate::model::BoxOptions;
        for base in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for scale in [0.5, 1.0, 4.0, 10.0] {
                for width in [0.5, 8.0, 72.0, f64::from(f32::MAX)] {
                    for style in [LegendStyle::Header, LegendStyle::Direct] {
                        let theme = base
                            .with_font_scale(scale)
                            .unwrap()
                            .with_line_width(width)
                            .unwrap()
                            .with_legend_style(style)
                            .unwrap();
                        let mut f = Figure::new(20.0, 12.0, theme.clone()).unwrap();
                        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
                            .unwrap()
                            .box_plot(
                                vec!["A".into()],
                                vec![vec![1.0, 2.0]],
                                BoxOptions {
                                    label: "Samples".into(),
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                        let layout =
                            crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).1;
                        let entry = &layout.legend.entries()[0];
                        let b = entry.swatch;
                        let Item::Group { items, .. } = swatch(&theme, entry) else {
                            panic!("small box")
                        };
                        assert_eq!(items.len(), 3);
                        for item in &items {
                            let (x0, y0, x1, y1) = bounds(item);
                            let (dx, dy) = match item {
                                Item::Stroke { style, .. } if y0 == y1 => {
                                    (0.0, f64::from(style.width) / 2.0)
                                }
                                Item::Stroke { style, .. } => {
                                    (f64::from(style.width) / 2.0, f64::from(style.width) / 2.0)
                                }
                                _ => (0.0, 0.0),
                            };
                            assert!(f64::from(x0) - dx >= b.x - 1e-3);
                            assert!(f64::from(x1) + dx <= b.x + b.width + 1e-3);
                            assert!(f64::from(y0) - dy >= b.center_y - b.height / 2.0 - 1e-3);
                            assert!(f64::from(y1) + dy <= b.center_y + b.height / 2.0 + 1e-3);
                            assert!(entry.label.x - (f64::from(x1) + dx) >= 4.0 - 1e-3);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn violin_swatch_strokes_fit_the_measured_bounds() {
        use crate::LegendStyle;
        use crate::model::ViolinOptions;
        for base in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for scale in [0.5, 1.0, 4.0, 10.0] {
                for width in [0.5, 8.0, 72.0, f64::from(f32::MAX)] {
                    for style in [LegendStyle::Header, LegendStyle::Direct] {
                        let theme = base
                            .with_font_scale(scale)
                            .unwrap()
                            .with_line_width(width)
                            .unwrap()
                            .with_legend_style(style)
                            .unwrap();
                        let mut f = Figure::new(20.0, 12.0, theme.clone()).unwrap();
                        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
                            .unwrap()
                            .violin(
                                vec!["A".into()],
                                vec![vec![1.0, 2.0]],
                                ViolinOptions {
                                    label: "Samples".into(),
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                        let layout =
                            crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).1;
                        let entry = &layout.legend.entries()[0];
                        let b = entry.swatch;
                        let Item::Group { items, .. } = swatch(&theme, entry) else {
                            panic!("filled lens")
                        };
                        assert_eq!(items.len(), 2);
                        for item in &items {
                            let (x0, y0, x1, y1) = bounds(item);
                            let (dx, dy) = match item {
                                Item::Stroke { style, .. } if y0 == y1 => {
                                    (0.0, f64::from(style.width) / 2.0)
                                }
                                Item::Stroke { style, .. } => {
                                    (f64::from(style.width) / 2.0, f64::from(style.width) / 2.0)
                                }
                                _ => (0.0, 0.0),
                            };
                            assert!(f64::from(x0) - dx >= b.x - 1e-3);
                            assert!(f64::from(x1) + dx <= b.x + b.width + 1e-3);
                            assert!(f64::from(y0) - dy >= b.center_y - b.height / 2.0 - 1e-3);
                            assert!(f64::from(y1) + dy <= b.center_y + b.height / 2.0 + 1e-3);
                            assert!(entry.label.x - (f64::from(x1) + dx) >= 4.0 - 1e-3);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn chip_tint_is_14_percent() {
        let color = Rgba {
            r: 120,
            g: 170,
            b: 250,
            a: 255,
        };
        // 0.14 × 255 = 35.7.
        assert_eq!(chip_tint(color), Rgba { a: 36, ..color });
        // The tint scales the color's own alpha: 0.14 × 128 = 17.92.
        assert_eq!(chip_tint(Rgba { a: 128, ..color }).a, 18);
        assert_eq!(chip_tint(Rgba { a: 0, ..color }).a, 0);

        // Every chip of a Midnight legend is filled with the tint of its entry's color.
        let (theme, layout) = layout(Theme::midnight());
        let Some(Item::Group { items, .. }) = legend(&theme, &layout) else {
            panic!("a legend group");
        };
        let entries = layout.legend.entries();
        assert_eq!(items.len(), 3 * entries.len());
        for (triple, entry) in items.chunks(3).zip(entries) {
            let Item::Fill {
                paint: Paint::Solid(fill),
                ..
            } = &triple[0]
            else {
                panic!("a chip is a solid fill");
            };
            assert_eq!(*fill, chip_tint(entry.swatch.color));
        }
    }

    #[test]
    fn each_chip_draws_its_pill_then_its_dot_then_its_label() {
        let (theme, layout) = layout(Theme::midnight());
        let Some(Item::Group { clip: None, items }) = legend(&theme, &layout) else {
            panic!("an unclipped legend group");
        };
        for (triple, entry) in items.chunks(3).zip(layout.legend.entries()) {
            let pill = entry.chip.expect("a chips row gives every entry a chip");
            // The pill fills its box; its ends are half circles.
            let (x0, y0, x1, y1) = bounds(&triple[0]);
            assert!((f64::from(x0) - pill.x).abs() < 1e-3);
            assert!((f64::from(x1) - pill.right()).abs() < 1e-3);
            assert!((f64::from(y0) - pill.y).abs() < 1e-3);
            assert!((f64::from(y1) - pill.bottom()).abs() < 1e-3);
            let Item::Fill { path, .. } = &triple[0] else {
                panic!("a chip is a fill");
            };
            let corners = path
                .verbs()
                .iter()
                .filter(|v| matches!(v, crate::geometry::PathVerb::CubicTo(..)))
                .count();
            assert_eq!(corners, 4);
            // Then the dot in the series color, inside the pill, and the label.
            let dot = entry.swatch;
            assert!(matches!(dot.kind, Swatch::Dot { .. }));
            assert!(
                matches!(&triple[1], Item::Fill { paint: Paint::Solid(c), .. } if *c == dot.color)
            );
            let (dx0, dy0, dx1, dy1) = bounds(&triple[1]);
            assert!(dx0 > x0 && dx1 < x1 && dy0 > y0 && dy1 < y1);
            let Item::Text(label) = &triple[2] else {
                panic!("a label after each dot");
            };
            assert_eq!(label.text, entry.label.text);
            assert_eq!(label.color, theme.text_color());
        }
    }

    #[test]
    fn a_short_chip_is_drawn_with_half_circle_ends() {
        use crate::geometry::PathVerb;

        // No title, a one-character label and a large font scale: the chip's dot and label
        // are narrower than the chip is tall. At this font scale the figure needs to be about
        // 4.5 in tall to leave room for the plot.
        let theme = Theme::pigment()
            .with_legend_style(crate::LegendStyle::Chips)
            .unwrap()
            .with_font_scale(10.0)
            .unwrap();
        let mut f = Figure::new(6.0, 6.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: ".".to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let [_, _, Item::Group { items, .. }, _] = scene.items.as_slice() else {
            panic!("chrome, marks, legend and baseline groups");
        };
        let Item::Fill { path, .. } = &items[0] else {
            panic!("the chip comes first");
        };
        let (x0, y0, x1, y1) = bounds(&items[0]);
        let (width, height) = (f64::from(x1 - x0), f64::from(y1 - y0));
        assert!(width >= height - 1e-3, "{width} x {height}");
        // Every corner is a quarter circle of half the height, horizontally and vertically.
        let verbs = path.verbs();
        let mut corners = 0;
        for (i, verb) in verbs.iter().enumerate() {
            let PathVerb::CubicTo(_, _, end) = verb else {
                continue;
            };
            let start = match verbs.get(i.wrapping_sub(1)) {
                Some(PathVerb::MoveTo(p) | PathVerb::LineTo(p) | PathVerb::CubicTo(_, _, p)) => *p,
                other => panic!("a corner after {other:?}"),
            };
            let (rx, ry) = (
                f64::from((end.x - start.x).abs()),
                f64::from((end.y - start.y).abs()),
            );
            assert!((rx - height / 2.0).abs() < 1e-3, "{rx} vs {}", height / 2.0);
            assert!((ry - height / 2.0).abs() < 1e-3, "{ry} vs {}", height / 2.0);
            corners += 1;
        }
        assert_eq!(corners, 4);
    }

    #[test]
    fn no_entries_no_group() {
        let theme = Theme::pigment()
            .with_legend_style(crate::LegendStyle::None)
            .unwrap();
        let (theme, layout) = layout(theme);
        assert!(legend(&theme, &layout).is_none());
    }

    #[test]
    fn each_direct_label_draws_its_dot_then_its_label_in_the_line_color() {
        // Graphite labels lines directly; the scatter falls back to the row.
        let (theme, layout) = layout(Theme::graphite());
        let Some(Item::Group { clip: None, items }) = legend(&theme, &layout) else {
            panic!("an unclipped legend group");
        };
        let row = layout.legend.entries();
        assert_eq!(row.len(), 1);
        assert_eq!(
            row[0].swatch.kind,
            Swatch::Dot {
                diameter: theme.marker_size()
            }
        );
        let direct = &layout.direct;
        assert_eq!(direct.len(), 2);
        // The row comes first, as in a header row, then a dot and a label per line.
        assert_eq!(items.len(), 2 + 2 * direct.len());
        for (pair, label) in items[2..].chunks(2).zip(direct) {
            let end = label.dot.expect("the line ends inside the plot");
            assert!(
                matches!(&pair[0], Item::Fill { paint: Paint::Solid(c), .. } if *c == label.color)
            );
            let (x0, y0, x1, y1) = bounds(&pair[0]);
            assert!((f64::from(x1 - x0) - end.diameter).abs() < 1e-3);
            assert!((f64::from(x0 + x1) / 2.0 - end.x).abs() < 1e-3);
            assert!((f64::from(y0 + y1) / 2.0 - end.y).abs() < 1e-3);
            let Item::Text(text) = &pair[1] else {
                panic!("a label after each dot");
            };
            assert_eq!(Some(&text.text), label.label.as_ref().map(|l| &l.text));
            assert_eq!(text.color, label.color);
            assert_ne!(text.color, theme.text_color());
        }
    }

    #[test]
    fn a_direct_label_without_a_dot_draws_its_label_only() {
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.5, 50.0],
                LineOptions {
                    label: "High".to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        let layout = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).1;
        let theme = Theme::graphite();
        let Some(Item::Group { items, .. }) = legend(&theme, &layout) else {
            panic!("a legend group");
        };
        assert!(matches!(items.as_slice(), [Item::Text(text)] if text.text == "High"));
    }
}
