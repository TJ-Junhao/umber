//! Circular geometry and measured inside/outside slice labels, in points.

use std::f64::consts::{PI, TAU};

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod tests;

use super::{Area, BODY_LINE, Label, StyleSizes, Styles, TextRole, measure, metrics};
use crate::model::pie::Pie;
use crate::text::SceneFonts;
use crate::ticks::TickValues;
use crate::{Error, LegendStyle, Point, Result, Rgba, Theme, Tick, TickFormat};

mod collision;
mod spread;

const RIM_GAP: f64 = 2.0;
const RADIAL: f64 = 8.0;
const ELBOW: f64 = 6.0;
const LABEL_GAP: f64 = 3.0;
const FIT_GAP: f64 = 2.0;
const ROOM_HINT: &str = "merge small slices into Other, use a larger figure or smaller text, or replace direct labels with legend(HEADER) or legend(CHIPS).";

/// A circle in panel coordinates.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Circle {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

impl Circle {
    /// Clockwise angle, measured from twelve o'clock.
    pub fn point(self, angle: f64, radius: f64) -> Point {
        Point {
            x: (self.x + angle.sin() * radius) as f32,
            y: (self.y - angle.cos() * radius) as f32,
        }
    }
}

/// A positive slice's angular interval and fill.
pub(crate) struct Wedge {
    pub start: f64,
    pub end: f64,
    pub color: Rgba,
}

/// A shaped label with the color of its backdrop and an optional outside leader.
pub(crate) struct SliceLabel {
    pub text: Label,
    pub fill: Option<Rgba>,
    pub leader: Vec<Point>,
}

/// Geometry consumed by the pie drawer.
pub(crate) struct PieLayout {
    pub circle: Circle,
    pub hole: f64,
    pub wedges: Vec<Wedge>,
    pub labels: Vec<SliceLabel>,
}

/// Both ways of writing a slice name and share, measured once before fitting.
struct MeasuredSlice {
    name: String,
    start: f64,
    end: f64,
    color: Rgba,
    inside: Vec<Label>,
    outside: Option<Label>,
}

impl MeasuredSlice {
    fn angle(&self) -> f64 {
        (self.start + self.end) / 2.0
    }
    fn side(&self) -> usize {
        usize::from(self.angle().sin() >= 0.0)
    }

    fn label_radius(&self, circle: Circle, hole: f64) -> f64 {
        if hole == 0.0 && self.end - self.start >= TAU - 1e-12 {
            0.0
        } else {
            circle.radius
                * if hole == 0.0 {
                    0.65
                } else {
                    (1.0 + hole) / 2.0
                }
        }
    }

    /// The padded text rectangle must lie entirely in the annular sector, including its
    /// closest point to the hole, not merely its four corners.
    fn fits(&self, circle: Circle, hole: f64, line: f64) -> bool {
        if self.inside.is_empty() {
            return true;
        }
        let distance = self.label_radius(circle, hole);
        let x = self.angle().sin() * distance;
        let y = -self.angle().cos() * distance;
        let half_w = self.inside.iter().map(Label::width).fold(0.0, f64::max) / 2.0 + FIT_GAP;
        let ink = self
            .inside
            .iter()
            .map(|label| f64::from(label.layout.ascent() + label.layout.descent()))
            .fold(0.0, f64::max);
        let half_h = (line * (self.inside.len() - 1) as f64 + ink) / 2.0 + FIT_GAP;
        let near = (x.abs() - half_w)
            .max(0.0)
            .hypot((y.abs() - half_h).max(0.0));
        if near < circle.radius * hole || (near == 0.0 && self.end - self.start < TAU) {
            return false;
        }
        for dx in [-half_w, half_w] {
            for dy in [-half_h, half_h] {
                let (cx, cy) = (x + dx, y + dy);
                if cx.hypot(cy) > circle.radius {
                    return false;
                }
                let delta = (cx.atan2(-cy) - self.angle() + PI).rem_euclid(TAU) - PI;
                if delta.abs() > (self.end - self.start) / 2.0 {
                    return false;
                }
            }
        }
        true
    }
}

