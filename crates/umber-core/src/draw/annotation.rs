//! Text at data positions: where [`Annotation`]s sit, their arrows, and the checks that keep
//! them on their axes and on the figure.
//!
//! An annotation's first line is placed by its line box, its advance width by the ascent and
//! descent of its font. Text of [`AnnotationPlacement::At`] has the line's aligned edge, or its
//! centre, and its vertical centre at the point. Text of [`AnnotationPlacement::Offset`] has
//! the part of the line box that faces the point at the offset point (see [`anchor`]). Further
//! lines go below the first, [`LINE_PITCH`](crate::layout::LINE_PITCH) sizes apart. The text
//! box spans every line: the widest line's width, from the first line's top to the last line's
//! descent. Each line is aligned within it: on the point by `align` for `At`, and towards the
//! point for `Offset` (left-aligned when the offset points right, right-aligned when it points
//! left, centred when it points straight up or down). The arrow runs from [`TEXT_GAP`] beyond
//! the text box's edge to [`POINT_GAP`] short of the point.

use super::chrome::{block_items, segment};
use super::mapping::{AxisMap, AxisMaps, axis_position, point};
use crate::error::{number, quoted};
use crate::geometry::Path;
use crate::layout::{Layout, TextBlock, annotation_text};
use crate::model::{Annotation, AnnotationPlacement, AxisValue};
use crate::resolve::{AxisScale, Resolved, ResolvedAxis};
use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, StrokeStyle};
use crate::text::SceneFonts;
use crate::{ColorSpec, Error, Result, Rgba, Theme};

/// The width of an arrow's shaft, in points.
const SHAFT_WIDTH: f32 = 0.75;
/// The length of an arrow's head, in points.
const HEAD_LENGTH: f64 = 5.0;
/// The width of an arrow's head at its base, in points.
const HEAD_WIDTH: f64 = 3.5;
/// The gap between an arrow's tip and its point, in points.
const POINT_GAP: f64 = 3.0;
/// The gap between the text box and the start of its arrow, in points.
const TEXT_GAP: f64 = 2.0;
/// The least distance between the text box and the point of an annotation with an arrow, in
/// points: both gaps and the head.
const ARROW_ROOM: f64 = TEXT_GAP + POINT_GAP + HEAD_LENGTH;

/// The hint of an annotation whose point lies outside the limits.
const LIMITS_HINT: &str = "set xlim/ylim to include it, or move the point.";

/// The scene items of `annotations` on the axes `resolved` laid out as `layout`, on a canvas
/// `canvas` (width, height) points large: for each annotation in order, its arrow, if any, then
/// its text. `None` without annotations.
///
/// # Errors
///
/// For the first annotation, in order, that cannot be drawn, under `context`:
/// - `Error::Data` when its point is at a category no mark of the axes has;
/// - `Error::Style` when its point lies outside the limits of either axis, when its text
///   reaches past an edge of the canvas, or when less than [`ARROW_ROOM`] lies between its
///   text and its point for an arrow;
/// - as [`annotation_text`] when its text cannot be laid out.
pub(crate) fn items(
    theme: &Theme,
    lib: &SceneFonts<'_>,
    resolved: &Resolved,
    layout: &Layout,
    annotations: &[Annotation],
    canvas: (f64, f64),
    context: &'static str,
) -> Result<Option<Item>> {
    if annotations.is_empty() {
        return Ok(None);
    }
    let maps = AxisMaps::new(&resolved.x, &resolved.y, layout.plot);
    let mut items = Vec::new();
    for annotation in annotations {
        let (x, y) = annotation.at();
        let at = (
            position(annotation, "x", &resolved.x, maps.x, x, context)?,
            position(annotation, "y", &resolved.y, maps.y, y, context)?,
        );
        let block = annotation_text(
            lib,
            theme,
            &name(annotation),
            annotation.text(),
            annotation.size(),
        )?;
        let placed = place(annotation.placement(), block, at);
        check_canvas(annotation, &placed, canvas, context)?;
        let color = text_color(theme, annotation.color());
        if let AnnotationPlacement::Offset { arrow: true, .. } = annotation.placement() {
            arrow(annotation, &placed, at, color, context, &mut items)?;
        }
        items.extend(block_items(&placed.block, color));
    }
    Ok(Some(Item::Group { clip: None, items }))
}

/// How errors name `annotation`: as the text of `text()` or the annotation of `annotate()`.
fn name(annotation: &Annotation) -> String {
    let kind = match annotation.placement() {
        AnnotationPlacement::At(_) => "text",
        AnnotationPlacement::Offset { .. } => "annotation",
    };
    format!("the {kind} {}", quoted(annotation.text()))
}

