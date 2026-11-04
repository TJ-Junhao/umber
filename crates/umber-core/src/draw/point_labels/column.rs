//! The values that scatters with [`Place::Column`](crate::Place::Column) write in the value
//! column of their axes, outside the plot, at the far end of the value axis
//! ([`crate::layout::ColumnInput`] describes the room the layout reserves for them).
//!
//! **Which values.** The same as next to the points: one per drawn point, measured and written
//! as the module of the parent describes, in the color of its mark at full opacity. A point
//! whose centre lies outside the plot gets none.
//!
//! **Sub-columns.** Each annotated mark that writes its values in the column and has a value
//! to write has a sub-column of its own, in mark order, so that two series on one category
//! axis, whose points share their rows, never collide; each group of a grouped scatter is a
//! mark of its own. Right of the plot each value sits level with its point, centred on it
//! vertically; its values are numbers, since a scatter on a category y axis takes numbers on x.
//! When every value of a sub-column is written in one notation, all in plain decimals or all in
//! scientific form, they are aligned on the decimal point, or where it would be (the end of the
//! digits before the exponent or the percent sign); otherwise they are right-aligned. Above
//! the plot each value, a number or a time, is centred on its point's x.
//!
//! **Overlap.** Two values of one sub-column whose boxes overlap along the category axis fail
//! the render, as does a value of a row above the plot that reaches into the row's title, and
//! a value or title that leaves the part of the canvas that belongs to its axes (the whole
//! canvas for a figure of one axes, its cells on a grid) or comes closer than
//! [`X_LABEL_EDGE_GAP`] to the left or right edge of the canvas, as x tick labels may not, as a
//! value centred on a point at the edge of the plot can. Between the cells of a grid the cell
//! is the bound. The layout reserves no room for that overhang, and the limits never change
//! to avoid it.

use super::{MarkLabels, PointLabel};
use crate::draw::chrome::text;
use crate::draw::mapping::{AxisPoint, axis_values};
use crate::draw::mark_context;
use crate::error::quoted;
use crate::layout::{
    CONTAINMENT_TOLERANCE, ColumnInput, ColumnSide, Frame, Label, Layout, X_LABEL_EDGE_GAP,
    value_text,
};
use crate::model::Axes;
use crate::resolve::{Resolved, ResolvedMark};
use crate::scene::Item;
use crate::text::SceneFonts;
use crate::{Error, Result, Rgba, SizeSpec, Theme, TickFormat};

/// The hint of two values of one sub-column right of the plot that overlap.
const COLUMN_HINT: &str =
    "make the figure taller, pass a smaller label_size=, or use another place.";
/// The hint of two values of one sub-row above the plot that overlap, or of a value that reaches
/// into the row's title.
const ROW_HINT: &str = "make the figure wider, pass a smaller label_size=, or use another place.";

/// The values of one mark in the value column, measured: one sub-column.
#[derive(Debug, Clone)]
pub(super) struct ColumnValues {
    /// The mark's index among the axes' marks, how messages name it, the color of its values
    /// and the values of its drawn points, in data order, as in the plot.
    labels: MarkLabels,
    /// For each value, the width of its text before its anchor: the decimal point, or where it
    /// would be, for values aligned on it; the whole width for right-aligned values. Unused
    /// above the plot, where each value is centred on its point.
    leads: Vec<f64>,
    /// Where the anchor lies from the sub-column's left edge right of the plot: the widest
    /// lead.
    anchor: f64,
    /// The sub-column's extent across the category axis: its width right of the plot, the
    /// anchor and the widest part of a value after it; its height above it, the tallest box.
    extent: f64,
}

