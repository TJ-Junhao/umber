//! Circular geometry and measured inside/outside slice labels, in points.

use std::f64::consts::{PI, TAU};

use super::{Area, BODY_LINE, Label, StyleSizes, Styles, TextRole, measure, metrics};
use crate::model::pie::Pie;
use crate::text::SceneFonts;
use crate::ticks::TickValues;
use crate::{Error, LegendStyle, Point, Result, Rgba, Theme, Tick, TickFormat};

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

    /// The padded text rectangle must lie entirely in the annular sector, including its
    /// closest point to the hole, not merely its four corners.
    fn fits(&self, circle: Circle, hole: f64, line: f64) -> bool {
        if self.inside.is_empty() {
            return true;
        }
        let distance = circle.radius
            * if hole == 0.0 {
                0.65
            } else {
                (1.0 + hole) / 2.0
            };
        let x = self.angle().sin() * distance;
        let y = -self.angle().cos() * distance;
        let half_w = self.inside.iter().map(Label::width).fold(0.0, f64::max) / 2.0 + FIT_GAP;
        let half_h = line * self.inside.len() as f64 / 2.0 + FIT_GAP;
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
        let vertical_room = if outside.iter().any(|&value| value) {
            2.0 * (RIM_GAP + RADIAL)
        } else {
            0.0
        };
        let radius = ((area.width - room[0] - room[1]).min(area.height - vertical_room) / 2.0)
            .min(circle.radius);
        if radius < 1.0 {
            return Err(room_error(
                slices
                    .iter()
                    .filter(|slice| slice.outside.is_some())
                    .map(|s| s.name.as_str()),
            ));
        }
        let shrunk = radius < circle.radius;
        circle.radius = radius;
        circle.x = (area.x + room[0] + radius)
            .max((area.x + area.width / 2.0).min(area.right() - room[1] - radius));
        if !changed && !shrunk {
            break;
        }
    }
    let mut labels = Vec::new();
    for (i, slice) in slices.iter().enumerate() {
        if outside[i] {
            continue;
        }
        let radius = circle.radius
            * if pie.options.hole == 0.0 {
                0.65
            } else {
                (1.0 + pie.options.hole) / 2.0
            };
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
        let mut indices: Vec<_> = slices
            .iter()
            .enumerate()
            .filter(|(i, s)| outside[*i] && s.side() == side && s.outside.is_some())
            .map(|(i, _)| i)
            .collect();
        indices.sort_by(|&a, &b| (-slices[a].angle().cos()).total_cmp(&(-slices[b].angle().cos())));
        if indices.len() as f64 * line > area.height {
            return Err(room_error(indices.iter().map(|&i| slices[i].name.as_str())));
        }
        let mut ys = Vec::new();
        for &i in &indices {
            let desired = circle.y - slices[i].angle().cos() * (circle.radius + RIM_GAP + RADIAL);
            let lower = ys
                .last()
                .map_or(area.y + line / 2.0, |previous| previous + line);
            ys.push(desired.max(lower));
        }
        if let Some(&last) = ys.last() {
            let shift = (last - (area.bottom() - line / 2.0)).max(0.0);
            for y in &mut ys {
                *y -= shift;
            }
        }
        // A backward pass keeps the upper labels in the panel without reducing their gap.
        if ys.first().is_some_and(|y| *y < area.y + line / 2.0) {
            for (row, y) in ys.iter_mut().enumerate() {
                *y = area.y + line * (row as f64 + 0.5);
            }
        }
        for (i, y) in indices.into_iter().zip(ys) {
            let slice = &slices[i];
            let Some(mut text) = slice.outside.clone() else {
                continue;
            };
            let sign = if side == 0 { -1.0 } else { 1.0 };
            let rim = circle.point(slice.angle(), circle.radius + RIM_GAP);
            let radial = circle.point(slice.angle(), circle.radius + RIM_GAP + RADIAL);
            // Move beyond the circle before spreading vertically, so neither the label
            // nor its connecting segments can fall back onto another slice.
            let column = (circle.x + sign * (circle.radius + RIM_GAP + RADIAL)) as f32;
            let elbow = Point {
                x: column + (sign * ELBOW) as f32,
                y: y as f32,
            };
            text.x =
                f64::from(elbow.x) + sign * LABEL_GAP - if side == 0 { text.width() } else { 0.0 };
            text.y = super::baseline_in_line(y - line / 2.0, line, ascent, descent);
            labels.push(SliceLabel {
                text,
                fill: None,
                leader: vec![
                    rim,
                    radial,
                    Point {
                        x: column,
                        y: radial.y,
                    },
                    Point {
                        x: column,
                        y: y as f32,
                    },
                    elbow,
                ],
            });
        }
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

fn room_error<'a>(names: impl Iterator<Item = &'a str>) -> Error {
    let names = names
        .map(crate::error::quoted)
        .collect::<Vec<_>>()
        .join(", ");
    Error::style("PieAxes.pie()", format!("labels for slices {names} do not fit in the panel; labels must stay inside its bounds without overlap."))
        .with_parameter("categories").with_hint(ROOM_HINT)
}
