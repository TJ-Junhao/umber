//! Legend drawing: swatches, chips, labels and the end dots of direct labels.
//!
//! Everything the legend draws is one unclipped group: the legend sits outside the plot area,
//! in the header.

use super::bar_shape::rounded_rect;
use super::chrome::{segment, text};
use super::line::line_style;
use super::mapping::point;
use super::marker::{marker_shape, polygon};
use crate::layout::{Area, Layout, RowEntry};
use crate::legend::Swatch;
use crate::scene::{FillRule, Item, LineCap, Paint};
use crate::{Marker, Rgba, Theme};

/// The opacity of a chip's tint, as a share of its series color's own.
const CHIP_TINT: f32 = 0.14;

/// The legend of `layout` as one unclipped group: for each row entry in order, its chip when it
/// has one, its swatch, and then its label in the theme's text color. `None` when the legend has
/// no entries, so a figure without a legend gets no group.
pub(crate) fn legend(theme: &Theme, layout: &Layout) -> Option<Item> {
    let entries = layout.legend.entries();
    if entries.is_empty() {
        return None;
    }
    let mut items = Vec::with_capacity(3 * entries.len());
    for entry in entries {
        if let Some(pill) = entry.chip {
            items.push(chip(pill, entry.swatch.color));
        }
        items.push(swatch(theme, entry));
        items.push(text(&entry.label, theme.text_color()));
    }
    Some(Item::Group { clip: None, items })
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
/// and the theme's line width, a filled dot, or a filled square.
fn swatch(theme: &Theme, entry: &RowEntry) -> Item {
    let b = entry.swatch;
    match b.kind {
        Swatch::Line { dash } => {
            let style = line_style(theme, b.color, dash);
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
        Swatch::Dot => {
            let center = point(b.x + b.width / 2.0, b.center_y);
            Item::Fill {
                path: marker_shape(Marker::Circle, b.width as f32).translated(center.x, center.y),
                paint: Paint::Solid(b.color),
                rule: FillRule::NonZero,
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
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
            crate::pipeline::plan_figure(&f, FontLibrary::global()).1,
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
            assert_eq!(dot.kind, Swatch::Dot);
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
        // are narrower than the chip is tall.
        let theme = Theme::pigment()
            .with_legend_style(crate::LegendStyle::Chips)
            .unwrap()
            .with_font_scale(10.0)
            .unwrap();
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
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
        let scene = f.to_scene(FontLibrary::global());
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
}