impl ColumnValues {
    /// Measures the values `points` of the mark at `mark` among the axes' marks, named `name`
    /// and drawn in `color`, written in `format` at `size` as values on the x axis (`on_x`, a
    /// column right of the plot) or on the y axis (a row above it), with `lib` in `theme`.
    ///
    /// # Errors
    ///
    /// As [`value_text`], for the part of a value before its decimal point.
    pub(super) fn measure(
        (lib, theme): (&SceneFonts<'_>, &Theme),
        (mark, name, color): (usize, String, Rgba),
        points: Vec<PointLabel>,
        (on_x, format, size): (bool, &TickFormat, SizeSpec),
    ) -> Result<ColumnValues> {
        // Values above the plot are centred on their points and need no anchor.
        let leads = if on_x {
            leads((lib, theme), &points, format, size)?
        } else {
            points.iter().map(|point| point.size().0).collect()
        };
        let anchor = leads.iter().copied().fold(0.0, f64::max);
        let extent = if on_x {
            points
                .iter()
                .zip(&leads)
                .map(|(point, lead)| anchor + point.size().0 - lead)
                .fold(0.0, f64::max)
        } else {
            points
                .iter()
                .map(|point| point.size().1)
                .fold(0.0, f64::max)
        };
        Ok(ColumnValues {
            labels: MarkLabels {
                mark,
                name,
                side: None,
                color,
                reach: (0.0, 0.0),
                points,
            },
            leads,
            anchor,
            extent,
        })
    }
}

/// The value column of `axes`, whose values lie on x when `on_x` and whose sub-columns are
/// `columns`, as the layout reserves it; `None` when it has no sub-column and the axes set no
/// title.
pub(super) fn input(axes: &Axes, on_x: bool, columns: &[ColumnValues]) -> Option<ColumnInput> {
    let options = axes.value_column_options();
    if columns.is_empty() && options.is_none_or(|options| options.title().is_empty()) {
        return None;
    }
    Some(ColumnInput {
        side: if on_x {
            ColumnSide::Right
        } else {
            ColumnSide::Above
        },
        extents: columns.iter().map(|column| column.extent).collect(),
        title: options.map_or("", |options| options.title()).to_owned(),
        size: options.map_or(SizeSpec::Auto, crate::ValueColumn::size),
    })
}

/// How a number is written, for decimal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notation {
    /// Plain decimals: `42`, `−28.4`, `12.5%`.
    Plain,
    /// Scientific form: `1.5e6`, `1e−5`.
    Scientific,
}

/// The notation of the number `number` and the byte index of its anchor in it: its decimal
/// point, or, without one, the end of its digits before the exponent or the percent sign.
fn anchor_of(number: &str) -> (Notation, usize) {
    let exponent = number.find('e');
    let notation = if exponent.is_some() {
        Notation::Scientific
    } else {
        Notation::Plain
    };
    let end = exponent.unwrap_or_else(|| number.trim_end_matches('%').len());
    (notation, number.find('.').unwrap_or(end))
}

