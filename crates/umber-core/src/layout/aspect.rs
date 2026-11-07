//! Equal aspect ([`AxesMut::equal_aspect`](crate::AxesMut::equal_aspect)): one data unit equally
//! long on both axes.
//!
//! A panel's room is the plot of the same figure without the setting
//! ([`crate::pipeline`]), so that no panel's limits or tick labels move another panel.
//! [`fit_equal_aspect`] shrinks the plot inside that room. The pipeline does this after every
//! solve, with the limits of that solve, so whatever reads a solved plot sees the final
//! rectangle. A plot too small to draw is only refused on the final layout
//! ([`check_equal_aspect_size`]), since hexbin cells can still widen provisional limits.

use super::inset::{CONTAINMENT_TOLERANCE, Room, XInset, fit_x_labels};
use super::panel::{PanelBands, PanelSlot, end_label_cap};
use super::{
    Area, MIN_PLOT, PADDING, X_LABEL_EDGE_GAP, Y_LABEL_GAP, widest, x_labels_overflow,
    y_labels_leave_no_room,
};
use crate::error::number;
use crate::{ContinuousScale, Error, Result};

/// The hint of the error for limits that leave no room for the plot.
const HINT: &str = "bring the x and y ranges closer together, or remove equal_aspect().";

/// The span of `scale`.
fn span(scale: &ContinuousScale) -> f64 {
    let (lo, hi) = scale.domain();
    hi - lo
}

/// The width and height of the largest plot in `room` with equal `x` and `y` scales. One side
/// is the room's, to the bit.
fn equal_size(room: &Area, [x, y]: [&ContinuousScale; 2]) -> (f64, f64) {
    let (x_span, y_span) = (span(x), span(y));
    let (x_scale, y_scale) = (room.width / x_span, room.height / y_span);
    if x_scale <= y_scale {
        (room.width, x_scale * y_span)
    } else {
        (y_scale * x_span, room.height)
    }
}

/// The largest plot in `room` with equal scales for the limits `scales`, centred, at least
/// [`MIN_PLOT`] on a side; [`check_equal_aspect_size`] refuses a smaller one in the end.
fn centred(room: &Area, scales: [&ContinuousScale; 2]) -> Area {
    let (width, height) = equal_size(room, scales);
    let (width, height) = (width.max(MIN_PLOT), height.max(MIN_PLOT));
    Area {
        x: room.x + (room.width - width) / 2.0,
        y: room.y + (room.height - height) / 2.0,
        width,
        height,
    }
}

/// Where the tick labels of a panel must stay: the left end of the y tick label column, and
/// the left and right ends of the x tick labels. The column stays right of the outer padding, or
/// of the frame next to another panel, and the x tick labels within the frame, up to
/// [`X_LABEL_EDGE_GAP`] from the canvas.
#[derive(Debug, Clone, Copy)]
struct LabelBounds {
    /// The leftmost x of the y tick label column.
    column: f64,
    /// The leftmost x of an x tick label.
    lo: f64,
    /// The rightmost x of an x tick label.
    hi: f64,
}

impl LabelBounds {
    /// The bounds in the frame of `slot`, `outer` saying whether its left and right sides are
    /// edges of the canvas.
    fn of(slot: &PanelSlot, outer: [bool; 2]) -> LabelBounds {
        let frame = slot.frame;
        LabelBounds {
            column: frame.x + if outer[0] { PADDING } else { 0.0 },
            lo: frame.x + if outer[0] { X_LABEL_EDGE_GAP } else { 0.0 },
            hi: frame.right() - if outer[1] { X_LABEL_EDGE_GAP } else { 0.0 },
        }
    }

    /// The left edges at which a plot `width` wide keeps the labels of `bands` in bounds and
    /// itself in `room`, as the least and the most; the first is above the second when there
    /// are none.
    fn left_edges(self, bands: &PanelBands, room: &Area, width: f64) -> (f64, f64) {
        let start = (
            room.x.max(self.column + bands.y_tick_column),
            room.right() - width,
        );
        bands.x_spans.iter().fold(start, |(least, most), span| {
            (
                least.max(self.lo + span.half - span.pos * width),
                most.min(self.hi - span.half - span.pos * width),
            )
        })
    }
}

