//! The values written next to scatter points ([`PointLabels`](crate::PointLabels)): measured
//! once per render, before layout ([`ScatterLabels::measure`]); on a fixed side they widen
//! automatic limits so that they fit ([`ScatterLabels::end_reaches`], see
//! [`crate::bubble_fit`]); they are placed over the marks once the marks are drawn
//! ([`items`]).
//!
//! **Which points.** A point gets a label when it is drawn: both coordinates present (a finite
//! number, a category, a time) and a fill (a color-mapped value that maps to a color), with its
//! centre inside the plot. The label writes the point's coordinate on the value axis, x on a
//! category y axis and y otherwise, in the mark's format, else the value axis's tick format:
//! numbers as bar labels write them ([`bar_labels::value_texts`]), times as [`time_texts`]
//! writes them.
//!
//! **Style.** Every label is set in the tick label style at the size of its mark's labels, in
//! the mark's resolved color at full opacity: the series color, each group's own with `by=`,
//! and the theme's text color for a color-mapped mark, whose per-point colors can lie close to
//! the background. Its box is its advance width by its ascent and descent.
//!
//! **Fixed sides.** [`Place::Above`] and [`Place::Below`] put the box [`GAP`] beyond the
//! marker's drawn edge (its shape's extent plus its outline, [`marker_reach`]), centred
//! horizontally on the point; [`Place::Left`] and [`Place::Right`] put it there centred
//! vertically. A fixed-side label never moves: one that leaves the plot fails the render.
//!
//! **Automatic place.** [`Place::Auto`] tries above, right, below and left, in that order, and
//! takes the first box that lies inside the plot and is clear: the box widened by [`PADDING`]
//! meets none of the [`Obstacles`] of the panel, which are the marks drawn in it
//! ([`obstacle_marks`]) and every label placed before. Labels are placed in input order, mark
//! by mark in draw order and point by point within a mark, after the bar labels, so the same
//! figure always places them the same way. Automatic labels add no reach to the limit fits:
//! they are judged against the limits the plot has without them, so the choice does not chase
//! itself. A label with no clear position fails the render.
//!
//! Text written by `text()` and `annotate()` is drawn after the labels and is not an obstacle.
//! Heatmap cell labels need two category axes, on which no scatter is drawn, so they never
//! meet point labels. Each layer of a twin-axes panel places its labels against its own marks.

use super::bar_labels::{self, contains};
use super::chrome::text;
use super::mapping::{AxisPoint, axis_values};
use super::obstacles::{Bounds, Obstacles};
use super::{DrawnMarks, has_fill, mark_context, marker_outline, marker_reach};
use crate::bubble_fit::{EndReach, ReachFrom};
use crate::color_scale::ResolvedColorScale;
use crate::error::quoted;
use crate::layout::{Label, Layout, value_text};
use crate::model::{Axes, AxisData, values_on_x};
use crate::resolve::{Resolved, ResolvedMark, ValueAxis};
use crate::scene::Item;
use crate::text::SceneFonts;
use crate::ticks::{Tick, TickValues};
use crate::time::{explicit_labels, iso_label, label_step};
use crate::{Error, Instant, NumberStyle, Place, Result, Rgba, Theme, TickFormat};

/// The gap between a marker's drawn edge and the box of its label, in points.
pub(crate) const GAP: f64 = bar_labels::END_GAP;
/// How far, in points, the box of an automatic label must stay from every obstacle.
pub(crate) const PADDING: f64 = 1.0;
/// The sides an automatic label tries, in order.
const AUTO_ORDER: [Place; 4] = [Place::Above, Place::Right, Place::Below, Place::Left];

/// The hint of an automatic label with no clear position.
const CLEAR_HINT: &str = "use place=Place.ABOVE (or another side) to fix the side, a larger figure, or a smaller label size.";
/// The hint of a fixed-side label that leaves the plot under automatic limits.
const ROOM_HINT: &str = "enlarge the figure, use another place, or a smaller label size.";
/// The hint of a fixed-side label that leaves the plot under explicit limits.
const LIMITS_HINT: &str = "widen xlim or ylim, or let the limits be automatic.";

