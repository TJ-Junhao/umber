//! The values written on bars ([`ValueLabels`]): measured once per render, they widen automatic
//! value limits so that the labels at bar ends fit (see [`crate::bubble_fit`]), and are drawn
//! over the marks.
//!
//! A plain or grouped bar's label sits [`END_GAP`] beyond the end of the bar, away from its
//! base, centred across the bar, in the theme's text color. A stacked segment's label is centred
//! in the drawn segment, in black or white, whichever contrasts more with the segment, and must
//! fit inside it with [`INSIDE_MARGIN`] to spare on every side. Every label is set in the tick
//! label style at the size of its mark's labels.

use super::bar::{GroupSlots, bar_span, frame, in_axis_order, segment_ends, stack_half_gap};
use super::chrome::text;
use super::heatmap::text_color_on;
use super::mapping::axis_value;
use super::{MarkContext, mark_context};
use crate::bubble_fit::EndReach;
use crate::error::{number, quoted};
use crate::layout::{Area, Label, Layout, value_text};
use crate::model::{Axes, ValueLabels};
use crate::resolve::{AxisScale, Orientation, Resolved, ResolvedMark};
use crate::scene::Item;
use crate::text::SceneFonts;
use crate::ticks::{self, TickValues};
use crate::{Error, Result, Rgba, ScaleKind, Theme, Tick, TickFormat};

/// The significant digits of a value written in the automatic format.
const SIGNIFICANT_DIGITS: usize = 6;
/// The gap between the end of a bar and its label, in points.
pub(crate) const END_GAP: f64 = 3.0;
/// The room a label inside a stacked segment keeps from each side of the segment, in points.
const INSIDE_MARGIN: f64 = 2.0;
/// How far, in points, a label may exceed the room it needs and still count as fitting: plot
/// edges come from rounded arithmetic.
const FIT_TOLERANCE: f64 = 1e-6;

/// The hint of a label that does not fit, when the figure, the format or `annotate` can help.
const ROOM_HINT: &str =
    "enlarge the figure, write fewer decimals with format, or pass annotate=False.";
/// The hint of an end label beyond explicit value limits.
const LIMITS_HINT: &str = "widen ylim (or xlim), or let the limits be automatic.";

/// How the bars of an annotated mark stand, which decides where their labels go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrangement {
    /// One bar per category, labelled at its end.
    Plain,
    /// Bars side by side in each category, each labelled at its end.
    Grouped,
    /// Segments stacked in each category, each labelled inside.
    Stacked,
}

/// One value and its label, measured.
#[derive(Debug, Clone)]
struct ValueLabel {
    /// The value.
    value: f64,
    /// Its label, unplaced.
    label: Label,
}

impl ValueLabel {
    /// The width and height of the label's box: its advance width by its ascent and descent.
    fn size(&self) -> (f64, f64) {
        let layout = &self.label.layout;
        (
            self.label.width(),
            f64::from(layout.ascent() + layout.descent()),
        )
    }
}

/// The measured labels of one annotated bar mark.
#[derive(Debug, Clone)]
struct MarkLabels {
    /// The mark's index among the axes' marks.
    mark: usize,
    /// How its bars stand.
    arrangement: Arrangement,
    /// Which way its bars run.
    orientation: Orientation,
    /// Whether the value axis is a log axis, whose bars always grow towards its high end.
    log: bool,
    /// For each series (one for a plain bar mark), for each category of the mark in the mark's
    /// order, the label of its value; `None` for NaN, which draws no bar.
    series: Vec<Vec<Option<ValueLabel>>>,
}

/// The measured value labels of the annotated bar marks of one axes.
#[derive(Debug, Clone, Default)]
pub(crate) struct BarLabels {
    /// One record per annotated mark, in mark order.
    marks: Vec<MarkLabels>,
}