/// The part of the room of `slot` that a plot may take with the tick labels of `bands` within
/// `bounds`: the widest plot that keeps them there.
///
/// # Errors
///
/// `Error::Style` in `context` when no plot of [`MIN_PLOT`] keeps them there, as a solve of the
/// grid reports it.
fn label_room(
    slot: &PanelSlot,
    bands: &PanelBands,
    bounds: LabelBounds,
    context: &'static str,
) -> Result<Area> {
    let room = slot.room;
    let frame = slot.frame;
    let left = room.x.max(bounds.column + bands.y_tick_column);
    let cap = end_label_cap(frame.width);
    let fit = Room {
        left,
        right: room.right(),
        left_max: left.max(frame.x + PADDING + cap),
        right_min: room.right().min(frame.right() - PADDING - cap),
        lo: bounds.lo,
        hi: bounds.hi,
        min_width: MIN_PLOT,
    };
    if matches!(fit_x_labels(&fit, &[]), XInset::Overflowing) {
        let y_room = room.right() - MIN_PLOT - bounds.column - Y_LABEL_GAP;
        return Err(widest(&bands.y_labels).map_or_else(
            || x_labels_overflow(context, &bands.x_labels, &fit, &bands.x_spans, 0.0),
            |label| y_labels_leave_no_room(context, label, y_room),
        ));
    }
    match fit_x_labels(&fit, &bands.x_spans) {
        XInset::Contained { left, right } => Ok(Area {
            x: left,
            width: right - left,
            ..room
        }),
        XInset::Overflowing => Err(x_labels_overflow(
            context,
            &bands.x_labels,
            &fit,
            &bands.x_spans,
            bands.colorbar_tick_width(),
        )),
    }
}

/// `plot` moved along x to the nearest place in `room` that keeps the tick labels of `bands`
/// within `bounds` at its own width, which a plot narrower than `room` can need: its ends are not
/// where the ends of the widest plot are.
///
/// `room` holds a plot as wide as it is ([`label_room`]), so such a place exists. A narrower plot
/// only relaxes what its right end and its labels ask of the left edge, except where a label
/// near the right end pushes the edge right as the plot narrows, against a label near the left
/// end pushing it left; two labels can only meet that way if their half widths add up to more
/// than the bounds, and each label alone fits the bounds.
fn moved_for_labels(plot: Area, room: &Area, bands: &PanelBands, bounds: LabelBounds) -> Area {
    let (least, most) = bounds.left_edges(bands, room, plot.width);
    Area {
        x: plot.x.max(least).min(most.max(least)),
        ..plot
    }
}

/// Shrinks the plot of `slot`, whose room the layout chose, along one side until the x and y
/// scales of `scales` have the same points per data unit, and centres it along that side. The
/// other side keeps the size it had, to the bit.
///
/// The room was chosen for the tick labels of the same figure without the setting. Other limits
/// and a smaller plot give other labels, and a plot narrower than its room has its ends elsewhere,
/// so the final labels are held to the same bounds here, on the rectangle that is drawn. When
/// they reach past them the plot takes the part of the room that keeps them inside
/// ([`label_room`]) at the width of its scales, in the nearest place that does
/// ([`moved_for_labels`]). All of this stays inside the room, so no other panel moves. Labels
/// that not even the widest plot in the room holds are refused.
///
/// What is placed from the slot follows the plot. The header moves down by what the plot left
/// above it and its box moves in by what the plot left at each side
/// ([`HeaderBox::inset`](super::panel::HeaderBox::inset)); the baseline of the x axis label
/// moves up by what the plot left below it. The frame stays, since it is the part of the canvas
/// that belongs to the panel.
///
/// # Convergence
///
/// The result is a function of the room, the tick labels and the two limits, and holds the equal
/// scale for those limits. It runs on every solve, with the limits of that solve, so no pass can
/// end with unequal scales, whether or not the limits have settled.
///
/// The room is fixed, so the limits settle because everything that moves them is bounded.
/// Without marks that reach by a number of points, the limits do not depend on the plot, and
/// only ticks can change, as on any plot ([`crate::pipeline`]). Bubbles and value labels are
/// fitted on one scale for both axes, the largest that holds them in the room, which is found
/// directly, so one fit normally ends them ([`crate::bubble_fit`]); it is made again only when
/// tick labels take some of the room, at most a fixed number of times. Automatic scatter labels
/// have their own limit of passes ([`crate::draw::point_labels`]). A hexbin on these axes is
/// counted once, and its limits only grow to hold whole cells, so it settles on the next pass.
/// Every step is deterministic: none reads anything but the figure and the previous pass.
///
/// A fit that runs out of attempts falls back to the limits of the data, which would leave the
/// plot clipping its bubbles; the pipeline reports that as an error on these axes rather than
/// settle for it. Labels that do not fit fail the render already.
///
/// # Errors
///
/// As [`label_room`].
pub(crate) fn fit_equal_aspect(
    slot: &mut PanelSlot,
    bands: &PanelBands,
    outer: [bool; 2],
    scales: [&ContinuousScale; 2],
    context: &'static str,
) -> Result<()> {
    let room = slot.room;
    let bounds = LabelBounds::of(slot, outer);
    let mut plot = centred(&room, scales);
    let (least, most) = bounds.left_edges(bands, &room, plot.width);
    if plot.x < least - CONTAINMENT_TOLERANCE || plot.x > most + CONTAINMENT_TOLERANCE {
        slot.plot_room = label_room(slot, bands, bounds, context)?;
        plot = moved_for_labels(
            centred(&slot.plot_room, scales),
            &slot.plot_room,
            bands,
            bounds,
        );
    }
    let drop = plot.y - room.y;
    slot.plot = plot;
    slot.header.inset = (plot.x - room.x, room.right() - plot.right());
    slot.header.top += drop;
    slot.xlabel_baseline -= room.bottom() - plot.bottom();
    Ok(())
}