/// One point's label, measured.
#[derive(Debug, Clone)]
struct PointLabel {
    /// The point's index in its mark's data.
    index: usize,
    /// The point's x and y coordinates as the limit fits read them; `None` on a category axis,
    /// whose limits never grow.
    at: [Option<ReachFrom>; 2],
    /// The label, unplaced.
    label: Label,
}

impl PointLabel {
    /// The width and height of the label's box: its advance width by its ascent and descent.
    fn size(&self) -> (f64, f64) {
        let layout = &self.label.layout;
        (
            self.label.width(),
            f64::from(layout.ascent() + layout.descent()),
        )
    }
}

/// The measured labels of one annotated scatter mark.
#[derive(Debug, Clone)]
struct MarkLabels {
    /// The mark's index among the axes' marks.
    mark: usize,
    /// How messages name the mark: `the series "Long training"`, or `scatter 2` (the second
    /// scatter of the axes) for a mark without a label.
    name: String,
    /// Where its labels go.
    place: Place,
    /// The color of its labels.
    color: Rgba,
    /// How far its markers reach from their centres along x and along y, outline included,
    /// in points.
    reach: (f64, f64),
    /// The labels of its drawn points, in data order.
    points: Vec<PointLabel>,
}

/// The measured value labels of the annotated scatter marks of one axes.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScatterLabels {
    /// One record per annotated mark, in mark order.
    marks: Vec<MarkLabels>,
    /// Whether either axis has explicit limits, which the hint of a label that leaves the plot
    /// names.
    explicit: bool,
}