/// For each value of `points`, numbers written in `format` at `size`, the width of its text
/// before its anchor when they are all written in one notation ([`anchor_of`]), so that they
/// align on their decimal points; otherwise, and for texts cut to fit, the whole width, so
/// that they align on their right ends.
///
/// # Errors
///
/// As [`value_text`], for the part of a value before its anchor.
fn leads(
    (lib, theme): (&SceneFonts<'_>, &Theme),
    points: &[PointLabel],
    format: &TickFormat,
    size: SizeSpec,
) -> Result<Vec<f64>> {
    let right = || points.iter().map(|point| point.size().0).collect();
    let (prefix, suffix) = (format.prefix(), format.suffix());
    let anchors: Option<Vec<(Notation, usize)>> = points
        .iter()
        .map(|point| {
            let text = point.label.text.as_str();
            let number = text.strip_prefix(prefix)?.strip_suffix(suffix)?;
            let (notation, at) = anchor_of(number);
            Some((notation, prefix.len() + at))
        })
        .collect();
    let Some(anchors) = anchors else {
        return Ok(right());
    };
    let Some(&(first, _)) = anchors.first() else {
        return Ok(Vec::new());
    };
    if anchors.iter().any(|&(notation, _)| notation != first) {
        return Ok(right());
    }
    points
        .iter()
        .zip(anchors)
        .map(|(point, (_, at))| {
            let text = point.label.text.as_str();
            if at >= text.len() {
                Ok(point.size().0)
            } else {
                Ok(value_text(lib, theme, &text[..at], size)?.width())
            }
        })
        .collect()
}

/// One value placed in a sub-column: its box along the category axis, from its low end to its
/// high end, and the index of its point among the mark's labelled points.
#[derive(Debug, Clone, Copy)]
struct Span {
    /// The low end.
    low: f64,
    /// The high end.
    high: f64,
    /// The point.
    point: usize,
}

/// The values that the marks `marks`, resolved as `resolved` and laid out as `layout`, write
/// in their value column, measured in `labels`, and the column's title, as one unclipped
/// group; `None` without a value column. Messages name the frame of the axes: the figure for
/// a figure of one axes, the panel on a grid (`on_grid`). The canvas is `canvas` pt wide.
///
/// # Errors
///
/// `Error::Style` under `context` for a title that leaves the frame or comes closer than
/// [`X_LABEL_EDGE_GAP`] to the left or right canvas edge; then, for the first sub-column, in
/// mark order: a value that does so or, above the plot, reaches into
/// the row's title, naming the mark and the value; and two values that overlap along the
/// category axis, naming the mark and both values.
pub(crate) fn column_items(
    theme: &Theme,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
    labels: &super::ScatterLabels,
    (context, on_grid, canvas): (&'static str, bool, f64),
) -> Result<Option<Item>> {
    let Some(placed) = &layout.value_column else {
        return Ok(None);
    };
    let plot = layout.plot;
    let maps = mark_context(theme, resolved, layout, marks).maps;
    let hint = match placed.side {
        ColumnSide::Right => COLUMN_HINT,
        ColumnSide::Above => ROW_HINT,
    };
    let bounds = text_bounds(layout.frame, canvas);
    let (container, edge) = if on_grid {
        ("panel", "the figure's")
    } else {
        ("figure", "its")
    };
    let leaves = |what: String| {
        Error::style(
            context,
            format!(
                "{what} reaches outside the {container}, or less than {X_LABEL_EDGE_GAP} pt \
                 from {edge} left or right edge."
            ),
        )
        .with_hint(hint)
    };
    let mut items = Vec::new();
    // The title's box, which the values of a row above the plot keep clear of.
    let title = match &placed.title {
        Some(title) => {
            let title_box = text_box(title, title.x, title.y);
            if !inside(bounds, title_box) {
                return Err(leaves(format!(
                    "the value column title {}",
                    quoted(&title.text)
                )));
            }
            items.push(text(title, theme.text_color()));
            Some(title_box)
        }
        None => None,
    };
    for (values, &(start, _)) in labels.columns.iter().zip(&placed.cells) {
        let mark_labels = &values.labels;
        let Some(ResolvedMark::Scatter { mark, .. }) = marks.get(mark_labels.mark) else {
            continue;
        };
        let xs = axis_values(&resolved.x, mark.x());
        let ys = axis_values(&resolved.y, mark.y());
        let mut spans = Vec::with_capacity(mark_labels.points.len());
        for (p, point) in mark_labels.points.iter().enumerate() {
            let (Some(&x), Some(&y)) = (xs.get(point.index), ys.get(point.index)) else {
                continue;
            };
            let centre = maps.to_plot(AxisPoint::new(x, y));
            if !(x.is_finite() && y.is_finite())
                || !super::contains(plot, [centre.x, centre.y, centre.x, centre.y])
            {
                continue;
            }
            let (width, height) = point.size();
            let mut label = point.label.clone();
            let ascent = f64::from(label.layout.ascent());
            let span = match placed.side {
                ColumnSide::Right => {
                    let top = centre.y - height / 2.0;
                    label.x = start + values.anchor - values.leads[p];
                    label.y = top + ascent;
                    (top, top + height)
                }
                ColumnSide::Above => {
                    label.x = centre.x - width / 2.0;
                    label.y = start + ascent;
                    if let Some([left, top, right, bottom]) = title
                        && label.x < right
                        && left < label.x + width
                        && top < start + height
                        && start < bottom
                    {
                        return Err(Error::style(
                            context,
                            format!(
                                "the value {} of {} reaches into the title of the value row.",
                                quoted(&point.label.text),
                                mark_labels.name,
                            ),
                        )
                        .with_hint(ROW_HINT));
                    }
                    (label.x, label.x + width)
                }
            };
            if !inside(bounds, text_box(&label, label.x, label.y)) {
                return Err(leaves(format!(
                    "the value {} of {}",
                    quoted(&point.label.text),
                    mark_labels.name
                )));
            }
            spans.push(Span {
                low: span.0,
                high: span.1,
                point: p,
            });
            items.push(text(&label, mark_labels.color));
        }
        if let Some((a, b)) = first_overlap(&mut spans) {
            let (first, second) = (a.min(b), a.max(b));
            let (place, axis, hint) = match placed.side {
                ColumnSide::Right => ("column", "y", COLUMN_HINT),
                ColumnSide::Above => ("row", "x", ROW_HINT),
            };
            return Err(Error::style(
                context,
                format!(
                    "the values {} and {} of {} overlap in the value {place}: their points lie \
                     too close along the {axis} axis.",
                    quoted(&mark_labels.points[first].label.text),
                    quoted(&mark_labels.points[second].label.text),
                    mark_labels.name,
                ),
            )
            .with_hint(hint));
        }
    }
    Ok(Some(Item::Group { clip: None, items }))
}

/// The box (left, top, right, bottom) of `label` with its baseline starting at `x`, `y`: its
/// advance width by its ascent and descent.
fn text_box(label: &Label, x: f64, y: f64) -> [f64; 4] {
    let layout = &label.layout;
    [
        x,
        y - f64::from(layout.ascent()),
        x + label.width(),
        y + f64::from(layout.descent()),
    ]
}

/// Where the values and the title of the value column of axes whose frame is `frame`, on a
/// canvas `canvas` pt wide, may go (left, top, right, bottom): the frame, less
/// [`X_LABEL_EDGE_GAP`] on a side that is the left or right edge of the canvas, which x tick
/// labels keep clear of too.
fn text_bounds(frame: Frame, canvas: f64) -> [f64; 4] {
    let tolerance = CONTAINMENT_TOLERANCE;
    let left = if frame.x <= tolerance {
        frame.x + X_LABEL_EDGE_GAP
    } else {
        frame.x
    };
    let right = if frame.right() >= canvas - tolerance {
        frame.right() - X_LABEL_EDGE_GAP
    } else {
        frame.right()
    };
    [left, frame.y, right, frame.bottom()]
}

/// Whether the box `inner` lies inside the box `outer`, to the containment tolerance of the
/// layout.
fn inside(outer: [f64; 4], inner: [f64; 4]) -> bool {
    inner[0] >= outer[0] - CONTAINMENT_TOLERANCE
        && inner[1] >= outer[1] - CONTAINMENT_TOLERANCE
        && inner[2] <= outer[2] + CONTAINMENT_TOLERANCE
        && inner[3] <= outer[3] + CONTAINMENT_TOLERANCE
}

/// The first two of `spans` that overlap, as the indices of their points, the spans sorted
/// from the low end up: each is compared with the one, among those before it, that reaches
/// highest. Spans that only touch do not overlap.
fn first_overlap(spans: &mut [Span]) -> Option<(usize, usize)> {
    spans.sort_by(|a, b| a.low.total_cmp(&b.low));
    let mut highest: Option<Span> = None;
    for &span in spans.iter() {
        if let Some(reach) = highest {
            if span.low < reach.high {
                return Some((reach.point, span.point));
            }
            if span.high <= reach.high {
                continue;
            }
        }
        highest = Some(span);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_sit_at_the_decimal_point_or_where_it_would_be() {
        assert_eq!(anchor_of("42"), (Notation::Plain, 2));
        assert_eq!(anchor_of("28.4"), (Notation::Plain, 2));
        assert_eq!(anchor_of("\u{2212}0.5"), (Notation::Plain, 4));
        assert_eq!(anchor_of("12%"), (Notation::Plain, 2));
        assert_eq!(anchor_of("12.5%"), (Notation::Plain, 2));
        assert_eq!(anchor_of("1.5e6"), (Notation::Scientific, 1));
        assert_eq!(anchor_of("1e\u{2212}5"), (Notation::Scientific, 1));
    }

    fn span(low: f64, high: f64, point: usize) -> Span {
        Span { low, high, point }
    }

    #[test]
    fn overlaps_are_found_among_sorted_spans() {
        // Touching spans do not overlap.
        let mut spans = [span(10.0, 20.0, 0), span(0.0, 10.0, 1), span(20.0, 30.0, 2)];
        assert_eq!(first_overlap(&mut spans), None);
        // A short span inside a long one, and the long one reaching past it.
        let mut spans = [span(0.0, 100.0, 0), span(10.0, 20.0, 1)];
        assert_eq!(first_overlap(&mut spans), Some((0, 1)));
        let mut spans = [span(0.0, 50.0, 0), span(10.0, 20.0, 1), span(40.0, 60.0, 2)];
        assert_eq!(first_overlap(&mut spans), Some((0, 1)));
        let mut spans = [span(30.0, 40.0, 2), span(0.0, 10.0, 0), span(35.0, 45.0, 1)];
        assert_eq!(first_overlap(&mut spans), Some((2, 1)));
        assert_eq!(first_overlap(&mut []), None);
    }
}
