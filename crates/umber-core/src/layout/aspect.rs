//! Equal aspect ([`AxesMut::equal_aspect`](crate::AxesMut::equal_aspect)): one data unit equally
//! long on both axes.
//!
//! The grid solve gives a panel its plot area without looking at its limits, so that no panel's
//! limits move another panel's plot. [`fit_equal_aspect`] then takes that area as the panel's
//! room, and shrinks the plot inside it. The pipeline does this after every solve, with the
//! limits that solve was made for, so whatever reads a solved plot sees the final rectangle. On
//! a figure of several panels the room is that of the same figure without the setting
//! ([`crate::pipeline`]).

use super::panel::PanelSlot;
use super::{Area, MIN_PLOT};
use crate::error::number;
use crate::{ContinuousScale, Error, Result};

/// The hint of the error for limits that leave no room for the plot.
const HINT: &str = "bring the x and y ranges closer together, or remove equal_aspect().";

/// Shrinks the plot of `slot`, solved as its room, along one side until the `x` and `y` scales
/// have the same points per data unit, and centres it along that side. The other side keeps
/// the size it had, to the bit.
///
/// What is placed from the slot follows the plot. The header moves down by what the plot left
/// above it and its box moves in by what the plot left at each side
/// ([`HeaderBox::inset`](super::panel::HeaderBox::inset)); the baseline of the x axis label
/// moves up by what the plot left below it. The frame stays, since it is the part of the canvas
/// that belongs to the panel.
///
/// # Convergence
///
/// The result is a function of the room and the two limits, and holds the equal scale for those
/// limits. The pipeline applies it to every solve, with the limits of that solve, so the scales
/// are equal after every pass, the last one included: no pass can end with unequal scales,
/// whether or not the limits have settled.
///
/// The limits settle because everything that moves them is bounded, whatever the plot does.
/// Without marks that reach by a number of points, the limits do not depend on the plot, so
/// only ticks can change, and those passes end as on any plot ([`crate::pipeline`]). A bubble or
/// value label fit only ever widens its extent, since the lengths it is measured against only
/// shrink and the span only grows, and it is made again at most a fixed number of times
/// ([`crate::bubble_fit`]). Automatic scatter labels have their own limit of passes
/// ([`crate::draw::point_labels`]). A hexbin on these axes is counted once, and its limits
/// only grow to hold whole cells, so it settles on the next pass. Each of these is deterministic:
/// no step reads anything but the figure and the previous pass.
///
/// A fit that runs out of attempts falls back to the limits of the data, which would leave the
/// plot clipping its bubbles; the pipeline reports that as an error on these axes rather than
/// settle for it. Labels that do not fit fail the render already.
///
/// # Errors
///
/// `Error::Style` in `context` when the plot would be less than [`MIN_PLOT`] on a side.
pub(crate) fn fit_equal_aspect(
    slot: &mut PanelSlot,
    x: &ContinuousScale,
    y: &ContinuousScale,
    context: &'static str,
) -> Result<()> {
    let room = slot.room;
    let span = |scale: &ContinuousScale| {
        let (lo, hi) = scale.domain();
        hi - lo
    };
    let (x_span, y_span) = (span(x), span(y));
    let (x_scale, y_scale) = (room.width / x_span, room.height / y_span);
    let (width, height) = if x_scale <= y_scale {
        (room.width, x_scale * y_span)
    } else {
        (y_scale * x_span, room.height)
    };
    if width.min(height) < MIN_PLOT {
        return Err(Error::style(
            context,
            format!(
                "equal aspect leaves the plot {} × {} pt for x and y ranges of {} and {}; each \
                 side needs at least {} pt.",
                number(width),
                number(height),
                number(x_span),
                number(y_span),
                number(MIN_PLOT),
            ),
        )
        .with_hint(HINT));
    }
    let inset = (room.width - width) / 2.0;
    let drop = (room.height - height) / 2.0;
    slot.plot = Area {
        x: room.x + inset,
        y: room.y + drop,
        width,
        height,
    };
    slot.header.inset = inset;
    slot.header.top += drop;
    slot.xlabel_baseline -= drop;
    Ok(())
}
