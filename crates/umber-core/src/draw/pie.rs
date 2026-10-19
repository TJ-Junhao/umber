//! Pie sectors, separators and direct labels drawn from their measured layout.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::layout::pie::{Circle, PieLayout, Wedge};
use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, StrokeStyle};
use crate::{Path, Point, Rgba, Theme};

/// Appends an exact circular arc represented by cubic segments of at most a quarter turn.
fn arc(path: &mut Path, circle: Circle, radius: f64, start: f64, end: f64) {
    let segments = ((end - start).abs() / FRAC_PI_2).ceil().max(1.0) as usize;
    let step = (end - start) / segments as f64;
    for i in 0..segments {
        let a = start + i as f64 * step;
        let b = start + (i + 1) as f64 * step;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let p = circle.point(a, radius);
        let q = circle.point(b, radius);
        path.cubic_to(
            Point {
                x: p.x + (k * radius * a.cos()) as f32,
                y: p.y + (k * radius * a.sin()) as f32,
            },
            Point {
                x: q.x - (k * radius * b.cos()) as f32,
                y: q.y - (k * radius * b.sin()) as f32,
            },
            q,
        );
    }
}

fn sector(circle: Circle, hole: f64, wedge: &Wedge) -> Path {
    let mut path = Path::new();
    path.move_to(circle.point(wedge.start, circle.radius));
    arc(&mut path, circle, circle.radius, wedge.start, wedge.end);
    let full = wedge.end - wedge.start >= TAU - 1e-12;
    if hole > 0.0 {
        let inner = circle.point(wedge.end, circle.radius * hole);
        if full {
            path.close();
            path.move_to(inner);
        } else {
            path.line_to(inner);
        }
        arc(
            &mut path,
            circle,
            circle.radius * hole,
            wedge.end,
            wedge.start,
        );
    } else if !full {
        path.line_to(Point {
            x: circle.x as f32,
            y: circle.y as f32,
        });
    }
    path.close();
    path
}

fn stroke(color: Rgba, width: f32) -> StrokeStyle {
    StrokeStyle {
        color,
        width,
        dash: Vec::new(),
        cap: LineCap::Butt,
        join: LineJoin::Round,
    }
}

/// Draws all fills, then their separators, then text and leader lines.
pub(crate) fn draw(theme: &Theme, pie: &PieLayout) -> Item {
    let mut items = Vec::new();
    let paths: Vec<_> = pie
        .wedges
        .iter()
        .map(|w| sector(pie.circle, pie.hole, w))
        .collect();
    for (wedge, path) in pie.wedges.iter().zip(&paths) {
        items.push(Item::Fill {
            path: path.clone(),
            paint: Paint::Solid(wedge.color),
            rule: FillRule::NonZero,
        });
    }
    if pie.wedges.len() > 1 {
        for wedge in &pie.wedges {
            let mut path = Path::new();
            path.move_to(pie.circle.point(wedge.start, pie.circle.radius * pie.hole));
            path.line_to(pie.circle.point(wedge.start, pie.circle.radius));
            items.push(Item::Stroke {
                path,
                style: stroke(theme.background(), 1.0),
            });
        }
    }
    for label in &pie.labels {
        if let Some((&first, rest)) = label.leader.split_first() {
            let mut path = Path::new();
            path.move_to(first);
            for &p in rest {
                path.line_to(p);
            }
            items.push(Item::Stroke {
                path,
                style: stroke(theme.muted_color(), 0.75),
            });
        }
        let color = label.fill.map_or(theme.text_color(), |fill| {
            super::heatmap::text_color_on(fill.over(theme.background()))
        });
        items.push(super::chrome::text(&label.text, color));
    }
    Item::Group { clip: None, items }
}