/// Refuses a final plot that the limits `scales` leave less than [`MIN_PLOT`] on a side.
///
/// # Errors
///
/// `Error::Style` in `context`, naming both sizes and both ranges.
pub(crate) fn check_equal_aspect_size(
    slot: &PanelSlot,
    scales: [&ContinuousScale; 2],
    context: &'static str,
) -> Result<()> {
    let (width, height) = equal_size(&slot.plot_room, scales);
    if width.min(height) >= MIN_PLOT {
        return Ok(());
    }
    Err(Error::style(
        context,
        format!(
            "equal aspect leaves the plot {} × {} pt for x and y ranges of {} and {}; each side \
             needs at least {} pt.",
            number(width),
            number(height),
            number(span(scales[0])),
            number(span(scales[1])),
            number(MIN_PLOT),
        ),
    )
    .with_hint(HINT))
}

#[cfg(test)]
mod tests {
    use super::super::measure_panel;
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions};
    use crate::pipeline::{plan_figure, with_layout_input};
    use crate::{FontLibrary, ScaleKind, Theme};

    /// A figure of one axes with y tick labels several digits wide, fitted into a room `width`
    /// wide starting right after the outer padding: the message of the refusal.
    fn refused(width: f64) -> Option<String> {
        let lib = FontLibrary::global();
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1000.0, 2000.0],
                LineOptions::default(),
            )
            .unwrap();
        let (resolved, layout) = plan_figure(&f, (0, 0), lib);
        with_layout_input(&f, (0, 0), lib, |input, grid| {
            let bands = measure_panel(
                input,
                &resolved,
                grid.span(input.axes.cells()),
                [true; 2],
                &super::super::LabelCache::new(input.lib),
            )
            .unwrap();
            let room = Area {
                x: PADDING,
                width,
                ..layout.plot
            };
            let mut slot = PanelSlot {
                plot: room,
                aligned: room,
                room,
                plot_room: room,
                header: super::super::panel::HeaderBox {
                    left: PADDING,
                    right: PADDING + width,
                    top: PADDING,
                    inset: (0.0, 0.0),
                },
                xlabel_baseline: room.bottom() + 20.0,
                frame: super::super::Frame {
                    x: 0.0,
                    y: 0.0,
                    width: grid.width,
                    height: grid.height,
                },
            };
            let scales = [resolved.x.continuous_scale(), resolved.y.continuous_scale()];
            fit_equal_aspect(&mut slot, &bands, [true, true], scales, "test")
                .err()
                .map(|error| error.to_string())
        })
    }

    #[test]
    fn tick_labels_that_leave_no_plot_are_refused() {
        // In a room narrower than the y tick label column no plot keeps the labels in bounds.
        let message = refused(10.0).unwrap();
        assert!(message.contains("y tick label"), "{message}");
        assert!(
            message.contains("leaves room for the plot area"),
            "{message}"
        );
        // In a wider room the plot moves right of the column instead.
        assert_eq!(refused(200.0), None);
    }
}