impl BarLabels {
    /// Measures the labels of every annotated bar mark among `marks`, the resolved marks of
    /// `axes`, in `theme`. Each mark writes its finite values in its format, or in the tick format
    /// of its value axis when it has none (see [`value_texts`]).
    ///
    /// # Errors
    ///
    /// As [`value_text`], for the first label that cannot be laid out.
    pub(crate) fn measure(
        theme: &Theme,
        lib: &SceneFonts<'_>,
        axes: &Axes,
        marks: &[ResolvedMark<'_>],
    ) -> Result<BarLabels> {
        let mut measured = Vec::new();
        for (index, mark) in marks.iter().enumerate() {
            let Some(annotated) = Annotated::of(mark) else {
                continue;
            };
            let (kind, axis_format) = match annotated.orientation {
                Orientation::Vertical => (axes.y_scale(), axes.y_tick_format()),
                Orientation::Horizontal => (axes.x_scale(), axes.x_tick_format()),
            };
            let format = annotated.labels.format.as_ref().unwrap_or(axis_format);
            let finite: Vec<f64> = annotated
                .values
                .iter()
                .flat_map(|values| values.iter().copied())
                .filter(|v| v.is_finite())
                .collect();
            let mut texts = value_texts(&finite, format).into_iter();
            let mut series = Vec::with_capacity(annotated.values.len());
            for values in &annotated.values {
                let mut row = Vec::with_capacity(values.len());
                for &value in *values {
                    // One text per finite value, in the same order.
                    let content = if value.is_finite() {
                        texts.next()
                    } else {
                        None
                    };
                    let label = content
                        .map(|content| {
                            value_text(lib, theme, &content, annotated.labels.size)
                                .map(|label| ValueLabel { value, label })
                        })
                        .transpose()?;
                    row.push(label);
                }
                series.push(row);
            }
            measured.push(MarkLabels {
                mark: index,
                arrangement: annotated.arrangement,
                orientation: annotated.orientation,
                log: matches!(kind, ScaleKind::Log { .. }),
                series,
            });
        }
        Ok(BarLabels { marks: measured })
    }

    /// How far the label at the end of each plain or grouped bar reaches past the bar's value on
    /// its value axis, for the fits of automatic limits: [`END_GAP`] and the label's extent
    /// along that axis, towards the high end for a bar that grows towards it (a non-negative
    /// value, or any value on a log axis) and towards the low end otherwise.
    pub(crate) fn end_reaches(&self) -> Vec<EndReach> {
        let mut reaches = Vec::new();
        for mark in &self.marks {
            if mark.arrangement == Arrangement::Stacked {
                continue;
            }
            for label in mark.series.iter().flatten().flatten() {
                let (width, height) = label.size();
                let extent = match mark.orientation {
                    Orientation::Vertical => height,
                    Orientation::Horizontal => width,
                };
                let reach = END_GAP + extent;
                let forward = mark.log || label.value >= 0.0;
                reaches.push(EndReach {
                    axis: mark.orientation.value_axis(),
                    value: label.value,
                    low: if forward { 0.0 } else { reach },
                    high: if forward { reach } else { 0.0 },
                });
            }
        }
        reaches
    }
}

/// An annotated bar mark, as its labels are measured.
struct Annotated<'a> {
    /// How its bars stand.
    arrangement: Arrangement,
    /// Which way its bars run.
    orientation: Orientation,
    /// Its value labels.
    labels: &'a ValueLabels,
    /// The values of each series, one series for a plain bar mark.
    values: Vec<&'a [f64]>,
}

impl<'a> Annotated<'a> {
    /// `mark` when it is a bar mark that writes its values; `None` otherwise.
    fn of(mark: &'a ResolvedMark<'_>) -> Option<Annotated<'a>> {
        let annotated = match mark {
            ResolvedMark::Bar {
                mark, orientation, ..
            } => Annotated {
                arrangement: Arrangement::Plain,
                orientation: *orientation,
                labels: &mark.options().annotation,
                values: vec![mark.heights()],
            },
            ResolvedMark::Grouped {
                mark,
                series,
                orientation,
                ..
            } => Annotated {
                arrangement: Arrangement::Grouped,
                orientation: *orientation,
                labels: mark.annotation(),
                values: series.iter().map(|(view, _)| view.values()).collect(),
            },
            ResolvedMark::Stacked {
                mark,
                series,
                orientation,
                ..
            } => Annotated {
                arrangement: Arrangement::Stacked,
                orientation: *orientation,
                labels: mark.annotation(),
                values: series.iter().map(|(view, _)| view.values()).collect(),
            },
            _ => return None,
        };
        annotated.labels.annotate.then_some(annotated)
    }
}

