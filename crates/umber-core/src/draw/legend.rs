//! Legend drawing: swatches, chips, labels and the end dots of direct labels.
//!
//! Everything the legend draws is one unclipped group: the legend sits outside the plot area,
//! in the header.

use super::chrome::{segment, text};
use super::line::line_style;
use super::mapping::point;
use super::marker::{marker_shape, polygon};
use crate::layout::{Layout, RowEntry};
use crate::legend::Swatch;
use crate::scene::{FillRule, Item, LineCap, Paint};
use crate::{Marker, Theme};

/// The legend of `layout` as one unclipped group: for each row entry in order, its swatch and
/// then its label in the theme's text color. `None` when the legend has no entries, so a figure
/// without a legend gets no group.
pub(crate) fn legend(theme: &Theme, layout: &Layout) -> Option<Item> {
    let entries = layout.legend.entries();
    if entries.is_empty() {
        return None;
    }
    let mut items = Vec::with_capacity(2 * entries.len());
    for entry in entries {
        items.push(swatch(theme, entry));
        items.push(text(&entry.label, theme.text_color()));
    }
    Some(Item::Group { clip: None, items })
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
    fn no_entries_no_group() {
        let theme = Theme::pigment()
            .with_legend_style(crate::LegendStyle::None)
            .unwrap();
        let (theme, layout) = layout(theme);
        assert!(legend(&theme, &layout).is_none());
    }
}