/// The plot coordinate of the coordinate `value` of `annotation` on the axis `axis` (named
/// `which`) mapped by `map`: `Error::Data` for a category the axis does not have, and
/// `Error::Style` for a value outside the axis's limits.
fn position(
    annotation: &Annotation,
    which: &str,
    axis: &ResolvedAxis,
    map: AxisMap,
    value: &AxisValue,
    context: &'static str,
) -> Result<f64> {
    let u = axis_position(axis, value);
    if let (AxisValue::Category(category), true) = (value, u.is_nan()) {
        return Err(Error::data(
            context,
            format!(
                "{} is at the {which} category {}; no mark on these axes has that category.",
                name(annotation),
                quoted(category)
            ),
        )
        .with_hint("draw a mark with that category first."));
    }
    let inside = match axis.scale {
        AxisScale::Category(_) => true,
        AxisScale::Continuous(_) | AxisScale::Time(_) => {
            u >= map.lo.min(map.hi) && u <= map.lo.max(map.hi)
        }
    };
    if !inside {
        return Err(Error::style(
            context,
            format!("{} lies outside the {which} limits.", name(annotation)),
        )
        .with_hint(LIMITS_HINT));
    }
    Ok(map.plot_at(u))
}

/// An annotation's text, placed, with its box.
struct Placed {
    /// The text, each line's baseline origin set.
    block: TextBlock,
    /// The left, top, right and bottom edges of its box.
    bounds: [f64; 4],
}

/// The text `block` of an annotation placed as `placement` for its point `(x, y)`, in plot
/// points: its first line where a text of one line would be, further lines below it.
fn place(placement: AnnotationPlacement, mut block: TextBlock, (x, y): (f64, f64)) -> Placed {
    let width = block.width();
    let first = block.first();
    let (ascent, descent) = (
        f64::from(first.layout.ascent()),
        f64::from(first.layout.descent()),
    );
    let height = ascent + descent;
    let (left, top) = match placement {
        AnnotationPlacement::At(align) => {
            let top = y - height / 2.0;
            block.place(top + ascent, |line| align.position(line, x, x));
            (align.position(width, x, x), top)
        }
        AnnotationPlacement::Offset { offset, .. } => {
            let (share_x, share_y) = anchor(offset);
            let left = x + offset.0 - share_x * width;
            let top = y - offset.1 - share_y * height;
            // Each line is aligned on the side of the box that faces the point.
            block.place(top + ascent, |line| {
                let slack = width - line;
                if slack > 0.0 {
                    left + share_x * slack
                } else {
                    left
                }
            });
            (left, top)
        }
    };
    let bottom = top + height + block.below_first();
    Placed {
        block,
        bounds: [left, top, left + width, bottom],
    }
}

/// The part of the text box at the offset point of an annotation offset by `(dx, dy)` points
/// (x to the right, y upwards), as shares of the box's width from its left and of its height
/// from its top: the part that faces the point. The direction of the offset falls in one of
/// eight sectors of 45° centred on the compass directions, each half-open on its clockwise
/// side: east gives the middle of the left edge, north-east the bottom left corner, north the
/// middle of the bottom edge, and so on round.
fn anchor((dx, dy): (f64, f64)) -> (f64, f64) {
    const SIDES: [(f64, f64); 8] = [
        (0.0, 0.5),
        (0.0, 1.0),
        (0.5, 1.0),
        (1.0, 1.0),
        (1.0, 0.5),
        (1.0, 0.0),
        (0.5, 0.0),
        (0.0, 0.0),
    ];
    let degrees = dy.atan2(dx).to_degrees();
    let sector = ((degrees + 22.5) / 45.0).floor().rem_euclid(8.0);
    // `sector` is a whole number in 0..8: the offset is finite and not zero.
    SIDES[sector as usize % SIDES.len()]
}

/// Checks that the box of `placed` lies on the canvas `(width, height)`: `Error::Style` under
/// `context` naming the edge it reaches furthest past, and by how much.
fn check_canvas(
    annotation: &Annotation,
    placed: &Placed,
    (width, height): (f64, f64),
    context: &'static str,
) -> Result<()> {
    let [left, top, right, bottom] = placed.bounds;
    let past = [
        (-left, "left"),
        (-top, "top"),
        (right - width, "right"),
        (bottom - height, "bottom"),
    ];
    let Some((reach, edge)) = past
        .into_iter()
        .filter(|(reach, _)| *reach > 0.0)
        .max_by(|a, b| a.0.total_cmp(&b.0))
    else {
        return Ok(());
    };
    Err(Error::style(
        context,
        format!(
            "{} reaches {} pt past the {edge} edge of the figure.",
            name(annotation),
            number((reach * 10.0).ceil() / 10.0)
        ),
    )
    .with_hint("change align or offset."))
}