/// The labels of finite `values` in `format`. The automatic format writes each value on its own
/// to [`SIGNIFICANT_DIGITS`] significant digits, as C's `%g` does (`42`, `28.4`, `0.12345`;
/// scientific outside `1e-4 <= |v| < 1e6`, see [`ticks::format_each`]); the other formats write
/// each value as they write ticks, and every format adds its prefix and suffix.
fn value_texts(values: &[f64], format: &TickFormat) -> Vec<String> {
    let ticks = values
        .iter()
        .zip(ticks::format_each(values, SIGNIFICANT_DIGITS))
        .map(|(&value, label)| Tick { value, label })
        .collect();
    format
        .relabel(ticks, TickValues::Exact)
        .into_iter()
        .map(|tick| tick.label)
        .collect()
}

/// The scene items of the value labels `labels` of the marks `marks` of `axes`, resolved as
/// `resolved` and laid out as `layout`, as one unclipped group; `None` without labels.
///
/// # Errors
///
/// `Error::Style` under `context` for the first label, in mark order and within a mark in the
/// order bars are drawn, that does not fit: an end label that reaches outside the plot, or a
/// stacked label that does not fit inside its segment with [`INSIDE_MARGIN`] to spare. The
/// message names the label, the bar's category and, for grouped and stacked bars, its series.
pub(crate) fn items(
    theme: &Theme,
    resolved: &Resolved,
    layout: &Layout,
    axes: &Axes,
    marks: &[ResolvedMark<'_>],
    labels: &BarLabels,
    context: &'static str,
) -> Result<Option<Item>> {
    if labels.marks.is_empty() {
        return Ok(None);
    }
    let base = mark_context(theme, resolved, layout, marks);
    let mut items = Vec::new();
    for mark_labels in &labels.marks {
        let Some(mark) = marks.get(mark_labels.mark) else {
            continue;
        };
        let explicit = match mark_labels.orientation {
            Orientation::Vertical => axes.y_limits().is_some(),
            Orientation::Horizontal => axes.x_limits().is_some(),
        };
        let drawing = Drawing {
            context: base.oriented(mark_labels.orientation),
            resolved,
            plot: layout.plot,
            explicit,
            error: context,
        };
        drawing.mark(mark, mark_labels, &mut items)?;
    }
    Ok(Some(Item::Group { clip: None, items }))
}

/// What placing the labels of one mark needs.
struct Drawing<'a> {
    /// The frame of the mark's orientation.
    context: MarkContext<'a>,
    /// The resolved axes.
    resolved: &'a Resolved,
    /// The plot area.
    plot: Area,
    /// Whether the value axis has explicit limits.
    explicit: bool,
    /// The context of errors.
    error: &'static str,
}