/// Measures and places a pie within the grid's available plot rectangle. Room for outside
/// text is reserved before choosing the final circular plot; the radius only decreases.
pub(crate) fn place(
    pie: &Pie,
    style: LegendStyle,
    area: Area,
    theme: &Theme,
    fonts: &SceneFonts<'_>,
) -> Result<PieLayout> {
    let text_style = Styles::new(
        theme,
        TextRole::Title,
        StyleSizes {
            body: pie.options.size,
            ..Default::default()
        },
    )
    .body;
    let (ascent, descent) = metrics(fonts, &text_style)?;
    let line = (BODY_LINE * f64::from(text_style.size)).max(ascent + descent);
    let format = pie
        .options
        .format
        .clone()
        .unwrap_or(TickFormat::percent(0)?);
    let shares = format.relabel(
        pie.slices
            .iter()
            .map(|slice| Tick {
                value: slice.share,
                label: crate::error::number(slice.share).to_string(),
            })
            .collect(),
        TickValues::Exact,
    );
    let mut slices = Vec::new();
    let mut start = 0.0;
    for (slice, share) in pie.slices.iter().zip(shares) {
        let end = (start + slice.share * TAU).min(TAU);
        if slice.share > 0.0 {
            let mut strings = Vec::new();
            if style == LegendStyle::Direct && !slice.name.is_empty() {
                strings.push(slice.name.clone());
            }
            if pie.options.annotate {
                strings.push(share.label);
            }
            let inside = strings
                .iter()
                .map(|text| measure(fonts, text, text_style))
                .collect::<Result<Vec<_>>>()?;
            let outside = (!strings.is_empty())
                .then(|| measure(fonts, &strings.join(" "), text_style))
                .transpose()?;
            slices.push(MeasuredSlice {
                name: slice.name.clone(),
                start,
                end,
                color: slice.color,
                inside,
                outside,
            });
        }
        start = end;
    }
    if let Some(last) = slices.last_mut() {
        last.end = TAU;
    }
    let mut circle = Circle {
        x: area.x + area.width / 2.0,
        y: area.y + area.height / 2.0,
        radius: area.width.min(area.height) / 2.0,
    };
    let minimum_radius = circle.radius / 2.0;
    let mut outside = vec![false; slices.len()];
    loop {
        let mut changed = false;
        for (i, slice) in slices.iter().enumerate() {
            if !outside[i] && !slice.fits(circle, pie.options.hole, line) {
                outside[i] = true;
                changed = true;
            }
        }
        let mut room = [0.0_f64; 2];
        for (i, slice) in slices.iter().enumerate() {
            if outside[i]
                && let Some(label) = &slice.outside
            {
                room[slice.side()] =
                    room[slice.side()].max(RIM_GAP + RADIAL + ELBOW + LABEL_GAP + label.width());
            }
        }
        let mut radius = ((area.width - room[0] - room[1]) / 2.0).min(circle.radius);
        let vertical = |r| vertical_extents(&slices, &outside, r, line, ascent + descent);
        if radius >= minimum_radius {
            let (top, bottom) = vertical(radius);
            if top + bottom > area.height {
                let mut low = 0.0;
                let mut high = radius;
                for _ in 0..48 {
                    let mid = (low + high) / 2.0;
                    let (top, bottom) = vertical(mid);
                    if top + bottom <= area.height {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                radius = low;
            }
        }
        if radius < minimum_radius {
            return Err(outside_error(&slices, &outside));
        }
        let shrunk = radius < circle.radius;
        circle.radius = radius;
        circle.x = (area.x + room[0] + radius)
            .max((area.x + area.width / 2.0).min(area.right() - room[1] - radius));
        let (top, bottom) = vertical(radius);
        circle.y = (area.y + area.height / 2.0)
            .max(area.y + top)
            .min(area.bottom() - bottom);
        if !changed && !shrunk {
            break;
        }
    }
    let mut labels = Vec::new();
    for (i, slice) in slices.iter().enumerate() {
        if outside[i] {
            continue;
        }
        let radius = slice.label_radius(circle, pie.options.hole);
        let center = circle.point(slice.angle(), radius);
        let top = f64::from(center.y) - line * slice.inside.len() as f64 / 2.0;
        for (row, label) in slice.inside.iter().enumerate() {
            let mut text = label.clone();
            text.x = f64::from(center.x) - text.width() / 2.0;
            text.y = super::baseline_in_line(top + row as f64 * line, line, ascent, descent);
            labels.push(SliceLabel {
                text,
                fill: Some(slice.color),
                leader: Vec::new(),
            });
        }
    }
    for side in 0..2 {
        let indices = side_indices(&slices, &outside, side);
        let ink = ascent + descent;
        if !indices.is_empty() && (indices.len() - 1) as f64 * line + ink > area.height {
            return Err(outside_error(&slices, &outside));
        }
        let desired: Vec<_> = indices
            .iter()
            .map(|&i| circle.y - slices[i].angle().cos() * (circle.radius + RIM_GAP + RADIAL))
            .collect();
        let ys = spread::centres(
            &desired,
            line,
            area.y + ink / 2.0,
            area.bottom() - ink / 2.0,
        );
        for (i, y) in indices.into_iter().zip(ys) {
            let slice = &slices[i];
            let Some(mut text) = slice.outside.clone() else {
                continue;
            };
            let sign = if side == 0 { -1.0 } else { 1.0 };
            let rim = circle.point(slice.angle(), circle.radius + RIM_GAP);
            let radial = circle.point(slice.angle(), circle.radius + RIM_GAP + RADIAL);
            let elbow = Point {
                x: radial.x + (sign * ELBOW) as f32,
                y: y as f32,
            };
            let mut leader = vec![rim, radial];
            if (y - f64::from(radial.y)).abs() > 1e-5 {
                leader.push(Point {
                    x: radial.x,
                    y: y as f32,
                });
            }
            leader.push(elbow);
            text.x =
                f64::from(elbow.x) + sign * LABEL_GAP - if side == 0 { text.width() } else { 0.0 };
            text.y = super::baseline_in_line(y - line / 2.0, line, ascent, descent);
            labels.push(SliceLabel {
                text,
                fill: None,
                leader,
            });
        }
    }
    if !collision::fits(&labels, circle, area) {
        return Err(outside_error(&slices, &outside));
    }
    Ok(PieLayout {
        circle,
        hole: pie.options.hole,
        wedges: slices
            .into_iter()
            .map(|s| Wedge {
                start: s.start,
                end: s.end,
                color: s.color,
            })
            .collect(),
        labels,
    })
}

fn side_indices(slices: &[MeasuredSlice], outside: &[bool], side: usize) -> Vec<usize> {
    let mut indices: Vec<_> = slices
        .iter()
        .enumerate()
        .filter(|(i, slice)| outside[*i] && slice.side() == side && slice.outside.is_some())
        .map(|(i, _)| i)
        .collect();
    indices.sort_by(|&a, &b| (-slices[a].angle().cos()).total_cmp(&(-slices[b].angle().cos())));
    indices
}

/// Actual top and bottom extents, including the minimally spread outside text.
fn vertical_extents(
    slices: &[MeasuredSlice],
    outside: &[bool],
    radius: f64,
    line: f64,
    ink: f64,
) -> (f64, f64) {
    let mut top = radius;
    let mut bottom = radius;
    for side in 0..2 {
        let indices = side_indices(slices, outside, side);
        let desired: Vec<_> = indices
            .iter()
            .map(|&i| -slices[i].angle().cos() * (radius + RIM_GAP + RADIAL))
            .collect();
        let ys = spread::centres(&desired, line, f64::NEG_INFINITY, f64::INFINITY);
        for (&original, &y) in desired.iter().zip(&ys) {
            top = top.max(-original).max(-y + ink / 2.0);
            bottom = bottom.max(original).max(y + ink / 2.0);
        }
    }
    (top, bottom)
}

fn outside_error(slices: &[MeasuredSlice], outside: &[bool]) -> Error {
    room_error(
        slices
            .iter()
            .zip(outside)
            .filter(|(_, is_outside)| **is_outside)
            .map(|(slice, _)| slice.name.as_str()),
    )
}

fn room_error<'a>(names: impl Iterator<Item = &'a str>) -> Error {
    let names = names
        .map(crate::error::quoted)
        .collect::<Vec<_>>()
        .join(", ");
    Error::style("PieAxes.pie()", format!("labels for slices {names} do not fit in the panel; labels must stay inside its bounds without overlap."))
        .with_parameter("categories").with_hint(ROOM_HINT)
}