/// Pushes the arrow of `annotation` from the box of `placed` to its point `at`, in `color`:
/// the shaft, then the head. `Error::Style` under `context` when less than [`ARROW_ROOM`]
/// lies between the box and the point along the line from the box's centre to the point.
fn arrow(
    annotation: &Annotation,
    placed: &Placed,
    (x, y): (f64, f64),
    color: Rgba,
    context: &'static str,
    items: &mut Vec<Item>,
) -> Result<()> {
    let [left, top, right, bottom] = placed.bounds;
    let centre = ((left + right) / 2.0, (top + bottom) / 2.0);
    let (vx, vy) = (x - centre.0, y - centre.1);
    let length = vx.hypot(vy);
    let (ux, uy) = (vx / length, vy / length);
    // How far the box reaches from its centre along the line to the point.
    let edge = ((right - left) / 2.0 / ux.abs()).min((bottom - top) / 2.0 / uy.abs());
    let room = length - edge;
    if room.is_nan() || room < ARROW_ROOM {
        return Err(Error::style(
            context,
            format!(
                "the arrow of {} has {} pt between the text and the point; an arrow needs at \
                 least {} pt.",
                name(annotation),
                number((room.max(0.0) * 10.0).round() / 10.0),
                number(ARROW_ROOM)
            ),
        )
        .with_hint("pass a larger offset, or arrow=False."));
    }
    let at = |distance: f64| (centre.0 + ux * distance, centre.1 + uy * distance);
    let start = at(edge + TEXT_GAP);
    let tip = at(length - POINT_GAP);
    let base = at(length - POINT_GAP - HEAD_LENGTH);
    let (nx, ny) = (-uy * HEAD_WIDTH / 2.0, ux * HEAD_WIDTH / 2.0);
    items.push(Item::Stroke {
        path: segment(point(start.0, start.1), point(base.0, base.1)),
        style: StrokeStyle {
            color,
            width: SHAFT_WIDTH,
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        },
    });
    let mut head = Path::new();
    head.move_to(point(base.0 + nx, base.1 + ny));
    head.line_to(point(tip.0, tip.1));
    head.line_to(point(base.0 - nx, base.1 - ny));
    head.close();
    items.push(Item::Fill {
        path: head,
        paint: Paint::Solid(color),
        rule: FillRule::NonZero,
    });
    Ok(())
}

/// The color of text set in `spec`: the theme's text color for the automatic color, at its
/// opacity; a palette slot or a fixed color as given.
fn text_color(theme: &Theme, spec: ColorSpec) -> Rgba {
    match spec {
        ColorSpec::Auto(alpha) => theme.text_color().with_alpha(alpha),
        ColorSpec::Palette { .. } | ColorSpec::Fixed(_) => spec.resolve(theme.palette(), 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_sectors_face_the_point() {
        let cases = [
            ((0.0, 22.0), (0.5, 1.0)),
            ((18.0, 18.0), (0.0, 1.0)),
            ((22.0, 0.0), (0.0, 0.5)),
            ((18.0, -18.0), (0.0, 0.0)),
            ((0.0, -22.0), (0.5, 0.0)),
            ((-18.0, -18.0), (1.0, 0.0)),
            ((-22.0, 0.0), (1.0, 0.5)),
            ((-18.0, 18.0), (1.0, 1.0)),
            // Just below west is still west.
            ((-1.0, -1e-12), (1.0, 0.5)),
            ((1.0, 0.3), (0.0, 0.5)),
            ((1.0, 0.5), (0.0, 1.0)),
        ];
        for (offset, expected) in cases {
            assert_eq!(anchor(offset), expected, "{offset:?}");
        }
    }

    #[test]
    fn the_automatic_color_is_the_theme_text_color() {
        let theme = Theme::midnight();
        assert_eq!(text_color(&theme, ColorSpec::AUTO), theme.text_color());
        let half = crate::Alpha::new(0.5).unwrap();
        assert_eq!(
            text_color(&theme, ColorSpec::Auto(half)),
            theme.text_color().with_alpha(half)
        );
        assert_eq!(text_color(&theme, ColorSpec::Fixed(Rgba::RED)), Rgba::RED);
        let slot = ColorSpec::palette(1, crate::Alpha::OPAQUE).unwrap();
        assert_eq!(text_color(&theme, slot), theme.palette()[1]);
    }
}