impl Drawing<'_> {
    /// Pushes the placed labels of `mark`, measured as `labels`, onto `items`.
    fn mark(
        &self,
        mark: &ResolvedMark<'_>,
        labels: &MarkLabels,
        items: &mut Vec<Item>,
    ) -> Result<()> {
        let (along, values) = self.resolved.along_and_values(labels.orientation);
        let AxisScale::Category(categories) = &along.scale else {
            return Ok(());
        };
        let (names, colors, stacks): (&[String], Vec<Rgba>, _) = match mark {
            ResolvedMark::Bar { mark, color, .. } => (mark.categories(), vec![*color], None),
            ResolvedMark::Grouped { mark, series, .. } => (
                mark.categories(),
                series.iter().map(|(_, color)| *color).collect(),
                None,
            ),
            ResolvedMark::Stacked {
                mark,
                series,
                stacks,
                ..
            } => (
                mark.categories(),
                series.iter().map(|(_, color)| *color).collect(),
                Some(stacks),
            ),
            _ => return Ok(()),
        };
        let series_names: Vec<&str> = match mark {
            ResolvedMark::Grouped { series, .. } | ResolvedMark::Stacked { series, .. } => {
                series.iter().map(|(view, _)| view.label()).collect()
            }
            _ => Vec::new(),
        };
        let slots = GroupSlots::new(&self.context, categories, labels.series.len());
        let width = match labels.arrangement {
            Arrangement::Grouped => slots.width,
            Arrangement::Plain | Arrangement::Stacked => {
                let (left, right) = bar_span(categories, 0);
                right - left
            }
        };
        let bars = frame(&self.context, values, width);
        let half_gap = stack_half_gap(&self.context);
        for (index, i) in in_axis_order(categories, names) {
            let Some(category) = names.get(i) else {
                continue;
            };
            for (j, row) in labels.series.iter().enumerate() {
                let Some(Some(value)) = row.get(i) else {
                    continue;
                };
                let bar = LabelledBar {
                    category,
                    series: series_names.get(j).map(|name| (j, *name)),
                };
                let span = match labels.arrangement {
                    Arrangement::Grouped => slots.span(categories, index, j),
                    Arrangement::Plain | Arrangement::Stacked => bar_span(categories, index),
                };
                match stacks {
                    Some(stacks) => {
                        // Only 0 has no segment and no label (NaN has no label at all).
                        let Some(&fill) = colors.get(j) else {
                            continue;
                        };
                        if value.value == 0.0 {
                            continue;
                        }
                        // A value too small to change its stack's total (`1e100 + 1.0`) has no
                        // segment, and one the stack gap leaves no height has no drawn height:
                        // either shows no room, so its label does not fit.
                        let ends = stacks
                            .segment(i, j)
                            .and_then(|segment| segment_ends(&bars, values, segment, half_gap))
                            .unwrap_or((bars.base(), bars.base()));
                        items.push(self.inside(value, &bar, span, ends, fill)?);
                    }
                    None => {
                        let end = axis_value(values, value.value);
                        let forward = end >= bars.base();
                        items.push(self.at_end(value, &bar, span, end, forward)?);
                    }
                }
            }
        }
        Ok(())
    }

    /// The label `value` of `bar` at the end of its bar, which spans `span` across and ends at
    /// `end` along the value axis (axis space), growing towards the high end of the value axis
    /// when `forward` holds.
    fn at_end(
        &self,
        value: &ValueLabel,
        bar: &LabelledBar<'_>,
        (left, right): (f64, f64),
        end: f64,
        forward: bool,
    ) -> Result<Item> {
        let maps = self.context.maps;
        let (width, height) = value.size();
        let (across, along) = maps.in_frame((width, height));
        let centre = (maps.x.plot_at(left) + maps.x.plot_at(right)) / 2.0;
        let at = maps.y.plot_at(end);
        let away = if forward { 1.0 } else { -1.0 } * maps.y.extent.signum();
        let (near, far) = (at + away * END_GAP, at + away * (END_GAP + along));
        let frame_box = (
            (centre - across / 2.0, centre + across / 2.0),
            (near.min(far), near.max(far)),
        );
        let bounds = in_scene(self.context.maps.frame, frame_box);
        if !contains(self.plot, bounds) {
            let hint = if self.explicit {
                LIMITS_HINT
            } else {
                ROOM_HINT
            };
            return Err(Error::style(
                self.error,
                format!(
                    "the value label {} of {} does not fit inside the plot.",
                    quoted(&value.label.text),
                    bar.describe()
                ),
            )
            .with_hint(hint));
        }
        Ok(placed(value, bounds, self.context.theme.text_color()))
    }

    /// The label `value` of `bar` centred in its stacked segment, which spans `span` across and
    /// runs between `ends` along the value axis (axis space), drawn in the color that contrasts
    /// more with `fill`.
    fn inside(
        &self,
        value: &ValueLabel,
        bar: &LabelledBar<'_>,
        (left, right): (f64, f64),
        (from, to): (f64, f64),
        fill: Rgba,
    ) -> Result<Item> {
        let maps = self.context.maps;
        let (x0, x1) = (maps.x.plot_at(left), maps.x.plot_at(right));
        let (y0, y1) = (maps.y.plot_at(from), maps.y.plot_at(to));
        let frame_box = ((x0.min(x1), x0.max(x1)), (y0.min(y1), y0.max(y1)));
        let [left, top, right, bottom] = in_scene(maps.frame, frame_box);
        // Only the part of the segment inside the plot is seen.
        let plot = self.plot;
        let room = [
            left.max(plot.x),
            top.max(plot.y),
            right.min(plot.right()),
            bottom.min(plot.bottom()),
        ];
        let (room_width, room_height) =
            ((room[2] - room[0]).max(0.0), (room[3] - room[1]).max(0.0));
        let (width, height) = value.size();
        let (needed_width, needed_height) =
            (width + 2.0 * INSIDE_MARGIN, height + 2.0 * INSIDE_MARGIN);
        if needed_width > room_width + FIT_TOLERANCE || needed_height > room_height + FIT_TOLERANCE
        {
            let shown = |v: f64| number((v * 10.0).round() / 10.0);
            return Err(Error::style(
                self.error,
                format!(
                    "the value label {} of {} needs {} by {} pt with its margins; the segment \
                     shows {} by {} pt.",
                    quoted(&value.label.text),
                    bar.describe(),
                    shown(needed_width),
                    shown(needed_height),
                    shown(room_width),
                    shown(room_height),
                ),
            )
            .with_hint(ROOM_HINT));
        }
        let centre = ((room[0] + room[2]) / 2.0, (room[1] + room[3]) / 2.0);
        let bounds = [
            centre.0 - width / 2.0,
            centre.1 - height / 2.0,
            centre.0 + width / 2.0,
            centre.1 + height / 2.0,
        ];
        Ok(placed(
            value,
            bounds,
            text_color_on(fill.over(self.context.theme.background())),
        ))
    }
}

