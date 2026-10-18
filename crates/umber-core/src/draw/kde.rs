//! Kernel density marks: drawn as the line through their density.

use super::MarkContext;
use super::line::{line_items, line_points, line_style};
use crate::Rgba;
use crate::model::KdeMark;
use crate::resolve::Resolved;
use crate::scene::Item;

/// The scene items of one density line in `color`, `width` pt wide: the stroke of its line
/// ([`KdeMark::line`]), reduced, clipped and stroked as a line mark's, and nothing when no part
/// of it is visible.
pub(super) fn kde_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &KdeMark,
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let line = mark.line();
    let (xs, ys) = line_points(context, resolved, line);
    let style = line_style(width, color, line.options().dash);
    line_items(context, &xs, &ys, &style, items);
}