impl ScatterLabels {
    /// Measures the labels of every annotated scatter mark among `marks`, the resolved marks
    /// of `axes`, in `theme`, with the axes' color scale `scale` deciding which color-mapped
    /// points are drawn.
    ///
    /// # Errors
    ///
    /// As [`value_text`], for the first label that cannot be laid out.
    pub(crate) fn measure(
        theme: &Theme,
        lib: &SceneFonts<'_>,
        axes: &Axes,
        marks: &[ResolvedMark<'_>],
        scale: Option<&ResolvedColorScale>,
    ) -> Result<ScatterLabels> {
        let on_x = values_on_x(axes.y_scale());
        let axis_format = if on_x {
            axes.x_tick_format()
        } else {
            axes.y_tick_format()
        };
        let mut measured = Vec::new();
        let mut scatters = 0;
        for (index, mark) in marks.iter().enumerate() {
            let ResolvedMark::Scatter {
                mark: data,
                color,
                size,
                mapped,
                ..
            } = mark
            else {
                continue;
            };
            scatters += 1;
            let options = data.options();
            let labels = &options.labels;
            if !labels.annotate {
                continue;
            }
            let drawn: Vec<usize> = (0..data.x().len())
                .filter(|&i| {
                    present(data.x(), i)
                        && present(data.y(), i)
                        && has_fill(theme, scale, *color, mapped.as_ref(), i)
                })
                .collect();
            let format = labels.format.as_ref().unwrap_or(axis_format);
            let values = if on_x { data.x() } else { data.y() };
            let texts = match values {
                AxisData::Numbers(numbers) => {
                    let picked: Vec<f64> = drawn.iter().map(|&i| numbers[i]).collect();
                    bar_labels::value_texts(&picked, format)
                }
                AxisData::Times(times) => {
                    let picked: Vec<Instant> = drawn.iter().filter_map(|&i| times[i]).collect();
                    time_texts(&picked, format)
                }
                // The model refuses a scatter on two category axes.
                AxisData::Categories(_) => Vec::new(),
            };
            let points = drawn
                .iter()
                .zip(texts)
                .map(|(&i, content)| {
                    Ok(PointLabel {
                        index: i,
                        at: [coordinate(data.x(), i), coordinate(data.y(), i)],
                        label: value_text(lib, theme, &content, labels.size)?,
                    })
                })
                .collect::<Result<_>>()?;
            let name = if options.label.is_empty() {
                format!("scatter {scatters}")
            } else {
                format!("the series {}", quoted(&options.label))
            };
            measured.push(MarkLabels {
                mark: index,
                name,
                place: labels.place,
                color: Rgba { a: 255, ..*color },
                reach: marker_reach(options.marker, *size, marker_outline(mapped.is_some())),
                points,
            });
        }
        let explicit = axes.x_limits().is_some()
            || axes.y_limits().is_some()
            || axes.x_time_limits().is_some()
            || axes.y_time_limits().is_some();
        Ok(ScatterLabels {
            marks: measured,
            explicit,
        })
    }

    /// Whether no mark writes labels.
    pub(crate) fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// How far each label on a fixed side reaches past its point, for the fits of automatic
    /// limits: along the axis of its side, the marker's reach, [`GAP`] and the label's extent,
    /// towards that side; along the other axis, half the label's extent on each side, since
    /// the label is centred there. Coordinates on a category axis add no reach.
    ///
    /// Labels placed automatically add none: [`Place::Auto`] is judged against the limits
    /// the plot has without them, and never grows them.
    pub(crate) fn end_reaches(&self) -> Vec<EndReach> {
        let mut reaches = Vec::new();
        for mark in &self.marks {
            let (reach_x, reach_y) = mark.reach;
            for point in &mark.points {
                let (width, height) = point.size();
                // Low and high reaches along x, then along y, in points.
                let (x, y) = match mark.place {
                    Place::Auto => continue,
                    Place::Above => ((width / 2.0, width / 2.0), (0.0, reach_y + GAP + height)),
                    Place::Below => ((width / 2.0, width / 2.0), (reach_y + GAP + height, 0.0)),
                    Place::Left => ((reach_x + GAP + width, 0.0), (height / 2.0, height / 2.0)),
                    Place::Right => ((0.0, reach_x + GAP + width), (height / 2.0, height / 2.0)),
                };
                for (axis, (low, high), at) in [
                    (ValueAxis::X, x, point.at[0]),
                    (ValueAxis::Y, y, point.at[1]),
                ] {
                    if let Some(value) = at {
                        reaches.push(EndReach {
                            axis,
                            value,
                            low,
                            high,
                        });
                    }
                }
            }
        }
        reaches
    }
}

/// The value labels `labels` of the marks `marks`, resolved as `resolved`, laid out as
/// `layout` and drawn as `drawn`, placed after the bar labels whose boxes are `placed`, as one
/// unclipped group; `None` without labels.
///
/// # Errors
///
/// `Error::Style` under `context` for the first label, in placing order, that cannot be placed:
/// a fixed-side label that leaves the plot, or an automatic label with no clear position. The
/// message names the mark and the label.
pub(crate) fn items(
    theme: &Theme,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
    labels: &ScatterLabels,
    (drawn, placed): (&DrawnMarks, &[Bounds]),
    context: &'static str,
) -> Result<Option<Item>> {
    if labels.is_empty() {
        return Ok(None);
    }
    let plot = layout.plot;
    let maps = mark_context(theme, resolved, layout, marks).maps;
    let mut obstacles = Obstacles::new(plot);
    for (index, mark) in marks.iter().enumerate() {
        if obstacle_marks(mark) {
            obstacles.add_items(drawn.of_mark(index));
        }
    }
    for &bounds in placed {
        obstacles.add_box(bounds);
    }
    let mut items = Vec::new();
    for mark_labels in &labels.marks {
        let Some(ResolvedMark::Scatter { mark, .. }) = marks.get(mark_labels.mark) else {
            continue;
        };
        let xs = axis_values(&resolved.x, mark.x());
        let ys = axis_values(&resolved.y, mark.y());
        for point in &mark_labels.points {
            let (Some(&x), Some(&y)) = (xs.get(point.index), ys.get(point.index)) else {
                continue;
            };
            let centre = maps.to_plot(AxisPoint::new(x, y));
            let centre = (centre.x, centre.y);
            if !(x.is_finite() && y.is_finite())
                || !contains(plot, [centre.0, centre.1, centre.0, centre.1])
            {
                continue;
            }
            let candidate = |place| candidate(place, centre, mark_labels.reach, point.size());
            let bounds = match mark_labels.place {
                Place::Auto => AUTO_ORDER
                    .into_iter()
                    .map(candidate)
                    .find(|&bounds| contains(plot, bounds) && !obstacles.hit(padded(bounds)))
                    .ok_or_else(|| {
                        Error::style(
                            context,
                            format!(
                                "the value label {} of {} has no clear position: above, right, \
                                 below and left of its point each leave the plot or overlap a \
                                 mark or a label.",
                                quoted(&point.label.text),
                                mark_labels.name,
                            ),
                        )
                        .with_hint(CLEAR_HINT)
                    })?,
                place => {
                    let bounds = candidate(place);
                    if !contains(plot, bounds) {
                        return Err(Error::style(
                            context,
                            format!(
                                "the value label {} of {} does not fit inside the plot.",
                                quoted(&point.label.text),
                                mark_labels.name,
                            ),
                        )
                        .with_hint(if labels.explicit {
                            LIMITS_HINT
                        } else {
                            ROOM_HINT
                        }));
                    }
                    bounds
                }
            };
            obstacles.add_box(bounds);
            let mut label = point.label.clone();
            label.x = bounds[0];
            label.y = bounds[1] + f64::from(label.layout.ascent());
            items.push(text(&label, mark_labels.color));
        }
    }
    Ok(Some(Item::Group { clip: None, items }))
}

/// Whether the drawn items of `mark` are obstacles of automatic labels: every data mark, but
/// not the cells and contours of fields nor reference lines and spans, which cover or cross
/// the whole plot.
fn obstacle_marks(mark: &ResolvedMark<'_>) -> bool {
    match mark {
        ResolvedMark::Line { .. }
        | ResolvedMark::Area { .. }
        | ResolvedMark::Scatter { .. }
        | ResolvedMark::Bubbles { .. }
        | ResolvedMark::Stem { .. }
        | ResolvedMark::Bar { .. }
        | ResolvedMark::Grouped { .. }
        | ResolvedMark::Stacked { .. }
        | ResolvedMark::StackedArea { .. }
        | ResolvedMark::Hist { .. }
        | ResolvedMark::Step { .. }
        | ResolvedMark::Band { .. }
        | ResolvedMark::ErrorBars { .. }
        | ResolvedMark::Box { .. }
        | ResolvedMark::Violin { .. }
        | ResolvedMark::Events { .. }
        | ResolvedMark::Points { .. }
        | ResolvedMark::Ecdf { .. }
        | ResolvedMark::Kde { .. } => true,
        ResolvedMark::Mesh { .. }
        | ResolvedMark::Hist2d { .. }
        | ResolvedMark::Hexbin { .. }
        | ResolvedMark::Heatmap { .. }
        | ResolvedMark::Contour { .. }
        | ResolvedMark::HLine { .. }
        | ResolvedMark::VLine { .. }
        | ResolvedMark::HSpan { .. }
        | ResolvedMark::VSpan { .. } => false,
    }
}

/// The box (left, top, right, bottom) of a label of `size` on the side `place` of a point
/// at `centre` whose marker reaches `reach` along x and y, as the module documentation
/// describes. [`Place::Auto`] has no box of its own and is taken as above.
fn candidate(
    place: Place,
    (x, y): (f64, f64),
    (reach_x, reach_y): (f64, f64),
    (width, height): (f64, f64),
) -> Bounds {
    match place {
        Place::Auto | Place::Above => {
            let bottom = y - reach_y - GAP;
            [x - width / 2.0, bottom - height, x + width / 2.0, bottom]
        }
        Place::Below => {
            let top = y + reach_y + GAP;
            [x - width / 2.0, top, x + width / 2.0, top + height]
        }
        Place::Right => {
            let left = x + reach_x + GAP;
            [left, y - height / 2.0, left + width, y + height / 2.0]
        }
        Place::Left => {
            let right = x - reach_x - GAP;
            [right - width, y - height / 2.0, right, y + height / 2.0]
        }
    }
}

/// The box `bounds` widened by [`PADDING`] on every side.
fn padded([left, top, right, bottom]: Bounds) -> Bounds {
    [
        left - PADDING,
        top - PADDING,
        right + PADDING,
        bottom + PADDING,
    ]
}

/// Whether the coordinate at `i` of `data` is present: a finite number, a category, a time.
fn present(data: &AxisData, i: usize) -> bool {
    match data {
        AxisData::Numbers(values) => values.get(i).is_some_and(|v| v.is_finite()),
        AxisData::Categories(names) => i < names.len(),
        AxisData::Times(times) => times.get(i).is_some_and(Option::is_some),
    }
}

/// The present coordinate at `i` of `data` as the limit fits read it; `None` for a category.
fn coordinate(data: &AxisData, i: usize) -> Option<ReachFrom> {
    match data {
        AxisData::Numbers(values) => values.get(i).copied().map(ReachFrom::Number),
        AxisData::Times(times) => times.get(i).copied().flatten().map(ReachFrom::Time),
        AxisData::Categories(_) => None,
    }
}

/// The labels of the times `times` of one mark in `format`, with its prefix and suffix: each
/// written down to the finest unit any of them needs ([`label_step`]), as the axis writes a
/// lone explicit tick for the automatic format (`2026`, `Mar 2026`, `Mar 3`, `Mar 3 06:30`),
/// or in ISO 8601 for [`TickFormat::iso`] (`2026-03-03T06:30`).
fn time_texts(times: &[Instant], format: &TickFormat) -> Vec<String> {
    let step = label_step(times);
    let ticks = times
        .iter()
        .map(|&t| {
            let label = match format.style() {
                NumberStyle::Iso => iso_label(t, step),
                // The model refuses number formats for times.
                NumberStyle::Auto
                | NumberStyle::Fixed(_)
                | NumberStyle::Percent(_)
                | NumberStyle::Scientific => explicit_labels(&[t], step).concat(),
            };
            Tick { value: 0.0, label }
        })
        .collect();
    format
        .relabel(ticks, TickValues::Exact)
        .into_iter()
        .map(|tick| tick.label)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(civil: (i64, u8, u8, u8, u8)) -> Instant {
        let (year, month, day, hour, minute) = civil;
        let days = crate::time::days_from_civil(year, i64::from(month), i64::from(day));
        Instant::from_micros(((days * 24 + i64::from(hour)) * 60 + i64::from(minute)) * 60_000_000)
            .unwrap()
    }

    #[test]
    fn fixed_sides_sit_beyond_the_marker_edge() {
        let (centre, reach, size) = ((100.0, 50.0), (4.0, 5.0), (20.0, 10.0));
        assert_eq!(
            candidate(Place::Above, centre, reach, size),
            [90.0, 32.0, 110.0, 42.0]
        );
        assert_eq!(
            candidate(Place::Below, centre, reach, size),
            [90.0, 58.0, 110.0, 68.0]
        );
        assert_eq!(
            candidate(Place::Right, centre, reach, size),
            [107.0, 45.0, 127.0, 55.0]
        );
        assert_eq!(
            candidate(Place::Left, centre, reach, size),
            [73.0, 45.0, 93.0, 55.0]
        );
    }

    #[test]
    fn times_are_written_to_the_finest_unit_they_need() {
        let days = [at((2026, 3, 1, 0, 0)), at((2026, 3, 15, 0, 0))];
        assert_eq!(time_texts(&days, &TickFormat::AUTO), ["Mar 1", "Mar 15"]);
        assert_eq!(
            time_texts(&days, &TickFormat::iso()),
            ["2026-03-01", "2026-03-15"]
        );
        let months = [at((2026, 3, 1, 0, 0)), at((2026, 4, 1, 0, 0))];
        assert_eq!(
            time_texts(&months, &TickFormat::AUTO),
            ["Mar 2026", "Apr 2026"]
        );
        let clock = [at((2026, 3, 3, 6, 30))];
        let local = TickFormat::AUTO.affix("", " local").unwrap();
        assert_eq!(time_texts(&clock, &local), ["Mar 3 06:30 local"]);
    }
}