/// The bar a label belongs to: its category, and its series' index and name in a grouped or
/// stacked mark.
struct LabelledBar<'a> {
    /// The bar's category.
    category: &'a str,
    /// The bar's series, by index and name; `None` in a plain bar mark.
    series: Option<(usize, &'a str)>,
}

impl LabelledBar<'_> {
    /// How messages name the bar: `the bar "B"`, `the series "North" in the bar "B"`, or for a
    /// series without a name `series[1] in the bar "B"`.
    fn describe(&self) -> String {
        let bar = format!("the bar {}", quoted(self.category));
        match self.series {
            None => bar,
            Some((index, "")) => format!("series[{index}] in {bar}"),
            Some((_, name)) => format!("the series {} in {bar}", quoted(name)),
        }
    }
}

/// The box `((across), (along))` of a frame in scene coordinates, as left, top, right and
/// bottom: the frame's first axis is the scene's x in the plot's own frame, and its y in the
/// frame of horizontal marks.
fn in_scene(frame: Orientation, ((a0, a1), (v0, v1)): ((f64, f64), (f64, f64))) -> [f64; 4] {
    match frame {
        Orientation::Vertical => [a0, v0, a1, v1],
        Orientation::Horizontal => [v0, a0, v1, a1],
    }
}

/// Whether the box `bounds` (left, top, right, bottom) lies inside `plot`, up to
/// [`FIT_TOLERANCE`].
fn contains(plot: Area, [left, top, right, bottom]: [f64; 4]) -> bool {
    left >= plot.x - FIT_TOLERANCE
        && top >= plot.y - FIT_TOLERANCE
        && right <= plot.right() + FIT_TOLERANCE
        && bottom <= plot.bottom() + FIT_TOLERANCE
}

/// The text item of `value` in the box `bounds` (left, top, right, bottom), in `color`.
fn placed(value: &ValueLabel, [left, top, ..]: [f64; 4], color: Rgba) -> Item {
    let mut label = value.label.clone();
    label.x = left;
    label.y = top + f64::from(label.layout.ascent());
    text(&label, color)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(values: &[f64], format: &TickFormat) -> Vec<String> {
        value_texts(values, format)
    }

    #[test]
    fn automatic_labels_write_each_value_on_its_own() {
        assert_eq!(
            texts(&[28.4, 42.0, -16.5], &TickFormat::AUTO),
            ["28.4", "42", "\u{2212}16.5"]
        );
        assert_eq!(
            texts(&[0.12345, 0.9], &TickFormat::AUTO),
            ["0.12345", "0.9"]
        );
        assert_eq!(texts(&[2.5e6, 1.0], &TickFormat::AUTO), ["2.5e6", "1"]);
    }

    #[test]
    fn other_formats_write_each_value() {
        let percent = TickFormat::percent(0).unwrap();
        assert_eq!(texts(&[0.25, 0.5], &percent), ["25%", "50%"]);
        let money = TickFormat::fixed(1).unwrap().affix("$", "").unwrap();
        assert_eq!(texts(&[3.0], &money), ["$3.0"]);
        let auto_unit = TickFormat::AUTO.affix("", " kg").unwrap();
        assert_eq!(texts(&[3.0], &auto_unit), ["3 kg"]);
    }

    #[test]
    fn bars_are_named_by_category_and_series() {
        let bar = |series| LabelledBar {
            category: "Q3",
            series,
        };
        assert_eq!(bar(None).describe(), "the bar \"Q3\"");
        assert_eq!(
            bar(Some((1, "North"))).describe(),
            "the series \"North\" in the bar \"Q3\""
        );
        assert_eq!(bar(Some((2, ""))).describe(), "series[2] in the bar \"Q3\"");
    }
}
